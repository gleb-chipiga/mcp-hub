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
The hub SHALL treat each inbound hub session as an isolated runtime and SHALL NOT share session-local upstream state across different inbound sessions.

#### Scenario: Separate inbound sessions do not share upstream session state
- **WHEN** two different inbound hub sessions connect to the same configured upstream server
- **THEN** the hub maintains separate upstream sessions for those inbound sessions

#### Scenario: Closing one inbound session does not terminate another session's upstream runtime
- **WHEN** one inbound hub session ends while another inbound hub session remains active
- **THEN** the hub only tears down upstream state associated with the ended session

### Requirement: Hub keeps the v1 inbound transport model explicit
The hub SHALL treat the current v1 implementation as a single-session-per-process transport shape and SHALL NOT silently extend that shape to same-process multi-session transports without adding a per-session runtime boundary.

#### Scenario: Stdio v1 serves one inbound session per process
- **WHEN** the v1 hub runs over stdio
- **THEN** one hub process owns one inbound MCP session and one isolated upstream runtime set

#### Scenario: Future multi-session transports require an explicit session factory boundary
- **WHEN** the project adds an inbound transport that can host multiple sessions in one process
- **THEN** that change must introduce a per-session hub-service factory or equivalent session-bound runtime initialization rather than reusing one prebuilt upstream runtime across sessions
