## MODIFIED Requirements

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

## ADDED Requirements

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
