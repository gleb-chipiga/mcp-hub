## 1. Foundations

- [x] 1.1 Add `rmcp` and the minimal supporting dependencies needed for a tool-only MCP hub, and replace the placeholder binary scaffold with a real module layout.
- [x] 1.2 Introduce `anyhow` alongside `thiserror` and define a deliberate boundary so typed domain or protocol errors use `thiserror`, while process bootstrap and integration harness code use `anyhow` with contextual propagation.
- [x] 1.3 Use the `mimalloc` crate with feature `v3` as the process-global allocator for project binaries.

## 2. Configuration Model

- [x] 2.1 Redesign the upstream configuration model as a documented TOML 1.1 surface rather than the current minimal generic TOML shape.
- [x] 2.2 Implement a compact simple-case upstream config that requires only process launch information unless advanced behavior is needed.
- [x] 2.3 Add optional advanced per-upstream fields for visible `prefix`, tool filtering, and per-tool annotation overrides without making them mandatory in the simple case.
- [x] 2.4 Separate internal upstream instance identity from visible outward prefixing so the hub can run unprefixed upstreams while still routing and logging deterministically.
- [x] 2.5 Implement startup validation for configuration correctness, including duplicate internal identities, outward tool-name collisions after filters and prefixes, and filter semantics where `include` takes precedence over `exclude`.
- [x] 2.6 Extend the documented config model so `include` and `exclude` accept `*` wildcard masks, and reserve literal `*` as unsupported in upstream tool names instead of introducing escaping.
- [x] 2.7 Implement the configuration surface using keyed server tables under `[servers.<instance_id>]`.
- [x] 2.8 Update the root-level `mcp-hub.example.toml` to use the keyed-table format and keep it current whenever config-related changes affect its accuracy or completeness.
- [x] 2.9 Preserve keyed server declaration order from TOML through config loading, startup validation, and related diagnostics.

## 3. Upstream Runtime

- [x] 3.1 Implement an inbound hub session runtime that creates an isolated upstream client set for each inbound session.
- [x] 3.2 Implement stdio-based upstream connection management and track unavailable upstreams without crashing the whole hub session.
- [x] 3.3 Build the merged tool registry using the configured outward prefix rules rather than mandatory upstream-id namespacing.
- [x] 3.4 Implement per-upstream tool filtering using `include` and `exclude`, with `include` semantics winning when both are present.
- [x] 3.5 Allow multiple configured copies of the same underlying upstream server binary with different arguments, environment, filters, or prefixes.
- [x] 3.6 Apply config-driven per-tool annotation overrides for read-only, destructive, and external side-effect or open-world hints while preserving unspecified upstream metadata.
- [x] 3.7 Filter out upstream tools that require task-based execution so the outward v1 surface remains tool-only.
- [x] 3.8 Implement wildcard-based tool filtering over original upstream tool names and deterministic handling of discovered tool names that contain literal `*`.

## 4. Outward MCP Hub Surface

- [x] 4.1 Implement the outward MCP hub server initialization so it advertises only tool capability for v1.
- [x] 4.2 Implement `tools/list` using the merged registry with partial-success behavior when some upstreams are unavailable.
- [x] 4.3 Implement `tools/call` routing from the outward tool name to the correct upstream server and original tool name.
- [x] 4.4 Implement error handling for unknown outward tool names and for routed calls whose upstream target is unavailable.
- [x] 4.5 Implement session shutdown so ending one inbound session only cleans up that session's upstream runtime.

## 5. Verification

- [x] 5.1 Add configuration-loading tests that exercise the documented TOML 1.1 config surface in both simple and advanced forms.
- [x] 5.2 Add integration coverage for `include` and `exclude` behavior, including the both-present `include`-wins case.
- [x] 5.3 Add integration coverage for optional visible prefixes, unprefixed tool names, and deterministic startup rejection of outward tool-name collisions after prefix application.
- [x] 5.4 Add integration coverage for multiple configured copies of the same upstream binary with different arguments, environment, filters, or prefixes.
- [x] 5.5 Add integration coverage for routed tool calls, unknown-tool errors, and omission of unavailable upstream tools from the outward inventory.
- [x] 5.6 Add integration coverage that proves separate inbound sessions do not share upstream session state.
- [x] 5.7 Add integration coverage that proves non-tool capabilities are not advertised and task-required tools are not exposed.
- [x] 5.8 Expand integration coverage to use real `rmcp` clients and upstream servers for protocol-sensitive scenarios instead of relying only on simplified fixtures.
- [x] 5.9 Add integration coverage for supported MCP specification-version negotiation paths, plus edge-case negotiation behavior for future or otherwise unknown versions.
- [x] 5.10 Add integration coverage that verifies config-driven annotation overrides for read-only, destructive, and external side-effect hints.
- [x] 5.11 Add integration coverage that verifies outward tool metadata preservation, including descriptions, schemas, and annotations that are not overridden by config.
- [x] 5.12 Add integration coverage for varied tool result or content shapes and metadata-bearing tool definitions so the hub is validated beyond text-only happy paths.
- [x] 5.13 Add integration coverage for wildcard `include` and `exclude` matching, plus startup behavior when an upstream tool name contains a literal `*`.
- [x] 5.14 Update configuration-loading coverage so the keyed `[servers.<instance_id>]` format is exercised directly and thoroughly.
- [x] 5.15 Add verification that the root-level example config stays parseable, aligned with the keyed-table format, and updated whenever config-surface changes affect it.
