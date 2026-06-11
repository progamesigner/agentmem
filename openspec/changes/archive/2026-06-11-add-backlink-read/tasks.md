## 1. Wikilink reverse-resolution helper

- [x] 1.1 Make `resolve_target` in `src/wikilink.rs` `pub(crate)` and add a `references_to(content, target_clean_path, rendered_scope, resolver, index) -> bool` helper that collects link targets via `rewrite_links`, strips the caller's suffix per target (wikilink and markdown forms), resolves each with `resolve_target`, and reports whether any resolution lands on the target's clean path.
- [x] 1.2 Unit tests in `src/wikilink.rs`: suffixed on-disk forms, all link forms (alias/heading/embed/markdown), ambiguous-basename tie-break agreement with forward resolution, dangling links resolve to nothing.

## 2. Tool schema

- [x] 2.1 Add a `ReadFields { path, backlinks: Option<bool> }` schemars struct in `src/tools.rs` and use it for `read_memory_note`'s input schema (delete keeps `PathFields`); describe the `backlinks` argument and result field.

## 3. Handler

- [x] 3.1 In `read_memory_note`, accept the optional `backlinks` argument (include it in the `resolve_scope` tool-fields list); on `true`, after the existing read succeeds, build the `LinkIndex` over `policy.list_visible_regions`, list the visible set, read each note's on-disk content (skipping unreadable/non-UTF-8 files), and collect referrers via `references_to`.
- [x] 3.2 Deduplicate, sort ascending, and attach `backlinks` to the structured result only when requested; absent/false leaves the response byte-identical to today.

## 4. Integration tests

- [x] 4.1 `tests/tools.rs`: backlinks returned for wikilink and markdown referrers; one entry per referring note; deterministic ordering.
- [x] 4.2 Cross-scope isolation: another scope's note linking to a shared target never appears; `scoped` policy excludes shared-region referrers.
- [x] 4.3 Default behavior unchanged: no `backlinks` field without the flag (guard with a schema snapshot update if needed).

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
