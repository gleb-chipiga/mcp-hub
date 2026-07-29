## 1. Configuration and documentation surface

- [x] 1.1 Add the default-enabled per-upstream `stderr` boolean to raw and validated configuration, with unit coverage for omission and explicit disablement.
- [x] 1.2 Document the `stderr` server field and show its opt-out use in `mcp-hub.example.toml`.
- [x] 1.3 Update README logging and operational behavior with the `mcp_hub::upstream_stderr` target, `RUST_LOG` filtering, default capture, and discard semantics; remove the completed roadmap item.

## 2. Upstream stderr runtime

- [x] 2.1 Replace default child stderr inheritance with `TokioChildProcess::builder`, selecting piped stderr for enabled upstreams and null stderr for disabled upstreams while preserving the MCP stdin/stdout transport.
- [x] 2.2 Implement the asynchronous, bounded, lossy-text stderr drain that emits correlated `INFO` records on `mcp_hub::upstream_stderr` and handles read failures without affecting routed MCP traffic.
- [x] 2.3 Store drain task ownership with each active upstream and cancel or await it consistently on initialization failure, startup rollback, omitted upstreams, and session shutdown.

## 3. Verification

- [x] 3.1 Extend the mock upstream and integration configuration fixture with deterministic stderr output and per-instance stderr configuration.
- [x] 3.2 Add end-to-end coverage that verifies enabled stderr is structured, target-tagged, and correlated to the correct upstream instance, including startup output that exceeds the pipe buffer.
- [x] 3.3 Add end-to-end coverage that `stderr = false` discards output and that a `RUST_LOG` directive suppresses the diagnostics target while the hub continues to drain and serve the upstream.
- [x] 3.4 Run `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test --all-features`, and strict OpenSpec validation.
