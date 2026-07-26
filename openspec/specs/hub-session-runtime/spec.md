## ADDED Requirements

### Requirement: Hub validates startup config before serving clients
The hub SHALL validate startup configuration before initializing the outward MCP server, including validation of the discovered outward tool registry for ambiguity after filtering and prefix rewriting.

#### Scenario: Reject duplicate outward tool names during startup validation
- **WHEN** startup discovery shows that two active upstream servers would expose the same outward tool name after include or exclude filtering, task-only tool omission, and prefix application
- **THEN** the hub fails startup before serving the outward MCP surface
- **AND** any diagnostics that identify the conflicting upstream entries reflect the order those servers were declared in the TOML configuration

#### Scenario: Accept unambiguous outward tool naming during startup validation
- **WHEN** startup discovery finishes and each exposed outward tool name is unique after filtering and prefix application
- **THEN** the hub may continue initialization and serve the outward MCP surface

### Requirement: Hub isolates upstream state per inbound session
The hub SHALL treat each `mcp-hub` process as one inbound stdio session with an
isolated upstream runtime. It SHALL NOT share session-local upstream state with a
separately launched hub process.

#### Scenario: Separate hub processes do not share upstream session state
- **WHEN** two MCP clients each launch `mcp-hub` with the same upstream configuration
- **THEN** each hub process maintains separate upstream sessions

#### Scenario: Closing one hub process does not terminate another process's upstream runtime
- **WHEN** one `mcp-hub` process ends while another separately launched process remains active
- **THEN** only the ended process tears down its upstream runtime

### Requirement: Hub keeps the v1 inbound transport model explicit
The hub SHALL expose one stdio MCP endpoint per process. Its standard input and
output belong to one MCP client, so a process SHALL NOT accept multiple independent
inbound clients.

#### Scenario: Stdio serves one client per process
- **WHEN** an MCP host launches the hub over stdio
- **THEN** that hub process owns one inbound MCP session and one isolated upstream runtime set
- **AND** a second MCP client must launch another hub process rather than connect to the existing process
