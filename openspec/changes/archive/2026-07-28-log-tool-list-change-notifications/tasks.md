## 1. Upstream notification handling

- [x] 1.1 Add a private upstream `ClientHandler` that retains default client
  initialization info and logs each `notifications/tools/list_changed` signal with
  `upstream_instance_id`.
- [x] 1.2 Initialize each upstream client with that handler while retaining the
  immutable startup-discovered routes and the existing outward tool capability.

## 2. Integration coverage

- [x] 2.1 Add an opt-in mock upstream tool that emits
  `notifications/tools/list_changed` through its MCP server peer.
- [x] 2.2 Add end-to-end coverage that triggers the notification, verifies one
  structured warning with the upstream ID, and verifies the outward inventory does
  not change.
- [x] 2.3 Assert that the hub does not advertise the tools `listChanged` capability
  or send an outward tool-list change notification.

## 3. Documentation

- [x] 3.1 Update the README operational behavior to document the warning and stable
  per-session inventory.
- [x] 3.2 Remove the completed roadmap item and narrow the unsupported-behavior
  wording to dynamic inventories rather than receipt of the upstream notification.

## 4. Verification

- [x] 4.1 Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and
  `cargo test --all-features`.
- [x] 4.2 Run `openspec validate log-tool-list-change-notifications --strict`.
