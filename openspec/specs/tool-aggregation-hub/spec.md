## Purpose

Defines discovery, aggregation, routing, and metadata semantics for the hub's
tool-only MCP surface.

## Requirements

### Requirement: Hub exposes a merged outward tool inventory
The hub SHALL construct one outward MCP tool inventory during startup discovery from
configured upstream MCP servers that initialize successfully, return their tool lists,
and contribute at least one usable outward route. It SHALL fail startup when discovery
finishes without a usable outward route.

#### Scenario: Merge tools from multiple upstream servers
- **WHEN** an inbound hub session lists tools and multiple upstream servers are available
- **THEN** the hub returns one combined tool list containing tools from those upstream servers

#### Scenario: Omit upstream tools unavailable during startup discovery
- **WHEN** one or more upstream servers cannot start, initialize, or list tools during startup discovery
- **THEN** the hub returns tools from successfully discovered upstream servers without requiring the entire hub startup to fail
- **AND** the hub starts only when at least one usable outward tool route remains

#### Scenario: Reject an empty usable inventory after discovery
- **WHEN** every configured upstream fails startup or discovery, or no discovered tool survives hub filtering
- **THEN** the hub fails startup before serving the outward MCP surface
- **AND** the startup error states that no usable upstream tools remain after discovery

### Requirement: Hub emits structured diagnostics for failed routed calls
The hub SHALL emit one structured warning when a routed call fails at the upstream
transport or protocol layer. The warning SHALL include the owning upstream instance
ID, the original upstream tool name, and the transport error.

#### Scenario: Routed upstream call fails
- **WHEN** a client invokes an outward tool and its owning upstream call returns a transport or protocol error
- **THEN** the hub emits one warning with `upstream_instance_id`, `original_tool_name`, and `transport_error` fields
- **AND** the hub returns an MCP error for that call

#### Scenario: Upstream returns a tool-level error result
- **WHEN** an upstream successfully returns a `CallToolResult` whose `is_error` value is true
- **THEN** the hub forwards that result unchanged
- **AND** the hub does not emit a routed-call transport failure warning

### Requirement: Hub keeps the discovered tool inventory stable for a session
The hub SHALL retain the outward tool registry discovered during startup for the
lifetime of the inbound session. It SHALL not probe upstream health, retry failed
startup, reconnect an upstream, or refresh the registry automatically. When an
active upstream sends `notifications/tools/list_changed`, the hub SHALL emit one
structured warning that includes `upstream_instance_id` and SHALL retain that same
registry without advertising or sending an outward tool-list change notification.

#### Scenario: Upstream exits after startup discovery
- **WHEN** an upstream server exits after its tools were added to the outward registry
- **THEN** its tools remain in the outward tool list for the current session
- **AND** a call to one of those tools returns an error that reports the upstream
  transport failure

#### Scenario: Upstream announces a tool-list change after startup discovery
- **WHEN** an active upstream sends `notifications/tools/list_changed` after its
  tools were added to the outward registry
- **THEN** the hub emits one warning with the `upstream_instance_id` field
- **AND** a subsequent `tools/list` response returns the same outward inventory
- **AND** the hub does not advertise or send an outward tool-list change notification

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
