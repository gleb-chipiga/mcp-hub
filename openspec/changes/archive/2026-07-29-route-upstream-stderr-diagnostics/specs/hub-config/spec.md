## ADDED Requirements

### Requirement: Hub controls per-upstream stderr diagnostics
The hub SHALL support an optional `stderr` boolean on each
`[servers.<instance_id>]` configuration table. It SHALL default to `true` when
omitted; `false` SHALL discard that upstream process's stderr.

#### Scenario: Omitted stderr setting enables diagnostics
- **WHEN** an upstream configuration omits `stderr`
- **THEN** the hub treats stderr diagnostics as enabled for that upstream

#### Scenario: Explicit false disables one upstream's diagnostics
- **WHEN** an upstream configuration sets `stderr = false`
- **THEN** the hub discards that child process's stderr
- **AND** the setting does not change stderr behavior for any other configured upstream
