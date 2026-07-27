## Context

The hub discovers, filters, prefixes, and validates every outward tool route during
startup. `SessionRuntime` then serves that immutable registry for one inbound stdio
session. Each upstream client currently uses `ClientInfo` as its `rmcp`
`ClientHandler`, whose default `on_tool_list_changed` implementation ignores the
notification.

The hub must make a post-startup upstream inventory change observable without
partially rebuilding routes or making an unsupported dynamic-inventory promise to
the inbound MCP client.

## Goals / Non-Goals

**Goals:**

- Log one structured warning for every upstream
  `notifications/tools/list_changed` notification.
- Identify the upstream instance in that warning.
- Keep the discovered registry and the outward MCP tool capability unchanged.
- Verify the behavior end to end with an upstream notification over stdio.

**Non-Goals:**

- Refreshing, reconciling, or rediscovering upstream tools during a session.
- Forwarding a tool-list-change notification to the inbound MCP client.
- Adding a health check, reconnection, retry, rate limit, or configuration option.

## Decisions

### Use a dedicated upstream client handler

Replace the `ClientInfo` handler passed to `ServiceExt::serve` with a private
`UpstreamClient` that stores the configured `UpstreamInstanceId`. It will retain the
default client initialization info and override only
`ClientHandler::on_tool_list_changed` to emit the warning.

This attaches the upstream identity at the point where `rmcp` receives the
notification, avoiding global peer-to-instance lookup and additional shared state.

**Alternatives considered:**

- Inspect raw transport messages: rejected because `rmcp` already parses and
  dispatches this standard notification through `ClientHandler`.
- Discover tools again in the callback: rejected because it can change filtering,
  collision, and routing semantics after the client has observed the registry.

### Preserve the startup registry and outward capability

The callback only logs. It does not acquire the runtime state lock, call
`tools/list`, mutate routes, or emit an outward `tools/list_changed` notification.
`HubServer` continues to advertise ordinary tools without the `listChanged`
capability.

This keeps the established stdio-session boundary clear: restarting the hub is the
only way to form a new inventory.

**Alternatives considered:**

- Forward the notification without refreshing inventory: rejected because clients
  could re-query tools and receive the same stale list.
- Rebuild the registry in place: rejected because a changed upstream could create
  collisions or invalidate overrides after startup, and concurrent tool calls would
  need a new consistency model.

### Test through an opt-in mock notification trigger

Extend the mock upstream with an environment-gated test tool that sends
`notifications/tools/list_changed` through its server peer. An integration test will
start the hub with captured stderr, invoke that tool, confirm the outward inventory
is unchanged, close the client, and assert exactly one warning with the upstream
instance ID.

The test trigger is confined to the mock binary and is not exposed by production
configuration or the hub API.

## Risks / Trade-offs

- Repeated upstream notifications can produce repeated warnings → each notification
  is intentionally logged so operators retain an accurate signal; rate limiting is
  deferred until a separate requirement exists.
- An upstream can notify despite not advertising `tools.listChanged` → the hub still
  logs the received standard message and preserves its inventory.
- A notification can race with shutdown → `rmcp` owns dispatching; no runtime state
  mutation means there is no additional shutdown race to coordinate.

## Migration Plan

No configuration, stored state, or protocol migration is required. Deploying the
change adds diagnostics only; rolling back restores the prior silent handling while
the startup inventory behavior remains the same.

## Open Questions

None.
