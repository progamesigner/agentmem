# Tasks: expand-frontmatter-property-links

## 1. Value walker

- [x] 1.1 Add recursive expand/strip walkers over `serde_json::Value` string leaves (reusing `rewrite_links` semantics; keys and non-strings untouched) beside `src/wikilink.rs`, with unit tests for nesting, arrays, dangling targets, and the leak-guard error path

## 2. Tool handlers

- [x] 2.1 `update_note_properties`: expand the supplied property values (leak guard included) before `frontmatter::merge` inside the locked read-modify-write; strip the merged result before returning `{ properties }`
- [x] 2.2 `read_note_properties`: strip the parsed property tree before returning; update the handler comment that previously justified the raw read
- [x] 2.3 Update both tool descriptions to state the clean-form contract; refresh `tests/schema_snapshots.rs` if descriptions are snapshotted

## 3. Tests

- [x] 3.1 Round-trip: set `related: "[[rust]]"` via the property tool, assert suffixed on disk, clean via both `read_note_properties` and `read_memory_note`
- [x] 3.2 Leak fix: a note written via `write_memory_note` with a frontmatter link no longer shows the suffix through `read_note_properties`
- [x] 3.3 Guard and edges: shared-region property linking own scope refused with file unchanged; shared targets stay clean; dangling and non-string values verbatim; nested arrays/objects transformed
- [x] 3.4 Backlinks: a property-only link counts toward the target's `backlinks` array

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; fix anything they surface
