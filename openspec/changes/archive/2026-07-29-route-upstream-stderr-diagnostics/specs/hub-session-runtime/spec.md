## ADDED Requirements

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
