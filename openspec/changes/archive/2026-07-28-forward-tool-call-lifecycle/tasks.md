## 1. Tool-Call Correlation

- [x] 1.1 Add session-local in-flight call and upstream-progress correlation state shared by `HubServer`, `SessionRuntime`, and each `UpstreamClient`.
- [x] 1.2 Route outward request contexts into `SessionRuntime`, send routed upstream calls through cancellable request handles, and bind the outward request ID to the generated upstream request ID and progress token.
- [x] 1.3 Handle outward `notifications/cancelled` by queuing or forwarding one owner-specific upstream cancellation with rewritten request ID and preserved reason and metadata.
- [x] 1.4 Handle upstream progress notifications by resolving the generated upstream token, forwarding only active tokenized calls with the original client token and unmodified payload fields, and logging notification-send failures without changing the final-result path.
- [x] 1.5 Remove request and progress correlations on every upstream completion or send failure and clear them during session shutdown.

## 2. Lifecycle Coverage

- [x] 2.1 Extend the mock upstream with deterministic cancellation-observation and progress-emission tools that use standard MCP request contexts and notifications.
- [x] 2.2 Add end-to-end coverage that cancels an in-flight routed call, verifies exactly its owning upstream receives the rewritten cancellation data, and verifies other upstreams remain unaffected.
- [x] 2.3 Add end-to-end coverage that records forwarded progress, verifies client-token restoration and payload preservation, and verifies unknown, stale, un-tokenized, or cancelled calls do not produce outward progress.

## 3. Documentation And Verification

- [x] 3.1 Update the README operational behavior with cancellation and progress semantics, and remove the completed roadmap subsection.
- [x] 3.2 Run `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test --all-features`, and `openspec validate forward-tool-call-lifecycle --strict`; resolve all findings.
