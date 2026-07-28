## Context

`HubServer` currently ignores the outward `RequestContext` when it delegates a
tool call to `SessionRuntime`. The runtime invokes `Peer::call_tool`, which
returns the final result but does not expose the upstream JSON-RPC request ID.
`UpstreamClient` handles only `tools/list_changed`, so upstream cancellation and
progress notifications are discarded.

The existing `rmcp` APIs provide the required primitives: an outward request
context has the client request ID and peer, `send_cancellable_request` returns a
`RequestHandle` with the upstream request ID, and both peer roles can send the
standard cancellation and progress notifications. `rmcp` generates a new
progress token for every outbound request, replacing any token in that request's
metadata. The hub must therefore translate the generated upstream token back to
the client token rather than relying on token pass-through.

The process remains a one-client stdio session with a separately initialized
client session for every configured upstream. No configuration change is needed.

## Goals / Non-Goals

**Goals:**

- Forward a client cancellation to exactly the upstream request that owns an
  active routed `tools/call`.
- Forward only progress emitted for an active routed call that had a valid
  client-supplied progress token, preserving the client-visible token and the
  rest of the progress payload.
- Handle concurrent calls, duplicate client token values, and cancellation that
  races upstream request creation without cross-upstream routing or leaked state.
- Preserve the present one-final-result call contract and existing error mapping.

**Non-Goals:**

- Guarantee that an upstream stops work or manufacture a cancelled result after
  forwarding the notification.
- Add retries, tool-call timeouts, task execution, custom partial results, or a
  streamed-result protocol.
- Forward arbitrary upstream notifications or an upstream-initiated cancellation
  to the client.

## Decisions

### Share a session-local in-flight call tracker across both protocol directions

`SessionRuntime` will own an `Arc`-backed tracker that is shared with
`HubServer` and every `UpstreamClient`. It will maintain a request map keyed by
the outward JSON-RPC request ID and a progress map keyed by
`(UpstreamInstanceId, upstream_progress_token)`. A call record contains the
owning upstream peer, the resolved upstream request ID once allocated, pending
cancellation data when necessary, and the progress-route key.

The tracker is registered before the first await that sends work upstream. Its
lock only reads or mutates the maps; notification I/O happens after releasing the
lock. This provides a shared correlation boundary without holding a Tokio lock
across stdio I/O. The outer request ID cannot be sent upstream because each MCP
connection owns an independent JSON-RPC ID namespace.

Alternative considered: infer the route from tool name on cancellation and
progress. This cannot distinguish concurrent calls to the same tool and progress
notifications contain a token, not a tool name or request ID. Alternative
considered: use only a progress-token map. That cannot forward cancellation and
could mix two upstreams using the same token.

### Use a cancellable upstream request and translate both identifiers and tokens

The runtime will construct the rewritten `CallToolRequestParams` as today, then
send it with `Peer::send_cancellable_request` and await its `RequestHandle` for
the final result. The handle's `id` is bound to the previously registered
outward request ID. The handle's generated `progress_token` is used as the
upstream-side progress-map key.

When the client supplied a valid `_meta.progressToken`, the matching progress
entry stores that original token and the outward server peer. When the owning
`UpstreamClient` receives progress for the generated token, it forwards a cloned
notification through the outward peer after replacing only its token with the
original client token. It preserves `progress`, `total`, `message`, and `_meta`.
Calls without a valid client token do not receive a progress entry, so their
upstream notifications are ignored.

Using the upstream-generated token is deliberate: `rmcp` assigns it when the
request is serialized, and it is unique per upstream peer. Forwarding that token
unchanged would break the client-side correlation; forcing the client token into
the upstream request would depend on internals and risks collisions.

### Queue a cancellation that arrives before upstream request binding

`HubServer::on_cancelled` will delegate notifications with a request ID to the
tracker. If the matching call already has an upstream request ID, it will prepare
one outgoing `notifications/cancelled`; if request creation is still pending, it
will retain the first cancellation parameters on that call record. Binding the
request handle then emits the retained notification before awaiting the final
result. The outgoing notification replaces `requestId` with the upstream ID and
preserves the incoming `reason` and `_meta`.

The tracker will mark the call cancelled and remove any progress route when the
cancellation is accepted, preventing new progress lookups for that call. Unknown
or already-completed request IDs, and cancellation notifications without an ID,
are ignored because no safe upstream correlation exists. Duplicate cancellation
notifications are coalesced to one downstream attempt.

Alternative considered: call `RequestHandle::cancel`. Its public API retains a
reason but cannot retain the incoming cancellation metadata; the tracker instead
sends the standard notification through the stored upstream peer.

### Remove correlations deterministically and keep failures non-fatal

The runtime will remove the request and any progress entry after the upstream
handle resolves, whether it yields a result or a transport/protocol error. It
will also clear the tracker during hub shutdown before upstream services are
cancelled. The cleanup operation is idempotent so a cancellation and a completed
call can race safely.

Notification-send failures do not alter the result path: they are recorded with
the upstream ID and relevant request or token context, and the hub continues to
serve. The existing final-result error conversion remains responsible for
upstream request failures. `rmcp` already suppresses an outward response for a
locally cancelled inbound request, so the hub does not invent another response.

## Risks / Trade-offs

- [An upstream ignores cancellation or finishes before it arrives] -> Forward
  the standard notification once and document cancellation as best effort; retain
  the normal final-result path internally.
- [Cancellation races request submission] -> Register before sending, retain a
  pending cancellation until the upstream request ID exists, then forward it.
- [An upstream progress notification races cancellation or completion] -> Remove
  the progress route at cancellation and cleanup; a notification already being
  forwarded may still win the transport race, consistent with asynchronous MCP
  notification ordering.
- [A client reuses one progress token for concurrent calls] -> Keep unique
  upstream-side keys and forward each matching event with the client-provided
  token; client-side disambiguation remains the client's responsibility.
- [Test clients automatically generate progress tokens] -> Use controllable
  request handles and a raw JSON-RPC fixture where needed to cover absence of an
  externally supplied token.

## Migration Plan

1. Add the session-local tracking and notification forwarding implementation with
   mock-upstream and end-to-end coverage.
2. Update the README operational behavior and remove the implemented roadmap
   subsection.
3. Release normally; no configuration migration, persisted state, or protocol
   capability renegotiation is required. A code rollback simply removes the
   forwarding behavior.

## Open Questions

None.
