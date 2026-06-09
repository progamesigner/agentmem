# AgentMem

An MCP server fronting a plain-markdown vault for durable, multi-tenant
agent memory. One server process serves many agents concurrently; each tool call
carries its own scope (agent, user, …) so files are namespaced on disk while a
human can still browse and hand-edit the vault directly with any editor (Obsidian
works well for it).

> **Status: released.** The current capability specs live under
> [`openspec/specs/`](openspec/specs/); proposed and archived changes live under
> [`openspec/changes/`](openspec/changes/).

## What it does

- Exposes a small, generic tool surface (nine tools — see [Tools](#tools)).
- Speaks the official MCP protocol over both **HTTP** (default, `127.0.0.1:8000`)
  and **stdio** via the [`rmcp`](https://crates.io/crates/rmcp) SDK.
- Namespaces each agent's files with a configurable VFS suffix scheme and
  enforces a single server-wide policy across two regions (inside / outside the
  agents folder).
- Writes atomically (temp-file + fsync + rename), prevents path traversal, honours
  `.ignore`/`.gitignore`/`.obsidianignore` (nested, per-directory) and hidden-file
  filters, and keeps all logs on stderr.

See [`docs/security.md`](docs/security.md) for the trust model.

## Install

```sh
cargo install agentmem            # from crates.io (once published)
cargo install --path .            # from a checkout
```

Pre-built release binaries for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`,
and `x86_64-pc-windows-msvc` are attached to each tagged release.

## Container image

A minimal multi-arch image (`linux/amd64`, `linux/arm64`) is published to the
GitHub Container Registry on every tagged release. It is a statically linked
binary on `scratch` (~8 MB, no shell or OS userland) running as a non-root user.

```sh
docker pull ghcr.io/progamesigner/agentmem:latest
```

Tags per release: `:<version>` (e.g. `:0.1.0`), the moving `:latest`, and an
immutable `:sha-<gitsha>`.

Run it with a vault mounted at `/vault` (the image sets `AGENTMEM_ROOT_DIR=/vault`
and binds the HTTP transport to `0.0.0.0:8000`):

```sh
# The mounted vault must be writable by the image's non-root UID (65532).
docker run --rm -p 8000:8000 \
  -e AGENTMEM_HTTP_BEARER=change-me \
  -v "$PWD/vault:/vault" \
  ghcr.io/progamesigner/agentmem:latest
```

Set `AGENTMEM_HTTP_BEARER` for any deployment reachable off-host — the endpoint
is otherwise unauthenticated (a startup `WARN` is logged). Override any
[configuration variable](#configuration) with `-e`.

**Reachable by hostname (Kubernetes, ingress).** The http transport applies
DNS-rebinding protection and, by default, only accepts the loopback hosts
`localhost`, `127.0.0.1`, and `::1` in the inbound `Host` header. When clients
reach the server through a Kubernetes Service DNS name or an ingress hostname,
set `AGENTMEM_HTTP_ALLOWED_HOSTS` to those hostnames or every request is
rejected with `403`:

```sh
docker run --rm -p 8000:8000 \
  -e AGENTMEM_HTTP_BEARER=change-me \
  -e AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.default.svc.cluster.local,agentmem.example.com \
  -v "$PWD/vault:/vault" \
  ghcr.io/progamesigner/agentmem:latest
```

A bare hostname matches any port; add `:port` to pin one. The single value `*`
disables `Host` validation entirely — only appropriate when an upstream proxy or
ingress already enforces `Host` trust.

**Health checks.** The image has no shell, so it cannot carry a Docker
`HEALTHCHECK`. Probe the HTTP routes from your orchestrator instead. There are
two: `GET /healthz` (liveness — succeeds as soon as the process is up, never
gated on the recall index) and `GET /readyz` (readiness — `503` until the recall
index has finished its eager build, `200` thereafter, or immediately when recall
is disabled). Both are reachable without a bearer token.

```yaml
# Kubernetes — the kubelet probes over HTTP, nothing runs inside the container.
livenessProbe:
  httpGet: { path: /healthz, port: 8000 }
readinessProbe:
  httpGet: { path: /readyz, port: 8000 }
# A long cold build over a large vault fits a startupProbe with a high
# failureThreshold, after which liveness/readiness take over.
startupProbe:
  httpGet: { path: /readyz, port: 8000 }
  failureThreshold: 60
  periodSeconds: 5
```

A Docker Compose `healthcheck:` runs its command *inside* the container and so
cannot work against this image (no shell, no `wget`/`curl`). Probe `/healthz`
from outside instead — the orchestrator, a sidecar, or an external monitor.

Every published image carries the full set of dynamic OCI labels
(`org.opencontainers.image.{created,revision,version,title,description,source,url,authors,documentation,vendor}`):

```sh
docker inspect ghcr.io/progamesigner/agentmem:latest -f '{{json .Config.Labels}}'
```

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
| `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` | `<root>/AGENT_SESSION_CONTEXT.md` | Path to the global session-context template (see [Session context](#session-context)). Relative paths resolve against the vault root. Need not exist — falls back to the compiled-in default. |
| `AGENTMEM_POLICY` | `namespaced` | One of `scoped`, `namespaced`, `readonly`, `readwrite` (see [Policies](#policies)). |
| `AGENTMEM_TRANSPORT` | `http` | `http` or `stdio`. |
| `AGENTMEM_HTTP_BIND` | `127.0.0.1:8000` | HTTP bind address (http transport only). |
| `AGENTMEM_HTTP_BEARER` | *(unset)* | If set, `POST/GET /mcp` requires `Authorization: Bearer <token>`. Unset → unauthenticated (a startup `WARN` is logged). |
| `AGENTMEM_HTTP_ALLOWED_HOSTS` | *(unset)* | Comma-separated `Host` allow-list for the http transport's DNS-rebinding protection. Unset → loopback only (`localhost`, `127.0.0.1`, `::1`). List the cluster/ingress hostnames clients use (e.g. `agentmem.default.svc.cluster.local,agentmem.example.com:8000`); a bare hostname matches any port. The single value `*` disables `Host` validation (logs a `WARN`) — only for deployments that terminate `Host` trust at an upstream proxy. |
| `AGENTMEM_TIMEZONE` | `UTC` | IANA timezone used to date diary entries. |
| `AGENTMEM_HONOR_IGNORE_FILES` | `true` | Honour `.ignore` / `.gitignore` / `.obsidianignore` (nested, composed per-directory like `git`) for list and direct addressing. Strict boolean (`true`/`false`). |
| `AGENTMEM_INCLUDE_HIDDEN` | `false` | Include dotfiles/dot-directories. Strict boolean. |
| `AGENTMEM_INCLUDE_HIDDEN_GLOBS` | *(empty)* | Comma-separated gitignore-style globs (relative to the vault root) whose matches — and their whole subtree — are exempt from hidden filtering while other dotfiles stay excluded. E.g. `.obsidian/**,**/.config`. Ignore-file rules still apply unless also disabled. |
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
stem, so a human opening the vault in any editor can immediately see whose file is
whose, and another scope's file is structurally unaddressable.

### Worked layouts

**Default config** (`AGENTMEM_AGENTS_DIR=Agents`, `AGENTMEM_VFS_SCHEME=<agent>.<user>`):

```
vault/
├── Agents/                       ← agent-owned region (scoped, suffixed)
│   └── coder.alice/
│       ├── PERSONA.coder.alice.md
│       ├── MEMORY.coder.alice.md
│       ├── HEARTBEAT.coder.alice.md
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
| `load_session_context` | Render the scope's session-context (see [Session context](#session-context)); returns `{ rendered, missing }`. |
| `evolve_core_persona` | Atomic write to one of the five foundational files (`persona`/`prompt`/`rules`/`user`/`memory`), selected by `which`. Enforces line caps: `USER.md` ≤ 100, `MEMORY.md` ≤ 200. |
| `update_task_heartbeat` | Atomic write to `HEARTBEAT.md`. |
| `append_diary_entry` | Append a timestamped section to `diary/<YYYY-MM-DD>.md`; a newly created file opens with a `# <YYYY-MM-DD>` H1, and an optional `title` makes the heading `## <HH:MM:SS> — <title>`. |

Every tool's input schema includes the scope parameters derived from the active
scheme; introspect them via the standard MCP `tools/list` call.

Inside the agents folder the root level is **wrapper-only**: the core files
(`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `MEMORY.md`, `HEARTBEAT.md`)
are changed only through `evolve_core_persona` / `update_task_heartbeat`.
`write_memory_note` / `edit_memory_note` / `delete_memory_note` may only target
paths under a subfolder; a root-level target is rejected with `path_not_permitted`.
Reads of root files remain allowed.

### Cross-note links

Notes may reference each other with Obsidian `[[wikilink]]` syntax and relative
markdown links `[text](path.md)`. Write the **shortest unambiguous note name**
(`[[rust]]`, or `[[topics/rust]]` when a bare basename is shared by two visible
notes) — the server resolves it against your visible set (your own scope plus the
shared region) exactly as Obsidian resolves by basename.

Links round-trip transparently. On read you only ever see clean shortest names; on
write a link to your own scoped note is rewritten to its on-disk suffixed form so a
human browsing the vault in Obsidian can follow it, and a link to a shared note is
left clean. Aliases (`[[t|alias]]`), headings (`[[t#h]]`), and embeds (`![[t]]`)
are preserved. A link that does not resolve is left verbatim as a dangling link.

Because a suffixed link would expose your scope's existence, a note in the **shared
region** that links to one of **your own scoped** notes is rejected with
`write_denied`; link shared notes from shared notes, or scoped notes from scoped
notes.

## Session context

A **session-context template** weaves the five foundational files
(`PERSONA`/`PROMPT`/`RULES`/`USER`/`MEMORY`) together with operator prose and an
auto-generated memory-tools guide into a single rendered bootstrap. It is an
ordinary markdown document with `{{…}}` placeholders:

- `{{files.persona}}`, `{{files.prompt}}`, `{{files.rules}}`, `{{files.user}}`, `{{files.memory}}` — the foundational file contents (a missing file renders a sentinel)
- `{{scope.<key>}}` — a scope value (e.g. `{{scope.agent}}`); `<key>` is any scheme placeholder
- `{{tools_guide}}` — the server-generated memory-tools guide (the live tool catalogue only)

The compiled-in default template orders the sections `PERSONA → RULES → MEMORY →
USER → PROMPT → {{tools_guide}}` and embeds a *suggested* (non-enforced) memory
layout plus the documented line caps. External-tool facts (camera, SSH, etc.)
belong in `PROMPT.md`. Operators who supply their own template fully control and
may override that guidance.

The active template is resolved per request, first hit wins:

1. a per-scope `AGENT_SESSION_CONTEXT.md` inside the agents folder (suffix-resolved like any scoped file)
2. the global file at `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` (default `<root>/AGENT_SESSION_CONTEXT.md`)
3. a compiled-in default

Nothing errors on absence — a fresh vault renders an instructions-only bootstrap.
Unknown `{{…}}` tokens are left literal. The same rendered output is exposed
through three MCP surfaces plus one plain-HTTP endpoint:

| Surface | Shape | For |
|---|---|---|
| `load_session_context` tool | `{ rendered, missing }` | the model pulling its own context mid-session |
| `session-context` resource | `agentmem://session-context/{…}` (params follow the scheme) | client auto-attach |
| `session-context` prompt | required args follow the scheme | user slash-command |
| `GET /v1/context` | `text/markdown` (or `{ rendered, missing }` JSON) | a harness/client fetching the system prompt without MCP |

### `GET /v1/context`

A versioned, stateless, read-only HTTP route (HTTP transport only) that renders
the same bootstrap for a harness to fetch directly. Each VFS-scheme placeholder is
a query parameter; the scope is bound in scheme order:

```sh
# Markdown by default — drop straight into a system prompt.
curl 'http://127.0.0.1:8000/v1/context?agent=default&user=alice'

# JSON ({ rendered, missing }) via content negotiation.
curl -H 'Accept: application/json' \
  'http://127.0.0.1:8000/v1/context?agent=default&user=alice'
```

Missing, empty, or unexpected scope parameters return `400` with a
`{ "error": … }` body; absent foundational files are never errors. The route sits
behind the same `AGENTMEM_HTTP_BEARER` gate as `/mcp` (add
`-H "Authorization: Bearer <token>"` when a bearer is configured); only the
`/healthz` and `/readyz` probes are always reachable.

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

Every MCP client connects over one of the two transports:

- **stdio sidecar** — the client *launches* `agentmem` itself, so it owns the
  process lifecycle. You supply `AGENTMEM_ROOT_DIR` and `AGENTMEM_TRANSPORT=stdio`
  through the client's env block. The binary must be on `PATH` (or give an
  absolute path).
- **Local HTTP** — you run `agentmem` yourself (HTTP is the default transport, on
  `127.0.0.1:8000`) and point the client at `http://127.0.0.1:8000/mcp`. Start the
  server *before* the client connects.

The snippets below show both transports per client. When `AGENTMEM_HTTP_BEARER`
is set, attach `Authorization: Bearer <token>` through whatever header mechanism
the client exposes (noted inline where relevant).

### Claude Desktop

`claude_desktop_config.json` (stdio only):

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

### Claude Code

Add it from the CLI — stdio (note the `--` before the server command):

```sh
claude mcp add agentmem \
  --env AGENTMEM_ROOT_DIR=/path/to/vault --env AGENTMEM_TRANSPORT=stdio \
  -- agentmem

# …or HTTP against an already-running server:
claude mcp add --transport http agentmem http://127.0.0.1:8000/mcp
```

Or commit a project-scoped `.mcp.json` (`mcpServers` key; `command`/`env` for
stdio, `type`/`url` for HTTP):

```json
{
  "mcpServers": {
    "agentmem-stdio": {
      "command": "agentmem",
      "env": { "AGENTMEM_ROOT_DIR": "/path/to/vault", "AGENTMEM_TRANSPORT": "stdio" }
    },
    "agentmem-http": {
      "type": "http",
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

### Codex CLI

`~/.codex/config.toml` — each server is a `[mcp_servers.<name>]` table. A `command`
key makes it a stdio server; a `url` key (no `command`) makes it streamable HTTP:

```toml
# stdio sidecar
[mcp_servers.agentmem]
command = "agentmem"
env = { AGENTMEM_ROOT_DIR = "/path/to/vault", AGENTMEM_TRANSPORT = "stdio" }

# …or HTTP against an already-running server (use one table or the other)
[mcp_servers.agentmem]
url = "http://127.0.0.1:8000/mcp"
# bearer_token_env_var = "AGENTMEM_TOKEN"   # if AGENTMEM_HTTP_BEARER is set
```

`codex mcp add agentmem -- agentmem` is the stdio shortcut; HTTP servers are added
by editing `config.toml`.

### Antigravity CLI

Edit the MCP config at `~/.gemini/antigravity/mcp_config.json` (in the IDE: Agent
panel → **MCP Servers** → **Manage MCP Servers** → **View raw config**). The
top-level key is `mcpServers`, and HTTP uses **`serverUrl`** — *not* `url`:

```json
{
  "mcpServers": {
    "agentmem-stdio": {
      "command": "agentmem",
      "env": { "AGENTMEM_ROOT_DIR": "/path/to/vault", "AGENTMEM_TRANSPORT": "stdio" }
    },
    "agentmem-http": {
      "serverUrl": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

### GitHub Copilot

Workspace `.vscode/mcp.json` — Copilot (agent mode) reads VS Code's MCP config.
The top-level key is **`servers`** (not `mcpServers`) and every entry carries an
explicit `type`. Optional `inputs` prompt for secrets:

```json
{
  "inputs": [
    { "type": "promptString", "id": "agentmem-token", "description": "AgentMem bearer", "password": true }
  ],
  "servers": {
    "agentmem-stdio": {
      "type": "stdio",
      "command": "agentmem",
      "env": { "AGENTMEM_ROOT_DIR": "${workspaceFolder}/vault", "AGENTMEM_TRANSPORT": "stdio" }
    },
    "agentmem-http": {
      "type": "http",
      "url": "http://127.0.0.1:8000/mcp",
      "headers": { "Authorization": "Bearer ${input:agentmem-token}" }
    }
  }
}
```

Drop the `inputs` block and the `headers` line when no bearer is configured.

### VS Code

VS Code's native MCP support uses the **same `.vscode/mcp.json` format** shown for
Copilot above (the `servers` key, `type: stdio` / `type: http`). The only
difference is *where* it can live: per workspace in `.vscode/mcp.json`, or for all
workspaces in your user profile — open the latter via the command palette
(**MCP: Open User Configuration**) rather than editing it by hand.

### OpenCode

`opencode.json` — servers live under the `mcp` key. OpenCode names the transports
`local`/`remote`, takes `command` as an **array**, and uses `environment` (not
`env`):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "agentmem-local": {
      "type": "local",
      "command": ["agentmem"],
      "enabled": true,
      "environment": { "AGENTMEM_ROOT_DIR": "/path/to/vault", "AGENTMEM_TRANSPORT": "stdio" }
    },
    "agentmem-remote": {
      "type": "remote",
      "url": "http://127.0.0.1:8000/mcp",
      "enabled": true
    }
  }
}
```

For a bearer-protected remote, add `"headers": { "Authorization": "Bearer …" }`.

### Generic MCP client

Most clients accept the canonical `mcpServers` block — `command`/`env` for a
stdio sidecar, `url` for Local HTTP:

```json
{
  "mcpServers": {
    "agentmem-stdio": {
      "command": "agentmem",
      "env": { "AGENTMEM_ROOT_DIR": "/path/to/vault", "AGENTMEM_TRANSPORT": "stdio" }
    },
    "agentmem-http": {
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

### curl (raw HTTP transport)

For poking the HTTP transport directly, without an MCP client:

```sh
# Liveness and readiness probes.
curl http://127.0.0.1:8000/healthz
curl http://127.0.0.1:8000/readyz

# An MCP request (Streamable HTTP requires the dual Accept header).
curl -X POST http://127.0.0.1:8000/mcp \
  -H 'Accept: application/json, text/event-stream' \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

When `AGENTMEM_HTTP_BEARER` is set, add `-H "Authorization: Bearer <token>"`.

## Human-in-the-loop editing

Because the on-disk layout is plain markdown, a human can open the vault in any
editor — Obsidian is a convenient choice — and hand-edit any `Agents/<scope>/...`
file directly. The agent will see
the human's edits as if it had written them itself — this is the supported channel
for curating or correcting an agent's memory. Creating `plan.coder.alice.md` by
hand makes it appear to the `coder.alice` scope as the virtual note `plan.md`.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```
