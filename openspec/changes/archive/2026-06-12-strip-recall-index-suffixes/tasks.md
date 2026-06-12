# Tasks: strip-recall-index-suffixes

## 1. Ingestion strip

- [x] 1.1 Locate the single content-read funnel in `src/recall/mod.rs` (reconcile/upsert + `on_write`) and apply `wikilink::strip_links` with the index's `IndexRegion::Scoped` scope before handing content to the backend; `Shared` ingests verbatim
- [x] 1.2 Verify all ingestion paths (warm build, watcher reconcile, stat-diff backstop, eviction rebuild, synchronous own-write) flow through the stripped read; consolidate if any path reads independently

## 2. Query-time cleanup

- [x] 2.1 Remove the now-redundant snippet strip at query time (`recall/mod.rs` hit mapping) and update its comment trail

## 3. Tests

- [x] 3.1 Regex and substring tests on both backends: `\[\[rust\]\]`-style patterns match suffixed stored notes; querying a scope ident that occurs only in link suffixes returns no hits
- [x] 3.2 Tantivy filter test: `eq` on a link-valued property matches the clean form after the note was persisted suffixed via `write_memory_note`
- [x] 3.3 Ingestion-parity test: the same note indexed via warm build, own-write hook, and post-eviction rebuild yields identical indexed/stored content
- [x] 3.4 Snippet and shared-region tests: snippets remain clean (no foreign or own suffix); shared notes match exactly as stored

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; fix anything they surface
