# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- Validate OpenSpec artifacts through `prek` locally and in CI.
- Emit a structured warning when an upstream reports `notifications/tools/list_changed`
  while retaining the fixed tool inventory for the current session.

### Changed

- Clarify the stdio single-client session model and upstream failure behavior in the README.
- Fail startup when upstream discovery leaves no usable tool routes, while continuing
  to support partial upstream availability.
- Emit structured diagnostics for routed upstream transport and protocol failures.
- Keep integration-test output quiet by suppressing ordinary child-process stderr.
- Simplify README roadmap headings.
- Document that upstream tool-list change notifications produce warnings without
  refreshing or forwarding the fixed per-session inventory.
- Migrate the project OpenSpec documents to the OpenSpec 1.6.0 baseline format.
- Document the Conventional Commits convention for project commits.
- Synchronize the project OpenSpec documentation with the current README.
- Document planned and intentionally unsupported tool, upstream diagnostic, and remote
  transport behaviour in the README roadmap.
- Simplify prebuilt-binary download instructions while retaining checksum and provenance
  verification guidance.
- Document that forwarding deprecated MCP Logging messages is not planned.

## [0.2.3] - 2026-07-26

### Changed

- Document logging and tracing configuration in the README.
- Document tool-name collision handling and independent upstream instances in the README.

## [0.2.2] - 2026-07-26

### Added

- Publish Linux, macOS, and Windows binary archives in GitHub Releases.
- Publish SHA-256 checksums for each release archive and a combined checksum file.
- Generate GitHub Artifact Attestations for release archives.

### Changed

- Add a crates.io badge to the README.
- Clarify the currently supported MCP capabilities in the README.
- Document downloading and verification of prebuilt binaries for every supported platform.
- Document installation from crates.io in the README.

## [0.2.1] - 2026-07-26

### Changed

- Run shared formatting, lint, and test checks through `prek` in CI.
- Document the lightweight single-binary deployment model.
