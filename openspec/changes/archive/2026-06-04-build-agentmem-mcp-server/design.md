## Context

The `agentmem` project today consists of OpenSpec scaffolding only — there is no Rust source. The user's intent is a single Rust MCP server that fronts an Obsidian-style markdown vault. Multiple agents will connect concurrently; each call carries its own scope keys (agent name, user name, and any additional keys the operator configures) so one server process serves many tenants. The vault is **also edited by a human in Obsidian**, so the server must clearly separate the region it owns (the *agents folder*, default `Agents/`) from the region the human owns (the rest of the vault, treatment governed by a single server-wide policy).

Constraints:
- Rust 2024 edition.
- Official `rmcp` SDK for MCP protocol implementation.
- Both stdio and Streamable HTTP/SSE transports required at v1.
- File-based storage only — no database, no vector index.
- Logging on stdio transport must never reach stdout.
- Vault layout is human-readable on disk so a human can browse it in Obsidian without translation tooling.

Stakeholders:
- Agents (LLM-driven MCP clients) — primary consumer of the tool surface.
- The human author of the vault (the user) — owns the non-`Agents/**` regions.
- Future orchestrators that may speak HTTP to a remotely-hosted server.

## Goals / Non-Goals

**Goals:**
- Provide a small, generic, predictable tool surface that agents can reason about without per-category special cases.
- Guarantee that agents cannot escape the vault root, cannot write into human-owned regions, and cannot collide with one another's files.
- Make the on-disk layout legible to a human reading the vault directly in Obsidian.
- Keep configuration declarative (env vars) so the same binary serves many deployment shapes.
- Ship both stdio and Streamable HTTP transports in v1.

**Non-Goals:**
- No authentication or per-tenant authorization beyond an optional static bearer token on the HTTP transport. Production multi-tenant auth is a follow-up.
- No vector search, embeddings, or semantic indexing.
- No file-watcher push notifications back to MCP clients.
- No git or remote-sync integration.
- No GUI or admin surface.
- No schema migration tool for the vault — the layout is conventional, not enforced.

## Decisions

### D1. Use `rmcp` (the official Rust MCP SDK) rather than a hand-rolled JSON-RPC layer
**Why:** It is the only SDK that tracks the MCP spec (transports, capability negotiation, tool/resource schemas) authoritatively. Hand-rolling JSON-RPC would re-implement what the SDK already provides and would drift from the spec.
**Alternatives considered:**
- `mcp-core` (community crate) — smaller surface but lags the spec.
- Hand-rolled `jsonrpsee` — full control but doubles the maintenance surface.

### D2. Scope (agent / user) is a per-call argument, not a session attribute
**Why:** The user explicitly wants one server process to serve many agents concurrently. Per-call scope means no session state to manage, no auth handshake required for stdio, and the same MCP server can be addressed by multiple parallel agent processes piping to the same HTTP transport.
**Implication:** The server **trusts** the claimed scope in v1. Path traversal is still prevented by canonicalisation. Cross-tenant impersonation is **explicitly deferred to a follow-up authentication change** (see "Deferred to follow-up changes" below) — the architecture below leaves the boundary clean so adding auth later is purely additive (a middleware that validates scope keys against an authenticated identity).
**Alternatives considered:**
- Scope from env vars at startup → forces one process per agent, contradicts the user's requirement.
- Scope from MCP session metadata → MCP `initialize` does not carry a standardised tenant claim; would require a bespoke negotiation, which the follow-up change will design once the auth model is chosen.

### D3. Single server-wide policy, two distinguished regions
**Why:** The vault is a hybrid space (human-owned + agent-owned, per-agent vs. shared). Per-path glob lists are unnecessarily flexible for the user's actual deployment shape; one server-wide policy plus a single distinguished "agents folder" captures every needed combination with far less configuration surface and is much easier to audit.

There are exactly **two regions** per server instance:
1. **Inside the agents folder** (`<root>/<AGENTMEM_AGENTS_DIR>/...`). Writes (when permitted by the policy) are subject to the VFS suffix template; the resolver always appends the caller's own suffix, making cross-scope access structurally impossible. Listings strip the suffix to present clean virtual paths.
2. **Outside the agents folder but inside the vault root.** No suffix is applied; every agent that can reach a file here sees the same physical file. Whether reads or writes are allowed is determined by the policy.

The policy (`AGENTMEM_POLICY`) is one of:

| Policy | Inside agents folder | Outside agents folder (still inside vault root) |
|---|---|---|
| `scoped` | own-scope R+W, suffix applied | **denied** |
| `namespaced` *(default)* | own-scope R+W, suffix applied | read-only, no suffix |
| `readonly` | own-scope read-only, suffix applied | read-only, no suffix |
| `readwrite` | own-scope R+W, suffix applied | R+W, no suffix |

Anything resolving outside the vault root is always *denied*. When the agents folder is set to the vault root itself (`AGENTMEM_AGENTS_DIR=.`), the "outside" region is empty and the policy's outside-folder column has no effect.

The own-scope strictness inside the agents folder: the resolver appends the caller's own suffix on every read/write/edit, so a request for `PERSONA.md` from scope keys `{agent: coder, user: alice}` resolves to `PERSONA.coder.alice.md` and there is no way to address `PERSONA.coder.bob.md`. Listing also filters by own-scope suffix.

**Alternatives considered:**
- Per-path glob classification (a separate list per class). Rejected: more configuration surface than needed for the actual deployment shapes; harder to reason about precedence.
- Coupling the policy and template (a single "mode" env var). Rejected: they are genuinely orthogonal — the template chooses how scope keys are spelled into the filesystem, the policy chooses what permissions exist where.

### D4. VFS suffix template language
**Why:** The user wants the human to be able to open the vault in Obsidian and immediately see whose file is whose, while the agent still sees clean virtual paths. Appending the scope to the **stem**, not only as a parent directory, keeps files at the same depth a human expects *and* makes the file self-identifying if it is ever exported or moved. Different deployments need different scope shapes — agent-only, user-only, agent+user, or richer ones with team / environment / session keys — so the format is a configurable template, not a fixed enum.

**Template grammar:**
```
template     := segment ( '.' segment )*
segment      := placeholder | literal
placeholder  := '<' ident '>'
literal      := [A-Za-z0-9_-]+
ident        := [A-Za-z_][A-Za-z0-9_]*
```
- Each `<ident>` becomes a required scope parameter on every tool call (its parameter name is the ident).
- Identical idents collapse to a single parameter; repeating the placeholder in the template repeats its value in the rendered suffix.
- `<agent>` and `<user>` are not reserved — they are simply the two friendly defaults; operators can name keys whatever fits the domain.
- An **empty template string** disables suffixing entirely (no scope parameters required; no own-scope filtering applied).

**Rendering rule:** at resolve time, every placeholder is replaced by the caller-supplied value to produce a single string `<rendered>`. The same `<rendered>` string is used both as the per-scope directory segment under the agents folder AND as the dotted suffix on the file stem.

**Examples:**
- Template `<agent>.<user>`, caller `{agent: "coder", user: "alice"}`, virtual `tasks/plan.md`
  → directory segment `coder.alice`, suffix `.coder.alice`
  → physical `<root>/<agents_dir>/coder.alice/tasks/plan.coder.alice.md`
- Template `<agent>`, caller `{agent: "coder"}`, virtual `HEARTBEAT-STATE.md`
  → physical `<root>/<agents_dir>/coder/HEARTBEAT-STATE.coder.md`
- Template `<team>.<agent>.<env>.<user>`, caller `{team: "platform", agent: "coder", env: "prod", user: "alice"}`, virtual `tasks/plan.md`
  → physical `<root>/<agents_dir>/platform.coder.prod.alice/tasks/plan.platform.coder.prod.alice.md`
- Template empty, virtual `tasks/plan.md`
  → physical `<root>/<agents_dir>/tasks/plan.md` (no per-scope subdirectory, no suffix)

**Alternatives considered:**
- A fixed enum (`agent-and-user` / `agent-only` / `user-only` / `none`). Rejected as the user explicitly asked for arbitrary additional scope keys (e.g. team, environment).
- Suffix as a parent dir only (no filename suffix). Rejected: when a human exports or moves a file, the scope identity disappears.
- JSON sidecar with metadata. Rejected: not human-readable in Obsidian.

### D5. Atomic writes via temp-file + rename
**Why:** Crash-safe. The Rust `tempfile::NamedTempFile::persist` API gives a rename-on-same-volume guarantee on Linux/macOS and a best-effort on Windows.
**Risk:** `edit_workspace_file` (search/replace) is also full-file rewrite under the hood; the atomicity covers it. Concurrency: per-path advisory lock with `fs2::FileExt::try_lock_exclusive` to prevent two concurrent writers in the same process from racing; cross-process races are tolerated by the rename (last writer wins) and explicitly documented.

### D6. Two transports via a thin selector, HTTP as default
**Why:** `rmcp` natively supports stdio and HTTP transports; selecting between them via `AGENTMEM_TRANSPORT={stdio,http}` keeps the binary single. **HTTP is the default** because the primary deployment model is a long-running service that multiplexes many agents; stdio is the secondary mode for editor/desktop sidecar integrations. HTTP uses `rmcp`'s Streamable HTTP + SSE transport behind `axum` so we can host additional `/health` route alongside.

**Default bind**: `127.0.0.1:8000` when `AGENTMEM_HTTP_BIND` is unset. Loopback-only by default eliminates the need for CORS or bearer-token setup during local development — the typical first-run path. Operators deploying to a container / shared host explicitly set `AGENTMEM_HTTP_BIND=0.0.0.0:<port>` and, in production, also `AGENTMEM_HTTP_BEARER`; the server emits a startup `WARN` when bound on a non-loopback interface without a bearer token.

**Logging discipline:** on stdio, `tracing-subscriber` is wired to `std::io::stderr()` only; on http, it can also write to stdout safely. A single env flag (`AGENTMEM_LOG=info,agentmem=debug`) controls level filtering.

### D7. Tool surface: generic + 4 ergonomic wrappers
Final list of v1 tools (9 total):
| Tool | Purpose |
|---|---|
| `list_memory_notes` | paginated listing of virtual paths visible to the given scope |
| `read_memory_note` | read by virtual path |
| `write_memory_note` | atomic full-file write |
| `edit_memory_note` | atomic search/replace (search string must be unique in the target file) |
| `delete_memory_note` | delete a single file by virtual path (rejected when the region is read-only under the active policy) |
| `load_session_context` | bootstrap read of the five foundational session files in a single call: `PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md` — each resolved relative to the agents folder for the active scope |
| `evolve_core_persona` | atomic write to exactly one of the five foundational files above, selected by a `which: "persona" \| "prompt" \| "rules" \| "user" \| "tools"` parameter — a single ergonomic wrapper that lets agents update any of the session-context files without having to spell out their conventional path |
| `update_task_heartbeat` | atomic write hardcoded to the scope's `HEARTBEAT-STATE.md` inside the agents folder |
| `append_diary_entry` | append a timestamped section to today's diary file `diary/YYYY-MM-DD.md` inside the scope's agents-folder area |

`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md`, `HEARTBEAT-STATE.md` and `diary/YYYY-MM-DD.md` are simply *conventional names* inside each scope's region of the agents folder — they get no special storage, only special tool entrypoints. This is the "thin wrappers over generic ops" model the user asked for.

**Conventional-file paths under non-default configuration:** because the agents folder name is configurable and may even be the vault root, the wrapper tools resolve their virtual paths relative to the agents folder, not as the literal string `Agents/...`. With default config they map to `Agents/PERSONA.md` etc.; with `AGENTMEM_AGENTS_DIR=.` they map to `PERSONA.md` at the vault root.

**Pagination semantics for `list_memory_notes`:** the tool accepts an optional `limit` (default 200, max 1000) and an optional opaque `cursor` (a base64-encoded byte offset into the walker's deterministic ordering). The response is `{ items: [VirtualPath], next_cursor: Option<String> }`. The walker order is stable for a given vault state so paging across calls is consistent absent concurrent writes.

**Diary semantics:** `append_diary_entry` takes a single `content` string. The server constructs today's date in the configured timezone (`AGENTMEM_TIMEZONE`, default UTC), resolves the virtual path `<agents_dir>/diary/<YYYY-MM-DD>.md`, and appends `\n## <HH:MM:SS>\n<content>\n` to the existing contents (creating the file with the section as its initial contents if absent). The full read-modify-write goes through the atomic-write procedure so concurrent appends within the same process are serialised by the per-target lock from D5. This is intentionally implemented as full-file rewrite rather than `O_APPEND` so the atomic-rename guarantee holds.

**Delete semantics:** `delete_memory_note` removes a single file. It honours policy + region + own-scope just like `write_memory_note`: rejected with `write_denied` under `readonly`, rejected with `write_denied` outside the agents folder under `namespaced` / `scoped`, structurally limited to the caller's own scope inside the agents folder. Deletion is a `std::fs::remove_file` (no recursive directory removal); the parent directory is left in place even if it becomes empty. Missing target yields `not_found`.

### D8. Error handling: `thiserror` enum → human strings at the MCP boundary
**Why:** Internal layers carry typed errors (`PathError::EscapesRoot`, `PolicyError::WriteDenied { class }`, `IoError`, etc.). At the MCP tool boundary they are formatted into a single user-facing string the LLM can read, with the typed kind included as a structured `code` field for orchestrators. Raw OS errors are never bubbled up verbatim.

### D9. Template-driven schema generation
The template is parsed once at startup into an ordered list of placeholder idents (`["agent", "user"]` by default). The MCP tool input schema is then assembled dynamically: each ident contributes a `string` field with the same name, all required. There is no compile-time `Mode` enum — the schema is data, derived from the template.

At runtime each tool call carries a `BTreeMap<String, String>` of scope values; the resolver validates that the exact set of keys matches the template's idents (no missing keys, no extra keys), renders the suffix string, and proceeds. This makes the binary trivially configurable for any deployment shape (agent-only, user-only, agent+user, team+agent+env+user, etc.) without code changes.

**Tool schemas are still introspectable** via the standard MCP `tools/list` response — the server runs the same template-to-schema generator at startup and the result is what clients receive. `insta` snapshot tests cover several representative templates to lock the JSON shape.

### D10. Visibility filters: ignore files + hidden entries
**Why:** A vault opened in Obsidian routinely contains `.obsidian/` (workspace state), `.git/` (if version-controlled), and `.gitignore`-listed scratch files. Surfacing these to the agent creates noise and risks the agent overwriting tool state. We adopt the same visibility model as `ripgrep` / Obsidian's own search.

- **Hidden filter:** any path segment starting with `.` is excluded from list / read / write / edit / delete by default. Toggle off with `AGENTMEM_INCLUDE_HIDDEN=true` for operators who explicitly want it.
- **Ignore-file filter:** `.gitignore` and `.obsidianignore` patterns are honoured by default via the `ignore` crate's `WalkBuilder` (the same machine ripgrep uses); patterns apply hierarchically per-directory. Toggle off with `AGENTMEM_HONOR_IGNORE_FILES=false`.
- **Scope of enforcement:** filters apply to `list_memory_notes` (excluded entries are absent from results) and to **direct addressing** of files in the read / write / edit / delete tools (excluded entries resolve to `path_not_permitted` rather than leaking their existence via `not_found`).
- **Bypass:** the agents folder itself is never filtered out even if it happens to match (e.g. `AGENTMEM_AGENTS_DIR=.agents`); the server resolves the agents folder before the filter consults the path.

**Alternatives considered:**
- Hardcoded blocklist (`.git`, `.obsidian`). Rejected — fails on the long tail of dotfiles each vault accumulates.
- A single bespoke ignore env var with custom syntax. Rejected — gitignore syntax is already standard and well-understood.

## Risks / Trade-offs

- **[Cross-process write races on the same file]** → the rename-on-write keeps each rename atomic, but two near-simultaneous writers can produce last-writer-wins. *Mitigation:* document; the per-scope layout makes intra-agent races rare and inter-agent races impossible at the same path inside the agents folder.
- **[Human-edited agent files inside the agents folder]** → if a human edits or creates `plan.coder.alice.md` directly in the agents folder, the agent will see it as `plan.md`. **This is an intentional feature, not a risk** — it is the supported channel by which a human can hand-curate or correct an agent's memory by opening the vault in Obsidian. The agent has no way to tell whether a value originated from itself or from the human; both are equally authoritative.
- **[`edit_memory_note` ambiguity]** → if the search string occurs multiple times, the LLM may not know which occurrence it edited. *Mitigation:* `edit_memory_note` rejects the call with a clear error (`edit_search_ambiguous`) when the search string occurs zero or more than once, so the agent retries with a longer, more specific snippet.
- **[Streamable HTTP transport churn]** → the MCP spec for HTTP transport has changed across draft revisions. *Mitigation:* pin `rmcp` to a known-good minor version and gate transport features behind a Cargo feature flag (`transport-http`) so a future stdio-only build is still possible.
- **[Ignore-file walker overhead]** → honouring `.gitignore`/`.obsidianignore` on every list requires a `ignore::WalkBuilder`-class walker. *Mitigation:* the `ignore` crate is fast enough for vaults under O(100k) files and respects per-directory ignore files; pagination caps the per-call cost; the walker is bypassed entirely on direct read / write / delete paths since those address a single file.

## Deferred to follow-up changes

These are deliberate v1 omissions, called out so they are not forgotten:

- **Per-tenant authentication on claimed scope.** v1 trusts the scope keys an agent supplies — anyone reachable via the configured transport can address any scope. The HTTP transport's static `AGENTMEM_HTTP_BEARER` only protects the endpoint, not individual tenants. A follow-up change will bind scope keys to authenticated identities (OAuth claims / mTLS subject / per-tenant tokens), reject mismatches at the tool boundary, and harden the multi-tenant story.
- **CORS / auth presets for non-local HTTP.** v1's default bind is `127.0.0.1:8000` so local development needs no CORS or bearer. Production hardening (CORS allow-list policy, mandatory bearer when bound on non-loopback) will land alongside the auth change.

## Migration Plan

- N/A for code: this is the initial implementation; nothing to migrate from.
- For a user's existing vault: point `AGENTMEM_ROOT_DIR` at the vault. The server will lazily create `Agents/<scope>/...` on first write. Existing human-authored files are left untouched and remain `shared_readonly` to agents.
- **Rollback:** stopping the server has zero side effects on the vault; on-disk files are plain markdown.

## Open Questions

_None remaining for v1._ The previously-open questions about `.obsidianignore` handling, CORS defaults, and a delete tool are now decided in D10, D6, and D7 respectively.
