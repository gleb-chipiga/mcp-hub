//! Per-session runtime that owns upstream MCP clients and the merged tool registry.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    future::Future,
    sync::Arc,
    time::Duration,
};

use rmcp::{
    ClientHandler, ErrorData as McpError, Peer, RoleClient, ServiceError, ServiceExt,
    model::{
        CallToolRequest, CallToolRequestParams, CallToolResult, CancelledNotification,
        CancelledNotificationParam, ClientNotification, ClientRequest, Extensions, Meta,
        ProgressNotification, ProgressNotificationParam, ProgressToken, RequestId,
        ServerNotification, ServerResult, TaskSupport, Tool,
    },
    service::{
        ClientInitializeError, MaybeSendFuture, NotificationContext, PeerRequestOptions,
        RequestContext, RoleServer,
    },
    transport::TokioChildProcess,
};
use tokio::{
    process::Command,
    sync::{Mutex, RwLock},
    task::JoinSet,
    time::timeout,
};
use tracing::{info, warn};

use crate::config::{HubConfig, ToolAnnotationOverride, UpstreamInstanceId, UpstreamServerConfig};

type UpstreamService = rmcp::service::RunningService<RoleClient, UpstreamClient>;
const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 5_000;

/// Runtime error for routed outward tool calls.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HubCallError {
    /// The outward tool name was not present in the merged registry.
    #[error("tool '{0}' is not available")]
    UnknownTool(String),
    /// The routed upstream call failed after resolution.
    #[error("tool '{tool_name}' failed on upstream '{upstream_id}': {source}")]
    UpstreamCall {
        /// Stable internal upstream identifier.
        upstream_id: String,
        /// Original upstream tool name.
        tool_name: String,
        /// Underlying `rmcp` service error.
        #[source]
        source: ServiceError,
    },
}

impl HubCallError {
    /// Converts the runtime error into an outward MCP error.
    pub(crate) fn into_mcp_error(self) -> McpError {
        match self {
            Self::UnknownTool(name) => {
                // `tools/call` carries the tool name as request data, so `invalid_params`
                // matches `rmcp`'s own router behavior for unknown tool names.
                McpError::invalid_params(format!("tool '{name}' is not available"), None)
            }
            Self::UpstreamCall {
                upstream_id,
                tool_name,
                source,
            } => McpError::internal_error(
                format!("tool '{tool_name}' failed on upstream '{upstream_id}'"),
                Some(serde_json::json!({ "reason": source.to_string() })),
            ),
        }
    }
}

/// Typed error for building the per-session upstream runtime.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SessionRuntimeBuildError {
    /// Discovery completed without leaving a route that can be exposed to the client.
    #[error("no usable upstream tools remain after discovery")]
    NoUsableTools,
    /// Two active upstreams produced the same outward tool name after filtering and prefixing.
    #[error(
        "outward tool name collision for '{outward_tool_name}' between upstreams '{first_upstream}' and '{second_upstream}'"
    )]
    RouteCollision {
        /// The conflicting outward tool name.
        outward_tool_name: String,
        /// The first upstream instance that claimed the outward name.
        first_upstream: String,
        /// The second upstream instance that claimed the outward name.
        second_upstream: String,
    },
    /// One discovered upstream tool name used a literal `*`, which is reserved for config masks.
    #[error(
        "upstream '{upstream_id}' advertised unsupported tool name '{tool_name}': literal '*' is reserved for include and exclude wildcard masks"
    )]
    UnsupportedToolName {
        /// The upstream instance that advertised the unsupported tool name.
        upstream_id: String,
        /// The unsupported original tool name from the upstream inventory.
        tool_name: String,
    },
    /// One or more configured override targets were not advertised by the upstream server.
    #[error(
        "upstream '{upstream_id}' configured annotation overrides for unknown tools: {tool_names}"
    )]
    UnknownOverrideTargets {
        /// The upstream instance that declared the unknown override targets.
        upstream_id: String,
        /// Comma-separated list of unknown tool names.
        tool_names: String,
    },
}

/// Per-session runtime state for the outward hub server.
pub(crate) struct SessionRuntime {
    state: RwLock<RuntimeState>,
    in_flight_calls: InFlightCallTracker,
}

#[derive(Default)]
struct RuntimeState {
    upstreams: BTreeMap<UpstreamInstanceId, ActiveUpstream>,
    routes: BTreeMap<String, ToolRoute>,
}

struct ActiveUpstream {
    peer: Peer<RoleClient>,
    service: UpstreamService,
}

type UpstreamProgressKey = (UpstreamInstanceId, ProgressToken);

/// Tracks active calls across the hub's outward and upstream MCP sessions.
#[derive(Clone, Default)]
struct InFlightCallTracker {
    state: Arc<Mutex<InFlightCallState>>,
}

/// Mutable correlation state for active routed tool calls.
#[derive(Default)]
struct InFlightCallState {
    calls: HashMap<RequestId, InFlightCall>,
    progress_routes: HashMap<UpstreamProgressKey, ProgressRoute>,
}

/// Correlation details retained for one outward tool call until it resolves.
struct InFlightCall {
    upstream_id: UpstreamInstanceId,
    upstream_peer: Peer<RoleClient>,
    upstream_request_id: Option<RequestId>,
    pending_cancellation: Option<CancelledNotificationParam>,
    cancelled: bool,
    progress_key: Option<UpstreamProgressKey>,
}

/// Outward destination for notifications emitted by one active upstream request.
#[derive(Clone)]
struct ProgressRoute {
    outward_peer: Peer<RoleServer>,
    outward_progress_token: ProgressToken,
}

/// Prepared standard cancellation notification for an owning upstream.
struct CancellationForward {
    upstream_id: UpstreamInstanceId,
    upstream_peer: Peer<RoleClient>,
    notification: CancelledNotificationParam,
}

impl InFlightCallTracker {
    /// Registers a routed outward call before the hub sends its upstream request.
    async fn register(
        &self,
        outward_request_id: RequestId,
        upstream_id: UpstreamInstanceId,
        upstream_peer: Peer<RoleClient>,
    ) {
        self.state.lock().await.calls.insert(
            outward_request_id,
            InFlightCall {
                upstream_id,
                upstream_peer,
                upstream_request_id: None,
                pending_cancellation: None,
                cancelled: false,
                progress_key: None,
            },
        );
    }

    /// Binds an allocated upstream request and returns a queued cancellation, if any.
    async fn bind_upstream_request(
        &self,
        outward_request_id: &RequestId,
        upstream_request_id: RequestId,
        upstream_progress_token: ProgressToken,
        outward_progress_token: Option<ProgressToken>,
        outward_peer: Peer<RoleServer>,
    ) -> Option<CancellationForward> {
        let mut state = self.state.lock().await;
        let (cancellation, progress_route) = {
            let call = state.calls.get_mut(outward_request_id)?;
            call.upstream_request_id = Some(upstream_request_id.clone());

            if call.cancelled {
                let notification = call.pending_cancellation.take().map(|mut notification| {
                    notification.request_id = Some(upstream_request_id);
                    CancellationForward {
                        upstream_id: call.upstream_id.clone(),
                        upstream_peer: call.upstream_peer.clone(),
                        notification,
                    }
                });
                (notification, None)
            } else if let Some(outward_progress_token) = outward_progress_token {
                let progress_key = (call.upstream_id.clone(), upstream_progress_token);
                call.progress_key = Some(progress_key.clone());
                (
                    None,
                    Some((
                        progress_key,
                        ProgressRoute {
                            outward_peer,
                            outward_progress_token,
                        },
                    )),
                )
            } else {
                (None, None)
            }
        };

        if let Some((progress_key, route)) = progress_route {
            state.progress_routes.insert(progress_key, route);
        }

        cancellation
    }

    /// Marks one outward request cancelled and prepares at most one upstream notification.
    async fn cancel(
        &self,
        notification: CancelledNotificationParam,
    ) -> Option<CancellationForward> {
        let outward_request_id = notification.request_id.clone()?;
        let mut state = self.state.lock().await;
        let (cancellation, progress_key) = {
            let call = state.calls.get_mut(&outward_request_id)?;
            if call.cancelled {
                return None;
            }

            call.cancelled = true;
            let progress_key = call.progress_key.take();
            let cancellation = call.upstream_request_id.clone().map(|upstream_request_id| {
                let mut upstream_notification = notification.clone();
                upstream_notification.request_id = Some(upstream_request_id);
                CancellationForward {
                    upstream_id: call.upstream_id.clone(),
                    upstream_peer: call.upstream_peer.clone(),
                    notification: upstream_notification,
                }
            });
            if cancellation.is_none() {
                call.pending_cancellation = Some(notification);
            }

            (cancellation, progress_key)
        };

        if let Some(progress_key) = progress_key {
            state.progress_routes.remove(&progress_key);
        }

        cancellation
    }

    /// Returns the outward progress route for one upstream-generated progress token.
    async fn progress_route(
        &self,
        upstream_id: &UpstreamInstanceId,
        upstream_progress_token: &ProgressToken,
    ) -> Option<ProgressRoute> {
        self.state
            .lock()
            .await
            .progress_routes
            .get(&(upstream_id.clone(), upstream_progress_token.clone()))
            .cloned()
    }

    /// Removes all correlation state associated with one completed routed call.
    async fn complete(&self, outward_request_id: &RequestId) {
        let mut state = self.state.lock().await;
        let progress_key = state
            .calls
            .remove(outward_request_id)
            .and_then(|call| call.progress_key);
        if let Some(progress_key) = progress_key {
            state.progress_routes.remove(&progress_key);
        }
    }

    /// Discards every active correlation while the session is shutting down.
    async fn clear(&self) {
        let mut state = self.state.lock().await;
        state.calls.clear();
        state.progress_routes.clear();
    }
}

/// Sends a prepared cancellation notification without holding the correlation lock.
async fn forward_cancellation(cancellation: CancellationForward) {
    let params = cancellation.notification;
    let upstream_request_id = params.request_id.clone();
    let mut notification = CancelledNotification::new(params);
    notification.extensions = notification_extensions(notification.params.meta.take());
    let notification = ClientNotification::CancelledNotification(notification);
    if let Err(error) = cancellation
        .upstream_peer
        .send_notification(notification)
        .await
    {
        warn!(
            upstream_instance_id = %cancellation.upstream_id,
            ?upstream_request_id,
            transport_error = %error,
            "failed to forward tool-call cancellation to upstream"
        );
    }
}

/// Handles standard MCP notifications received from one configured upstream.
#[derive(Clone)]
struct UpstreamClient {
    upstream_id: UpstreamInstanceId,
    in_flight_calls: InFlightCallTracker,
}

impl UpstreamClient {
    /// Creates a notification handler for one configured upstream instance.
    fn new(upstream_id: UpstreamInstanceId, in_flight_calls: InFlightCallTracker) -> Self {
        Self {
            upstream_id,
            in_flight_calls,
        }
    }
}

impl ClientHandler for UpstreamClient {
    /// Records an upstream inventory change without modifying the session registry.
    fn on_tool_list_changed(
        &self,
        _context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + MaybeSendFuture + '_ {
        warn!(
            upstream_instance_id = %self.upstream_id,
            notification = %"tools/list_changed",
            "upstream reported a tool-list change; retaining the startup tool inventory"
        );
        std::future::ready(())
    }

    /// Forwards progress for an active routed call to the originating outward peer.
    fn on_progress(
        &self,
        notification: ProgressNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + MaybeSendFuture + '_ {
        let upstream_id = self.upstream_id.clone();
        let in_flight_calls = self.in_flight_calls.clone();
        async move {
            let notification_meta = notification_context_meta(&context);
            let Some(route) = in_flight_calls
                .progress_route(&upstream_id, &notification.progress_token)
                .await
            else {
                return;
            };

            let mut outward_notification = notification;
            outward_notification.progress_token = route.outward_progress_token;
            let mut notification = ProgressNotification::new(outward_notification);
            notification.extensions = notification_extensions(notification_meta);
            let notification = ServerNotification::ProgressNotification(notification);
            if let Err(error) = route.outward_peer.send_notification(notification).await {
                warn!(
                    upstream_instance_id = %upstream_id,
                    transport_error = %error,
                    "failed to forward upstream tool-call progress"
                );
            }
        }
    }
}

#[derive(Clone)]
struct ToolRoute {
    outward_tool: Tool,
    peer: Peer<RoleClient>,
    upstream_id: UpstreamInstanceId,
    original_tool_name: String,
}

/// Typed error for creating and initializing one upstream client session.
#[derive(Debug, thiserror::Error)]
enum ConnectUpstreamError {
    /// The child process transport could not be created.
    #[error("failed to spawn upstream process: {source}")]
    Spawn {
        /// Underlying process or transport startup failure.
        source: std::io::Error,
    },
    /// The MCP client handshake against the spawned upstream failed.
    #[error("failed to initialize MCP client session with upstream: {source}")]
    Initialize {
        /// Underlying `rmcp` client initialization failure.
        source: Box<ClientInitializeError>,
    },
}

impl SessionRuntime {
    /// Connects to configured upstreams, validates the startup registry, and builds runtime state.
    pub(crate) async fn build(config: &HubConfig) -> Result<Self, SessionRuntimeBuildError> {
        let in_flight_calls = InFlightCallTracker::default();
        let state = validate_startup_config(config, in_flight_calls.clone()).await?;

        Ok(Self {
            state: RwLock::new(state),
            in_flight_calls,
        })
    }

    /// Returns the merged outward tool inventory for the current session.
    pub(crate) async fn list_tools(&self) -> Vec<Tool> {
        let state = self.state.read().await;
        state
            .routes
            .values()
            .map(|route| route.outward_tool.clone())
            .collect()
    }

    /// Routes an outward tool call to the correct upstream server and original tool name.
    pub(crate) async fn call_tool(
        &self,
        request: &CallToolRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResult, HubCallError> {
        let (peer, upstream_id, original_tool_name) = {
            let state = self.state.read().await;
            let route = state
                .routes
                .get(request.name.as_ref())
                .ok_or_else(|| HubCallError::UnknownTool(request.name.to_string()))?;

            (
                route.peer.clone(),
                route.upstream_id.clone(),
                route.original_tool_name.clone(),
            )
        };

        let mut upstream_request = CallToolRequestParams::new(original_tool_name.clone());
        upstream_request.arguments = request.arguments.clone();
        upstream_request.meta = (!context.meta.0.is_empty()).then(|| context.meta.clone());
        upstream_request.task = request.task.clone();

        let outward_request_id = context.id.clone();
        let outward_progress_token = context.meta.get_progress_token();
        self.in_flight_calls
            .register(
                outward_request_id.clone(),
                upstream_id.clone(),
                peer.clone(),
            )
            .await;

        let request_handle = match peer
            .send_cancellable_request(
                ClientRequest::CallToolRequest(CallToolRequest::new(upstream_request)),
                PeerRequestOptions::no_options(),
            )
            .await
        {
            Ok(request_handle) => request_handle,
            Err(source) => {
                self.in_flight_calls.complete(&outward_request_id).await;
                return Err(upstream_call_error(
                    &upstream_id,
                    &original_tool_name,
                    source,
                ));
            }
        };

        let upstream_request_id = request_handle.id.clone();
        let upstream_progress_token = request_handle.progress_token.clone();
        if let Some(cancellation) = self
            .in_flight_calls
            .bind_upstream_request(
                &outward_request_id,
                upstream_request_id,
                upstream_progress_token,
                outward_progress_token,
                context.peer.clone(),
            )
            .await
        {
            forward_cancellation(cancellation).await;
        }

        let result = match request_handle.await_response().await {
            Ok(ServerResult::CallToolResult(result)) => Ok(result),
            Ok(_) => Err(ServiceError::UnexpectedResponse),
            Err(source) => Err(source),
        };
        self.in_flight_calls.complete(&outward_request_id).await;

        result.map_err(|source| upstream_call_error(&upstream_id, &original_tool_name, source))
    }

    /// Forwards one correlated outward cancellation notification to its owning upstream.
    pub(crate) async fn cancel_tool_call(&self, notification: CancelledNotificationParam) {
        if let Some(cancellation) = self.in_flight_calls.cancel(notification).await {
            forward_cancellation(cancellation).await;
        }
    }

    /// Gracefully shuts down all upstream clients owned by this session.
    pub(crate) async fn shutdown(&self) {
        self.in_flight_calls.clear().await;
        let upstreams = {
            let mut state = self.state.write().await;
            state.routes.clear();
            std::mem::take(&mut state.upstreams)
        };

        cancel_active_upstreams(upstreams).await;
    }
}

/// Places optional MCP metadata in a message envelope for wire serialization.
fn notification_extensions(meta: Option<Meta>) -> Extensions {
    let mut extensions = Extensions::new();
    if let Some(meta) = meta {
        extensions.insert(meta);
    }
    extensions
}

/// Returns metadata retained by `rmcp` for one incoming upstream notification.
fn notification_context_meta(context: &NotificationContext<RoleClient>) -> Option<Meta> {
    (!context.meta.0.is_empty())
        .then(|| context.meta.clone())
        .or_else(|| context.extensions.get::<Meta>().cloned())
}

/// Wraps one upstream request failure in the hub's outward routed-call error.
fn upstream_call_error(
    upstream_id: &UpstreamInstanceId,
    original_tool_name: &str,
    source: ServiceError,
) -> HubCallError {
    warn!(
        upstream_instance_id = %upstream_id,
        original_tool_name = %original_tool_name,
        transport_error = %source,
        "routed upstream tool call failed"
    );
    HubCallError::UpstreamCall {
        upstream_id: upstream_id.to_string(),
        tool_name: original_tool_name.to_string(),
        source,
    }
}

/// Discovers the startup registry and rejects invalid outward naming before serving clients.
async fn validate_startup_config(
    config: &HubConfig,
    in_flight_calls: InFlightCallTracker,
) -> Result<RuntimeState, SessionRuntimeBuildError> {
    let mut state = RuntimeState::default();
    let startup_timeout = startup_discovery_timeout();

    for upstream in &config.servers {
        match timeout(
            startup_timeout,
            connect_upstream(upstream, in_flight_calls.clone()),
        )
        .await
        {
            Ok(Ok(active_upstream)) => {
                match timeout(startup_timeout, active_upstream.peer.list_all_tools()).await {
                    Ok(Ok(tools)) => {
                        let routes =
                            match build_routes(upstream, active_upstream.peer.clone(), tools) {
                                Ok(routes) => routes,
                                Err(error) => {
                                    let _ = active_upstream.service.cancel().await;
                                    cancel_active_upstreams(std::mem::take(&mut state.upstreams))
                                        .await;
                                    return Err(error);
                                }
                            };
                        if routes.is_empty() {
                            info!(
                                upstream = %upstream.instance_id,
                                "upstream exposed no usable tools"
                            );
                            let _ = active_upstream.service.cancel().await;
                            continue;
                        }

                        for route in routes {
                            let outward_tool_name = route.outward_tool.name.to_string();
                            if let Some(existing_route) = state.routes.get(&outward_tool_name) {
                                let collision = SessionRuntimeBuildError::RouteCollision {
                                    outward_tool_name,
                                    first_upstream: existing_route.upstream_id.to_string(),
                                    second_upstream: upstream.instance_id.to_string(),
                                };
                                let _ = active_upstream.service.cancel().await;
                                cancel_active_upstreams(std::mem::take(&mut state.upstreams)).await;
                                return Err(collision);
                            }

                            state
                                .routes
                                .insert(route.outward_tool.name.to_string(), route);
                        }

                        state
                            .upstreams
                            .insert(upstream.instance_id.clone(), active_upstream);
                    }
                    Ok(Err(error)) => {
                        warn!(
                            upstream = %upstream.instance_id,
                            %error,
                            "failed to list tools for upstream; omitting it from the session"
                        );
                        let _ = active_upstream.service.cancel().await;
                    }
                    Err(_) => {
                        warn!(
                            upstream = %upstream.instance_id,
                            timeout_ms = startup_timeout.as_millis() as u64,
                            "timed out while listing tools for upstream; omitting it from the session"
                        );
                        let _ = active_upstream.service.cancel().await;
                    }
                }
            }
            Ok(Err(error)) => {
                warn!(
                    upstream = %upstream.instance_id,
                    %error,
                    "failed to start upstream; omitting it from the session"
                );
            }
            Err(_) => {
                warn!(
                    upstream = %upstream.instance_id,
                    timeout_ms = startup_timeout.as_millis() as u64,
                    "timed out while starting upstream; omitting it from the session"
                );
            }
        }
    }

    if state.routes.is_empty() {
        cancel_active_upstreams(std::mem::take(&mut state.upstreams)).await;
        return Err(SessionRuntimeBuildError::NoUsableTools);
    }

    Ok(state)
}

/// Returns the startup timeout used for upstream connect and inventory discovery.
fn startup_discovery_timeout() -> Duration {
    std::env::var("MCP_HUB_STARTUP_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_STARTUP_TIMEOUT_MS))
}

/// Starts one stdio upstream process and initializes an MCP client session against it.
async fn connect_upstream(
    config: &UpstreamServerConfig,
    in_flight_calls: InFlightCallTracker,
) -> Result<ActiveUpstream, ConnectUpstreamError> {
    let mut command = Command::new(&config.command);
    command.args(&config.args);
    command.envs(
        config
            .env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    command.kill_on_drop(true);

    let transport =
        TokioChildProcess::new(command).map_err(|source| ConnectUpstreamError::Spawn { source })?;
    let service: UpstreamService = UpstreamClient::new(config.instance_id.clone(), in_flight_calls)
        .serve(transport)
        .await
        .map_err(|source| ConnectUpstreamError::Initialize {
            source: Box::new(source),
        })?;
    let peer = service.peer().clone();

    Ok(ActiveUpstream { peer, service })
}

/// Rewrites usable upstream tools into outward names and captures routing metadata.
fn build_routes(
    upstream: &UpstreamServerConfig,
    peer: Peer<RoleClient>,
    tools: Vec<Tool>,
) -> Result<Vec<ToolRoute>, SessionRuntimeBuildError> {
    validate_override_targets(upstream, &tools)?;

    let routes = tools
        .into_iter()
        .map(
            |tool| -> Result<Option<ToolRoute>, SessionRuntimeBuildError> {
                let original_tool_name = tool.name.to_string();
                validate_discovered_tool_name(upstream, &original_tool_name)?;
                if tool.task_support() == TaskSupport::Required {
                    return Ok(None);
                }
                if !upstream.tools.exposes_tool(&original_tool_name) {
                    return Ok(None);
                }

                let mut outward_tool = tool;
                apply_annotation_override(
                    &mut outward_tool,
                    upstream.tools.annotation_override(&original_tool_name),
                );
                outward_tool.name = upstream.outward_tool_name(&original_tool_name).into();

                Ok(Some(ToolRoute {
                    outward_tool,
                    peer: peer.clone(),
                    upstream_id: upstream.instance_id.clone(),
                    original_tool_name,
                }))
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(routes.into_iter().flatten().collect())
}

/// Rejects annotation overrides that target tools not advertised by the upstream inventory.
fn validate_override_targets(
    upstream: &UpstreamServerConfig,
    tools: &[Tool],
) -> Result<(), SessionRuntimeBuildError> {
    let discovered_tool_names = tools
        .iter()
        .map(|tool| tool.name.to_string())
        .collect::<BTreeSet<_>>();
    let unknown_override_targets = upstream
        .tools
        .override_names()
        .filter(|tool_name| !discovered_tool_names.contains(*tool_name))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if unknown_override_targets.is_empty() {
        Ok(())
    } else {
        Err(SessionRuntimeBuildError::UnknownOverrideTargets {
            upstream_id: upstream.instance_id.to_string(),
            tool_names: unknown_override_targets.join(", "),
        })
    }
}

/// Applies one optional config-driven annotation override to an outward tool.
fn apply_annotation_override(tool: &mut Tool, override_config: Option<&ToolAnnotationOverride>) {
    let Some(override_config) = override_config else {
        return;
    };

    let mut annotations = tool.annotations.clone().unwrap_or_default();
    if let Some(read_only) = override_config.read_only {
        annotations.read_only_hint = Some(read_only);
    }
    if let Some(destructive) = override_config.destructive {
        annotations.destructive_hint = Some(destructive);
    }
    if let Some(open_world) = override_config.open_world {
        annotations.open_world_hint = Some(open_world);
    }
    tool.annotations = Some(annotations);
}

/// Rejects discovered upstream tool names that use reserved wildcard syntax.
fn validate_discovered_tool_name(
    upstream: &UpstreamServerConfig,
    tool_name: &str,
) -> Result<(), SessionRuntimeBuildError> {
    if tool_name.contains('*') {
        Err(SessionRuntimeBuildError::UnsupportedToolName {
            upstream_id: upstream.instance_id.to_string(),
            tool_name: tool_name.to_string(),
        })
    } else {
        Ok(())
    }
}

/// Cancels all active upstream services gathered during one runtime build or shutdown.
async fn cancel_active_upstreams(upstreams: BTreeMap<UpstreamInstanceId, ActiveUpstream>) {
    let mut cancellations = JoinSet::new();

    for (upstream_id, upstream) in upstreams {
        cancellations.spawn(async move { (upstream_id, upstream.service.cancel().await) });
    }

    while let Some(result) = cancellations.join_next().await {
        match result {
            Ok((upstream_id, Err(error))) => {
                warn!(upstream = %upstream_id, %error, "failed to shut down upstream client");
            }
            Ok((_upstream_id, Ok(_quit_reason))) => {}
            Err(error) => {
                warn!(%error, "failed to join upstream shutdown task");
            }
        }
    }
}
