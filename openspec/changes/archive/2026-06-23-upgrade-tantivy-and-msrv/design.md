## Context

`tantivy` is an optional dependency behind the `recall-tantivy` feature (off by
default; `default = ["transport-http"]`). Its entire integration lives in
`src/recall/tantivy.rs` (~494 lines) behind the `BackendIndex` trait, with backend
selection in `src/recall/mod.rs` / `src/config.rs` / `src/tools.rs`. The current
pin is `tantivy = "0.22.0"`; upgrading to `0.26.1` is the only way to reach a
non-vulnerable `lru` (≥ 0.16.3), and 0.26.1 also requires `time ^0.3.47` (closing
the medium alert). Because the dep is feature-gated, Dependabot flags it from
`Cargo.lock` even though default builds never compile it.

The tantivy API surface this crate touches (the migration blast radius):

- Schema: `Schema::builder()`, `add_text_field`, field flags `STRING | STORED`,
  `TEXT`, `STORED`.
- Index lifecycle: `Index::create_in_ram`, `index.writer(heap)`,
  `reader_builder().reload_policy(ReloadPolicy::Manual).try_into()`.
- Write path: `IndexWriter::delete_term`, `Term::from_field_text`, `add_document`,
  `commit`; `IndexReader::reload`.
- Read path: `reader.searcher()`, `searcher.num_docs()`,
  `searcher.search(&query, &TopDocs::with_limit(n))`,
  `searcher.doc::<TantivyDocument>(address)`.
- Query: `QueryParser::for_index`, `parse_query`, `AllQuery`.
- Docs/values: `TantivyDocument`, `add_text`, `get_first`, `OwnedValue::Str`.
- Snippets: `SnippetGenerator::create`, `snippet_from_doc`, `.fragment()`.

## Goals / Non-Goals

**Goals:**
- Close both Dependabot alerts: `lru ≥ 0.16.3` and `time = 0.3.47` in `Cargo.lock`.
- Upgrade `tantivy 0.22.0 → 0.26.1` with **identical observable recall behavior**.
- Make the declared MSRV honest: `rust-version 1.85 → 1.95` (latest stable; ≥ the
  1.88 floor the new stack needs).

**Non-Goals:**
- No change to recall semantics, scoring, snippet shape, or filter behavior.
- No change to the `simple` backend or to default-feature builds.
- No new recall features or configuration surface.
- Not adopting tantivy's on-disk directory or any new tantivy capability.

## Decisions

- **Compile-driven migration, behavior pinned by tests.** Treat the existing
  `recall/tantivy.rs` unit tests (BM25 rank+snippet, eq/contains/numeric filters,
  text+filter compose, regex over candidates, path-only hits, remove-then-flush)
  plus the `recall-search` integration tests as the behavioral contract. Bump the
  pin, then resolve each compiler error at the API touchpoints above, changing the
  smallest surface that keeps every test green. Do not refactor opportunistically.
- **Single combined change.** The MSRV bump and the tantivy upgrade are coupled:
  tantivy 0.26.1 pulls `time 0.3.47`, which mandates Rust ≥ 1.88. Bumping MSRV
  without tantivy would understate intent; bumping tantivy without MSRV would
  leave the manifest dishonest. Ship them together.
- **MSRV target = 1.95 (latest stable), not the 1.88 floor.** The user asked for
  latest; CI already builds on `stable` (1.95), so 1.95 matches reality and avoids
  claiming support for toolchains CI never exercises. `rust-toolchain.toml` stays
  `channel = "stable"` (no pin needed).
- **No spec delta.** `recall-search` and `configuration` describe the tantivy
  backend behaviorally and pin no version; MSRV is not spec-governed. Preserved
  behavior keeps every requirement satisfied, so there are no requirement deltas.
- **Verify with the feature on.** Default builds exclude tantivy, so CI/local
  verification for this change must run `--features recall-tantivy` (fmt, clippy
  `--all-targets --all-features`, and the test suite with the feature) — otherwise
  the migrated code is never compiled.

## Risks / Trade-offs

- **Unknown breadth of API breakage.** Four minor releases (0.23–0.26) may have
  renamed/retyped any touchpoint above (e.g. snippet API, document/value types,
  searcher `doc` signature). Mitigation: the touchpoints are enumerated and small,
  fully covered by tests; migration is mechanical and bounded to one file.
- **Behavioral drift in BM25/snippets.** A tantivy scoring or tokenizer change
  could shift ranking or snippet fragments. Mitigation: tests assert ranking
  order, hit sets, and exact path-only snippets; any drift surfaces as a failure to
  triage explicitly rather than silently ship.
- **MSRV raised to 1.95.** Consumers building `recall-tantivy` on older toolchains
  must upgrade Rust. Accepted and documented; default-feature consumers unaffected.
- **Transitive churn.** The lockfile refresh may move other tantivy-only
  transitives. Mitigation: scope the diff to tantivy's subtree; re-run
  `cargo update -p <pkg> --precise` only where an alert or build error requires it.
