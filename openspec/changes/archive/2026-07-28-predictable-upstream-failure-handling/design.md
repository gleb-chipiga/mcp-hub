## Context

`SessionRuntime::build` connects to configured upstreams sequentially, discovers
their tools, filters them, and registers outward routes. A failed spawn,
initialization, discovery request, or discovery timeout is logged and omitted. The
current builder returns successfully even when no routes were registered.

The runtime fixes the route registry for the session. A routed call retains the
upstream instance ID, original tool name, and `ServiceError` in `HubCallError`, but no
structured warning is emitted before the error is converted to the outward MCP error.

Existing end-to-end tests already verify partial availability and collisions. The mock
upstream can delay inventory discovery, but it has no deterministic way to terminate
after the hub has registered its inventory.

## Goals / Non-Goals

**Goals:**

- Fail before serving the outward MCP endpoint when discovery produces zero usable
  routes.
- Keep startup available when one or more routes are usable, regardless of omitted
  upstreams.
- Produce a queryable warning for upstream transport or protocol failures during a
  routed call.
- Exercise total startup failure and post-discovery upstream termination
  end-to-end.
- Keep README behavior and roadmap aligned with the implementation.

**Non-Goals:**

- Retry, reconnect, restart, health-check, or refresh upstream sessions.
- Remove routes after an upstream terminates.
- Change tool-level error results into transport errors or warnings.
- Add call timeouts, automatic retries, configuration fields, or MCP protocol
  extensions.

## Decisions

### Reject an empty usable route registry after discovery

After every configured upstream has been considered, startup validation will return a
dedicated build error when `RuntimeState.routes` is empty. A route is usable only when
its upstream initialized and listed tools successfully and at least one advertised
tool survives task filtering and configured include or exclude rules.

This check belongs at the end of aggregate discovery rather than in an individual
upstream branch: it preserves best-effort startup for partial availability and gives
the same outcome to all zero-route causes. The error text will state that no usable
upstream tools remain after discovery; preceding per-upstream warnings retain each
individual cause.

The builder will cancel any retained active upstreams before returning this error. The
current route-registration order means this set should already be empty in the
zero-route case, but explicit cleanup keeps the failure path correct if that invariant
changes.

Alternative considered: fail on the first unavailable upstream. Rejected because it
would remove the existing partial-availability contract.

Alternative considered: start an empty MCP session and surface an error only on
`tools/list`. Rejected because startup cannot then distinguish an upstream outage from
an intentionally empty server and callers receive no clear readiness boundary.

### Emit a warning only for failed routed transport operations

The `peer.call_tool` error path will emit one `tracing::warn!` event before creating
`HubCallError::UpstreamCall`. It will expose stable structured fields named
`upstream_instance_id`, `original_tool_name`, and `transport_error`, with a stable
event message such as `routed upstream tool call failed`.

The existing MCP conversion remains the client-facing error behavior. A valid
`CallToolResult` with `is_error: true` is a tool-level response, not a transport or
protocol failure, so it passes through unchanged and does not emit this warning.

Alternative considered: log only the formatted `HubCallError` in the outward handler.
Rejected because conversion consumes the error and an unstructured message would make
the relevant route attributes difficult to query.

### Use an explicit mock control to terminate an upstream after registration

The test mock will gain a deterministic test-only control that causes its process to
exit when a dedicated routed operation is invoked after startup. The integration test
will first prove that the ordinary tool is present, invoke the termination control,
then verify that the ordinary tool remains listed and its routed call fails through
the existing MCP error path.

This avoids timing-dependent sleeps or process-ID manipulation while directly proving
that an upstream can terminate after the inventory becomes fixed. The control will be
included in test fixture inventory helpers so filtering and collision tests retain
their existing semantic coverage.

Alternative considered: exit after a fixed post-discovery delay. Rejected because the
hub's startup and test scheduling would make that test inherently flaky.

### Keep integration coverage at the process boundary

The all-unavailable test will use the existing startup-failure process helper and
assert the dedicated startup diagnostic. The existing partial-availability and
collision tests will remain as coverage for the unchanged semantics. The termination
test will use the normal client-to-hub-to-upstream process chain to verify both stable
inventory and the propagated call failure.

## Risks / Trade-offs

- [A previously accepted empty inventory becomes a startup error] -> This is an
  intentional breaking behavior change, documented in the README and proposal.
- [The aggregate startup error does not include every individual failure] -> Existing
  structured per-upstream startup warnings identify causes; the aggregate error states
  the readiness outcome.
- [Raw transport errors can vary by operating system or `rmcp` version] -> Tests assert
  the stable MCP error category and hub-owned message, not an exact dependency error
  string.
- [A test-only termination control expands the mock inventory] -> Keep it isolated to
  the test binary and update inventory helper expectations together.

## Migration Plan

No configuration migration is required. Deploy the runtime and documentation change
together. Hosts whose configured upstreams yield no usable routes will now receive a
non-zero startup result and should correct availability or filtering before restart.

Rollback consists of restoring the previous release; no persisted data or protocol
state needs migration.

## Open Questions

- None.
