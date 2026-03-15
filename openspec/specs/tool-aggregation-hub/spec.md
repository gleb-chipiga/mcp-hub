## ADDED Requirements

### Requirement: Hub exposes a merged outward tool inventory
The hub SHALL present one outward MCP tool inventory composed from all configured and currently available upstream MCP servers.

#### Scenario: Merge tools from multiple upstream servers
- **WHEN** an inbound hub session lists tools and multiple upstream servers are available
- **THEN** the hub returns one combined tool list containing tools from those upstream servers

#### Scenario: Omit unavailable upstream tools
- **WHEN** an inbound hub session lists tools and one or more upstream servers are unavailable
- **THEN** the hub returns tools from available upstream servers without requiring the entire inventory request to fail

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
