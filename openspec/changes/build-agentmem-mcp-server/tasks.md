## 1. Crate scaffold

- [ ] 1.1 Initialise a Rust 2024 single-crate package at the repository root (`Cargo.toml` + `rust-toolchain.toml` pinning a stable channel that ships edition 2024). _The original task said "workspace"; we collapsed to a single crate per user direction since there is only one binary._
- [ ] 1.2 Create the `agentmem` binary at the repository root with `src/main.rs`, `src/lib.rs`, and modules `config`, `template`, `path`, `policy`, `storage`, `tools`, `transport`, `mcp`, `error`, `telemetry`
- [ ] 1.3 Add runtime dependencies: `rmcp` (pinned), `tokio` (multi-thread, macros, rt, fs, signal), `tracing`, `tracing-subscriber` (env-filter), `serde`, `serde_json`, `schemars`, `thiserror`, `anyhow`, `clap` (derive), `tempfile`, `fs2`, `axum`, `tower`, `tower-http`, `chrono`, `chrono-tz`, `ignore` (ripgrep's walker, for gitignore/obsidianignore/hidden filtering), `base64` (for opaque pagination cursors)
- [ ] 1.4 Add dev dependencies: `assert_fs`, `predicates`, `insta`, `rstest`, `serde_json`, an MCP client harness suitable for stdio + http integration tests
- [ ] 1.5 Configure `[features]` so the HTTP transport is gated behind `transport-http` (default on)
- [ ] 1.6 Add `Cargo.lock` to version control and a CI workflow stub that runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test`
- [ ] 1.7 Add a `README.md` skeleton that points to the OpenSpec change and notes that the project is in active development

## 2. Configuration layer

- [x] 2.1 Define a `Config` struct in `config.rs` matching every env var listed in `specs/configuration/spec.md`
- [x] 2.2 Implement `Config::from_env()` returning a typed error per the configuration spec's fail-fast scenarios
- [x] 2.3 Implement `Config::from_cli_and_env()` so CLI flags from `clap` override env values
- [x] 2.4 Validate that `AGENTMEM_ROOT_DIR` exists, is a directory, and is canonicalisable
- [x] 2.5 Validate that `AGENTMEM_AGENTS_DIR` is a relative path with no traversal, or `.`/empty for "vault root", and join it onto the canonicalised root
- [x] 2.6 Parse `AGENTMEM_TRANSPORT` into a `Transport` enum (`Stdio`, `Http { bind: SocketAddr, bearer: Option<String> }`); when unset, default to `Http { bind: 127.0.0.1:8000, bearer: None }`
- [x] 2.7 Parse `AGENTMEM_TIMEZONE` into a `chrono_tz::Tz` value, defaulting to `UTC`, with a clear startup error on invalid IANA identifiers
- [x] 2.8 Parse `AGENTMEM_POLICY` into a `Policy` enum (`Scoped`, `Namespaced`, `Readonly`, `Readwrite`), defaulting to `Namespaced`
- [x] 2.9 Parse `AGENTMEM_HONOR_IGNORE_FILES` and `AGENTMEM_INCLUDE_HIDDEN` as strict booleans, defaulting to `true` and `false` respectively; invalid values exit non-zero with a clear stderr message
- [x] 2.10 Compute the active `EnvFilter` for `tracing_subscriber` from `AGENTMEM_LOG` (default `warn,agentmem=info`)
- [x] 2.11 Unit-test every scenario in `specs/configuration/spec.md`

## 3. VFS template

- [x] 3.1 Define a `Template { segments: Vec<Segment> }` type where `Segment` is `Literal(String) | Placeholder(String)`
- [x] 3.2 Implement `Template::parse(s: &str) -> Result<Template, TemplateError>` honouring the grammar in `design.md` D4; reject malformed brackets and invalid placeholder idents with structured errors
- [x] 3.3 Implement `Template::placeholders(&self) -> Vec<&str>` returning the ordered, de-duplicated list of placeholder idents (the required scope parameter names)
- [x] 3.4 Implement `Template::render(&self, scope: &BTreeMap<String, String>) -> Result<String, RenderError>` validating that scope keys exactly match the placeholder set and producing the dotted rendered string
- [x] 3.5 Implement `Template::is_empty(&self) -> bool` and ensure all downstream consumers (resolver, policy, list) treat empty templates as "no suffix, no own-scope filtering"
- [x] 3.6 Implement `Template::to_json_schema(&self) -> serde_json::Value` producing the scope-fields fragment that gets merged into each tool's input schema
- [x] 3.7 Unit-test every "VFS template resolution" scenario in `specs/vault-storage/spec.md`, plus malformed templates

## 4. Path resolution

- [x] 4.1 Define `VirtualPath(Utf8PathBuf)` and `PhysicalPath(PathBuf)` newtypes with constructors that reject empty, absolute, or traversal-bearing inputs
- [x] 4.2 Implement `PathResolver::detect_region(virtual_path) -> Region` returning `InsideAgentsFolder | OutsideAgentsFolder` based on the resolved agents-folder path
- [x] 4.3 Implement `PathResolver::resolve(scope, virtual_path) -> PhysicalPath`:
  - inside the agents folder with non-empty template → append rendered suffix to filename stem AND insert rendered string as the first path segment under the agents folder
  - inside the agents folder with empty template → no suffix, no per-scope segment
  - outside the agents folder → no transformation
- [x] 4.4 Implement `PathResolver::strip_suffix(physical, scope) -> Option<VirtualPath>` for listing inside the agents folder; returns `None` when the file does not belong to the caller's scope
- [x] 4.5 Unit-test traversal vectors: `..`, absolute paths, embedded null bytes, percent-encoded segments, and symlink escapes (use `tempfile` to construct fixtures)
- [x] 4.6 Unit-test cross-scope unreachability: every input (legitimate or crafted) for scope A inside the agents folder resolves either to scope A's physical path or to `not_found` — never to scope B's file

## 5. Policy enforcement

- [x] 5.1 Define `Permission { read: bool, write: bool }` and `Policy::permission_for(region: Region) -> Permission` covering the four policies × two regions matrix from `specs/vault-storage/spec.md`
- [x] 5.2 Implement a `gate_read(policy, region) -> Result<(), PolicyError>` and `gate_write(policy, region) -> Result<(), PolicyError>` helpers that all tool handlers call before any I/O
- [x] 5.3 Implement `Policy::list_visible_regions(template_is_empty: bool) -> Vec<Region>` so `list_workspace_files` knows which regions to walk
- [x] 5.4 Unit-test every "Policy enforcement" scenario in `specs/vault-storage/spec.md`

## 6. Storage layer

- [x] 6.1 Implement `Storage::read(physical) -> Result<String, StorageError>` returning UTF-8 contents and mapping IO errors to typed kinds (`NotFound`, `Io`)
- [x] 6.2 Implement `Storage::write_atomic(physical, content)` using `tempfile::NamedTempFile::new_in(parent)`, `write_all`, `as_file().sync_all()`, then `persist`
- [x] 6.3 Implement `Storage::edit_search_replace(physical, search, replace)` with uniqueness preconditions returning `EditSearchNotFound` or `EditSearchAmbiguous`
- [x] 6.4 Implement `Storage::delete(physical)` calling `std::fs::remove_file`; map missing target to `NotFound`; do NOT remove parent directories
- [x] 6.5 Implement a `Walker` wrapping `ignore::WalkBuilder` configured from `AGENTMEM_HONOR_IGNORE_FILES` and `AGENTMEM_INCLUDE_HIDDEN`; ensure the agents folder is always traversable even when it begins with `.`
- [x] 6.6 Implement `Storage::list_inside_agents_folder(scope, template, walker)` using the walker, applying suffix-stripping, and filtering files not owned by the scope
- [x] 6.7 Implement `Storage::list_outside_agents_folder(walker, vault_root, agents_root)` using the walker for the rest of the vault (when permitted by policy) and returning clean virtual paths
- [x] 6.8 Implement `Storage::is_visible(virtual_path)` reusing the walker's match logic so direct read / write / edit / delete can reject hidden or ignored paths with `path_not_permitted` before any IO
- [x] 6.9 Implement opaque pagination cursors: `Cursor::encode(offset: u64) -> String` (base64) and `Cursor::decode(&str) -> Result<u64, _>`; threaded through `Storage::list_*` functions
- [x] 6.10 Implement `Storage::mkdirs_for(physical)` to auto-create parent directories during writes (both regions)
- [x] 6.11 Add an in-process per-target advisory lock (e.g. `dashmap<PathBuf, Mutex<()>>`) covering concurrent writes from the same process; back it with `fs2::FileExt::try_lock_exclusive` on the target's parent if needed for cross-process safety
- [x] 6.12 Cover every `specs/vault-storage/spec.md` scenario with `assert_fs` integration tests, including the visibility-filter scenarios
- [ ] 6.13 Add a crash-safety test: simulate process exit between temp-write and rename (kill child process from parent) and assert the target file is unchanged

## 7. Error model

- [x] 7.1 Define a top-level `AgentmemError` enum in `error.rs` with `thiserror` variants for every error code referenced in the specs (`path_escapes_root`, `path_not_permitted`, `write_denied`, `missing_scope`, `not_found`, `edit_search_not_found`, `edit_search_ambiguous`, `invalid_argument`, `io`, `config`, `transport`)
- [x] 7.2 Implement `impl From<AgentmemError> for rmcp::ErrorData` (or the equivalent MCP tool-error type) producing both a human-readable `text` and a structured `code`
- [x] 7.3 Unit-test that no variant leaks a raw OS error string into the MCP-facing message

## 8. Tool schemas and handlers

- [ ] 8.1 Implement a `ToolSchemaBuilder` that takes the parsed `Template` and produces each tool's input schema by merging the template-derived scope fields with the tool-specific fields (path, content, search/replace, which, limit, cursor, etc.); use `schemars` for the tool-specific parts and JSON-merge the scope fragment
- [ ] 8.2 Implement `list_memory_notes` handler with pagination (`limit` default 200, max 1000; opaque `cursor`) covering the six scenarios in `specs/memory-tools/spec.md`
- [ ] 8.3 Implement `read_memory_note` handler
- [ ] 8.4 Implement `write_memory_note` handler (returns byte count)
- [ ] 8.5 Implement `edit_memory_note` handler with uniqueness-precondition checks (distinct error codes for not-found vs ambiguous)
- [ ] 8.6 Implement `delete_memory_note` handler honouring policy / region / own-scope and mapping missing target to `not_found`
- [ ] 8.7 Implement `load_session_context` handler reading all five foundational files (`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md`) under the agents folder for the active scope, returning each content (or null) plus a `missing` array
- [ ] 8.8 Implement `evolve_core_persona` handler whose input schema includes a required string-enum `which` (`persona`/`prompt`/`rules`/`user`/`tools`); resolves to the matching `.md` file under the agents folder for the active scope and performs an atomic write
- [ ] 8.9 Implement `update_task_heartbeat` handler hardcoded to `<agents_dir>/HEARTBEAT-STATE.md`
- [ ] 8.10 Implement `append_diary_entry` handler: derive `YYYY-MM-DD` and `HH:MM:SS` from `Utc::now().with_timezone(&config.tz)`, resolve virtual path `<agents_dir>/diary/<date>.md`, read current contents (treat missing as empty), append `\n## <HH:MM:SS>\n<content>\n` (omit the leading `\n` when the file was missing), and persist via the atomic-write procedure under the per-target lock
- [ ] 8.11 Snapshot-test the JSON schemas exposed by `tools/list` for several representative templates (empty, `<agent>`, `<agent>.<user>`, `<team>.<agent>.<env>.<user>`) with `insta`
- [ ] 8.12 Unit-test each handler against every scenario in `specs/memory-tools/spec.md` using an in-process MCP harness, including the concurrent-append serialisation test for `append_diary_entry`, the pagination consistency test, and the delete-unreachable-other-scope test

## 9. Stdio transport

- [ ] 9.1 Wire `rmcp` stdio transport in `transport::stdio::serve(config, server)`
- [ ] 9.2 Configure `tracing_subscriber` so its writer is `std::io::stderr()` only and assert (in a test) that nothing is written to stdout besides MCP frames
- [ ] 9.3 Handle `SIGTERM`/`SIGINT` via `tokio::signal::ctrl_c`, drain in-flight requests, then exit zero
- [ ] 9.4 Integration test: launch the binary as a child process with `assert_cmd`, perform `initialize` + `tools/list` + a round-trip `read_workspace_file`, assert clean shutdown and zero stdout pollution

## 10. HTTP transport (default)

- [ ] 10.1 Build an `axum::Router` mounting `POST /mcp` and `GET /mcp` to the `rmcp` Streamable HTTP transport, plus `GET /health`
- [ ] 10.2 Add a `tower` middleware that enforces `Authorization: Bearer <AGENTMEM_HTTP_BEARER>` when the env var is set, returning HTTP 401 otherwise
- [ ] 10.3 Emit a startup `WARN` log when `AGENTMEM_HTTP_BEARER` is unset
- [ ] 10.4 Default the bind address to `127.0.0.1:8000`; emit a startup `WARN` only when a non-loopback bind is configured without `AGENTMEM_HTTP_BEARER`
- [ ] 10.5 Integration test: spawn the server with `AGENTMEM_ROOT_DIR` as the only override, hit `GET /health` on `127.0.0.1:8000`, run an MCP `initialize` + `tools/list` over Streamable HTTP, assert success
- [ ] 10.6 Integration test: assert that an unauthenticated request returns 401 when `AGENTMEM_HTTP_BEARER` is set
- [ ] 10.7 Integration test: spawn the server with `AGENTMEM_HTTP_BIND=0.0.0.0:0` and no bearer; assert the startup `WARN` line is emitted

## 11. Binary plumbing

- [ ] 11.1 In `main.rs`, parse `Config`, install tracing, build the MCP server with all tools registered (template-driven schemas), dispatch to the selected transport, and await its termination
- [ ] 11.2 Add a `--print-config` CLI flag that prints the effective configuration to stderr and exits zero (useful for debugging)
- [ ] 11.3 Add a `--version` flag wired to `clap`'s built-in handling

## 12. Documentation

- [ ] 12.1 Update `README.md` with: install, full env var reference (`AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_TEMPLATE`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, `AGENTMEM_LOG`), worked layout examples for default config and vault-root-as-agents-folder, sample `claude_desktop_config.json` (stdio sidecar) and local `mcp.json` HTTP server entries, sample `curl` for HTTP transport, note that HTTP+`127.0.0.1:8000` is the default and explicitly document the expected workflow for humans to hand-edit `Agents/<scope>/...` files directly in Obsidian
- [ ] 12.2 Add a `docs/security.md` documenting the trust model: claimed scope keys are trusted in v1 (per-tenant auth deferred), traversal is not, own-scope strictness inside the agents folder is structural, the four policies and their guarantees outside the agents folder, the visibility filters and how to widen them

## 13. Release readiness

- [ ] 13.1 `cargo test` green across unit + integration tiers
- [ ] 13.2 `cargo clippy -- -D warnings` clean
- [ ] 13.3 Build static-ish release binaries for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` in CI on tag push
- [ ] 13.4 Publish a `0.1.0` tag once all tasks above are checked
