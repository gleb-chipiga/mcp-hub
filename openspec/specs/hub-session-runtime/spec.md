## Purpose

Defines startup validation, stdio session boundaries, and upstream runtime isolation
for `mcp-hub`.
## Requirements
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

### Requirement: Hub captures enabled upstream stderr as structured diagnostics
For each configured upstream whose `stderr` setting is enabled, the hub SHALL drain
the child process's stderr asynchronously from process spawn until that upstream
runtime is torn down. It SHALL emit each diagnostic record at `INFO` on the
`mcp_hub::upstream_stderr` tracing target with `upstream_instance_id` and
`upstream_stderr` fields. It SHALL bound an unterminated diagnostic record without
blocking the MCP transport or allocating without limit.

#### Scenario: Enabled upstream emits diagnostics
- **WHEN** an enabled upstream writes a diagnostic record to stderr
- **THEN** the hub emits a structured `INFO` event on `mcp_hub::upstream_stderr`
- **AND** the event includes that upstream's instance ID and decoded diagnostic text

#### Scenario: Startup diagnostics do not block initialization
- **WHEN** an enabled upstream emits stderr before completing MCP initialization
- **THEN** the hub drains that stderr while initialization is in progress
- **AND** the upstream can complete normal startup discovery

#### Scenario: Individual upstream diagnostics are disabled
- **WHEN** an upstream is configured with `stderr = false`
- **THEN** its stderr is discarded instead of being inherited or emitted through hub tracing
- **AND** the upstream's MCP stdin and stdout transport continue to operate normally

#### Scenario: Operators filter the diagnostics target
- **WHEN** the hub process has a `RUST_LOG` directive that disables `mcp_hub::upstream_stderr`
- **THEN** stderr diagnostic events are not written by the hub tracing subscriber
- **AND** the hub continues draining the enabled upstream stderr stream

#### Scenario: Upstream runtime ends
- **WHEN** the hub cancels an upstream service during startup rollback or session shutdown
- **THEN** the hub stops that upstream's stderr drain without retaining a detached diagnostic task
