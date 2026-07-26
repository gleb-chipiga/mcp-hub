## ADDED Requirements

### Requirement: Hub exposes a merged outward tool inventory
The hub SHALL construct one outward MCP tool inventory during startup discovery from
configured upstream MCP servers that initialize successfully and return their tool
lists.

#### Scenario: Merge tools from multiple upstream servers
- **WHEN** an inbound hub session lists tools and multiple upstream servers are available
- **THEN** the hub returns one combined tool list containing tools from those upstream servers

#### Scenario: Omit upstream tools unavailable during startup discovery
- **WHEN** one or more upstream servers cannot start, initialize, or list tools during startup discovery
- **THEN** the hub returns tools from successfully discovered upstream servers without requiring the entire hub startup to fail
- **AND** the hub may start with an empty tool inventory when no upstream is successfully discovered

### Requirement: Hub keeps the discovered tool inventory stable for a session
The hub SHALL retain the outward tool registry discovered during startup for the
lifetime of the inbound session. It SHALL not probe upstream health, retry failed
startup, reconnect an upstream, or refresh the registry automatically.

#### Scenario: Upstream exits after startup discovery
- **WHEN** an upstream server exits after its tools were added to the outward registry
- **THEN** its tools remain in the outward tool list for the current session
- **AND** a call to one of those tools returns an error that reports the upstream
  transport failure

#### Scenario: New hub process performs new discovery
- **WHEN** the MCP host starts a new `mcp-hub` process
- **THEN** the hub creates a new inbound session and independently discovers a new
  outward tool inventory

### Requirement: Hub applies optional visible prefixes to outward tool names
The hub SHALL expose each outward tool name either as the original upstream tool name or as `<prefix>.<tool_name>` when a visible prefix is configured for that upstream.

#### Scenario: Preserve original outward tool name without a prefix
- **WHEN** an upstream server is configured without a visible prefix
- **THEN** its outward tool names remain the original upstream tool names

#### Scenario: Prefix outward tool names when requested
- **WHEN** an upstream server is configured with a visible prefix
- **THEN** each exposed tool name from that upstream is rewritten as `<prefix>.<tool_name>`

#### Scenario: Reject unknown outward tool names
- **WHEN** a client calls a tool name that is not present in the hub registry
- **THEN** the hub returns an error indicating that the tool is not available

### Requirement: Hub routes tool calls to the correct upstream tool
The hub SHALL resolve an outward tool name to its owning upstream server and original tool name, and SHALL invoke that exact upstream tool.

#### Scenario: Call a routed upstream tool
- **WHEN** a client calls an outward tool exposed by the hub
- **THEN** the hub invokes the matching original tool on the matching upstream server

### Requirement: Hub exposes a tool-only MCP surface in v1
The hub SHALL advertise and implement only tool-oriented MCP behavior in v1 and SHALL NOT claim support for non-tool capabilities that it does not implement.

#### Scenario: Non-tool capabilities are not advertised
- **WHEN** a client initializes against the hub
- **THEN** the hub advertises tool capability without advertising prompts, resources, roots, sampling, or elicitation capabilities

#### Scenario: Task-required tools are not exposed
- **WHEN** an upstream tool requires task-based execution and the hub does not implement task execution
- **THEN** the hub does not expose that tool in its outward tool inventory

### Requirement: Hub preserves or intentionally overrides outward tool metadata semantics
The hub SHALL preserve semantically relevant metadata from each exposed upstream tool, except for fields that must change because of visible prefixing, unsupported-tool filtering, or explicit config-driven annotation overrides.

#### Scenario: Preserve tool annotations and schemas
- **WHEN** an upstream tool is exposed through the hub without a matching override rule
- **THEN** the outward tool preserves its description, input schema, and metadata or annotations such as read-only, destructive, and external side-effect hints

#### Scenario: Apply per-tool annotation overrides
- **WHEN** a configuration defines an override for a specific upstream tool's read-only, destructive, or external side-effect semantics
- **THEN** the outward tool reflects those configured attributes while preserving unspecified metadata from upstream

#### Scenario: Filtered tools are omitted instead of partially rewritten
- **WHEN** an upstream tool is excluded because the hub does not support its execution model
- **THEN** the hub omits that tool entirely rather than exposing a lossy outward stub with incomplete metadata
