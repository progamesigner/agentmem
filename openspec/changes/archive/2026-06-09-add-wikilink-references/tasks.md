## 1. Suffix primitives (path.rs)

- [x] 1.1 Expose a link-target variant of `apply_suffix_to_filename` operating on a basename (no extension), reusing the existing exact rendered-scope match
- [x] 1.2 Expose a link-target variant of `strip_scope_from_filename` for basenames
- [x] 1.3 Add unit tests covering basename suffix/strip round-trips, including the `x.coder.alice` collision case

## 2. Visible-set resolution index (storage.rs)

- [x] 2.1 Add a method that builds a clean-name index (basename → set of clean paths) for a scope across visible regions, reusing `walk_files` / `list_visible`
- [x] 2.2 Ensure excluded (hidden/ignored) notes are absent from the index
- [x] 2.3 Unit-test the index: own-scope + shared membership, exclusion of other scopes and ignored files

## 3. Resolution + shortest-name (wikilink.rs)

- [x] 3.1 Create `src/wikilink.rs` with the module skeleton and wire it into `lib.rs`
- [x] 3.2 Implement target resolution: exact clean-path → unique basename → shortest unambiguous path; return the resolved clean path + region, or `None` for dangling
- [x] 3.3 Implement shortest-unambiguous-name rendering used by both read and write
- [x] 3.4 Unit-test resolution: unique basename, basename collision → qualified path, dangling, target in another scope

## 4. Link parsing for all forms

- [x] 4.1 Parse `[[target]]`, `[[target|alias]]`, `[[target#heading]]`, and `![[target]]`, isolating the target while preserving alias/heading/embed-prefix
- [x] 4.2 Parse relative markdown links `[text](path.md)`; skip external (`http(s)://`) and anchor-only (`#section`) targets
- [x] 4.3 Unit-test the parser on mixed content, escaping, and the skip cases

## 5. Transform functions (wikilink.rs)

- [x] 5.1 Implement `strip_links(content, scope)` — strip the caller's own suffix from every link target and render shortest names
- [x] 5.2 Implement `expand_links(content, scope, region, &index)` — rewrite own-scope→own-scope targets with the suffix, leave shared targets clean, leave danglers verbatim
- [x] 5.3 Implement the leak guard: a shared-region file linking to an own-scope target returns a `write_denied`-class error naming the target; no bytes written
- [x] 5.4 Implement the markdown two-part transform (scope directory + stem suffix) per design D5
- [x] 5.5 Property test: `strip_links(expand_links(x)) == normalize(x)` for own-scope content across all link forms

## 6. Tool wiring (tools.rs)

- [x] 6.1 Apply `strip_links` to `read_memory_note` output
- [x] 6.2 Apply `expand_links` in `write_memory_note` and `append_diary_entry` before persisting
- [x] 6.3 Apply the write-transform to `edit_memory_note`'s `search_string` and `replace_string`
- [x] 6.4 Surface the leak-guard rejection through the existing error boundary with a clear message
- [x] 6.5 Add integration tests per the spec scenarios (read strip, write expand, edit match, shared→scoped reject, dangling preserved)

## 7. Docs

- [x] 7.1 Add a cross-scope link-leakage row and the shared→scoped rejection rule to `docs/security.md`
- [x] 7.2 Document the `[[wikilink]]` and markdown-link behavior in the user-facing tool guidance / README

## 8. Verification

- [x] 8.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`
- [ ] 8.2 Manually verify in an Obsidian vault that a persisted own-scope link resolves for a human browser

## 9. Core-file coverage

- [x] 9.1 Expand link targets in `evolve_core_persona` (PERSONA/PROMPT/RULES/USER/MEMORY) and `update_task_heartbeat` writes
- [x] 9.2 Strip the caller's own suffix from foundational files rendered by `load_session_context` / `render_session_context`
- [x] 9.3 Integration-test the MEMORY.md and HEARTBEAT.md round-trips through the session context and read path
