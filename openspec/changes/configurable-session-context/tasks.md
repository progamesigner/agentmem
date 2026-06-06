## 1. Configuration

- [ ] 1.1 Add `AGENTMEM_SESSION_CONTEXT_FILE` env var constant and a `session_context_file: PathBuf` field to `Config` in `src/config.rs`, defaulting to `<root>/AGENT_SESSION_CONTEXT.md` (relative values resolved against the vault root)
- [ ] 1.2 Wire the new var into `Config::from_env`, the CLI override struct, and `--print-config` output
- [ ] 1.3 Add a config test asserting the default path and a custom-path override

## 2. Session-context layout module

- [ ] 2.1 Create `src/session_context.rs` defining the namespaced placeholder set (`{{files.<name>}}`, `{{scope.<key>}}`, `{{tools_guide}}`) and a substitution function that leaves unknown tokens literal (logging once)
- [ ] 2.2 Embed the compiled-in default layout (interleaved foundational sections + a `{{tools_guide}}` slot) as a `const`/`include_str!`
- [ ] 2.3 Implement layered layout resolution: per-scope `AGENT_SESSION_CONTEXT.md` (via the scope suffix mechanism) → global `session_context_file` → compiled-in default; absence at any layer is non-fatal
- [ ] 2.4 Implement the missing-foundational-file sentinel substitution and accumulation of the `missing` list
- [ ] 2.5 Implement `tools_guide` generation from the live tool catalogue (names + one-line usage)
- [ ] 2.6 Implement `render_session_context(scope) -> { rendered, missing }` tying the above together
- [ ] 2.7 Register the module in `src/lib.rs`
- [ ] 2.8 Unit tests: placeholder substitution, unknown-token passthrough, sentinel for missing files, file-vs-scope namespace distinction, and the three resolution layers

## 3. Tool surface

- [ ] 3.1 Rewrite `Toolbox::load_session_context` to call the shared renderer and return `{ rendered, missing }` (drop the five raw fields)
- [ ] 3.2 Confirm the tool's input schema still accepts only scope parameters (no `path`/`which`)

## 4. Resource template surface

- [ ] 4.1 Enable the resources capability in `get_info` (`ServerCapabilities`) in `src/mcp.rs`
- [ ] 4.2 Implement `list_resource_templates` emitting `agentmem://session-context/{…}` with URI params derived from the VFS template placeholders (in order)
- [ ] 4.3 Implement `read_resource`: parse scope from the URI path segments, call the renderer, return `rendered` as resource contents; empty-vault scopes succeed (no not-found)

## 5. Prompt surface

- [ ] 5.1 Enable the prompts capability in `get_info`
- [ ] 5.2 Implement `list_prompts` declaring `session-context` with required string arguments derived from the VFS template placeholders
- [ ] 5.3 Implement `get_prompt`: validate required args, call the renderer, return `rendered` as the message content; missing required arg returns a naming error

## 6. Tests, snapshots, docs

- [ ] 6.1 Update `tests/tools.rs` for the new `load_session_context` return shape
- [ ] 6.2 Update/regenerate the schema snapshots referencing `load_session_context`
- [ ] 6.3 Add integration coverage for `resources/templates/list` + `resources/read` and `prompts/list` + `prompts/get`, including the empty-vault and VFS-template-variation cases
- [ ] 6.4 Update `README.md`: new env var in the config table and the three session-context surfaces
- [ ] 6.5 Run `cargo test` and `cargo clippy`; ensure `openspec validate configurable-session-context` passes
