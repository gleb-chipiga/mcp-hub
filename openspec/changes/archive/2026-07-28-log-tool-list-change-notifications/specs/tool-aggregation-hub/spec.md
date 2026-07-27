## MODIFIED Requirements

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
