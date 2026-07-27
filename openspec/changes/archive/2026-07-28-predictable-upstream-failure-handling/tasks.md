## 1. Runtime availability and diagnostics

- [x] 1.1 Add a typed startup-validation error for a zero-route discovery result and
  return it only after every configured upstream has been considered; cancel any
  retained upstream sessions before returning the error.
- [x] 1.2 Emit one structured `tracing::warn!` event from the failed routed-call path
  with `upstream_instance_id`, `original_tool_name`, and `transport_error`, while
  preserving the existing outward MCP error conversion.
- [x] 1.3 Keep successful upstream `CallToolResult` error responses as passthrough
  results without the new transport-failure warning.

## 2. End-to-end coverage

- [x] 2.1 Extend the mock upstream with a deterministic, test-only operation that
  terminates its process after the hub has completed startup discovery, and update
  fixture inventory helpers for that operation.
- [x] 2.2 Add an integration test proving that all unavailable upstreams make the hub
  fail startup with the zero-route diagnostic.
- [x] 2.3 Add an integration test proving that an upstream that exits after discovery
  remains in the fixed inventory and that subsequent routed calls return an MCP error.
- [x] 2.4 Capture hub stderr in routed-call integration coverage and assert the warning
  message and all three structured fields; verify an upstream tool-level error result
  still passes through without that warning.
- [x] 2.5 Retain or tighten the existing partial-startup and tool-name-collision tests
  so they explicitly cover the unchanged partial-availability and pre-serve failure
  contracts.

## 3. Documentation

- [x] 3.1 Update the README operational behavior to state that startup fails when no
  usable tools remain, while partial upstream availability remains supported.
- [x] 3.2 Update the README roadmap to remove or mark complete the implemented
  predictable-upstream-failure-handling item.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check` and apply formatting fixes if required.
- [x] 4.2 Run `cargo clippy --all-targets --all-features` and resolve all findings.
- [x] 4.3 Run `cargo test --all-features` and resolve all failures.
- [x] 4.4 Validate the OpenSpec change in strict mode.
