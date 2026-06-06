## 1. Configuration

- [ ] 1.1 Add `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` env var constant and a `session_context_template_file: PathBuf` field to `Config` in `src/config.rs`, defaulting to `<root>/AGENT_SESSION_CONTEXT.md` (relative values resolved against the vault root)
- [ ] 1.2 Wire the new var into `Config::from_env`/`build`, the `Cli` override (`--session-context-template-file`) and `as_overrides`, and the `describe()`/`--print-config` output
- [ ] 1.3 Add a config test asserting the default path and a custom-path override

## 2. Generic template type (`src/template.rs`)

- [ ] 2.1 Create `src/template.rs` defining `Template`: parse a source string into literal and `{{key}}` placeholder segments, where `key` is a dotted identifier (e.g. `files.persona`, `scope.agent`, `tools_guide`)
- [ ] 2.2 Implement `render(context: &Map<String, String>) -> Rendered`: substitute recognised keys, leave unknown `{{…}}` tokens verbatim, and return the encountered unknown tokens so the caller can log them once. `Template` knows nothing about files/scope/missing
- [ ] 2.3 Register the module in `src/lib.rs` (`pub mod template;`)
- [ ] 2.4 Unit tests: literal/placeholder parsing, key substitution, unknown-token passthrough + reporting

## 3. Session-context renderer (`src/session_context.rs`)

- [ ] 3.1 Create `src/session_context.rs`; define the namespaced key set (`files.<name>`, `scope.<key>`, `tools_guide`) and the missing-sentinel constant
- [ ] 3.2 Embed the compiled-in default template (interleaved foundational sections + a `{{tools_guide}}` slot) as a `const`/`include_str!`
- [ ] 3.3 Implement layered template resolution: per-scope `AGENT_SESSION_CONTEXT.md` (via the scope suffix mechanism) → global `session_context_template_file` → compiled-in default; absence at any layer is non-fatal
- [ ] 3.4 Build the context map: read the five foundational files (reuse `FOUNDATIONAL`), substituting the sentinel and recording the `missing` list for absent ones; add `scope.<key>` values from the scheme; add `tools_guide`
- [ ] 3.5 Implement `tools_guide` generation from the live tool catalogue (names + one-line usage)
- [ ] 3.6 Implement `render_session_context(scope) -> { rendered, missing }` tying resolution + context assembly + `Template::render` together
- [ ] 3.7 Register the module in `src/lib.rs`
- [ ] 3.8 Unit tests: sentinel for missing files, file-vs-scope namespace distinction, the three resolution layers, and `tools_guide` reflecting the catalogue

## 4. Tool surface

- [ ] 4.1 Rewrite `Toolbox::load_session_context` to call the shared renderer and return `{ rendered, missing }` (drop the five raw fields)
- [ ] 4.2 Confirm the tool's input schema still accepts only scope parameters (no `path`/`which`)

## 5. Resource surface

- [ ] 5.1 Enable the resources capability in `get_info` (`ServerCapabilities`) in `src/mcp.rs`
- [ ] 5.2 Implement `list_resource_templates` emitting `agentmem://session-context/{…}` with URI params derived from the scheme's placeholders (in order)
- [ ] 5.3 Implement `read_resource`: parse scope from the URI path segments, call the renderer, return `rendered` as resource contents; empty-vault scopes succeed (no not-found)

## 6. Prompt surface

- [ ] 6.1 Enable the prompts capability in `get_info`
- [ ] 6.2 Implement `list_prompts` declaring `session-context` with required string arguments derived from the scheme's placeholders
- [ ] 6.3 Implement `get_prompt`: validate required args, call the renderer, return `rendered` as the message content; missing required arg returns a naming error

## 7. Tests, snapshots, docs

- [ ] 7.1 Update `tests/tools.rs` for the new `load_session_context` return shape
- [ ] 7.2 Update/regenerate the schema snapshots referencing `load_session_context`
- [ ] 7.3 Add integration coverage for `resources/templates/list` + `resources/read` and `prompts/list` + `prompts/get`, including the empty-vault and scheme-variation cases
- [ ] 7.4 Update `README.md`: new env var in the config table and the three session-context surfaces
- [ ] 7.5 Run `cargo test` and `cargo clippy`; ensure `openspec validate configurable-session-context` passes
