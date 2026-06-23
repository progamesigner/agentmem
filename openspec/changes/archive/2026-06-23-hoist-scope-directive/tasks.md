# Tasks

## 1. Renderer

- [x] 1.1 Extract the `key=value` scope-join into a shared helper (e.g. `scope_keys_csv(scope) -> Option<String>`, `None` for an empty scope) in `src/session_context.rs`, and route `tools_guide` through it.
- [x] 1.2 Add a `scope_directive(scope)` builder that returns the prominent banner line: names the keys via the shared helper for a non-empty scope, generic phrasing for an empty scope.
- [x] 1.3 Insert `{{scope_directive}}` into the renderer context map in `render_session_context`.

## 2. Default template

- [x] 2.1 Add `{{scope_directive}}` as a bare banner directly under the `# Session Context` H1, above `<PERSONA>`, in `DEFAULT_TEMPLATE`. Leave section order and all other content unchanged.

## 3. Tests

- [x] 3.1 Renderer test: non-empty scope → `{{scope_directive}}` names the keys as `key=value` in deterministic order.
- [x] 3.2 Renderer test: empty scope → `{{scope_directive}}` uses generic phrasing, names no specific key.
- [x] 3.3 Default-template test: the rendered banner appears before `<PERSONA>` (assert byte index of the directive precedes `<PERSONA>`) and is bare (not wrapped in a tag).
- [x] 3.4 Update existing template-content tests for the new leading banner. (No change needed: existing assertions use `.contains(...)` and none assert the head region's exact layout; verified by the full suite passing.)

## 4. Verification

- [x] 4.1 `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`. (fmt clean, clippy clean, 158+9 tests pass + 4 new.)
- [x] 4.2 `openspec validate hoist-scope-directive --strict`. (valid)
