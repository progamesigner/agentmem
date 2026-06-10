## 1. Simple backend

- [x] 1.1 In `src/recall/simple.rs`, make `score_doc` count `query.substring` and `query.regex` matches across the clean path as well as the body, summing into the existing match-count score (equal weight).
- [x] 1.2 Make the clean path available to scoring/snippet assembly (it is already the `docs` key) without re-reading files.
- [x] 1.3 In `snippets_for`, when the path matches but no body line does, emit the clean path as the single snippet.

## 2. Tantivy backend

- [x] 2.1 In `src/recall/tantivy.rs`, add an indexed `path` field to the schema and populate it on `upsert`.
- [x] 2.2 Include the `path` field in `query`/`regex` matching so path matches surface as hits, approximating equal weight with the body field.
- [x] 2.3 Ensure a path-only match yields the path as a snippet, consistent with the simple backend.

## 3. Snippet suffix-stripping

- [x] 3.1 Confirm path-derived snippets pass through the same own-scope link-suffix stripping as body snippets in `src/recall/mod.rs` (no foreign suffix leakage).

## 4. Tests

- [x] 4.1 simple backend: `regex`/`query` matching a path (e.g. a date) but not the body returns the note, with the path as a snippet.
- [x] 4.2 simple backend: a path match and a body match contribute equally to the raw score.
- [x] 4.3 simple backend: path matching does not break existing body-only matching or isolation.
- [x] 4.4 tantivy backend (feature-gated): path matching returns path-only hits end to end.
- [x] 4.5 Structural isolation still holds: a path match never returns another scope's note.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`.
- [x] 5.2 Run the tantivy-feature tests: `cargo test --features recall-tantivy`.
