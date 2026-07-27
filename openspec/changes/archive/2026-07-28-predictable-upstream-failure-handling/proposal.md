## Why

The hub currently continues startup with an empty tool inventory when every upstream
fails discovery or contributes no routable tools. This gives MCP clients a valid
session with no useful capability and makes total upstream failure hard to distinguish
from an intentionally empty configuration.

When an upstream exits after startup, the client receives an MCP error but hub logs do
not contain a structured record that identifies the failed route and transport cause.

## What Changes

- **BREAKING** Reject startup when discovery finishes without any usable outward tool
  route, while retaining partial availability when at least one route remains usable.
- Emit a structured warning for a routed tool call that fails at the upstream transport
  or protocol layer, including the upstream instance ID, original tool name, and
  underlying error.
- Preserve the fixed session inventory after an upstream exits; its advertised tools
  remain listed and later calls return an MCP error.
- Add end-to-end coverage for total discovery failure and for an upstream process that
  exits after its inventory was registered. Retain and align existing partial-startup
  and tool-name-collision coverage.
- Update the README operational behavior and roadmap to match the completed behavior.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `tool-aggregation-hub`: Require at least one usable discovered route, define
  post-startup upstream failure diagnostics, and retain fixed inventory behavior.
- `hub-session-runtime`: Require startup validation to reject an empty usable route
  registry after all configured upstreams have been considered.

## Impact

- Affected runtime behavior: `src/runtime.rs` and the hub startup error surface.
- Affected observability: `tracing` warnings emitted for failed routed calls.
- Affected tests: `src/bin/mock_upstream_server.rs` and
  `tests/tool_aggregation_hub.rs`.
- Affected documentation: README operational behavior and roadmap.
- No configuration, dependency, or MCP API additions are required.
