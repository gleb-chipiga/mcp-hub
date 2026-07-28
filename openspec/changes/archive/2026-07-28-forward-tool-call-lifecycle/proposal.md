## Why

The hub currently routes a `tools/call` result but discards the MCP lifecycle
signals around that call. Clients therefore cannot stop work already delegated
to an upstream or observe standard upstream progress for a long-running tool.

## What Changes

- Track each in-flight outward `tools/call` through its owning upstream request
  for the lifetime of the call.
- Forward an incoming `notifications/cancelled` for an active tool call to the
  owning upstream, rewriting only the request ID and preserving the cancellation
  reason and metadata. Cancellation remains best effort.
- Forward an upstream `notifications/progress` only when it matches an active
  routed call whose client supplied an MCP progress token; preserve the token and
  progress payload.
- Keep the existing single final `tools/call` result contract; do not add custom
  partial-result or streaming behavior.
- Add integration coverage and update the README's operational behavior and
  roadmap to describe the completed lifecycle forwarding.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tool-aggregation-hub`: Routed tool calls gain standard cancellation and
  progress-notification forwarding semantics.
- `project-documentation`: Operational documentation and the roadmap reflect the
  implemented lifecycle-forwarding behavior.

## Impact

- Affects `src/hub.rs` and `src/runtime.rs`, where the outward request context,
  upstream request IDs, and upstream notifications meet.
- Extends the mock upstream and end-to-end integration tests with cancellable and
  progress-emitting tools plus notification-recording client handlers.
- Uses the existing `rmcp` request handles and notification APIs; no configuration
  or dependency changes are expected.
