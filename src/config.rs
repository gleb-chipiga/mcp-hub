//! Configuration loading and validation for upstream MCP server definitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use serde::Deserialize;

use crate::error::ConfigError;

/// Full validated application configuration for `mcp-hub`.
#[derive(Debug, Clone)]
pub(crate) struct HubConfig {
    /// Upstream MCP servers that the hub should connect to.
    pub(crate) servers: Vec<UpstreamServerConfig>,
}

/// One validated upstream MCP server definition.
#[derive(Debug, Clone)]
pub(crate) struct UpstreamServerConfig {
    /// Stable internal identifier used for routing and diagnostics.
    pub(crate) instance_id: UpstreamInstanceId,
    /// Executable path or command name for the upstream server process.
    pub(crate) command: PathBuf,
    /// Command-line arguments passed to the upstream server process.
    pub(crate) args: Vec<String>,
    /// Environment overrides applied to the upstream server process.
    pub(crate) env: BTreeMap<String, String>,
    /// Whether the hub captures this upstream process's stderr through tracing.
    pub(crate) stderr: bool,
    /// Optional outward tool-name prefix.
    pub(crate) prefix: Option<ToolPrefix>,
    /// Per-upstream tool selection and annotation-override rules.
    pub(crate) tools: UpstreamToolConfig,
}

/// Per-upstream tool selection and annotation-override rules.
#[derive(Debug, Clone, Default)]
pub(crate) struct UpstreamToolConfig {
    include: Vec<ToolNamePattern>,
    exclude: Vec<ToolNamePattern>,
    overrides: BTreeMap<String, ToolAnnotationOverride>,
}

/// One compiled tool-name selector used by `include` and `exclude`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolNamePattern {
    raw: String,
    segments: Vec<String>,
    starts_with_wildcard: bool,
    ends_with_wildcard: bool,
}

/// Optional per-tool annotation overrides applied to outward tool metadata.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolAnnotationOverride {
    /// Overrides the read-only hint when present.
    pub(crate) read_only: Option<bool>,
    /// Overrides the destructive hint when present.
    pub(crate) destructive: Option<bool>,
    /// Overrides the open-world or external-side-effect hint when present.
    pub(crate) open_world: Option<bool>,
}

/// Stable internal identifier for one configured upstream server instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct UpstreamInstanceId(String);

/// Optional outward prefix applied to tools from one configured upstream.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ToolPrefix(String);

#[derive(Debug, Deserialize)]
struct RawHubConfig {
    #[serde(default)]
    servers: IndexMap<String, RawUpstreamServerConfig>,
}

#[derive(Debug, Deserialize)]
struct RawUpstreamServerConfig {
    #[serde(default)]
    prefix: Option<String>,
    command: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default = "default_stderr_enabled")]
    stderr: bool,
    #[serde(default)]
    tools: RawUpstreamToolConfig,
}

#[derive(Debug, Default, Deserialize)]
struct RawUpstreamToolConfig {
    #[serde(default)]
    include: BTreeSet<String>,
    #[serde(default)]
    exclude: BTreeSet<String>,
    #[serde(default)]
    overrides: BTreeMap<String, RawToolAnnotationOverride>,
}

#[derive(Debug, Default, Deserialize)]
struct RawToolAnnotationOverride {
    #[serde(default)]
    read_only: Option<bool>,
    #[serde(default)]
    destructive: Option<bool>,
    #[serde(
        default,
        alias = "external",
        alias = "external_side_effect",
        alias = "sends_external_data"
    )]
    open_world: Option<bool>,
}

impl HubConfig {
    /// Loads and validates the hub configuration from a TOML file.
    pub(crate) async fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|source| ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        Self::parse(path, &raw)
    }

    /// Parses and validates one TOML configuration document.
    fn parse(path: &Path, raw: &str) -> Result<Self, ConfigError> {
        let config = toml::from_str::<RawHubConfig>(raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        Self::validate(config)
    }

    /// Validates raw deserialized configuration and normalizes keyed upstream identifiers.
    fn validate(config: RawHubConfig) -> Result<Self, ConfigError> {
        if config.servers.is_empty() {
            return Err(ConfigError::Validation(
                "at least one upstream server must be configured".to_string(),
            ));
        }

        let mut servers = Vec::with_capacity(config.servers.len());

        for (raw_instance_id, server) in config.servers {
            let instance_id = UpstreamInstanceId::try_from(raw_instance_id).map_err(|error| {
                ConfigError::Validation(format!("invalid upstream identity: {error}"))
            })?;

            if server.command.as_os_str() == OsStr::new("") {
                return Err(ConfigError::Validation(format!(
                    "upstream '{}' has an empty command",
                    instance_id
                )));
            }

            let prefix = server
                .prefix
                .map(ToolPrefix::try_from)
                .transpose()
                .map_err(|error| {
                    ConfigError::Validation(format!("invalid upstream prefix: {error}"))
                })?;
            let tools =
                UpstreamToolConfig::try_from(server.tools).map_err(ConfigError::Validation)?;

            servers.push(UpstreamServerConfig {
                instance_id,
                command: server.command,
                args: server.args,
                env: server.env,
                stderr: server.stderr,
                prefix,
                tools,
            });
        }

        Ok(Self { servers })
    }
}

/// Returns the default stderr capture policy for configured upstream processes.
fn default_stderr_enabled() -> bool {
    true
}

impl UpstreamServerConfig {
    /// Returns the outward tool name for one original upstream tool.
    pub(crate) fn outward_tool_name(&self, tool_name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}.{tool_name}"),
            None => tool_name.to_string(),
        }
    }
}

impl UpstreamToolConfig {
    /// Returns `true` when one original upstream tool should be exposed.
    pub(crate) fn exposes_tool(&self, tool_name: &str) -> bool {
        if !self.include.is_empty() {
            self.include
                .iter()
                .any(|pattern| pattern.matches(tool_name))
        } else {
            !self
                .exclude
                .iter()
                .any(|pattern| pattern.matches(tool_name))
        }
    }

    /// Returns one configured annotation override for the original tool name.
    pub(crate) fn annotation_override(&self, tool_name: &str) -> Option<&ToolAnnotationOverride> {
        self.overrides.get(tool_name)
    }

    /// Returns the exact upstream tool names that have configured annotation overrides.
    pub(crate) fn override_names(&self) -> impl Iterator<Item = &str> {
        self.overrides.keys().map(String::as_str)
    }
}

impl TryFrom<RawUpstreamToolConfig> for UpstreamToolConfig {
    type Error = String;

    /// Validates raw per-upstream tool configuration.
    fn try_from(value: RawUpstreamToolConfig) -> Result<Self, Self::Error> {
        validate_tool_patterns("include", value.include.iter())?;
        validate_tool_patterns("exclude", value.exclude.iter())?;
        validate_exact_tool_names("overrides", value.overrides.keys())?;

        Ok(Self {
            include: value
                .include
                .into_iter()
                .map(ToolNamePattern::try_from)
                .collect::<Result<_, _>>()?,
            exclude: value
                .exclude
                .into_iter()
                .map(ToolNamePattern::try_from)
                .collect::<Result<_, _>>()?,
            overrides: value
                .overrides
                .into_iter()
                .map(|(tool_name, override_config)| {
                    (
                        tool_name,
                        ToolAnnotationOverride {
                            read_only: override_config.read_only,
                            destructive: override_config.destructive,
                            open_world: override_config.open_world,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl ToolNamePattern {
    /// Returns `true` when one original upstream tool name matches this selector.
    fn matches(&self, tool_name: &str) -> bool {
        if self.segments.is_empty() {
            return true;
        }

        if !self.starts_with_wildcard && !self.ends_with_wildcard && self.segments.len() == 1 {
            return tool_name == self.raw;
        }

        let mut search_start = 0;
        let mut segment_index = 0;

        if !self.starts_with_wildcard {
            let prefix = &self.segments[0];
            if !tool_name.starts_with(prefix) {
                return false;
            }
            search_start = prefix.len();
            segment_index = 1;
        }

        let middle_end = if self.ends_with_wildcard {
            self.segments.len()
        } else {
            self.segments.len().saturating_sub(1)
        };

        for segment in &self.segments[segment_index..middle_end] {
            let Some(offset) = tool_name[search_start..].find(segment) else {
                return false;
            };
            search_start += offset + segment.len();
        }

        if self.ends_with_wildcard {
            true
        } else {
            let Some(last_segment) = self.segments.last() else {
                return true;
            };
            tool_name[search_start..].ends_with(last_segment)
        }
    }
}

impl TryFrom<String> for ToolNamePattern {
    type Error = String;

    /// Validates and compiles one `include` or `exclude` wildcard selector.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("tool selector must not be empty".to_string());
        }

        Ok(Self {
            segments: value
                .split('*')
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            starts_with_wildcard: value.starts_with('*'),
            ends_with_wildcard: value.ends_with('*'),
            raw: value,
        })
    }
}

impl TryFrom<String> for UpstreamInstanceId {
    type Error = String;

    /// Validates and wraps one configured upstream identity.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_identifier(value, "upstream identity").map(Self)
    }
}

impl TryFrom<String> for ToolPrefix {
    type Error = String;

    /// Validates and wraps one configured outward tool prefix.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_identifier(value, "tool prefix").map(Self)
    }
}

impl std::fmt::Display for UpstreamInstanceId {
    /// Formats the upstream identity without additional decoration.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for ToolPrefix {
    /// Formats the outward tool prefix without additional decoration.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Validates one configured identifier used for upstream names or outward prefixes.
fn validate_identifier(value: String, label: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }

    if value.contains('.') {
        return Err(format!(
            "{label} '{value}' must not contain '.' because '.' is reserved for outward tool prefixing"
        ));
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "{label} '{value}' may only contain ASCII letters, digits, '-' or '_'"
        ));
    }

    Ok(value)
}

/// Validates that one tool-pattern collection does not contain empty entries.
fn validate_tool_patterns<'a>(
    label: &str,
    patterns: impl IntoIterator<Item = &'a String>,
) -> Result<(), String> {
    patterns
        .into_iter()
        .find(|pattern| pattern.is_empty())
        .map_or(Ok(()), |_| {
            Err(format!("{label} must not contain an empty tool selector"))
        })
}

/// Validates that one exact-name collection does not contain empty or wildcard entries.
fn validate_exact_tool_names<'a>(
    label: &str,
    names: impl IntoIterator<Item = &'a String>,
) -> Result<(), String> {
    names
        .into_iter()
        .find_map(|name| {
            if name.is_empty() {
                Some(format!("{label} must not contain an empty tool name"))
            } else if name.contains('*') {
                Some(format!(
                    "{label} entry '{name}' must not contain '*' because wildcard matching is only supported in include and exclude filters"
                ))
            } else {
                None
            }
        })
        .map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::HubConfig;

    /// Verifies the simple configuration case requires only launch information.
    #[test]
    fn parses_simple_config_without_name_or_prefix() {
        let raw = r#"
[servers.filesystem]
command = "/usr/bin/mock-server"
"#;

        let config =
            HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect("simple config should parse");

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].instance_id.to_string(), "filesystem");
        assert!(config.servers[0].prefix.is_none());
        assert_eq!(config.servers[0].command, Path::new("/usr/bin/mock-server"));
        assert!(config.servers[0].stderr);
        assert!(config.servers[0].tools.exposes_tool("echo"));
    }

    /// Verifies the advanced configuration shape supports filters and overrides.
    #[test]
    fn parses_advanced_config_with_filters_and_overrides() {
        let raw = r#"
[servers.github]
prefix = "gh"
command = "/usr/bin/mock-server"
args = ["--mode", "advanced"]
stderr = false

[servers.github.env]
MOCK_SERVER_NAME = "github"

[servers.github.tools]
include = ["echo", "publish_webhook"]
exclude = ["duplicate"]

[servers.github.tools.overrides.publish_webhook]
read_only = false
destructive = false
external = true
"#;

        let config =
            HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect("advanced config should parse");
        let server = &config.servers[0];

        assert_eq!(server.instance_id.to_string(), "github");
        assert_eq!(
            server.prefix.as_ref().map(ToString::to_string),
            Some("gh".to_string())
        );
        assert_eq!(
            server.args,
            vec!["--mode".to_string(), "advanced".to_string()]
        );
        assert_eq!(
            server.env.get("MOCK_SERVER_NAME"),
            Some(&"github".to_string())
        );
        assert!(!server.stderr);
        assert!(server.tools.exposes_tool("echo"));
        assert!(!server.tools.exposes_tool("duplicate"));
        assert_eq!(server.outward_tool_name("echo"), "gh.echo");

        let override_config = server
            .tools
            .annotation_override("publish_webhook")
            .expect("override should be present");
        assert_eq!(override_config.read_only, Some(false));
        assert_eq!(override_config.destructive, Some(false));
        assert_eq!(override_config.open_world, Some(true));
    }

    /// Verifies keyed server tables preserve the declaration order from TOML.
    #[test]
    fn preserves_keyed_server_declaration_order() {
        let raw = r#"
[servers.beta]
command = "/usr/bin/beta-server"

[servers.alpha]
command = "/usr/bin/alpha-server"
"#;

        let config =
            HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect("ordered config should parse");

        let instance_ids = config
            .servers
            .iter()
            .map(|server| server.instance_id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(instance_ids, vec!["beta".to_string(), "alpha".to_string()]);
    }

    /// Verifies wildcard include and exclude selectors match full upstream tool names.
    #[test]
    fn parses_wildcard_filters_and_matches_tools() {
        let raw = r#"
[servers.github]
command = "/usr/bin/mock-server"

[servers.github.tools]
include = ["read_*shot", "*webhook", "resource_*"]
exclude = ["*counter*"]
"#;

        let config =
            HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect("wildcard config should parse");
        let tools = &config.servers[0].tools;

        assert!(tools.exposes_tool("read_snapshot"));
        assert!(tools.exposes_tool("publish_webhook"));
        assert!(tools.exposes_tool("resource_bundle"));
        assert!(!tools.exposes_tool("session_counter"));
        assert!(!tools.exposes_tool("echo"));
    }

    /// Verifies `*` acts as a full match-all selector for tool names.
    #[test]
    fn wildcard_star_matches_all_tools() {
        let raw = r#"
[servers.github]
command = "/usr/bin/mock-server"

[servers.github.tools]
include = ["*"]
"#;

        let config =
            HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect("wildcard config should parse");
        let tools = &config.servers[0].tools;

        assert!(tools.exposes_tool("echo"));
        assert!(tools.exposes_tool("read_snapshot"));
        assert!(tools.exposes_tool("publish_webhook"));
        assert!(tools.exposes_tool("resource_bundle"));
        assert!(tools.exposes_tool("session_counter"));
    }

    /// Verifies override keys stay exact-only and do not accept wildcard syntax.
    #[test]
    fn rejects_wildcard_override_names() {
        let raw = r#"
[servers.github]
command = "/usr/bin/mock-server"

[servers.github.tools.overrides."publish_*"]
open_world = false
"#;

        let error = HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect_err("config must fail");
        assert!(
            error
                .to_string()
                .contains("must not contain '*' because wildcard matching is only supported"),
            "unexpected error: {error}"
        );
    }

    /// Verifies invalid keyed upstream identifiers are rejected during validation.
    #[test]
    fn rejects_invalid_keyed_upstream_identifier() {
        let raw = r#"
[servers."server.with.dot"]
command = "/usr/bin/mock-server"
"#;

        let error = HubConfig::parse(Path::new("mcp-hub.toml"), raw).expect_err("config must fail");
        assert!(
            error.to_string().contains("must not contain '.'"),
            "unexpected error: {error}"
        );
    }

    /// Verifies the root example config stays parseable and aligned with the supported surface.
    #[test]
    fn parses_root_example_config() {
        let raw = include_str!("../mcp-hub.example.toml");
        let config = HubConfig::parse(Path::new("mcp-hub.example.toml"), raw)
            .expect("root example config should parse");

        assert_eq!(config.servers.len(), 3);

        let find_server = |instance_id: &str| {
            config
                .servers
                .iter()
                .find(|server| server.instance_id.to_string() == instance_id)
                .unwrap_or_else(|| panic!("server '{instance_id}' should exist in the example"))
        };

        let filesystem = find_server("filesystem");
        assert!(filesystem.prefix.is_none());
        assert!(filesystem.stderr);
        assert!(filesystem.tools.exposes_tool("read_file"));

        let github_read = find_server("github_read");
        assert_eq!(
            github_read.prefix.as_ref().map(ToString::to_string),
            Some("gh".to_string())
        );
        assert!(github_read.tools.exposes_tool("list_issues"));
        assert!(github_read.tools.exposes_tool("read_issue"));
        assert!(github_read.tools.exposes_tool("search_code"));
        assert!(github_read.tools.exposes_tool("create_comment"));
        assert!(!github_read.tools.exposes_tool("delete_issue"));
        assert_eq!(
            github_read
                .tools
                .annotation_override("create_comment")
                .expect("create_comment override should exist")
                .open_world,
            Some(true)
        );

        let github_admin = find_server("github_admin");
        assert!(!github_admin.stderr);
        assert_eq!(
            github_admin.prefix.as_ref().map(ToString::to_string),
            Some("gh_admin".to_string())
        );
        assert!(!github_admin.tools.exposes_tool("list_issues"));
        assert!(!github_admin.tools.exposes_tool("read_issue"));
        assert!(!github_admin.tools.exposes_tool("search_code"));
        assert!(github_admin.tools.exposes_tool("delete_issue"));
        assert!(github_admin.tools.exposes_tool("create_release"));
        let delete_issue = github_admin
            .tools
            .annotation_override("delete_issue")
            .expect("delete_issue override should exist");
        assert_eq!(delete_issue.destructive, Some(true));
        assert_eq!(delete_issue.open_world, None);
    }
}
