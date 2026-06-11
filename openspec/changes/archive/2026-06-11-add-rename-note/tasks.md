## 1. Error code

- [x] 1.1 Add a `DestinationExists { virtual_path }` variant to `src/error.rs` with code `destination_exists`; cover it in the error-code unit tests.

## 2. Wikilink retarget helper

- [x] 2.1 Add `retarget_links(content, source_clean_path, destination_entry, rendered_scope, resolver, index) -> Result<(String, usize)>` in `src/wikilink.rs`: via `rewrite_links`, replace only targets whose resolution selects the source, deriving the new target text with the existing shortest-name/suffix/physical-path logic against a post-rename index; return the rewritten content and replacement count.
- [x] 2.2 Unit tests: decorations preserved; non-matching same-basename links untouched; shortest-name re-derivation; markdown physical-path rewrite; shared-referrer-to-scoped-destination yields the leak-guard error.

## 3. Tool schema and registration

- [x] 3.1 Add `RenameFields { path, new_path }` schemars struct, register `rename_memory_note` in `TOOL_NAMES`/`build_tools`, and dispatch it in `Toolbox::call`.

## 4. Handler

- [x] 4.1 Phase 1 (validate, no writes): resolve + visibility-check source; reject root-reserved paths on both ends; policy-gate both regions; resolve destination and reject `destination_exists`; strip source content and re-expand for the destination region (self-references re-pointed; leak guard surfaces here); scan referrers via the backlink helper; verify every referrer's region is writable; compute every rewritten referrer content via `retarget_links`.
- [x] 4.2 Phase 2 (mutate): `write_atomic` the destination, write each rewritten referrer, `delete` the source last; call `recall_on_write` for destination, every rewritten referrer, and the deleted source path.
- [x] 4.3 Return `{ renamed: true, path, new_path, notes_rewritten }`.

## 5. Integration tests

- [x] 5.1 `tests/tools.rs`: happy-path rename with wikilink + markdown referrers rewritten and round-tripping on read; self-reference case; `notes_rewritten` count.
- [x] 5.2 Guard tests: `destination_exists`, root-reserved on either end, `namespaced` policy denial outside the agents folder, missing source `not_found`, shared→scoped refusal with shared referrers, leak guard on moved content (scoped→shared with own-scope links).
- [x] 5.3 Recall integration: post-rename recall hits the new path only, with no watcher delay.
- [x] 5.4 Update schema snapshots (`tests/schema_snapshots.rs`) and the README tool table.

## 6. Verification

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
