## 1. Renderer core: kinds, onboarding directive, drop tools guide

- [x] 1.1 Add a render-kind enum (`Context` | `Bootstrap`) to `src/session_context.rs` and thread it into `render_session_context`.
- [x] 1.2 Remove `tools_guide()` and the `{{tools_guide}}` context key; drop the now-dead `tools: &[Tool]` parameter from `render_session_context` and update all callers (`src/tools.rs`, `src/mcp.rs` wrapper, `src/transport/http.rs`).
- [x] 1.3 Compute `{{onboarding_directive}}`: empty string when `missing` is empty, else the interview/`evolve_core_persona` directive (lifted from the old layout bootstrapping prose); insert into the context map.
- [x] 1.4 Split the compiled-in default into `DEFAULT_CONTEXT` (today's default minus `<AGENTMEM:TOOLS>` and `<AGENTMEM:LAYOUT>`, plus `{{onboarding_directive}}` and a one-line layout pointer) and `DEFAULT_BOOTSTRAP` (scope banner + `<PERSONA>`/`<RULES>` + `{{onboarding_directive}}` + pointers to `load_session_context` and the layout surface).
- [x] 1.5 Generalize `resolve_template_source` to take the kind and select filename + env path + default per kind (`AGENT_SESSION_CONTEXT` / `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` vs `AGENT_SESSION_BOOTSTRAP` / `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE`).

## 2. Layout renderer

- [x] 2.1 Add `DEFAULT_LAYOUT` constant in `src/session_context.rs` (the prose lifted from the old `<AGENTMEM:LAYOUT>` section, excluding the missing-files onboarding paragraph).
- [x] 2.2 Add `render_layout(storage, …, scope)` that resolves `AGENT_MEMORY_LAYOUT[.scope].md` → `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` → `DEFAULT_LAYOUT` and renders through the `Template` engine with the scope context.

## 3. Configuration

- [x] 3.1 Add `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` (default `<root>/AGENT_SESSION_BOOTSTRAP.md`) and `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` (default `<root>/AGENT_MEMORY_LAYOUT.md`) to the config struct, env parsing, and defaults.
- [x] 3.2 Plumb both new paths through to the renderer/layout call sites.

## 4. HTTP endpoints

- [x] 4.1 Add `GET /v1/bootstrap` to the `axum` router, rendering the `Bootstrap` kind; reuse the `context` handler's scope binding, response negotiation, auth gate, and error mapping.
- [x] 4.2 Add `GET /v1/layout` to the router, calling `render_layout`; reuse the same binding/negotiation/auth/error behavior.
- [x] 4.3 Confirm `GET /v1/context` handler code is unchanged (only its rendered content shrinks).

## 5. MCP resources

- [x] 5.1 Register `session-bootstrap` (`agentmem://session-bootstrap/{…}`) and `session-layout` (`agentmem://session-layout/{…}`) in `list_resource_templates`; fix the stale "memory-tools guide" description on the existing `session-context` resource.
- [x] 5.2 Branch `read_resource` by URI prefix to dispatch to the bootstrap render, the layout render, or the existing context render; generalize scope-from-URI parsing across the three prefixes.
- [x] 5.3 Extend the scoped-token gate so `session-bootstrap`/`session-layout` reads and `GET /v1/bootstrap`/`GET /v1/layout` are authorized against the token grant exactly like the context surfaces.

## 6. Tests

- [x] 6.1 Update `src/session_context.rs` unit tests: drop the `tools_guide_*` and `<AGENTMEM:TOOLS>`/`<AGENTMEM:LAYOUT>` assertions; add tests for the two kinds, `{{onboarding_directive}}` gating, the layout renderer, and the layered resolution for the new template files.
- [ ] 6.2 Update/add HTTP transport tests for `GET /v1/bootstrap` and `GET /v1/layout` (render, absent-files-ok, scope binding, negotiation, auth/scoped-token gating); assert `GET /v1/context` output no longer contains the tools guide or layout prose.
- [ ] 6.3 Add MCP tests for the two new resources (templates list, read, empty-vault, scoped-token gating).
- [ ] 6.4 Add config tests for the two new env vars (defaults, custom paths, absent-file fallback).

## 7. Docs and verification

- [ ] 7.1 Update `docs/session-context-hooks.md`: recommend `GET /v1/bootstrap` as the SessionStart hook target, document `GET /v1/layout` and the `session-bootstrap`/`session-layout` resources, and note the `{{tools_guide}}` removal.
- [ ] 7.2 Add a changelog entry calling out the breaking `{{tools_guide}}` removal and the shrunken full-render content (tools guide + layout no longer inline).
- [ ] 7.3 Measure the lean bootstrap payload against a real SessionStart inline budget; if it exceeds the cap, document the limitation and the operator lever (trim `PERSONA.md`/`RULES.md`).
- [ ] 7.4 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` before committing.
