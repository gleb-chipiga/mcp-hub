# mcp-hub

[![crates.io](https://img.shields.io/crates/v/mcp-hub.svg?label=crates.io)](https://crates.io/crates/mcp-hub)

`mcp-hub` is a tools-only [Model Context Protocol](https://modelcontextprotocol.io/)
(MCP) aggregation server. It starts configured stdio MCP servers and exposes their
available tools through one stdio MCP endpoint.

The hub itself is lightweight and ships as a compact single binary, with no separate
runtime service to deploy or manage.

Use it to give an MCP client one curated tool inventory instead of configuring every
upstream server separately, while retaining control over the names, availability, and
safety metadata of exposed tools. Prefixes prevent tool-name collisions, and named
instances let the same upstream run with different settings.

## Features

- Aggregates tools from multiple stdio upstream MCP servers and routes calls to the
  owning upstream.
- Supports optional visible prefixes such as `github.search_issues` to prevent
  tool-name collisions; unprefixed tools keep their original names. Ambiguous outward
  names fail startup rather than being silently shadowed.
- Curates each upstream with `include` or `exclude` patterns. `*` is a full-name,
  case-sensitive wildcard and `include` takes precedence when both are set.
- Preserves tool descriptions, schemas, annotations, and result content; configuration
  can override the `read_only`, `destructive`, and `open_world` annotations.
- Allows several independently configured instances of the same upstream binary, with
  different arguments, environment variables, prefixes, and tool filters; unavailable
  or slow upstreams do not take down the remaining inventory.
- Advertises only MCP tools. Prompts, resources, roots, sampling, elicitation, and
  task-required tools are intentionally out of scope.

## Install

Install the latest published release from [crates.io](https://crates.io/crates/mcp-hub):

```bash
cargo install mcp-hub
```

### Prebuilt binaries

Prebuilt binaries are available from [GitHub Releases](https://github.com/gleb-chipiga/mcp-hub/releases/latest).

Each release includes `sha256.sum` and a per-archive `.sha256` checksum file. Verify
a downloaded archive with:

```bash
curl -LO https://github.com/gleb-chipiga/mcp-hub/releases/latest/download/<archive>.sha256
sha256sum -c <archive>.sha256
```

Verify its provenance with [GitHub CLI](https://cli.github.com/) and the release
workflow attestation:

```bash
gh attestation verify <archive> \
  -R gleb-chipiga/mcp-hub \
  --signer-workflow gleb-chipiga/mcp-hub/.github/workflows/release.yml
```

### mise

Install the latest GitHub Release for the current platform:

```bash
mise use -g github:gleb-chipiga/mcp-hub@latest
```

## Run

```bash
mcp-hub mcp-hub.example.toml
```

The first command-line argument is the TOML configuration path. Alternatively, set
`MCP_HUB_CONFIG`. The hub speaks MCP over standard input/output and writes tracing
logs to standard error.

### Logging and tracing

`mcp-hub` uses `tracing` and writes log records to standard error through a
non-blocking writer. Standard output is reserved exclusively for MCP traffic; never
send logs to standard output.

By default, or when `RUST_LOG` is invalid, the filter is `info,mcp_hub=debug`:
dependency logs at `info` and above, and hub logs at `debug` and above. Set
`RUST_LOG` in the environment of the `mcp-hub` process to replace that filter:

```bash
# Show only warnings and errors from all crates.
RUST_LOG=warn mcp-hub mcp-hub.example.toml

# Keep normal dependency logs and enable verbose hub diagnostics.
RUST_LOG=info,mcp_hub=debug mcp-hub mcp-hub.example.toml

# Diagnose MCP transport behaviour as well.
RUST_LOG=warn,mcp_hub=trace,rmcp=debug mcp-hub mcp-hub.example.toml
```

To retain logs when starting the binary directly, redirect standard error. The hub
does not create log files or rotate them; use a supervisor or your system's log
rotation for long-running processes.

```bash
mcp-hub mcp-hub.example.toml 2>> mcp-hub.log
```

When an MCP client starts `mcp-hub`, configure `RUST_LOG` in that client's process
environment or MCP-server entry. The `[servers.<instance_id>.env]` table in the hub
configuration is passed only to the upstream child process; it does not configure
the hub's own tracing.

### Operational behavior

Each `mcp-hub` process serves one inbound MCP session over stdio. It creates a
separate MCP client session for every configured upstream, so it terminates MCP on
both sides instead of forwarding a shared session or raw JSON-RPC messages. A stdio
process has one MCP client; multiple clients must start separate `mcp-hub` processes,
each with its own upstream sessions.

At startup, `mcp-hub` starts and initializes configured upstreams sequentially, then
discovers their tools and fixes the outward tool inventory for that session.
`MCP_HUB_STARTUP_TIMEOUT_MS` limits each upstream startup and tool-discovery step;
the default is 5000 ms. It does not limit tool-call duration. Several unavailable
upstreams can therefore delay startup by up to one timeout each.

An upstream that fails to start, initialize, or list its tools is omitted from the
current session and reported in logs. The hub continues with the remaining upstreams
when at least one usable tool remains after discovery. If none remain, startup fails
with a clear error. Tool-name collisions, unsupported upstream tool names, and
annotation overrides targeting unknown tools are also startup errors; the hub does not
start with a partial registry in those cases.

This startup readiness check is not a continuous health check. `mcp-hub` does not
retry failed startup, periodically probe upstreams, reconnect a crashed upstream, or
refresh its tool inventory during a session. When an active upstream sends
`notifications/tools/list_changed`, the hub logs a structured warning with its
instance ID and retains the startup inventory without forwarding that notification.
It does not set a hub-level timeout or retry for routed tool calls. If an already
connected upstream exits, its tools remain listed and calls to them fail. Restarting
the hub creates a new inbound session and new upstream sessions, which rediscover the
inventory.

When a routed tool call fails at the upstream transport or protocol layer, the hub
emits a structured warning with the upstream instance ID, original tool name, and
transport error. A valid upstream tool result with `is_error: true` remains a
tool-level result and is forwarded unchanged.

For an active routed `tools/call`, the hub forwards a client
`notifications/cancelled` notification only to the owning upstream. It replaces
the outward request ID with the upstream request ID and preserves the cancellation
reason and metadata. Cancellation is best effort: the upstream may already have
finished or may continue working. Unknown, completed, and duplicate cancellations
are ignored and never sent to another upstream.

When the client supplies a valid `_meta.progressToken` for an active routed tool
call, the hub forwards standard upstream `notifications/progress` notifications
back to that client. It restores the client token while preserving the upstream
progress value, optional total, message, and metadata. Progress without a matching
active tokenized call, including progress after cancellation or completion, is
ignored. Each tool call still returns one final result; the hub does not implement
a custom streamed-result protocol.

When the outward MCP session ends, `mcp-hub` cancels all active upstream client
sessions.

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

### Run from source

Pass the configuration path as the first argument, or set `MCP_HUB_CONFIG`:

```bash
cargo run -- mcp-hub.toml
MCP_HUB_CONFIG=mcp-hub.toml cargo run
```

See [mcp-hub.example.toml](mcp-hub.example.toml) for a compact combined example.

## Roadmap

The current implementation connects to local upstream servers over stdio only.

### Upstream diagnostics

- Route each upstream's stderr into structured hub tracing with its upstream
  instance ID. This diagnostic output will be enabled by default and share one
  `mcp_hub::upstream_stderr` target for `RUST_LOG` filtering.
- Allow an individual upstream configuration to disable its stderr output when it
  is too noisy. Disabled stderr will be discarded rather than written raw to the
  hub's stderr.

### Remote upstream transports

Support for remote upstream MCP servers over a network transport may be added as a
separate feature. It is not a high-availability mechanism and requires an explicit
design for transport and session boundaries before implementation.

### Not planned

The following behaviour is intentionally not planned for the current stdio-only
model:

- Periodic health checks or MCP ping loops.
- Automatic retry after failed upstream startup, or automatic restart and reconnect
  of an upstream process.
- Automatic retries of `tools/call`.
- Automatic tool-call timeouts that change call behaviour.
- Removing, adding, or otherwise refreshing tools in the advertised registry after a
  runtime failure.
- Task-based tools or task status tracking. The hub only exposes ordinary
  request-response `tools/call` operations.
- A dynamic tool inventory within an active stdio session.
- Pagination for `tools/list`; the hub returns the complete startup inventory in a
  single response.
- Custom partial or streamed tool results. MCP progress notifications are sufficient
  for intermediate status, while every tool call keeps one final result.
- Forwarding deprecated MCP Logging messages or `logging/setLevel`. Upstream
  diagnostics will use tagged stderr output through the hub's structured tracing.
- Expanding tool-annotation overrides beyond the supported safety-related fields.
- Restarting `mcp-hub` itself; process supervision belongs to the MCP host or an
  external supervisor.

These behaviours create ambiguous client-visible states. A transport failure does not
show whether a tool already performed an external side effect, and changing the
registry after the client has discovered it requires an explicit dynamic-inventory
protocol. A new process is the clear boundary for a new stdio MCP session.
