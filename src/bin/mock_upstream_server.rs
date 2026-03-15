//! Mock upstream MCP server used by integration tests.

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
#[path = "../telemetry.rs"]
mod telemetry;

use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult, Meta,
        PaginatedRequestParams, ProtocolVersion, RawResource, ServerCapabilities, ServerInfo,
        TaskSupport, Tool, ToolAnnotations, ToolExecution,
    },
    service::{RequestContext, RoleServer},
    transport::stdio,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

/// Uses `mimalloc` as the global allocator when the default feature is enabled.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Mock tool server with deterministic tool behavior for integration tests.
#[derive(Clone)]
struct MockUpstreamServer {
    server_name: String,
    list_tools_delay: Option<Duration>,
    protocol_version: ProtocolVersion,
    extra_star_tool_name: Option<String>,
    session_counter: Arc<Mutex<u64>>,
}

impl MockUpstreamServer {
    /// Builds the mock server from CLI args, environment variables, and defaults.
    fn from_env() -> Self {
        Self {
            server_name: configured_server_name(),
            list_tools_delay: configured_list_tools_delay(),
            protocol_version: configured_protocol_version(),
            extra_star_tool_name: configured_star_tool_name(),
            session_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Returns the static tool inventory exposed by the mock server.
    fn tools(&self) -> Vec<Tool> {
        let mut tools = vec![
            Tool::new(
                "echo",
                "Echo the provided message with the server name.",
                single_string_arg_schema("message", true),
            ),
            Tool::new(
                "duplicate",
                "Return a stable response used to test duplicate tool names.",
                empty_object_schema(),
            ),
            Tool::new(
                "session_counter",
                "Increment a session-local counter and return its new value.",
                empty_object_schema(),
            ),
            Tool::new(
                "read_snapshot",
                "Read a stable snapshot of mock state without mutating anything.",
                empty_object_schema(),
            )
            .with_title("Read Snapshot")
            .with_raw_output_schema(report_output_schema())
            .with_annotations(
                ToolAnnotations::with_title("Read Snapshot")
                    .read_only(true)
                    .destructive(false)
                    .idempotent(true)
                    .open_world(false),
            )
            .with_meta(tool_meta("read")),
            Tool::new(
                "delete_record",
                "Delete a record from the mock state.",
                single_string_arg_schema("record_id", true),
            )
            .with_title("Delete Record")
            .with_annotations(
                ToolAnnotations::with_title("Delete Record")
                    .read_only(false)
                    .destructive(true)
                    .idempotent(false)
                    .open_world(false),
            )
            .with_meta(tool_meta("destructive")),
            Tool::new(
                "publish_webhook",
                "Send an outbound webhook to an external destination.",
                single_string_arg_schema("url", true),
            )
            .with_title("Publish Webhook")
            .with_annotations(
                ToolAnnotations::with_title("Publish Webhook")
                    .read_only(false)
                    .destructive(false)
                    .idempotent(false)
                    .open_world(true),
            )
            .with_meta(tool_meta("external")),
            Tool::new(
                "structured_report",
                "Return structured content and metadata for protocol-level forwarding tests.",
                empty_object_schema(),
            )
            .with_title("Structured Report")
            .with_raw_output_schema(report_output_schema()),
            Tool::new(
                "resource_bundle",
                "Return mixed content including resource links and embedded resources.",
                empty_object_schema(),
            )
            .with_title("Resource Bundle")
            .with_meta(tool_meta("bundle")),
            Tool::new(
                "error_result",
                "Return a tool-level error result without turning it into a transport error.",
                empty_object_schema(),
            )
            .with_title("Error Result")
            .with_meta(tool_meta("error")),
            Tool::new(
                "task_only",
                "A task-required tool that the hub must filter from v1.",
                empty_object_schema(),
            )
            .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required)),
        ];

        if let Some(tool_name) = &self.extra_star_tool_name {
            tools.push(Tool::new(
                tool_name.clone(),
                "A deliberately invalid tool name used to test startup rejection.",
                empty_object_schema(),
            ));
        }

        tools
    }
}

impl ServerHandler for MockUpstreamServer {
    /// Describes the mock server as a tools-only MCP peer for integration tests.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                format!("mock-upstream-{}", self.server_name),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(self.protocol_version.clone())
    }

    /// Returns the mock tool inventory used by integration coverage.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        if let Some(delay) = self.list_tools_delay {
            tokio::time::sleep(delay).await;
        }

        Ok(ListToolsResult {
            tools: self.tools(),
            next_cursor: None,
            meta: None,
        })
    }

    /// Executes deterministic mock tool behavior for the requested tool name.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "echo" => {
                let message = request
                    .arguments
                    .as_ref()
                    .and_then(|arguments| arguments.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "{}:{}",
                    self.server_name, message
                ))]))
            }
            "duplicate" => Ok(CallToolResult::success(vec![Content::text(format!(
                "duplicate:{}",
                self.server_name
            ))])),
            "session_counter" => {
                let mut counter = self.session_counter.lock().await;
                *counter += 1;
                Ok(CallToolResult::success(vec![Content::text(
                    counter.to_string(),
                )]))
            }
            "read_snapshot" => Ok(CallToolResult::structured(json!({
                "server": self.server_name,
                "kind": "read_snapshot",
                "readOnly": true
            }))
            .with_meta(Some(result_meta("read_snapshot")))),
            "delete_record" => {
                let record_id = string_argument(&request, "record_id");
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "deleted:{}:{}",
                    self.server_name, record_id
                ))]))
            }
            "publish_webhook" => {
                let url = string_argument(&request, "url");
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "published:{}:{}",
                    self.server_name, url
                ))]))
            }
            "structured_report" => Ok(CallToolResult::structured(json!({
                "server": self.server_name,
                "kind": "structured_report",
                "version": self.protocol_version.as_str(),
            }))
            .with_meta(Some(result_meta("structured_report")))),
            "resource_bundle" => {
                let mut resource_link = RawResource::new(
                    format!("memory://{}/bundle.json", self.server_name),
                    format!("{} bundle", self.server_name),
                );
                resource_link.description =
                    Some("Structured bundle exported by the upstream".to_string());
                resource_link.mime_type = Some("application/json".to_string());
                resource_link.size = Some(128);

                Ok(CallToolResult::success(vec![
                    Content::text(format!("bundle:{}", self.server_name)),
                    Content::resource_link(resource_link),
                    Content::embedded_text(
                        format!("memory://{}/embedded.txt", self.server_name),
                        format!("embedded:{}", self.server_name),
                    ),
                    Content::image("ZmFrZS1pbWFnZS1kYXRh", "image/png"),
                ])
                .with_meta(Some(result_meta("resource_bundle"))))
            }
            "error_result" => Ok(CallToolResult::error(vec![Content::text(format!(
                "upstream-error:{}",
                self.server_name
            ))])
            .with_meta(Some(result_meta("error_result")))),
            "task_only" => Ok(CallToolResult::success(vec![Content::text(format!(
                "task-only:{}",
                self.server_name
            ))])),
            other => Err(McpError::invalid_params(
                if self.extra_star_tool_name.as_deref() == Some(other) {
                    format!("invalid-star-tool:{}:{}", self.server_name, other)
                } else {
                    format!("unknown mock tool '{other}'")
                },
                None,
            )),
        }
    }
}

/// Boots tracing, builds the Tokio runtime explicitly, and runs the mock upstream server.
fn main() -> Result<()> {
    let _tracing = telemetry::init_tracing("info");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for mock upstream server")?;

    runtime
        .block_on(run())
        .context("mock upstream server terminated with an error")
}

/// Runs the mock upstream MCP server over stdio for integration testing.
async fn run() -> Result<()> {
    let service = MockUpstreamServer::from_env()
        .serve(stdio())
        .await
        .context("failed to initialize mock upstream over stdio")?;
    service
        .waiting()
        .await
        .context("mock upstream service terminated unexpectedly")?;
    Ok(())
}

/// Parses the configured protocol version override for the mock upstream server.
fn configured_protocol_version() -> ProtocolVersion {
    std::env::var("MOCK_SERVER_PROTOCOL_VERSION")
        .map(|value| parse_protocol_version(&value))
        .unwrap_or_else(|_| ProtocolVersion::default())
}

/// Parses one optional tool-list delay used to simulate slow startup inventory responses.
fn configured_list_tools_delay() -> Option<Duration> {
    std::env::var("MOCK_SERVER_LIST_TOOLS_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
}

/// Resolves one optional extra tool name used to test invalid literal `*` discovery.
fn configured_star_tool_name() -> Option<String> {
    std::env::var("MOCK_SERVER_STAR_TOOL_NAME")
        .ok()
        .filter(|value| !value.is_empty())
}

/// Resolves the configured server name from CLI args, environment, or the default name.
fn configured_server_name() -> String {
    configured_cli_value("--server-name")
        .or_else(|| std::env::var("MOCK_SERVER_NAME").ok())
        .unwrap_or_else(|| "mock".to_string())
}

/// Reads one optional `--flag value` pair from the current mock-server command line.
fn configured_cli_value(flag: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    args.find(|argument| argument == flag)?;
    args.next()
}

/// Parses one protocol version string using `rmcp`'s own deserializer.
fn parse_protocol_version(value: &str) -> ProtocolVersion {
    serde_json::from_value(json!(value)).expect("mock protocol version must be valid JSON")
}

/// Extracts one string argument from a tool request, defaulting to the empty string.
fn string_argument(request: &CallToolRequestParams, name: &str) -> String {
    request
        .arguments
        .as_ref()
        .and_then(|arguments| arguments.get(name))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Builds an empty JSON object schema used by mock tools without arguments.
fn empty_object_schema() -> Arc<rmcp::model::JsonObject> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("mock schema must be valid"),
    )
}

/// Builds a simple JSON object schema with one named string property.
fn single_string_arg_schema(name: &str, required: bool) -> Arc<rmcp::model::JsonObject> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                name: {
                    "type": "string"
                }
            },
            "required": if required { vec![name] } else { Vec::<&str>::new() },
            "additionalProperties": false
        }))
        .expect("mock schema must be valid"),
    )
}

/// Builds a structured output schema used by metadata-preservation tests.
fn report_output_schema() -> Arc<rmcp::model::JsonObject> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "server": { "type": "string" },
                "kind": { "type": "string" }
            },
            "required": ["server", "kind"],
            "additionalProperties": false
        }))
        .expect("mock output schema must be valid"),
    )
}

/// Builds tool-level metadata used by outward inventory preservation tests.
fn tool_meta(kind: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("mockKind".to_string(), json!(kind));
    meta.insert("source".to_string(), json!("mock-upstream"));
    meta
}

/// Builds result-level metadata used by forwarded tool-result tests.
fn result_meta(kind: &str) -> Meta {
    let mut meta = Meta::new();
    meta.insert("resultKind".to_string(), json!(kind));
    meta.insert("emittedBy".to_string(), json!("mock-upstream"));
    meta
}
