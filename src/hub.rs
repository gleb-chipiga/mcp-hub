//! Outward MCP-facing tool-only hub server.

use std::{future::Future, sync::Arc};

use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, CancelledNotificationParam, Implementation,
        ListToolsResult, Meta, PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    service::{NotificationContext, RequestContext, RoleServer},
};

use crate::{
    config::HubConfig,
    runtime::{SessionRuntime, SessionRuntimeBuildError},
};

/// Tool-only outward MCP server that fronts a per-session upstream runtime.
///
/// In the current stdio-only v1, one process serves one inbound MCP session, so one
/// `HubServer` instance owns exactly one `SessionRuntime`. Any future multi-session
/// transport must create one `HubServer` per inbound session instead of sharing a
/// prebuilt runtime across sessions.
#[derive(Clone)]
pub(crate) struct HubServer {
    runtime: Arc<SessionRuntime>,
}

impl HubServer {
    /// Creates a hub server and eagerly validates startup config before serving.
    ///
    /// This eager construction matches the current stdio-only process model and
    /// ensures outward tool-name collisions are rejected before the outward MCP
    /// server starts serving clients. If the project adds a multi-session inbound
    /// transport later, runtime construction should move behind a per-session
    /// factory or lazy session-bound initializer.
    pub(crate) async fn from_config(config: HubConfig) -> Result<Self, SessionRuntimeBuildError> {
        Ok(Self {
            runtime: Arc::new(SessionRuntime::build(&config).await?),
        })
    }

    /// Gracefully shuts down all upstream clients associated with this server instance.
    pub(crate) async fn shutdown(&self) {
        self.runtime.shutdown().await;
    }
}

impl ServerHandler for HubServer {
    /// Describes the outward hub as a tools-only MCP server.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("mcp-hub", env!("CARGO_PKG_VERSION")))
            .with_instructions("Tool-only MCP aggregation hub. This server exposes tools only.")
    }

    /// Returns the merged tool inventory from the session runtime.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.runtime.list_tools().await,
            next_cursor: None,
            meta: None,
        })
    }

    /// Routes one outward namespaced tool call through the session runtime.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.runtime
            .call_tool(&request, &context)
            .await
            .map_err(|error| error.into_mcp_error())
    }

    /// Forwards a cancellation for an active outward tool call to its owning upstream.
    fn on_cancelled(
        &self,
        mut notification: CancelledNotificationParam,
        context: rmcp::service::NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + rmcp::service::MaybeSendFuture + '_ {
        let runtime = self.runtime.clone();
        async move {
            notification.meta = notification_context_meta(&context);
            runtime.cancel_tool_call(notification).await;
        }
    }
}

/// Returns metadata retained by `rmcp` for one incoming notification.
fn notification_context_meta(context: &NotificationContext<RoleServer>) -> Option<Meta> {
    (!context.meta.0.is_empty())
        .then(|| context.meta.clone())
        .or_else(|| context.extensions.get::<Meta>().cloned())
}
