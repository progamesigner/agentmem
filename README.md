# AgentMem

An MCP server fronting an Obsidian-style markdown vault for durable, multi-tenant
agent memory. One server process serves many agents concurrently; each tool call
carries its own scope (agent, user, …) so files are namespaced on disk while the
human can still browse and hand-edit the vault directly in Obsidian.

> **Status: active development.** The design, specs, and task breakdown live under
> [`openspec/changes/build-agentmem-mcp-server/`](openspec/changes/build-agentmem-mcp-server/).
> Interfaces may change until the `0.1.0` tag is published.

## What it does

- Exposes a small, generic tool surface (nine tools — see [Tools](#tools)).
- Speaks the official MCP protocol over both **HTTP** (default, `127.0.0.1:8000`)
  and **stdio** via the [`rmcp`](https://crates.io/crates/rmcp) SDK.
- Namespaces each agent's files with a configurable VFS suffix scheme and
  enforces a single server-wide policy across two regions (inside / outside the
  agents folder).
- Writes atomically (temp-file + fsync + rename), prevents path traversal, honours
  `.gitignore`/`.obsidianignore` and hidden-file filters, and keeps all logs on
  stderr.

See [`docs/security.md`](docs/security.md) for the trust model.

## Install

```sh
cargo install agentmem            # from crates.io (once published)
cargo install --path .            # from a checkout
```

Pre-built release binaries for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`,
and `x86_64-pc-windows-msvc` are attached to each tagged release.

## Quick start

```sh
# HTTP transport (default), loopback, no auth — ideal for local development.
AGENTMEM_ROOT_DIR=/path/to/vault agentmem

# Inspect the effective configuration without starting the server.
AGENTMEM_ROOT_DIR=/path/to/vault agentmem --print-config
```

## Configuration

Everything is configured through environment variables. Every CLI flag mirrors —
and overrides — the matching variable (`--root-dir`, `--policy`, `--http-bind`, …).
`AGENTMEM_ROOT_DIR` is the only required variable; all others have defaults.

| Variable | Default | Description |
|---|---|---|
| `AGENTMEM_ROOT_DIR` | *(required)* | Absolute path to the vault root. Must exist, be a directory, and be canonicalisable. |
| `AGENTMEM_AGENTS_DIR` | `Agents` | Agents folder relative to the root. `.` or empty means the vault root itself is the agents folder. Must be relative with no traversal. |
| `AGENTMEM_VFS_SCHEME` | `<agent>.<user>` | VFS suffix scheme. Each `<ident>` becomes a required scope parameter on every tool call. Empty string disables suffixing. |
| `AGENTMEM_POLICY` | `namespaced` | One of `scoped`, `namespaced`, `readonly`, `readwrite` (see [Policies](#policies)). |
| `AGENTMEM_TRANSPORT` | `http` | `http` or `stdio`. |
| `AGENTMEM_HTTP_BIND` | `127.0.0.1:8000` | HTTP bind address (http transport only). |
| `AGENTMEM_HTTP_BEARER` | *(unset)* | If set, `POST/GET /mcp` requires `Authorization: Bearer <token>`. Unset → unauthenticated (a startup `WARN` is logged). |
| `AGENTMEM_TIMEZONE` | `UTC` | IANA timezone used to date diary entries. |
| `AGENTMEM_HONOR_IGNORE_FILES` | `true` | Honour `.gitignore` / `.obsidianignore` for list and direct addressing. Strict boolean (`true`/`false`). |
| `AGENTMEM_INCLUDE_HIDDEN` | `false` | Include dotfiles/dot-directories. Strict boolean. |
| `AGENTMEM_LOG` | `warn,agentmem=info` | `tracing` env-filter directive. Logs always go to stderr. |

### VFS scheme

The scheme defines how scope keys are spelled into directory segments and
filename stems. With the default `<agent>.<user>` scheme and caller
`{agent: "coder", user: "alice"}`, the virtual path `Agents/tasks/plan.md`
resolves to:

```
<root>/Agents/coder.alice/tasks/plan.coder.alice.md
```

The scope appears both as the per-scope directory and as a suffix on the file
stem, so a human opening the vault in Obsidian can immediately see whose file is
whose, and another scope's file is structurally unaddressable.

### Worked layouts

**Default config** (`AGENTMEM_AGENTS_DIR=Agents`, `AGENTMEM_VFS_SCHEME=<agent>.<user>`):

```
vault/
├── Agents/                       ← agent-owned region (scoped, suffixed)
│   └── coder.alice/
│       ├── PERSONA.coder.alice.md
│       ├── HEARTBEAT-STATE.coder.alice.md
│       └── diary/2026-05-25.coder.alice.md
└── Actions/release.md            ← human-owned region (shared, no suffix)
```

**Vault root as agents folder** (`AGENTMEM_AGENTS_DIR=.`): the whole vault is the
agents folder, there is no "outside" region, and wrapper tools resolve to
`PERSONA.coder.alice.md` at the vault root.

## Tools

| Tool | Purpose |
|---|---|
| `list_memory_notes` | Paginated listing of virtual paths visible to the scope (`limit` default 200, max 1000; opaque `cursor`; optional `path_prefix`). |
| `read_memory_note` | Read a note by virtual path. |
| `write_memory_note` | Atomic full-file write; returns the byte count. |
| `edit_memory_note` | Atomic search/replace; the search string must occur exactly once. |
| `delete_memory_note` | Delete a single file (never directories). |
| `load_session_context` | Read `PERSONA`/`PROMPT`/`RULES`/`USER`/`TOOLS` `.md` in one call. |
| `evolve_core_persona` | Atomic write to one of those five, selected by `which`. |
| `update_task_heartbeat` | Atomic write to `HEARTBEAT-STATE.md`. |
| `append_diary_entry` | Append a timestamped section to `diary/<YYYY-MM-DD>.md`. |

Every tool's input schema includes the scope parameters derived from the active
scheme; introspect them via the standard MCP `tools/list` call.

## Policies

There are two regions per server: **inside** the agents folder (scoped,
suffix-applied) and **outside** it (shared, no suffix). One server-wide policy
governs both:

| Policy | Inside agents folder | Outside agents folder |
|---|---|---|
| `scoped` | own-scope read/write | **denied** |
| `namespaced` *(default)* | own-scope read/write | read-only |
| `readonly` | own-scope read-only | read-only |
| `readwrite` | own-scope read/write | read/write |

Anything resolving outside the vault root is always denied.

## Client configuration

### Claude Desktop (stdio sidecar)

`claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "agentmem": {
      "command": "agentmem",
      "env": {
        "AGENTMEM_ROOT_DIR": "/Users/me/vault",
        "AGENTMEM_TRANSPORT": "stdio"
      }
    }
  }
}
```

### Local HTTP server (`mcp.json`)

```json
{
  "mcpServers": {
    "agentmem": {
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

### curl (HTTP transport)

```sh
# Liveness probe.
curl http://127.0.0.1:8000/health

# An MCP request (Streamable HTTP requires the dual Accept header).
curl -X POST http://127.0.0.1:8000/mcp \
  -H 'Accept: application/json, text/event-stream' \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

When `AGENTMEM_HTTP_BEARER` is set, add `-H "Authorization: Bearer <token>"`.

## Human-in-the-loop editing

Because the on-disk layout is plain markdown, a human can open the vault in
Obsidian and hand-edit any `Agents/<scope>/...` file directly. The agent will see
the human's edits as if it had written them itself — this is the supported channel
for curating or correcting an agent's memory. Creating `plan.coder.alice.md` by
hand makes it appear to the `coder.alice` scope as the virtual note `plan.md`.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```
