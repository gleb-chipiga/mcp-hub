## Context

The project currently has no implementation beyond a Rust binary scaffold, so this change defines the first real architecture rather than extending an existing system. The target is a narrow but production-shaped first version of `mcp-hub`: a tool-only MCP hub that can connect to multiple upstream MCP servers, merge their tool inventories, and expose one outward MCP-facing surface.

The design is intentionally shaped around the following constraints:

- v1 must stay tool-only;
- the hub configuration must be a documented TOML 1.1 surface rather than an underspecified generic TOML file;
- the configuration model should follow the principle "simple things should stay simple, while complex things should remain possible";
- the project should avoid hand-rolling MCP lifecycle and transport behavior if a suitable SDK already provides it;
- upstream servers may be stateful, so session behavior must be explicit rather than accidental;
- the outward behavior should remain useful even when some upstreams are unavailable;
- error handling should preserve typed semantics inside the hub while still giving rich context at process and test boundaries;
- protocol-sensitive behavior should be verified with real `rmcp` peers rather than only ad hoc fixtures.

## Goals / Non-Goals

**Goals:**

- Provide one outward MCP-facing hub surface for listing and calling tools.
- Connect to multiple upstream MCP servers and maintain a merged tool registry.
- Allow each configured upstream to declare process launch settings, tool filtering, and optional tool-annotation overrides.
- Ensure tools from different upstreams can coexist through optional visible prefixes and deterministic collision handling.
- Route tool calls to the correct upstream server based on the outward tool name.
- Allow multiple configured copies of the same underlying upstream server with different settings and outward prefixes.
- Isolate upstream state per inbound hub session to avoid cross-session leakage.
- Keep the first implementation small enough to validate the project's core shape quickly.

**Non-Goals:**

- Implement prompts, resources, roots, sampling, elicitation, or other non-tool MCP capabilities.
- Implement task-based tool execution in v1.
- Act as a fully transparent MCP proxy.
- Build a management UI, long-term persistence layer, or full auth platform in this change.

### Use keyed TOML 1.1 server tables

The hub configuration should be designed and documented as a TOML 1.1 document built around keyed server tables under `[servers.<instance_id>]`, with examples and tests written against that format.

Rationale:

- The original product shape is explicitly configuration-heavy, so the config format is part of the public contract.
- TOML is a good fit for checked-in local process configuration, but the expected version should be explicit rather than implied.
- Calling out TOML 1.1 keeps examples and validation rules intentional when the configuration surface grows.
- Keyed server tables match the product model better than anonymous array entries because upstream instances already need stable unique identities.
- A checked-in example file in the project root gives users one canonical reference for the intended config style and supported options.

Alternatives considered:

- Unversioned "some TOML file": rejected because it leaves too much ambiguity around the intended config surface.
- JSON or YAML: rejected because TOML is a better fit for compact local tooling configuration and was part of the initial task framing.

### Ship one canonical root-level example config

The repository should include a root-level example configuration file, preferably `mcp-hub.example.toml`, that demonstrates the intended TOML 1.1 surface in a polished and readable form.

Rationale:

- The hub is configuration-driven, so the example file is part of the product experience rather than incidental documentation.
- Many users will understand the configuration surface faster from one concrete example than from prose requirements or source-level tests.
- A checked-in example reduces the risk that the documented config model drifts away from what the implementation actually accepts.
- Treating the example as a maintained contract keeps repository onboarding honest when config semantics evolve.

Expected shape:

- the file should live in the project root so it is easy to discover;
- it should use keyed tables under `[servers.<instance_id>]`;
- each server example should remain its own `[servers.<instance_id>]` block rather than collapsing server definitions into one large inline structure;
- small leaf maps such as `env` may use TOML 1.1 inline-table syntax when that improves readability;
- more structural sections such as `tools` and `tools.overrides` should remain regular tables for readability as they grow;
- it should show at least one minimal upstream entry and one more advanced upstream entry;
- it should demonstrate readable use of command, args, env, optional prefix, wildcard include or exclude filters, and per-tool annotation overrides;
- it should be updated whenever config-surface changes would make the example stale, misleading, or incomplete;
- it should stay attractive and readable rather than trying to enumerate every edge case mechanically.

Alternatives considered:

- Only documenting configuration in prose or tests: rejected because it makes the public config surface harder to discover.
- Hiding examples under `docs/` or `examples/`: rejected because the initial onboarding path should be obvious from the repository root.

### Keep the keyed-table config model ergonomic: simple things simple, complex things possible

The configuration should keep the keyed-table shape compact in the simple case while moving advanced behavior into optional nested fields or tables that only appear when needed.

Rationale:

- A minimal upstream definition should remain short even though the upstream instance id is explicit in the table header.
- Advanced features such as tool filtering and annotation overrides should remain available without forcing every upstream entry into a verbose shape.
- The configuration surface is part of the product, not just an implementation detail.

Expected shape:

- the simple case should look like `[servers.<instance_id>]` plus launch information and an optional prefix;
- advanced cases may add tool filters and per-tool override tables;
- the required keyed instance id should stay separate from visible outward prefixing.

Alternatives considered:

- A flat but always-verbose schema: rejected because it makes the common case noisy.
- A magic implicit schema with many derived behaviors and no clear shape: rejected because it becomes hard to reason about and validate.

### Preserve keyed server declaration order from TOML through runtime setup

The hub should preserve the declaration order of `[servers.<instance_id>]` entries from the TOML document instead of re-sorting upstreams alphabetically during deserialization or runtime construction.

Rationale:

- Users naturally read large config files top-to-bottom and expect startup behavior and diagnostics to follow that order.
- Startup validation errors such as outward-name collisions are easier to interpret when the "first" and "second" upstreams match the order written in the config file.
- Keeping declaration order makes the keyed-table model feel intentional instead of surprising users with hidden map sorting semantics.

Implications:

- deserialization must preserve keyed server-table order rather than normalizing to a sorted map type;
- runtime startup validation and upstream connection setup should iterate servers in declaration order;
- diagnostics that mention colliding or otherwise related upstream entries should use the preserved declaration order.

Alternatives considered:

- Alphabetical normalization by instance id: rejected because it is deterministic but user-hostile in diagnostics.

## Decisions

### Use `rmcp` as the protocol substrate

The hub will use the official Rust MCP SDK, `rmcp`, for MCP protocol handling rather than hand-rolling JSON-RPC, lifecycle, or transport behavior.

Rationale:

- `rmcp` already provides both MCP client and MCP server roles.
- It already handles lifecycle, request correlation, cancellation, progress, and transport abstractions.
- It already includes streamable HTTP and stdio support, which keeps future transport expansion possible without reshaping the project.

Alternatives considered:

- Hand-rolled MCP implementation: rejected because it would spend the first change on protocol plumbing rather than hub behavior.
- Transport-specific HTTP/stdio adapters without MCP abstractions: rejected because the project intent is MCP-native, not a generic RPC bridge.

### Split `thiserror` and `anyhow` by responsibility

The hub should use `thiserror` for typed, semantically meaningful internal errors and `anyhow` for top-level orchestration, CLI/bootstrap, and test harness flows where the priority is contextual diagnosis rather than pattern matching.

Rationale:

- `thiserror` is the right fit for domain and protocol boundary errors that callers may need to classify.
- `anyhow` is the right fit for application wiring and integration harness code where errors are usually bubbled upward with additional context.
- Using both crates without a boundary would blur intent and make the error layer harder to reason about.

Planned boundary:

- `thiserror`: configuration validation errors, routed tool invocation errors, lifecycle/state errors, protocol-facing translation boundaries.
- `anyhow`: process entrypoints, runtime/bootstrap orchestration, spawned test peer setup, multi-step integration assertions with contextual `.context(...)`.

Alternatives considered:

- `thiserror` only: rejected because top-level bootstrap and integration harness code becomes noisy and context-poor.
- `anyhow` only: rejected because protocol and domain errors lose useful structure and explicit mapping semantics.

### Expose the hub as an aggregation facade backed by an internal runtime

Externally, the hub will behave like one MCP server. Internally, each inbound hub session will own a runtime that manages multiple upstream MCP client connections.

Rationale:

- This keeps the product story simple: one hub, one outward MCP surface.
- It preserves freedom to manage policy, isolation, and upstream lifecycle internally.
- It avoids collapsing the project into a pure proxy, which is too narrow for multi-server tool aggregation.

Alternatives considered:

- Host/runtime only, with no outward MCP surface: rejected for v1 because the repository intent is better served by an actual hub interface.
- Pure proxy/gateway: rejected because merging tool inventories and renaming tools is no longer proxy behavior.

### Separate internal upstream identity from outward tool prefix

The hub should treat the keyed table name, `[servers.<instance_id>]`, as the required internal upstream identity and keep outward tool prefix as a separate optional field. An upstream may have no outward prefix at all while still having a stable internal identity for routing, logging, and diagnostics.

Rationale:

- The original requirement describes prefixing as optional rather than mandatory.
- The keyed table name is the right place for the stable instance identity because it makes large configs easier to scan.
- Reusing that required identity as the user-visible prefix makes configurations noisier and conflates two different concerns.
- The project needs to support multiple configured copies of the same upstream server with different settings and prefixes, which is easier to reason about when prefix and identity are distinct.

Implications:

- without a configured prefix, outward tool names should stay as the upstream tool names;
- with a configured prefix, outward tool names should become `<prefix>.<tool_name>`;
- if two configured upstreams would produce the same outward tool name after filtering and prefixing, the hub should fail deterministically rather than silently shadowing one tool.

Alternatives considered:

- Mandatory visible prefix for every upstream: rejected because it violates the "simple things simple" requirement.
- Reusing one required upstream id as both internal identity and visible prefix: rejected because it conflates two different concerns.

### Validate startup config before serving the outward MCP surface

The hub should treat startup as a validation boundary. Static configuration should be validated at load time, and the startup runtime build should validate the discovered outward tool registry before the outward MCP server starts serving clients.

Rationale:

- Config correctness is not only about TOML syntax; it also includes whether the configured prefixes and filters produce an unambiguous outward tool namespace.
- Outward name collisions are easier to diagnose when they fail startup deterministically rather than surfacing later as shadowed tools or request-routing ambiguity.
- The hub already performs eager upstream discovery in the stdio-first shape, so startup is the natural place to enforce this contract.

Implications:

- duplicate internal identities remain a static config validation error;
- outward tool-name collisions must be checked after applying include or exclude filtering, task-required tool omission, and visible prefix rewriting;
- if startup discovery finds two active upstreams that would expose the same outward tool name, the hub must fail before the outward MCP server is initialized.

Alternatives considered:

- Lazy collision detection on first `tools/list`: rejected because it delays a deterministic startup error into normal request handling.
- Silently shadowing the later tool: rejected because it would make config mistakes hard to notice and tool routing ambiguous.

### Isolate upstream clients per inbound hub session

Each inbound hub session will create its own upstream client set rather than sharing one global pool of live upstream sessions.

Rationale:

- Upstream MCP servers may keep session-local state.
- Isolation prevents context leakage between inbound clients.
- The behavior is easier to reason about and matches MCP session semantics more closely.

Alternatives considered:

- Shared global upstream pool: rejected for v1 because it is cheaper operationally but unsafe for stateful upstream tools.

Implementation note:

- The current v1 implementation is stdio-only, so one hub process serves one inbound MCP session and owns one upstream client set.
- This satisfies the isolation requirement for the currently supported transport, but it is not a reusable multi-session transport shape.
- If the project adds streamable HTTP or any other multi-session inbound transport, that change must begin by introducing a per-session hub-service factory or a lazy session-bound runtime initializer.
- Reusing one prebuilt `SessionRuntime` across multiple inbound sessions in the same process would violate the intended isolation model and is explicitly out of bounds for the current design.

### Support per-upstream tool selection with `include` and `exclude`

Each upstream configuration should be able to restrict the visible tool set through optional `include` and `exclude` lists.

Rationale:

- The hub is meant to be a curated tool surface, not merely a full mirror of every upstream.
- Per-upstream filtering is necessary when upstream servers expose too many tools or when different copies of the same upstream should expose different subsets.
- The filtering model should be understandable without inventing a more complex rule language in v1.

Semantics:

- when `include` is present, only the listed tool names are exposed from that upstream;
- when `include` is absent and `exclude` is present, all upstream tools except the listed ones are exposed;
- when both are present, `include` wins so behavior stays deterministic, even though the combination should be documented as discouraged.

Alternatives considered:

- Global filters only: rejected because the original task requires per-server control.
- A full expression language for filtering: rejected because it is unnecessary for the first implementation.

### Use simple `*` wildcard masks for `include` and `exclude`

The hub should treat `include` and `exclude` entries as simple full-string wildcard masks where `*` matches zero or more arbitrary characters anywhere in the tool name.

Rationale:

- Users often need broader curation than exact-name matching, but regular expressions would make the config surface harder to read and validate.
- A single reserved wildcard metacharacter keeps the filtering model compact and predictable.
- Matching should stay simple enough to explain in one short ruleset rather than turning filtering into a mini language.

Semantics:

- a pattern without `*` is an exact tool-name match;
- `*` may appear at the beginning, middle, or end of the pattern;
- matching is case-sensitive and applies to the entire original upstream tool name;
- `include` remains allowlist mode and still takes precedence over `exclude`;
- repeated `*` characters are equivalent to a single wildcard span.

Unsupported-name boundary:

- `*` is reserved for wildcard semantics in filter patterns;
- literal asterisks in upstream tool names are out of scope and should be treated as unsupported rather than escaped or matched literally;
- if startup discovery finds an upstream tool whose original name contains a literal `*`, the hub should reject that tool or fail deterministically rather than pretending the name can participate safely in exact and wildcard matching.

Alternatives considered:

- Regular expressions: rejected because they are more powerful than the product needs and much harder to explain safely.
- Prefix-only or suffix-only masks: rejected because users often need one consistent wildcard rule instead of several special cases.
- Escaping rules for literal `*`: rejected for now because the simpler product rule is to reserve `*` completely.

### Support optional config-driven tool annotation overrides

Each upstream configuration should be able to define optional per-tool overrides for safety-relevant tool annotations such as read-only, destructive, and external side-effect or open-world behavior.

Rationale:

- Some upstream servers may omit or misstate safety metadata even when the integrator knows better.
- The override mechanism should let operators tighten or correct outward semantics without forking the upstream server.
- This needs to stay optional so it does not complicate configurations that simply trust upstream metadata.

Expected merge behavior:

- override entries should be keyed by the original upstream tool name;
- only explicitly specified attributes should overwrite upstream metadata;
- unspecified annotation fields should continue to come from the upstream tool definition.

Alternatives considered:

- No overrides at all: rejected because the original requirement explicitly calls for them.
- Full schema replacement for tool metadata: rejected because it is too heavy for the initial need and violates the ergonomics goal.

### Treat unavailable upstreams as partial degradation, not total failure

The hub will tolerate unavailable upstreams during inventory construction and expose tools from available upstreams instead of failing the entire `tools/list` surface.

Rationale:

- A hub should remain useful when one upstream is down.
- Partial availability is operationally more practical than fail-fast behavior for the whole hub.

Alternatives considered:

- Fail all inventory requests if any upstream is unavailable: rejected because it turns one bad dependency into total hub outage.

### Keep v1 tool-only and filter unsupported tool shapes

The hub will not advertise non-tool MCP capabilities and will not expose tools that require task-based execution unless the hub implements that behavior.

Rationale:

- This keeps the change narrow and implementation-ready.
- It avoids pretending to support MCP behavior that the hub does not actually implement.

Alternatives considered:

- Expose future-looking capabilities early: rejected because it creates misleading outward contracts and test burden.

### Prefer stdio-first upstream integration

The first implementation should prioritize stdio upstream servers. The architecture will stay compatible with streamable HTTP, but HTTP-first complexity is not required to validate the hub.

Rationale:

- stdio is simpler to bootstrap locally.
- HTTP adds session, header, and deployment concerns that are not necessary for the first end-to-end proof.

Alternatives considered:

- HTTP-first implementation: rejected for v1 because it raises the complexity floor without improving the core hub design.

### Use `mimalloc` v3 as the process-global allocator

Project binaries should use the `mimalloc` crate with feature `v3` as the global allocator.

Rationale:

- The hub is a long-lived, allocation-heavy orchestration process that benefits from a production-grade general-purpose allocator.
- Choosing the allocator explicitly keeps runtime behavior intentional instead of inheriting the platform default accidentally.
- Using one shared allocator choice across the main hub binary and the integration-test upstream binary avoids allocator drift across real end-to-end test coverage.

Alternatives considered:

- System allocator only: rejected because the project wants an explicit allocator choice rather than implicit platform variance.
- A per-binary ad hoc allocator choice: rejected because it would make end-to-end behavior less consistent.

### Validate behavior with real `rmcp` integration peers

Integration coverage should use real `rmcp` clients and servers running over actual transports whenever the behavior under test depends on MCP lifecycle, negotiation, or tool metadata semantics.

Rationale:

- Version negotiation and lifecycle edge cases only become credible when exercised through the actual SDK stack.
- Tool annotations and schemas can be lost or rewritten accidentally in ways that unit tests around local structs will not catch.
- The hub is protocol glue, so tests should observe real protocol boundaries instead of only internal helper behavior.

Test matrix expectations:

- use real `rmcp` clients against the hub and real `rmcp` upstream servers under child-process control;
- cover supported protocol-version negotiation paths and edge-case negotiation behavior for future or otherwise unknown client versions as implemented by `rmcp`;
- cover tool filtering, outward prefixing, duplicate-name collision handling, and multiple configured copies of the same upstream server;
- cover tool metadata preservation and config-driven overrides, including descriptions, schemas, and annotations such as read-only, destructive, and open-world or external side-effect hints;
- cover tool result/content shapes that matter to outward interoperability, not only happy-path text responses.

## Risks / Trade-offs

- [Stateful upstream tools may still behave unexpectedly across reconnects] -> Keep per-session isolation and add integration tests around reconnection and re-listing.
- [Partial availability can make tool inventories appear to fluctuate] -> Log upstream health transitions clearly and define deterministic omit-on-failure behavior.
- [Optional prefixing can still create outward-name collisions] -> Detect collisions after filtering and prefix application and fail deterministically.
- [Relying on `rmcp` ties the project to a still-maturing SDK] -> Keep v1 narrow, test the exact MCP surface we expose, and avoid less mature protocol areas.
- [Per-session upstream isolation increases connection/process cost] -> Accept the higher cost in v1 in exchange for correctness; revisit pooling only after behavior is proven.
- [Using both `thiserror` and `anyhow` can devolve into inconsistency] -> Define and enforce a clear boundary for when each crate is allowed.
- [Purely synthetic tests can miss SDK-level negotiation or metadata regressions] -> Keep protocol-sensitive integration coverage on real `rmcp` peers.
- [Config ergonomics can regress as features are added] -> Keep minimal examples compact and push advanced behavior into optional nested fields.

## Migration Plan

This is the first functional change in the repository, so there is no production migration of existing behavior.

Implementation rollout plan:

1. Add the MCP-facing hub and the TOML 1.1 upstream configuration model.
2. Support stdio upstreams first and validate end-to-end tool listing and calling.
3. Add config-driven filtering, optional prefixing, collision detection, and optional annotation overrides.
4. Add integration coverage for routing, session isolation, partial upstream failure, filtering, naming, and tool metadata semantics using real `rmcp` peers.
5. Add a version-aware integration matrix for supported protocol negotiation paths and failure cases.
6. Keep the outward contract limited to tool behavior until the base architecture is stable.

Rollback strategy:

- revert to the empty scaffold if the initial architecture proves unworkable;
- because no existing user-facing implementation exists yet, rollback cost is low.

## Open Questions

- Should the first implementation allow lazy upstream connection on first `tools/list`, or eagerly connect at session start?
- Should unavailable upstreams be retried inside the session automatically, or only on the next inventory refresh?
- Do we want the first outward surface to emit `tools/list_changed`, or can that wait until the merged registry is stable?
- Which exact MCP protocol versions should the project commit to supporting in its integration matrix from day one?
- Should the simple config case expose an optional internal `name`, or should the implementation derive internal identities automatically from config position and launch settings?
