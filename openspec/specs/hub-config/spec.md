## Purpose

Defines the TOML configuration contract for local upstream servers, including
launch settings, tool selection, naming, and metadata overrides.
## Requirements
### Requirement: Hub uses a TOML 1.1 configuration surface for upstream stdio servers
The hub SHALL load its upstream stdio server definitions from a documented TOML 1.1 configuration file.

#### Scenario: Keyed server tables define upstreams
- **WHEN** a user defines upstream servers in configuration
- **THEN** each upstream is declared under `[servers.<instance_id>]`

#### Scenario: Keyed server tables preserve declaration order
- **WHEN** a user declares multiple upstream servers under `[servers.<instance_id>]`
- **THEN** the hub preserves their declaration order from the TOML document during startup processing and validation

#### Scenario: Minimal keyed upstream process definition
- **WHEN** a user configures one upstream server using `[servers.<instance_id>]`, its executable path or command name, and no advanced options
- **THEN** the hub can load that TOML configuration and start the upstream server

#### Scenario: Upstream process definition with arguments and environment
- **WHEN** a user configures one keyed upstream server with command-line arguments and environment variables
- **THEN** the hub launches that upstream process with the configured arguments and environment

### Requirement: Hub keeps the config ergonomic
The hub SHALL keep the common configuration case compact while making advanced behavior available through optional fields or nested tables only when needed.

#### Scenario: Simple upstream configuration stays compact
- **WHEN** a user wants to connect one upstream server without filtering or annotation overrides
- **THEN** the configuration does not require unrelated fields such as visible prefixes or per-tool override tables beyond the required keyed instance header

#### Scenario: Advanced upstream configuration remains possible
- **WHEN** a user needs per-upstream filtering or per-tool annotation overrides
- **THEN** the configuration can express those behaviors without changing the simple case into the default shape

### Requirement: Hub ships a discoverable example configuration
The repository SHALL include a root-level example TOML 1.1 configuration file that demonstrates the intended configuration style and supported options.

#### Scenario: Root example file demonstrates the simple and advanced shapes
- **WHEN** a user opens the root-level example configuration file
- **THEN** the file shows at least one compact upstream definition and one advanced upstream definition with optional settings

#### Scenario: Root example file demonstrates supported advanced options
- **WHEN** a user reads the root-level example configuration file
- **THEN** the file demonstrates readable usage of command-line arguments, environment variables, optional prefixes, wildcard `include` or `exclude` filters, and per-tool annotation overrides

#### Scenario: Root example file keeps server definitions visually separate
- **WHEN** a user reads the root-level example configuration file
- **THEN** each server definition appears as its own `[servers.<instance_id>]` block rather than being collapsed into one larger inline structure
- **AND** small leaf maps such as `env` may use TOML 1.1 inline-table syntax when that improves readability
- **AND** structural sections such as `tools` and `tools.overrides` remain regular tables so the example stays readable as options grow

#### Scenario: Root example file stays current when config behavior changes
- **WHEN** a change affects the supported config surface, naming model, or exampled options
- **THEN** the root-level example configuration file is updated in the same change and kept aligned with the implementation and docs

### Requirement: Hub supports per-upstream tool filtering
The hub SHALL support optional per-upstream `include` and `exclude` tool filters.

#### Scenario: Include only explicitly listed tools
- **WHEN** an upstream configuration contains an `include` list
- **THEN** the hub exposes only those listed tools from that upstream, subject to other hub-level exclusions such as unsupported execution models

#### Scenario: Exclude selected tools when no include list is present
- **WHEN** an upstream configuration contains an `exclude` list and no `include` list
- **THEN** the hub exposes all upstream tools except the excluded ones

#### Scenario: Include takes precedence over exclude
- **WHEN** an upstream configuration contains both `include` and `exclude`
- **THEN** the hub applies `include` semantics and ignores `exclude`

#### Scenario: Wildcard include patterns match full tool names
- **WHEN** an upstream configuration contains an `include` entry with `*` in the beginning, middle, or end of the pattern
- **THEN** the hub treats that entry as a full-string wildcard mask over original upstream tool names

#### Scenario: Wildcard exclude patterns match full tool names
- **WHEN** an upstream configuration contains an `exclude` entry with `*` in the beginning, middle, or end of the pattern and no `include` list
- **THEN** the hub excludes every original upstream tool name that matches that wildcard mask

#### Scenario: Literal asterisks in tool names are unsupported
- **WHEN** startup discovery finds an upstream tool whose original name contains a literal `*`
- **THEN** the hub rejects that tool name deterministically instead of treating `*` as both data and filter syntax

### Requirement: Hub allows multiple configured copies of the same upstream server
The hub SHALL allow the configuration to start multiple copies of the same underlying upstream server binary with different launch settings, tool filters, or visible prefixes.

#### Scenario: Multiple copies of one upstream binary with different prefixes
- **WHEN** two configured upstream entries point to the same executable but use different arguments or prefixes
- **THEN** the hub treats them as separate upstream instances and exposes their tool sets independently

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
