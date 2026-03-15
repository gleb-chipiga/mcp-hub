## Why

The repository is still an empty scaffold, so the first change needs to define a real product shape instead of drifting into ad hoc protocol experiments. The intended `mcp-hub` is not only a tool aggregator, but a configuration-driven stdio hub that can launch multiple upstream MCP servers from one TOML configuration, selectively expose their tools, and optionally adjust tool safety annotations where the upstream metadata is not sufficient.

## What Changes

- Introduce the first working version of `mcp-hub` as a tool-only MCP aggregation hub.
- Read upstream stdio server definitions from a TOML 1.1 configuration file using keyed server tables under `[servers.<instance_id>]`.
- Preserve keyed upstream server processing order from the TOML document so runtime behavior and diagnostics follow declaration order rather than sorted instance ids.
- Add a root-level example TOML 1.1 configuration file that demonstrates the intended config style and supported options.
- Support configuring each upstream server with an executable path or command name, command-line arguments, and environment variables.
- Support connecting to multiple upstream MCP servers and collecting their advertised tools.
- Support per-upstream tool filtering via `include` and `exclude` lists, with `include` taking precedence when both are present.
- Support `include` and `exclude` wildcard masks using `*` as a reserved metacharacter anywhere in the pattern.
- Support an optional outward tool-name prefix per configured upstream, separate from the upstream's internal identity.
- Allow multiple configured copies of the same underlying upstream server binary with different settings and prefixes.
- Expose a merged outward tool inventory through one hub-facing MCP surface.
- Route tool invocations from the outward hub surface to the correct upstream server.
- Define outward tool naming rules that preserve original names when no prefix is configured and prepend a configured prefix when it is configured.
- Validate startup configuration before serving clients, including rejection of outward tool-name collisions after filters and prefixes are applied.
- Support optional per-tool annotation overrides for read-only, destructive, and external side-effect or open-world semantics.
- Reserve `*` for filter-mask semantics and treat literal asterisks in upstream tool names as unsupported rather than trying to disambiguate them.
- Keep the configuration model ergonomic: keyed server tables should stay compact in the simple case, while advanced filtering and overrides remain available without distorting that shape.
- Provide a polished root-level example config that shows both compact and advanced keyed upstream definitions, including arguments, environment variables, prefixes, wildcard filters, and annotation overrides, keeps each server as its own `[servers.<instance_id>]` block, and stays current whenever related config behavior changes.
- Define session and lifecycle behavior for the hub and its upstream connections.
- Use the `mimalloc` crate with feature `v3` as the process-global allocator for project binaries.
- Adopt an explicit error-handling split between `thiserror` and `anyhow` so typed domain errors and top-level contextual failures are not conflated.
- Expand verification to use real `rmcp` clients and servers for protocol-level integration coverage, including specification-version and tool-metadata edge cases.
- Explicitly exclude non-tool MCP capabilities from v1, including prompts, resources, roots, sampling, elicitation, and task-oriented tool execution.

## Capabilities

### New Capabilities
- `hub-config`: Define the keyed TOML 1.1 configuration surface, filtering model, and canonical root example for configured upstream servers.
- `tool-aggregation-hub`: Expose the outward tool-only MCP surface that merges upstream tools, applies optional visible prefixes, preserves or overrides tool metadata as configured, and routes calls to the correct upstream target.
- `hub-session-runtime`: Define startup validation, collision handling, and per-session runtime isolation for the hub's managed upstream clients.
- `hub-protocol-compat`: Define the protocol-version negotiation expectations that the hub preserves when speaking MCP through `rmcp`.

### Modified Capabilities

None.

## Impact

- A new MCP-facing server/runtime will be added to this Rust project.
- The project will depend on an MCP protocol implementation rather than remaining a plain binary scaffold.
- The project will define a documented TOML 1.1 configuration surface rather than an ad hoc one-off settings file.
- The repository will ship a root-level example config so users can see the intended keyed TOML 1.1 shape without reading the implementation first, and that file must stay synchronized with config-surface changes.
- The project will also adopt `mimalloc` v3 as its allocator choice in the shipped binaries.
- Core implementation work will center on upstream connection management, session isolation, tool selection rules, outward naming and collision rules, optional annotation overrides, merged tool registry construction, and routed tool invocation.
- Tool-selection rules now also need a documented wildcard-mask model and a clear unsupported-name boundary for literal `*` in upstream tool names.
- Startup must validate both static config shape and discovered outward naming so invalid prefixed tool layouts fail before the hub begins serving.
- Error handling needs a deliberate boundary between typed internal errors and contextual application/test failures.
- Integration verification must cover real MCP interactions instead of stopping at synthetic or purely local stubs.
- The initial outward API surface will intentionally be limited to tool-oriented MCP behavior.
