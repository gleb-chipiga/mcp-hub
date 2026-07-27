## Why

`mcp-hub` deliberately keeps its tool inventory fixed for one stdio session, but it
currently discards an upstream `notifications/tools/list_changed` notification.
Operators therefore cannot distinguish an unchanged upstream from one whose tools
changed after startup.

## What Changes

- Receive upstream `notifications/tools/list_changed` notifications through a
  dedicated upstream client handler.
- Emit one structured warning per notification with the upstream instance ID and an
  explicit statement that the startup inventory remains in use.
- Retain the existing immutable per-session inventory and do not advertise or send
  outward tool-list change notifications.
- Add integration coverage for the warning and unchanged outward inventory.
- Update the README operational behavior and resolve the roadmap item without
  claiming that all tool-list change notifications are unsupported.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tool-aggregation-hub`: Warn when an upstream reports a tool-list change while
  retaining the startup-discovered inventory for the session.

## Impact

- Affected code: `src/runtime.rs` upstream client initialization and handler.
- Affected tests: `src/bin/mock_upstream_server.rs` and
  `tests/tool_aggregation_hub.rs`.
- Affected documentation: README operational behavior, roadmap, and unsupported
  behavior list.
- No configuration, dependency, or outward MCP API changes.
