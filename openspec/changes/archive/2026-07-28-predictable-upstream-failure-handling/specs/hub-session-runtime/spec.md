## MODIFIED Requirements

### Requirement: Hub validates startup config before serving clients
The hub SHALL validate startup configuration before initializing the outward MCP
server, including validation that the discovered outward tool registry is unambiguous
after filtering and prefix rewriting and contains at least one usable outward route.

#### Scenario: Reject duplicate outward tool names during startup validation
- **WHEN** startup discovery shows that two active upstream servers would expose the same outward tool name after include or exclude filtering, task-only tool omission, and prefix application
- **THEN** the hub fails startup before serving the outward MCP surface
- **AND** any diagnostics that identify the conflicting upstream entries reflect the order those servers were declared in the TOML configuration

#### Scenario: Reject an empty usable route registry during startup validation
- **WHEN** startup discovery has considered every configured upstream and no usable outward route was registered
- **THEN** the hub fails startup before serving the outward MCP surface
- **AND** the error clearly states that no usable upstream tools remain after discovery

#### Scenario: Accept unambiguous outward tool naming during startup validation
- **WHEN** startup discovery finishes, each exposed outward tool name is unique after filtering and prefix application, and at least one usable outward route remains
- **THEN** the hub may continue initialization and serve the outward MCP surface
