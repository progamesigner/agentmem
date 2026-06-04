## Why

AI agents in this project lack durable, structured, multi-tenant memory. Today an agent forgets context across sessions and has no safe, namespaced place to record persona, working state, diaries, or learned skills. We want a single MCP server, in front of an Obsidian-style markdown vault, that lets multiple agents read and write memory concurrently while keeping the human-curated parts of the vault untouched.

We are building this **now** because (1) the Rust `rmcp` SDK has stabilised enough to use the official MCP protocol over both stdio and HTTP, and (2) the vault layout has been used long enough informally that the directory conventions are stable and ready to be encoded into a server.

## What Changes

- Introduce a new Rust 2024 binary crate `agentmem` that runs an MCP server using the official `rmcp` SDK.
- Expose a small, mostly-generic tool surface to agents:
  - generic memory ops — `list_memory_notes` (paginated), `read_memory_note`, `write_memory_note`, `edit_memory_note` (search/replace, search string must be unique), `delete_memory_note`;
  - session bootstrap — `load_session_context` (returns the agent's `PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md` in a single call);
  - thin domain wrappers for the most-used patterns — `evolve_core_persona` (atomic update to any one of the five foundational session files, selected by a `which` parameter), `update_task_heartbeat` (atomic update to `HEARTBEAT-STATE.md`), `append_diary_entry` (appends a timestamped section to today's diary file `diary/YYYY-MM-DD.md`).
- Implement a **two-axis policy model** consisting of:
  - A configurable **agents folder** (`AGENTMEM_AGENTS_DIR`, default `Agents`) that demarcates the scoped, suffix-applied region of the vault. The agents folder may be the vault root itself (set to `.` or empty), in which case the whole vault is treated as agent memory.
  - A configurable **VFS suffix template** (`AGENTMEM_VFS_TEMPLATE`, default `<agent>.<user>`) that defines how scope keys are spelled into directory segments and filename stems. Each `<keyname>` in the template becomes a required scope parameter on every tool call; the schema for the tool surface is **generated from the template**. Default placeholders are `<agent>` and `<user>`; any additional placeholder (`<team>`, `<env>`, …) is permitted and adds a required parameter. An empty template disables suffixing entirely.
  - A single **server-wide policy** (`AGENTMEM_POLICY`, one of `scoped`, `namespaced`, `readonly`, `readwrite`; default `namespaced`) that governs what permissions exist where:
    - *scoped* — inside the agents folder: own-scope read/write with suffix; outside the agents folder: **denied**.
    - *namespaced* — inside the agents folder: own-scope read/write with suffix; outside the agents folder: **read-only** (no suffix; same file visible to every agent).
    - *readonly* — inside the agents folder: own-scope read-only with suffix; outside the agents folder: read-only. **No writes anywhere.**
    - *readwrite* — inside the agents folder: own-scope read/write with suffix; outside the agents folder: read/write (no suffix; same file visible/writable for every agent).
  - The own-scope rule inside the agents folder applies whenever the template is non-empty; an empty template degenerates the agents folder into a plain shared directory governed by the policy's outside-folder rules.
  - Anything outside the configured vault root is always *denied*.
- The scope keys are supplied **per tool call** so one server process can serve many agents at once.
- Support both transports in v1, with `http` as the **default**:
  - `http` (default) — Streamable HTTP / SSE via `axum`, default bind `127.0.0.1:8000` so local development needs no CORS or auth configuration; remote deployments override the bind explicitly;
  - `stdio` — for local/sidecar use such as Claude Desktop and Claude Code;
  - Transport selected via `AGENTMEM_TRANSPORT`; if unset, `http` is used.
- Enforce security invariants:
  - canonicalise every virtual path and reject any resolution that escapes the configured vault root;
  - all full-file writes go through a `write-temp-then-rename` atomic pattern;
  - all server logs go to stderr (never stdout) so stdio JSON-RPC is never corrupted.
- Apply **visibility filters** to every list / read / write so agents never see noise or accidentally trample human tooling state:
  - hidden entries (any path segment starting with `.`) are excluded by default;
  - `.gitignore` and `.obsidianignore` patterns inside the vault are honoured by default for list and write;
  - both filters can be disabled via env vars when an operator genuinely wants full visibility.
- Return human-readable, actionable errors (e.g. *"namespace 'coder.alice' is not permitted to write under 'Actions/'"*) rather than raw OS errors.

## Capabilities

### New Capabilities
- `mcp-server`: the MCP protocol surface itself — server lifecycle, transports (stdio + Streamable HTTP/SSE), tool registration, error mapping, logging discipline.
- `vault-storage`: the on-disk vault model — vault root, configurable agents folder, VFS suffix template (with arbitrary `<key>` placeholders), single server-wide policy (`scoped` / `namespaced` / `readonly` / `readwrite`), atomic writes, traversal prevention.
- `memory-tools`: the agent-facing tool API — `list_memory_notes`, `read_memory_note`, `write_memory_note`, `edit_memory_note`, `delete_memory_note`, `load_session_context`, `evolve_core_persona`, `update_task_heartbeat`, `append_diary_entry`, including JSON schema contracts and error semantics.
- `configuration`: environment-variable-driven configuration — `AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_TEMPLATE`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, `AGENTMEM_LOG`, and how they map onto runtime behaviour.

### Modified Capabilities
<!-- None: this is the first change for this project. The `openspec/specs/` directory is empty. -->

## Impact

- **New crate**: `agentmem` (Rust 2024, binary). Adds dependencies on `rmcp`, `tokio`, `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `schemars`, `thiserror`, `anyhow`, `axum` (HTTP transport), `tower`, `tower-http`, `clap` (light CLI for flags that mirror env vars), `chrono`/`chrono-tz` (timezone for diary), `ignore` (the `ripgrep` walker, for `.gitignore`/`.obsidianignore`/hidden filtering), and dev-deps `tempfile`, `assert_fs`, `insta` (snapshot tests for tool schemas).
- **No changes to existing code** — the repo currently has no Rust source, only PRDs and OpenSpec scaffolding, so the change is purely additive.
- **Vault layout**: requires the user to point `AGENTMEM_ROOT_DIR` at an Obsidian vault (or any markdown directory). The server will create `Agents/<...>` subdirectories on first write but will never touch files outside the configured namespaced areas.
- **Distribution**: shipped as `cargo install agentmem` and as a release binary in CI. No language runtime required on the host.
- **Out of scope for this change** (deferred to follow-ups):
  - **Per-tenant authentication / authorization on claimed scope.** In v1 the server trusts the scope keys an agent supplies — a misbehaving client could impersonate another agent's scope. Real authentication (OAuth / mTLS / per-tenant tokens that bind to scope keys) is the subject of a follow-up change.
  - Vector or graph indexing of vault content.
  - Sync with remote git.
  - File-watch-based change notifications back to the agent.
