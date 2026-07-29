## Context

The hub currently starts an upstream with `TokioChildProcess::new`, whose child
stderr setting inherits the hub process's standard error. That output bypasses the
non-blocking tracing subscriber and has no upstream instance correlation. `rmcp`
already exposes `TokioChildProcess::builder`, which can pipe stderr separately from
the MCP stdin/stdout transport and returns a `ChildStderr` handle after spawn.

The hub starts and owns one upstream runtime per configured instance. It already
uses Tokio for process I/O and a non-blocking tracing writer, so stderr forwarding
must remain asynchronous, per-instance, and tied to the upstream lifecycle.

## Goals / Non-Goals

**Goals:**

- Capture enabled upstream stderr from process spawn through teardown without
  interfering with MCP traffic or startup discovery.
- Emit filterable structured diagnostics with a stable target and an upstream
  instance identifier.
- Keep the default configuration useful while allowing noisy upstreams to be
  silenced without raw output leaking to the hub stderr.
- Cover configuration defaults, enabled forwarding, per-upstream suppression, and
  `RUST_LOG` target filtering end to end.

**Non-Goals:**

- Parse, interpret, retain, redact, rotate, or persist upstream log records.
- Forward deprecated MCP logging messages or add a new MCP diagnostics protocol.
- Add process restart, health-check, retry, or remote-transport behavior.
- Let upstream configuration select arbitrary tracing targets, log levels, or sinks.

## Decisions

### Use an optional `stderr` boolean with a default of `true`

`RawUpstreamServerConfig` and `UpstreamServerConfig` will carry a `stderr` field
whose serde default is `true`. Omitting the field retains the default diagnostic
capture; `stderr = false` selects `Stdio::null()` and discards the child's output.
The concise name reflects the child stream being configured and keeps the common
upstream definition unchanged.

Alternative considered: a nested logging table or a multi-valued output policy.
Those interfaces imply destinations and policies the hub does not support. An opt-in
field would also contradict the roadmap's default-enabled diagnostic behavior.

### Build the child transport with piped or null stderr before the MCP handshake

`connect_upstream` will replace `TokioChildProcess::new` with its builder. Enabled
instances use `Stdio::piped()` and disabled instances use `Stdio::null()`; stdout
and stdin remain exclusively owned by the existing MCP transport. Once spawn
returns, the runtime will start draining the returned `ChildStderr` before awaiting
the `serve` handshake. This prevents a child that emits startup diagnostics from
filling its pipe and blocking protocol initialization.

Alternative considered: retain inherited stderr and add a tracing wrapper around the
hub writer. Inherited output cannot be attributed to a configured upstream and
continues to bypass `RUST_LOG` filtering. Starting the drain only after handshake
would retain a pipe-full startup deadlock.

### Emit bounded, text-safe diagnostics on one stable target

The drain task will frame ordinary newline-delimited stderr records, decode bytes
lossily so invalid UTF-8 cannot stop draining, and emit `INFO` events on exactly
the `mcp_hub::upstream_stderr` target. Each event will include
`upstream_instance_id` and an `upstream_stderr` field containing the decoded text.
The default `mcp_hub=debug` filter includes these events; operators can suppress
them with a more specific directive such as
`RUST_LOG=info,mcp_hub::upstream_stderr=off`.

One record is bounded at 64 KiB. An overlong non-newline record is emitted in
bounded fragments with a continuation field instead of accumulating unbounded
memory; ordinary lines remain one event. Read errors emit a structured warning with
the same target and instance ID, then end that drain task without changing the MCP
request path.

Alternative considered: `AsyncBufReadExt::lines()`. It treats invalid UTF-8 as an
error and has no record-size boundary. Logging raw byte chunks avoids unbounded
allocation but produces arbitrary fragments for normal line-oriented diagnostics.

### Own and cancel stderr drains with their upstream runtime

`ActiveUpstream` will retain the drain task handle in addition to the service and
peer. Every startup-failure and normal-shutdown path will first cancel the upstream
service, then stop and await the associated drain task. If client initialization
fails after spawning a child, `connect_upstream` will stop the newly created drain
before returning its error. This prevents detached readers from outliving their
upstream runtime while avoiding an indefinite wait for an inherited stderr handle
from a forked descendant.

Alternative considered: fire-and-forget tasks. Dropping a Tokio `JoinHandle` detaches
the task, which can leave it active after the owning upstream has been removed.

### Test through the existing child-process integration fixtures

The mock upstream will gain environment-controlled direct stderr output during
startup and for a diagnostic tool. Integration fixtures will serialize the optional
`stderr` field and capture the hub stderr. Tests will assert the target, correlated
instance ID, disabled discard behavior, and filtering via `RUST_LOG`. A startup
diagnostic volume larger than a pipe buffer with the target disabled will verify
that draining starts before the MCP handshake without making test output noisy.

## Risks / Trade-offs

- [Upstream output includes secrets or excessive volume] -> Preserve the required
  default capture but provide `stderr = false`, a dedicated `RUST_LOG` target, and
  bounded records; document that the hub does not redact output.
- [A child emits invalid UTF-8 or a very long line] -> Lossy decode bounded fragments
  and continue draining rather than failing or allocating without limit.
- [A read task survives its child] -> Store its handle with `ActiveUpstream` and
  cancel it on every service teardown path.
- [A tracing sink is slow or disabled] -> Use the existing non-blocking tracing
  writer; filtering affects emission but not pipe draining.
- [A child inherits stderr through a descendant] -> Abort the drain after service
  shutdown instead of waiting indefinitely for EOF.

## Migration Plan

1. Add the default-enabled configuration field, piped/null process setup, and
   lifecycle-owned stderr drain.
2. Extend the example TOML, README operational documentation, and integration tests.
3. Release normally with no migration requirement: existing configuration acquires
   default diagnostics, and operators can add `stderr = false` where needed.
4. Rollback removes the new configuration field and forwarding behavior; no persisted
   state or MCP capability is involved.

## Open Questions

None.
