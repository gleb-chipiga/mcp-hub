# mcp-hub

`mcp-hub` is a tools-only [Model Context Protocol](https://modelcontextprotocol.io/)
(MCP) aggregation server. It starts configured stdio MCP servers and exposes their
available tools through one stdio MCP endpoint.

The hub itself is lightweight and ships as a compact single binary, with no separate
runtime service to deploy or manage.

Use it to give an MCP client one curated tool inventory instead of configuring every
upstream server separately, while retaining control over the names, availability, and
safety metadata of exposed tools.

## Features

- Aggregates tools from multiple stdio upstream MCP servers and routes calls to the
  owning upstream.
- Supports optional visible prefixes such as `github.search_issues`; unprefixed tools
  keep their original names. Ambiguous outward names fail startup rather than being
  silently shadowed.
- Curates each upstream with `include` or `exclude` patterns. `*` is a full-name,
  case-sensitive wildcard and `include` takes precedence when both are set.
- Preserves tool descriptions, schemas, annotations, and result content; configuration
  can override the `read_only`, `destructive`, and `open_world` annotations.
- Allows several differently configured instances of the same upstream binary and
  omits unavailable or slow upstreams without taking down the remaining inventory.
- Advertises only MCP tools in v1. Prompts, resources, roots, sampling, elicitation,
  and task-required tools are intentionally out of scope.

## Run

```bash
cargo run -- mcp-hub.example.toml
```

The first command-line argument is the TOML configuration path. Alternatively, set
`MCP_HUB_CONFIG`. The hub speaks MCP over standard input/output and writes tracing
logs to standard error.

## Configuration

Configuration files use TOML 1.1. They must define at least one
`[servers.<instance_id>]` table. An instance ID identifies an upstream in logs and
startup errors; it is not exposed to MCP clients unless it is also used as `prefix`.
Instance IDs and prefixes may contain ASCII letters, digits, `-`, and `_`, but not
`.`.

### Server fields

| Field | Required | Description |
| --- | --- | --- |
| `command` | Yes | Executable path or command name for the upstream stdio server. |
| `args` | No | Array of command-line arguments. Defaults to `[]`. |
| `env` | No | Environment variables added to or overriding the child process environment. |
| `prefix` | No | Outward name prefix. A tool named `search` becomes `<prefix>.search`. |
| `tools.include` | No | Allowlist of original upstream tool names or `*` masks. |
| `tools.exclude` | No | Denylist used only when `include` is absent. |
| `tools.overrides.<tool>` | No | Annotation overrides for one original upstream tool name. |

### Minimal server

The smallest configuration starts one upstream process and exposes all of its
non-task-required tools with their original names:

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]
```

### Multiple upstreams and prefixes

Every server entry starts an independent upstream process. Use prefixes when two
servers could expose the same tool names, or when namespacing makes the client-facing
inventory clearer:

```toml
[servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]

[servers.github]
prefix = "gh"
command = "uvx"
args = ["mcp-server-github"]

[servers.github.env]
GITHUB_TOKEN = "replace-with-token"
GITHUB_OWNER = "acme"
GITHUB_REPO = "platform"
```

The filesystem tool `read_file` remains `read_file`; a GitHub tool `search_issues`
is exposed as `gh.search_issues`. Without distinct prefixes, two active upstreams
that expose the same outward name cause startup to fail.

`env` values are forwarded verbatim to the child process. The hub does not expand
`${VAR}` placeholders, so provide already-resolved values through your deployment
configuration and do not commit secrets.

### Select tools

Filters match the **original upstream name**, before applying `prefix`. A filter
without `*` is an exact match; `*` matches zero or more characters across the whole,
case-sensitive name.

```toml
[servers.github_read]
prefix = "gh"
command = "uvx"
args = ["mcp-server-github"]

[servers.github_read.tools]
include = ["list_*", "read_*", "search_*", "create_comment"]
```

This exposes only matching tools, such as `gh.list_issues` and `gh.create_comment`.
Other useful masks include `"*webhook"`, `"read_*shot"`, and `"resource_*"`.

Use `exclude` to remove a small set of tools from an otherwise complete inventory:

```toml
[servers.github_admin]
prefix = "gh_admin"
command = "uvx"
args = ["mcp-server-github", "--mode", "admin"]

[servers.github_admin.tools]
exclude = ["list_*", "read_*", "search_*"]
```

If both `include` and `exclude` are present, `include` wins and `exclude` is ignored.
Literal `*` is reserved for filters: an upstream tool name or an override target that
contains `*` is rejected.

### Override safety annotations

The hub preserves upstream tool metadata by default. Set only the annotation hints
that need correction; unspecified hints remain unchanged:

```toml
[servers.github_read.tools.overrides.create_comment]
read_only = false
open_world = true

[servers.github_admin.tools.overrides.delete_issue]
destructive = true
```

Supported fields are `read_only`, `destructive`, and `open_world`. The latter also
accepts the aliases `external`, `external_side_effect`, and `sends_external_data`.
An override must use the exact original name of a tool advertised by that upstream;
a misspelled target fails startup.

### Run and startup behavior

Pass the configuration path as the first argument, or set `MCP_HUB_CONFIG`:

```bash
cargo run -- mcp-hub.toml
MCP_HUB_CONFIG=mcp-hub.toml cargo run
```

At startup, the hub connects to each upstream and discovers its tools. The default
connection and discovery timeout is five seconds and can be changed with
`MCP_HUB_STARTUP_TIMEOUT_MS`. An unavailable or timed-out upstream is omitted while
the remaining tools stay available. Configuration errors, duplicate outward tool
names, invalid tool names, and unknown annotation-override targets fail startup.

See [mcp-hub.example.toml](mcp-hub.example.toml) for a compact combined example.

## Roadmap

The current implementation connects to local upstream servers over stdio only.
Support for remote upstream MCP servers over a network transport is planned.
