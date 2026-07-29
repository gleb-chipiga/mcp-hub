## Why

Upstream stderr currently inherits the hub's stderr, bypassing structured tracing
and making output from multiple configured instances difficult to identify or
filter. Operators need correlated diagnostics without risking raw child output on
the hub's own stderr.

## What Changes

- Capture stderr for every enabled upstream process and emit its diagnostic records
  through the shared `mcp_hub::upstream_stderr` tracing target with the upstream
  instance ID.
- Add an optional per-upstream `stderr` boolean configuration field that defaults to
  enabled; `stderr = false` discards that child's stderr.
- Keep stderr draining asynchronous and independent from the MCP stdin/stdout
  transport so diagnostic output cannot block upstream initialization or protocol
  traffic.
- Document the configuration, `RUST_LOG` filtering behavior, and completed runtime
  behavior; remove the implemented upstream-diagnostics roadmap item.
- Add configuration and end-to-end coverage for enabled, disabled, and filtered
  diagnostics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `hub-config`: Upstream process configuration gains a documented optional stderr
  capture switch with a default-enabled behavior.
- `hub-session-runtime`: The session runtime captures configured upstream stderr as
  structured diagnostics while preserving process and MCP transport lifecycle
  behavior.
- `project-documentation`: README operational behavior and roadmap reflect the
  implemented upstream diagnostics capability.

## Impact

- Affects upstream configuration parsing, child-process setup, runtime shutdown, and
  mock-upstream integration fixtures.
- Uses the existing `rmcp` child-process transport builder and Tokio asynchronous I/O;
  no new dependency or MCP capability negotiation is required.
- Adds observable hub log events and an opt-out configuration field without changing
  the outward MCP tool protocol.
