//! End-to-end integration coverage for the tool-only MCP hub.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use rmcp::{
    ClientHandler, ServiceError, ServiceExt,
    model::{CallToolRequestParams, ClientInfo, Meta, ProtocolVersion, Tool},
    service::{MaybeSendFuture, NotificationContext},
    transport::TokioChildProcess,
};
use serde::Serialize;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    io::AsyncReadExt,
    process::{ChildStderr, Command},
    runtime::Runtime,
};

/// Serializable top-level test config for writing hub integration fixtures to TOML.
#[derive(Serialize)]
struct TestConfig {
    servers: IndexMap<String, TestUpstreamConfig>,
}

/// Serializable upstream process entry used by integration coverage.
#[derive(Serialize)]
struct TestUpstreamConfig {
    #[serde(skip_serializing)]
    instance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "TestToolConfig::is_empty")]
    tools: TestToolConfig,
}

/// Serializable per-upstream tool filtering and override config for tests.
#[derive(Default, Serialize)]
struct TestToolConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    overrides: BTreeMap<String, TestToolOverrideConfig>,
}

/// Serializable per-tool annotation override config used by integration fixtures.
#[derive(Default, Serialize)]
struct TestToolOverrideConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    destructive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_world: Option<bool>,
}

/// Records unexpected tool-list change notifications forwarded by the hub.
#[derive(Clone)]
struct ToolListChangeRecordingClient {
    notification_count: Arc<AtomicUsize>,
}

impl ToolListChangeRecordingClient {
    /// Creates a client handler with no observed tool-list change notifications.
    fn new() -> Self {
        Self {
            notification_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the shared count of forwarded tool-list change notifications.
    fn notification_count(&self) -> Arc<AtomicUsize> {
        self.notification_count.clone()
    }
}

impl ClientHandler for ToolListChangeRecordingClient {
    /// Records every tool-list change notification sent by the hub.
    fn on_tool_list_changed(
        &self,
        _context: NotificationContext<rmcp::RoleClient>,
    ) -> impl Future<Output = ()> + MaybeSendFuture + '_ {
        self.notification_count.fetch_add(1, Ordering::SeqCst);
        std::future::ready(())
    }
}

impl TestUpstreamConfig {
    /// Replaces the child-process command-line arguments for this test upstream.
    fn with_args(mut self, args: &[&str]) -> Self {
        self.args = args
            .iter()
            .map(|argument| (*argument).to_string())
            .collect();
        self
    }

    /// Applies a visible outward prefix to this test upstream definition.
    fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_string());
        self
    }

    /// Overrides the advertised protocol version for this test upstream.
    fn with_protocol_version(mut self, protocol_version: &str) -> Self {
        self.env.insert(
            "MOCK_SERVER_PROTOCOL_VERSION".to_string(),
            protocol_version.to_string(),
        );
        self
    }

    /// Enables the mock-only tool that terminates its process during one routed call.
    fn with_termination_tool(mut self) -> Self {
        self.env.insert(
            "MOCK_SERVER_ENABLE_TERMINATION_TOOL".to_string(),
            "1".to_string(),
        );
        self
    }

    /// Enables the mock-only tool that emits an upstream tool-list change notification.
    fn with_tool_list_change_notification_tool(mut self) -> Self {
        self.env.insert(
            "MOCK_SERVER_ENABLE_TOOL_LIST_CHANGED_NOTIFICATION_TOOL".to_string(),
            "1".to_string(),
        );
        self
    }

    /// Replaces the per-upstream tool filtering and override config for this fixture.
    fn with_tools(mut self, tools: TestToolConfig) -> Self {
        self.tools = tools;
        self
    }
}

impl TestToolConfig {
    /// Returns `true` when this fixture has no optional tool configuration fields set.
    fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty() && self.overrides.is_empty()
    }
}

/// Verifies the minimal command-only config shape can start the hub end to end.
#[test]
fn minimal_command_only_config_starts_upstream() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for minimal config test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![TestUpstreamConfig {
                instance_id: "filesystem".to_string(),
                prefix: None,
                command: mock_upstream_binary().display().to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                tools: TestToolConfig::default(),
            }],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("echo"));

        let result = client
            .call_tool(
                CallToolRequestParams::new("echo").with_arguments(rmcp::model::object(
                    serde_json::json!({ "message": "hello" }),
                )),
            )
            .await?;
        assert_eq!(first_text(&result)?, "mock:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies one unprefixed upstream keeps original outward tool names.
#[test]
fn unprefixed_inventory_preserves_original_tool_names() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for unprefixed inventory test")?;
        let config_path = write_config(temp_dir.path(), vec![mock_upstream("alpha")]).await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("echo"));
        assert!(inventory.contains("duplicate"));
        assert!(!inventory.iter().any(|name| name.starts_with("alpha.")));

        let result = client
            .call_tool(
                CallToolRequestParams::new("echo").with_arguments(rmcp::model::object(
                    serde_json::json!({ "message": "hello" }),
                )),
            )
            .await?;
        assert_eq!(first_text(&result)?, "alpha:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies exclude-only filtering removes tools when no include list is present.
#[test]
fn exclude_only_filters_tools() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new().context("failed to create temp dir for exclude-only test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: Vec::new(),
                        exclude: vec![
                            "duplicate".to_string(),
                            "publish_webhook".to_string(),
                            "resource_bundle".to_string(),
                        ],
                        overrides: BTreeMap::new(),
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("alpha.echo"));
        assert!(!inventory.contains("alpha.duplicate"));
        assert!(!inventory.contains("alpha.publish_webhook"));
        assert!(!inventory.contains("alpha.resource_bundle"));

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies include filters are applied and win over exclude when both are present.
#[test]
fn include_filters_tools_and_wins_over_exclude() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for include or exclude test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: vec!["echo".to_string(), "publish_webhook".to_string()],
                        exclude: vec![
                            "echo".to_string(),
                            "duplicate".to_string(),
                            "publish_webhook".to_string(),
                        ],
                        overrides: BTreeMap::new(),
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert_eq!(
            inventory,
            BTreeSet::from([
                "alpha.echo".to_string(),
                "alpha.publish_webhook".to_string(),
            ])
        );

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies include-only filtering exposes exactly the requested tools.
#[test]
fn include_only_filters_tools() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new().context("failed to create temp dir for include-only test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: vec!["echo".to_string(), "error_result".to_string()],
                        exclude: Vec::new(),
                        overrides: BTreeMap::new(),
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert_eq!(
            inventory,
            BTreeSet::from(["alpha.echo".to_string(), "alpha.error_result".to_string(),])
        );

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies wildcard include selectors support `*` at the start, middle, and end.
#[test]
fn wildcard_include_filters_tools() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for wildcard-include test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: vec![
                            "*webhook".to_string(),
                            "read_*shot".to_string(),
                            "resource_*".to_string(),
                        ],
                        exclude: Vec::new(),
                        overrides: BTreeMap::new(),
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert_eq!(
            inventory,
            BTreeSet::from([
                "alpha.publish_webhook".to_string(),
                "alpha.read_snapshot".to_string(),
                "alpha.resource_bundle".to_string(),
            ])
        );

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies wildcard exclude selectors remove matching tools while leaving others routable.
#[test]
fn wildcard_exclude_filters_tools() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for wildcard-exclude test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: Vec::new(),
                        exclude: vec![
                            "*webhook".to_string(),
                            "read_*shot".to_string(),
                            "resource_*".to_string(),
                        ],
                        overrides: BTreeMap::new(),
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(!inventory.contains("alpha.publish_webhook"));
        assert!(!inventory.contains("alpha.read_snapshot"));
        assert!(!inventory.contains("alpha.resource_bundle"));
        assert!(inventory.contains("alpha.echo"));

        let result = client
            .call_tool(CallToolRequestParams::new("alpha.echo").with_arguments(
                rmcp::model::object(serde_json::json!({ "message": "wildcard" })),
            ))
            .await?;
        assert_eq!(first_text(&result)?, "alpha:wildcard");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies startup validation accepts a registry that becomes collision-free after task omission.
#[test]
fn task_omission_can_resolve_potential_startup_collisions() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for task-omission collision test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha"),
                mock_upstream("beta").with_tools(TestToolConfig {
                    include: vec!["task_only".to_string()],
                    exclude: Vec::new(),
                    overrides: BTreeMap::new(),
                }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        // Both mock upstreams expose `task_only`, so v1 task omission is what prevents an
        // outward-name collision on that tool while keeping the ordinary shared tools routable.
        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("echo"));
        assert!(!inventory.contains("task_only"));

        let result = client
            .call_tool(
                CallToolRequestParams::new("echo").with_arguments(rmcp::model::object(
                    serde_json::json!({ "message": "hello" }),
                )),
            )
            .await?;
        assert_eq!(first_text(&result)?, "alpha:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies startup validation accepts a registry that becomes collision-free after exclude filtering.
#[test]
fn exclude_filtering_can_resolve_potential_startup_collisions() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for exclude collision test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha"),
                mock_upstream("beta").with_tools(TestToolConfig {
                    include: Vec::new(),
                    exclude: all_mock_tool_names(),
                    overrides: BTreeMap::new(),
                }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("echo"));
        assert!(inventory.contains("duplicate"));

        let result = client
            .call_tool(
                CallToolRequestParams::new("duplicate")
                    .with_arguments(rmcp::model::object(serde_json::json!({}))),
            )
            .await?;
        assert_eq!(first_text(&result)?, "duplicate:alpha");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies startup validation rejects discovered upstream tool names that contain literal `*`.
#[test]
fn literal_star_in_upstream_tool_name_is_rejected_on_startup() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for star-tool startup test")?;
        let mut upstream = mock_upstream("alpha");
        upstream.env.insert(
            "MOCK_SERVER_STAR_TOOL_NAME".to_string(),
            "literal*tool".to_string(),
        );
        let config_path = write_config(temp_dir.path(), vec![upstream]).await?;

        let output = run_hub_startup_failure(&config_path).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("failed startup validation while building hub runtime from config")
                && stderr.contains("unsupported tool name")
                && stderr.contains("literal*tool"),
            "unexpected startup stderr: {stderr}"
        );

        Ok(())
    })
}

/// Verifies startup validation rejects annotation overrides for unknown upstream tools.
#[test]
fn unknown_override_target_is_rejected_on_startup() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for override-target startup test")?;
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "typo_tool_name".to_string(),
            TestToolOverrideConfig {
                read_only: Some(true),
                destructive: None,
                open_world: None,
            },
        );
        let config_path = write_config(
            temp_dir.path(),
            vec![mock_upstream("alpha").with_tools(TestToolConfig {
                include: Vec::new(),
                exclude: Vec::new(),
                overrides,
            })],
        )
        .await?;

        let output = run_hub_startup_failure(&config_path).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("failed startup validation while building hub runtime from config")
                && stderr.contains("configured annotation overrides for unknown tools")
                && stderr.contains("typo_tool_name"),
            "unexpected startup stderr: {stderr}"
        );

        Ok(())
    })
}

/// Verifies prefixed copies of one upstream binary can coexist with different settings.
#[test]
fn prefixed_copies_of_same_binary_stay_distinct() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for multi-copy upstream test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha").with_prefix("left"),
                mock_upstream("beta").with_prefix("right"),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        let expected_inventory = ["left", "right"]
            .into_iter()
            .flat_map(|prefix| {
                all_mock_tool_names()
                    .into_iter()
                    .filter(|name| name != "task_only")
                    .map(move |name| format!("{prefix}.{name}"))
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(inventory, expected_inventory);

        let left_result =
            client
                .call_tool(CallToolRequestParams::new("left.echo").with_arguments(
                    rmcp::model::object(serde_json::json!({ "message": "hello" })),
                ))
                .await?;
        let right_result = client
            .call_tool(CallToolRequestParams::new("right.echo").with_arguments(
                rmcp::model::object(serde_json::json!({ "message": "hello" })),
            ))
            .await?;

        assert_eq!(first_text(&left_result)?, "alpha:hello");
        assert_eq!(first_text(&right_result)?, "beta:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies child-process command-line arguments are forwarded to the upstream server.
#[test]
fn upstream_launch_arguments_are_forwarded() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for args-forwarding test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                TestUpstreamConfig {
                    instance_id: "arg-upstream".to_string(),
                    prefix: Some("arg".to_string()),
                    command: mock_upstream_binary().display().to_string(),
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    tools: TestToolConfig::default(),
                }
                .with_args(&["--server-name", "from-args"]),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let result =
            client
                .call_tool(CallToolRequestParams::new("arg.echo").with_arguments(
                    rmcp::model::object(serde_json::json!({ "message": "hello" })),
                ))
                .await?;
        assert_eq!(first_text(&result)?, "from-args:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies unresolved outward-name collisions fail hub initialization deterministically.
#[test]
fn duplicate_unprefixed_tool_names_are_rejected() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for outward collision test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![mock_upstream("alpha"), mock_upstream("beta")],
        )
        .await?;

        let output = run_hub_startup_failure(&config_path).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("failed startup validation while building hub runtime from config")
                && stderr.contains("outward tool name collision for 'echo'"),
            "unexpected startup stderr: {stderr}"
        );

        Ok(())
    })
}

/// Verifies startup validation rejects collisions introduced by identical visible prefixes.
#[test]
fn duplicate_prefixed_tool_names_are_rejected_on_startup() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for prefixed collision test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha").with_prefix("shared"),
                mock_upstream("beta").with_prefix("shared"),
            ],
        )
        .await?;

        let output = run_hub_startup_failure(&config_path).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("failed startup validation while building hub runtime from config")
                && stderr.contains("outward tool name collision for 'shared.echo'"),
            "unexpected startup stderr: {stderr}"
        );

        Ok(())
    })
}

/// Verifies config-driven annotation overrides are applied while preserving unspecified hints.
#[test]
fn config_overrides_tool_annotations() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for annotation override test")?;
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "echo".to_string(),
            TestToolOverrideConfig {
                read_only: Some(true),
                destructive: Some(false),
                open_world: Some(false),
            },
        );
        overrides.insert(
            "publish_webhook".to_string(),
            TestToolOverrideConfig {
                read_only: None,
                destructive: None,
                open_world: Some(false),
            },
        );
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tools(TestToolConfig {
                        include: Vec::new(),
                        exclude: Vec::new(),
                        overrides,
                    }),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;
        let tools = client
            .list_all_tools()
            .await
            .context("failed to list tools from hub for override assertions")?;

        let echo = find_tool(&tools, "alpha.echo")?;
        let echo_annotations = echo
            .annotations
            .as_ref()
            .context("echo tool annotations must exist after override")?;
        assert_eq!(echo_annotations.read_only_hint, Some(true));
        assert_eq!(echo_annotations.destructive_hint, Some(false));
        assert_eq!(echo_annotations.idempotent_hint, None);
        assert_eq!(echo_annotations.open_world_hint, Some(false));

        let webhook = find_tool(&tools, "alpha.publish_webhook")?;
        let webhook_annotations = webhook
            .annotations
            .as_ref()
            .context("publish_webhook annotations must exist")?;
        assert_eq!(webhook_annotations.read_only_hint, Some(false));
        assert_eq!(webhook_annotations.destructive_hint, Some(false));
        assert_eq!(webhook_annotations.idempotent_hint, Some(false));
        assert_eq!(webhook_annotations.open_world_hint, Some(false));

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies all unavailable upstreams prevent the hub from serving an empty inventory.
#[test]
fn all_unavailable_upstreams_fail_startup() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for unavailable-upstream test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                unavailable_upstream("missing-one", "/definitely/not/a/real/binary-one"),
                unavailable_upstream("missing-two", "/definitely/not/a/real/binary-two"),
            ],
        )
        .await?;

        let output = run_hub_startup_failure(&config_path).await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("failed startup validation while building hub runtime from config")
                && stderr.contains("no usable upstream tools remain after discovery"),
            "unexpected startup stderr: {stderr}"
        );

        Ok(())
    })
}

/// Verifies routed calls work and unavailable upstreams are omitted from inventory.
#[test]
fn routed_calls_and_partial_availability_work() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for partial availability test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha").with_prefix("alpha"),
                unavailable_upstream("missing", "/definitely/not/a/real/binary"),
            ],
        )
        .await?;
        let (client, hub_stderr) = spawn_hub_with_captured_stderr(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("alpha.echo"));
        assert!(!inventory.iter().any(|name| name.starts_with("missing.")));

        let result = client
            .call_tool(CallToolRequestParams::new("alpha.echo").with_arguments(
                rmcp::model::object(serde_json::json!({ "message": "hello" })),
            ))
            .await?;
        assert_eq!(first_text(&result)?, "alpha:hello");

        let error = client
            .call_tool(CallToolRequestParams::new("missing.echo"))
            .await
            .expect_err("missing outward tool must error");
        match error {
            ServiceError::McpError(error) => {
                assert!(error.message.contains("not available"));
            }
            other => panic!("unexpected error type: {other:?}"),
        }

        let error_result = client
            .call_tool(CallToolRequestParams::new("alpha.error_result"))
            .await
            .context("upstream error result must pass through the hub as tool data")?;
        assert_eq!(error_result.is_error, Some(true));
        assert_eq!(first_text(&error_result)?, "upstream-error:alpha");
        assert_eq!(
            meta_entry(
                error_result
                    .meta
                    .as_ref()
                    .context("error result must preserve result meta")?,
                "resultKind",
            )?,
            &json!("error_result")
        );

        let invented_error = client
            .call_tool(CallToolRequestParams::new("invented.tool"))
            .await
            .expect_err("invented outward tool must error");
        match invented_error {
            ServiceError::McpError(error) => {
                assert!(error.message.contains("not available"));
            }
            other => panic!("unexpected error type: {other:?}"),
        }

        let _ = client.cancel().await;
        let stderr = read_captured_stderr(hub_stderr).await?;
        assert!(
            !stderr.contains("routed upstream tool call failed"),
            "tool-level errors must not emit routed-call failure warnings: {stderr}"
        );
        Ok(())
    })
}

/// Verifies a terminated upstream keeps its fixed inventory and logs failed routing details.
#[test]
fn upstream_exit_after_startup_keeps_inventory_and_warns_on_routed_calls() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for upstream-exit test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_termination_tool(),
            ],
        )
        .await?;
        let (client, hub_stderr) = spawn_hub_with_captured_stderr(&config_path).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("alpha.echo"));
        assert!(inventory.contains("alpha.terminate_after_startup"));

        let termination_error = client
            .call_tool(CallToolRequestParams::new("alpha.terminate_after_startup"))
            .await
            .expect_err("terminating the upstream must fail the routed call");
        assert_upstream_call_error(termination_error, "alpha", "terminate_after_startup");

        let inventory_after_exit = tool_names(&client.list_all_tools().await?);
        assert_eq!(inventory_after_exit, inventory);

        let routed_error = client
            .call_tool(CallToolRequestParams::new("alpha.echo"))
            .await
            .expect_err("a call to an exited upstream must fail");
        assert_upstream_call_error(routed_error, "alpha", "echo");

        let _ = client.cancel().await;
        let stderr = read_captured_stderr(hub_stderr).await?;
        assert_eq!(
            stderr.matches("routed upstream tool call failed").count(),
            2,
            "each failed routed call must emit one warning: {stderr}"
        );
        assert!(
            stderr.contains("upstream_instance_id=alpha")
                && stderr.contains("original_tool_name=echo")
                && stderr.contains("transport_error="),
            "routed-call warning is missing structured fields: {stderr}"
        );

        Ok(())
    })
}

/// Verifies upstream tool-list change notifications warn without refreshing the registry.
#[test]
fn upstream_tool_list_change_warns_without_refreshing_inventory() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new()
            .context("failed to create temp dir for tool-list change notification test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_tool_list_change_notification_tool(),
            ],
        )
        .await?;
        let notification_client = ToolListChangeRecordingClient::new();
        let notification_count = notification_client.notification_count();
        let (client, hub_stderr) =
            spawn_hub_with_captured_stderr_and_handler(&config_path, notification_client).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("alpha.emit_tool_list_changed"));

        let result = client
            .call_tool(CallToolRequestParams::new("alpha.emit_tool_list_changed"))
            .await?;
        assert_eq!(first_text(&result)?, "tool-list-change-notification:alpha");

        let inventory_after_notification = tool_names(&client.list_all_tools().await?);
        assert_eq!(inventory_after_notification, inventory);
        assert_eq!(
            notification_count.load(Ordering::SeqCst),
            0,
            "the hub must not forward an upstream tool-list change notification"
        );

        let _ = client.cancel().await;
        let stderr = read_captured_stderr(hub_stderr).await?;
        assert_eq!(
            stderr
                .matches(
                    "upstream reported a tool-list change; retaining the startup tool inventory"
                )
                .count(),
            1,
            "each upstream notification must emit one warning: {stderr}"
        );
        assert!(
            stderr.contains("upstream_instance_id=alpha")
                && stderr.contains("notification=tools/list_changed"),
            "tool-list change warning is missing structured fields: {stderr}"
        );
        Ok(())
    })
}

/// Verifies slow upstream inventory discovery is bounded by startup timeout and omitted.
#[test]
fn startup_timeout_omits_slow_upstreams() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for startup-timeout test")?;
        let mut slow_env = BTreeMap::new();
        slow_env.insert("MOCK_SERVER_NAME".to_string(), "slow".to_string());
        slow_env.insert(
            "MOCK_SERVER_PROTOCOL_VERSION".to_string(),
            ProtocolVersion::default().as_str().to_string(),
        );
        slow_env.insert(
            "MOCK_SERVER_LIST_TOOLS_DELAY_MS".to_string(),
            "2000".to_string(),
        );
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha").with_prefix("alpha"),
                TestUpstreamConfig {
                    instance_id: "slow".to_string(),
                    prefix: Some("slow".to_string()),
                    command: mock_upstream_binary().display().to_string(),
                    args: Vec::new(),
                    env: slow_env,
                    tools: TestToolConfig::default(),
                },
            ],
        )
        .await?;
        let client = spawn_hub_with_startup_timeout(&config_path, 200).await?;

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(inventory.contains("alpha.echo"));
        assert!(!inventory.iter().any(|name| name.starts_with("slow.")));

        let result = client
            .call_tool(CallToolRequestParams::new("alpha.echo").with_arguments(
                rmcp::model::object(serde_json::json!({ "message": "hello" })),
            ))
            .await?;
        assert_eq!(first_text(&result)?, "alpha:hello");

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies separate hub sessions do not share upstream session-local state.
#[test]
fn separate_hub_sessions_keep_upstream_state_isolated() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for session isolation test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![mock_upstream("alpha").with_prefix("alpha")],
        )
        .await?;
        let client_one = spawn_hub(&config_path).await?;
        let client_two = spawn_hub(&config_path).await?;

        let first_session_result = client_one
            .call_tool(CallToolRequestParams::new("alpha.session_counter"))
            .await?;
        let second_session_result = client_two
            .call_tool(CallToolRequestParams::new("alpha.session_counter"))
            .await?;
        let first_session_second_call = client_one
            .call_tool(CallToolRequestParams::new("alpha.session_counter"))
            .await?;

        assert_eq!(first_text(&first_session_result)?, "1");
        assert_eq!(first_text(&second_session_result)?, "1");
        assert_eq!(first_text(&first_session_second_call)?, "2");

        let _ = client_one.cancel().await;
        let _ = client_two.cancel().await;
        Ok(())
    })
}

/// Verifies one session can close without breaking another active session's upstream runtime.
#[test]
fn closing_one_session_does_not_break_another() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for session-close test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![mock_upstream("alpha").with_prefix("alpha")],
        )
        .await?;
        let client_one = spawn_hub(&config_path).await?;
        let client_two = spawn_hub(&config_path).await?;

        let _ = client_one.cancel().await;

        let first_result = client_two
            .call_tool(CallToolRequestParams::new("alpha.session_counter"))
            .await?;
        let second_result = client_two
            .call_tool(CallToolRequestParams::new("alpha.session_counter"))
            .await?;

        assert_eq!(first_text(&first_result)?, "1");
        assert_eq!(first_text(&second_result)?, "2");

        let _ = client_two.cancel().await;
        Ok(())
    })
}

/// Verifies the hub advertises tools only and filters task-required tools.
#[test]
fn hub_advertises_only_tools_and_filters_task_required_tools() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new().context("failed to create temp dir for capability test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![mock_upstream("alpha").with_prefix("alpha")],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let peer_info = client
            .peer_info()
            .expect("hub must expose peer info after initialization");
        assert!(peer_info.capabilities.tools.is_some());
        assert_eq!(
            peer_info
                .capabilities
                .tools
                .as_ref()
                .and_then(|tools| tools.list_changed),
            None
        );
        assert!(peer_info.capabilities.prompts.is_none());
        assert!(peer_info.capabilities.resources.is_none());
        assert!(peer_info.capabilities.tasks.is_none());
        assert!(peer_info.capabilities.logging.is_none());
        assert!(peer_info.capabilities.completions.is_none());

        let inventory = tool_names(&client.list_all_tools().await?);
        assert!(!inventory.contains("alpha.task_only"));

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies the hub negotiates all known supported protocol versions with real `rmcp` clients.
#[test]
fn hub_negotiates_supported_protocol_versions() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for protocol negotiation test")?;
        let config_path = write_config(temp_dir.path(), vec![mock_upstream("alpha")]).await?;

        for version in ProtocolVersion::KNOWN_VERSIONS {
            let client = spawn_hub_with_client_info(
                &config_path,
                ClientInfo::default().with_protocol_version(version.clone()),
            )
            .await
            .with_context(|| {
                format!(
                    "supported protocol version '{}' should initialize cleanly",
                    version
                )
            })?;

            let peer_info = client
                .peer_info()
                .context("hub must expose peer info after version negotiation")?;
            assert_eq!(peer_info.protocol_version, version.clone());

            let _ = client.cancel().await;
        }

        Ok(())
    })
}

/// Verifies unknown future client protocol versions negotiate down to the hub's supported version.
#[test]
fn hub_negotiates_unknown_future_protocol_versions() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new()
            .context("failed to create temp dir for future protocol negotiation test")?;
        let config_path = write_config(temp_dir.path(), vec![mock_upstream("alpha")]).await?;
        let future_version = parse_protocol_version("2026-12-31")?;

        let client = spawn_hub_with_client_info(
            &config_path,
            ClientInfo::default().with_protocol_version(future_version),
        )
        .await
        .context("future protocol version should still initialize cleanly")?;
        let peer_info = client
            .peer_info()
            .context("hub must expose peer info after future-version negotiation")?;
        assert_eq!(peer_info.protocol_version, ProtocolVersion::LATEST);

        let _ = client.cancel().await;

        Ok(())
    })
}

/// Verifies outward tools preserve metadata, schemas, and annotations when not overridden.
#[test]
fn hub_preserves_tool_metadata_and_annotations() -> Result<()> {
    run_async_test(async {
        let temp_dir =
            TempDir::new().context("failed to create temp dir for metadata preservation test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_protocol_version(ProtocolVersion::V_2024_11_05.as_str()),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;
        let tools = client
            .list_all_tools()
            .await
            .context("failed to list tools from hub for metadata assertions")?;

        let read_tool = find_tool(&tools, "alpha.read_snapshot")?;
        assert_eq!(read_tool.title.as_deref(), Some("Read Snapshot"));
        assert_eq!(
            read_tool.description.as_deref(),
            Some("Read a stable snapshot of mock state without mutating anything.")
        );
        let read_input_schema = serde_json::to_value(&read_tool.input_schema)
            .context("read tool input schema must be serializable")?;
        assert_eq!(read_input_schema.get("type"), Some(&json!("object")));
        assert_eq!(
            read_input_schema.get("additionalProperties"),
            Some(&json!(false))
        );
        // `rmcp` currently models and forwards `output_schema` without version-gating legacy
        // protocol negotiation, so this assertion captures the hub's pass-through behavior. If
        // the SDK later starts stripping the field for older versions, move this check to a
        // protocol-version-specific test instead of weakening the preservation invariant silently.
        assert!(read_tool.output_schema.is_some());
        assert_eq!(
            meta_entry(
                read_tool
                    .meta
                    .as_ref()
                    .context("read tool meta must exist")?,
                "mockKind"
            )?,
            &json!("read")
        );
        let read_annotations = read_tool
            .annotations
            .as_ref()
            .context("read tool annotations must exist")?;
        assert_eq!(read_annotations.title.as_deref(), Some("Read Snapshot"));
        assert_eq!(read_annotations.read_only_hint, Some(true));
        assert_eq!(read_annotations.destructive_hint, Some(false));
        assert_eq!(read_annotations.idempotent_hint, Some(true));
        assert_eq!(read_annotations.open_world_hint, Some(false));

        let destructive_tool = find_tool(&tools, "alpha.delete_record")?;
        let destructive_input_schema = serde_json::to_value(&destructive_tool.input_schema)
            .context("destructive tool input schema must be serializable")?;
        assert_eq!(
            destructive_input_schema
                .get("required")
                .and_then(Value::as_array)
                .and_then(|required| required.first()),
            Some(&json!("record_id"))
        );
        let destructive_annotations = destructive_tool
            .annotations
            .as_ref()
            .context("destructive tool annotations must exist")?;
        assert_eq!(destructive_annotations.read_only_hint, Some(false));
        assert_eq!(destructive_annotations.destructive_hint, Some(true));
        assert_eq!(destructive_annotations.open_world_hint, Some(false));

        let external_tool = find_tool(&tools, "alpha.publish_webhook")?;
        let external_annotations = external_tool
            .annotations
            .as_ref()
            .context("external tool annotations must exist")?;
        assert_eq!(external_annotations.read_only_hint, Some(false));
        assert_eq!(external_annotations.destructive_hint, Some(false));
        assert_eq!(external_annotations.open_world_hint, Some(true));

        let echo_tool = find_tool(&tools, "alpha.echo")?;
        assert!(
            echo_tool.annotations.is_none(),
            "tools without upstream annotations must remain unannotated"
        );

        let duplicate_tool = find_tool(&tools, "alpha.duplicate")?;
        assert!(
            duplicate_tool.annotations.is_none(),
            "duplicate tool must not gain empty annotations during forwarding"
        );

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Verifies routed tool results preserve structured payloads, mixed content, and result metadata.
#[test]
fn hub_preserves_structured_and_mixed_tool_results() -> Result<()> {
    run_async_test(async {
        let temp_dir = TempDir::new()
            .context("failed to create temp dir for tool result preservation test")?;
        let config_path = write_config(
            temp_dir.path(),
            vec![
                mock_upstream("alpha")
                    .with_prefix("alpha")
                    .with_protocol_version(ProtocolVersion::V_2025_03_26.as_str()),
            ],
        )
        .await?;
        let client = spawn_hub(&config_path).await?;

        let structured = client
            .call_tool(CallToolRequestParams::new("alpha.structured_report"))
            .await
            .context("failed to call structured_report through hub")?;
        assert_eq!(structured.is_error, Some(false));
        assert_eq!(
            structured
                .structured_content
                .as_ref()
                .context("structured report must include structured content")?
                .get("kind"),
            Some(&json!("structured_report"))
        );
        assert_eq!(
            meta_entry(
                structured
                    .meta
                    .as_ref()
                    .context("structured report must include result meta")?,
                "resultKind",
            )?,
            &json!("structured_report")
        );

        let bundle = client
            .call_tool(CallToolRequestParams::new("alpha.resource_bundle"))
            .await
            .context("failed to call resource_bundle through hub")?;
        assert_eq!(bundle.is_error, Some(false));
        assert_eq!(
            meta_entry(
                bundle
                    .meta
                    .as_ref()
                    .context("resource bundle must include result meta")?,
                "resultKind",
            )?,
            &json!("resource_bundle")
        );
        let resource_link = bundle
            .content
            .iter()
            .find_map(|content| content.as_resource_link())
            .context("resource bundle must include a resource link")?;
        assert_eq!(resource_link.mime_type.as_deref(), Some("application/json"));
        assert!(
            bundle
                .content
                .iter()
                .any(|content| content.as_resource().is_some()),
            "resource bundle must include an embedded resource"
        );
        assert!(
            bundle
                .content
                .iter()
                .any(|content| content.as_image().is_some()),
            "resource bundle must include an image content block"
        );

        let _ = client.cancel().await;
        Ok(())
    })
}

/// Spawns one hub child process and initializes a default `rmcp` client session.
async fn spawn_hub(
    config_path: &Path,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientInfo>> {
    spawn_hub_with_client_info(config_path, ClientInfo::default()).await
}

/// Spawns one hub child process with a custom startup timeout and default client info.
async fn spawn_hub_with_startup_timeout(
    config_path: &Path,
    startup_timeout_ms: u64,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientInfo>> {
    let mut command = Command::new(hub_binary());
    command.env("MCP_HUB_CONFIG", config_path);
    command.env("MCP_HUB_STARTUP_TIMEOUT_MS", startup_timeout_ms.to_string());
    command.kill_on_drop(true);

    let transport = TokioChildProcess::new(command).with_context(|| {
        format!(
            "failed to spawn hub child process from '{}'",
            hub_binary().display()
        )
    })?;
    ClientInfo::default()
        .serve(transport)
        .await
        .context("failed to initialize hub client with custom startup timeout")
}

/// Spawns one hub child process and initializes it with custom client capabilities.
async fn spawn_hub_with_client_info(
    config_path: &Path,
    client_info: ClientInfo,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientInfo>> {
    let mut command = Command::new(hub_binary());
    command.env("MCP_HUB_CONFIG", config_path);
    command.kill_on_drop(true);

    let protocol_version = client_info.protocol_version.clone();
    let transport = TokioChildProcess::new(command).with_context(|| {
        format!(
            "failed to spawn hub child process from '{}'",
            hub_binary().display()
        )
    })?;
    let client: rmcp::service::RunningService<rmcp::RoleClient, ClientInfo> =
        client_info.serve(transport).await.with_context(|| {
            format!("failed to initialize hub client using protocol version '{protocol_version}'")
        })?;
    Ok(client)
}

/// Spawns one hub child process, initializing a client while retaining its stderr stream.
async fn spawn_hub_with_captured_stderr(
    config_path: &Path,
) -> Result<(
    rmcp::service::RunningService<rmcp::RoleClient, ClientInfo>,
    ChildStderr,
)> {
    spawn_hub_with_captured_stderr_and_handler(config_path, ClientInfo::default()).await
}

/// Spawns one hub child process with a custom inbound MCP client handler and captured stderr.
async fn spawn_hub_with_captured_stderr_and_handler<Handler>(
    config_path: &Path,
    handler: Handler,
) -> Result<(
    rmcp::service::RunningService<rmcp::RoleClient, Handler>,
    ChildStderr,
)>
where
    Handler: ClientHandler,
{
    let mut command = Command::new(hub_binary());
    command.env("MCP_HUB_CONFIG", config_path);
    command.env("NO_COLOR", "1");
    command.kill_on_drop(true);

    let (transport, stderr) = TokioChildProcess::builder(command)
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "failed to spawn hub child process from '{}' with captured stderr",
                hub_binary().display()
            )
        })?;
    let stderr = stderr.context("hub child process must expose piped stderr")?;
    let client = handler
        .serve(transport)
        .await
        .context("failed to initialize hub client with captured stderr")?;

    Ok((client, stderr))
}

/// Reads all output from a hub stderr stream after its client service has stopped.
async fn read_captured_stderr(mut stderr: ChildStderr) -> Result<String> {
    let mut output = String::new();
    stderr
        .read_to_string(&mut output)
        .await
        .context("failed to read captured hub stderr")?;
    Ok(output)
}

/// Runs the hub binary and captures stderr for startup-failure assertions.
async fn run_hub_startup_failure(config_path: &Path) -> Result<std::process::Output> {
    let output = Command::new(hub_binary())
        .env("MCP_HUB_CONFIG", config_path)
        .env("NO_COLOR", "1")
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to spawn hub child process from '{}'",
                hub_binary().display()
            )
        })?;

    assert!(
        !output.status.success(),
        "hub startup was expected to fail but exited successfully"
    );

    Ok(output)
}

/// Builds a Tokio runtime used by synchronous integration test entrypoints.
fn build_runtime() -> std::io::Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
}

/// Executes one async integration test body on an explicitly constructed runtime.
fn run_async_test<F>(future: F) -> Result<()>
where
    F: Future<Output = Result<()>>,
{
    let runtime = build_runtime().context("failed to build tokio runtime for integration test")?;
    runtime.block_on(future)
}

/// Writes one TOML hub config file into the test temp directory.
async fn write_config(directory: &Path, servers: Vec<TestUpstreamConfig>) -> Result<PathBuf> {
    let config = TestConfig {
        servers: servers
            .into_iter()
            .map(|server| (server.instance_id.clone(), server))
            .collect(),
    };
    let path = directory.join("mcp-hub.toml");
    let raw = toml::to_string(&config).context("failed to serialize hub test config to TOML")?;
    tokio::fs::write(&path, raw)
        .await
        .with_context(|| format!("failed to write test config to '{}'", path.display()))?;
    Ok(path)
}

/// Builds a standard mock upstream entry with deterministic env-driven identity.
fn mock_upstream(name: &str) -> TestUpstreamConfig {
    let mut env = BTreeMap::new();
    env.insert("MOCK_SERVER_NAME".to_string(), name.to_string());
    env.insert(
        "MOCK_SERVER_PROTOCOL_VERSION".to_string(),
        ProtocolVersion::default().as_str().to_string(),
    );

    TestUpstreamConfig {
        instance_id: name.to_string(),
        prefix: None,
        command: mock_upstream_binary().display().to_string(),
        args: Vec::new(),
        env,
        tools: TestToolConfig::default(),
    }
}

/// Returns the complete mock tool inventory used by the integration upstream server.
fn all_mock_tool_names() -> Vec<String> {
    vec![
        "echo".to_string(),
        "duplicate".to_string(),
        "session_counter".to_string(),
        "read_snapshot".to_string(),
        "delete_record".to_string(),
        "publish_webhook".to_string(),
        "structured_report".to_string(),
        "resource_bundle".to_string(),
        "error_result".to_string(),
        "task_only".to_string(),
    ]
}

/// Builds one upstream config that intentionally points at an unavailable executable.
fn unavailable_upstream(name: &str, command: &str) -> TestUpstreamConfig {
    TestUpstreamConfig {
        instance_id: name.to_string(),
        prefix: None,
        command: command.to_string(),
        args: Vec::new(),
        env: BTreeMap::new(),
        tools: TestToolConfig::default(),
    }
}

/// Asserts that one routed call produced the hub's upstream-failure MCP error.
fn assert_upstream_call_error(error: ServiceError, upstream_id: &str, tool_name: &str) {
    match error {
        ServiceError::McpError(error) => {
            assert!(
                error.message.contains(&format!(
                    "tool '{tool_name}' failed on upstream '{upstream_id}'"
                )),
                "unexpected routed-call error: {error:?}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }
}

/// Collects outward tool names into a sorted set for concise assertions.
fn tool_names(tools: &[Tool]) -> BTreeSet<String> {
    tools.iter().map(|tool| tool.name.to_string()).collect()
}

/// Finds one outward tool by name and fails with context when it is absent.
fn find_tool<'a>(tools: &'a [Tool], name: &str) -> Result<&'a Tool> {
    tools
        .iter()
        .find(|tool| tool.name == name)
        .with_context(|| format!("tool '{name}' was not present in outward inventory"))
}

/// Extracts one metadata entry by key with a contextual assertion message.
fn meta_entry<'a>(meta: &'a Meta, key: &str) -> Result<&'a Value> {
    meta.get(key)
        .with_context(|| format!("metadata entry '{key}' was not present"))
}

/// Returns the first text content block from one tool result with contextual failure.
fn first_text(result: &rmcp::model::CallToolResult) -> Result<String> {
    result
        .content
        .first()
        .and_then(|content| content.as_text())
        .map(|text| text.text.clone())
        .context("tool result must contain a text block")
}

/// Returns the compiled hub test binary path for child-process spawning.
fn hub_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mcp-hub"))
}

/// Returns the compiled mock-upstream test binary path for child-process spawning.
fn mock_upstream_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock_upstream_server"))
}

/// Parses one protocol-version string using `rmcp`'s own deserializer.
fn parse_protocol_version(value: &str) -> Result<ProtocolVersion> {
    serde_json::from_value(json!(value))
        .with_context(|| format!("failed to parse protocol version '{value}'"))
}
