# Tasks: Add Recall Benchmarks

## 1. Engine observability

- [x] 1.1 Add `pub fn resident_scope_count(&self) -> usize` to `RecallEngine` in `src/recall/mod.rs`, returning `state.scopes.len()` under the state lock, with a doc comment noting it backs the eviction-bound tests and benches
- [x] 1.2 Add a unit test in `src/recall/mod.rs` covering the accessor: 0 before `warm()`, equals the scope count after

## 2. Eviction-bound integration test

- [x] 2.1 Create `tests/recall_eviction.rs`: build a 10-scope vault (a few notes per scope) with `assert_fs::TempDir`, scheme `<agent>.<user>`, agents dir `Agents`, mirroring the fixture in `src/recall/mod.rs` tests
- [x] 2.2 Construct a `RecallEngine` with `max_resident_scopes = 3` and a large `freshness`; `warm()`, then issue a `recall()` for each scope in turn and assert `resident_scope_count() <= 3` after every query
- [x] 2.3 Run the test under both `cargo test` and `cargo test --all-features` to confirm it passes on both backends' builds

## 3. Criterion wiring

- [x] 3.1 Add `criterion = { version = "0.5.1", features = ["html_reports"] }` to `[dev-dependencies]` and a `[[bench]] name = "recall", harness = false` section in `Cargo.toml`; refresh `Cargo.lock` with `cargo check`
- [x] 3.2 Create `benches/recall.rs` with the synthetic vault generator: `(notes, scopes) = (10_000, 10)`, deterministic lorem-like bodies seeding the keyword `borrow` in a fixed subset, plus a helper that builds a `RecallEngine` over the vault for a given backend with large `freshness`

## 4. Benchmark groups

- [x] 4.1 `recall/cold_start/10k_notes`: `iter_batched` constructing a fresh engine in setup and timing `warm()`, with `sample_size(10)`
- [x] 4.2 `recall/warm_query/simple`: engine warmed once, time `recall()` with a text query for the seeded keyword against one scope with both regions
- [x] 4.3 `recall/warm_query/tantivy` under `#[cfg(feature = "recall-tantivy")]`: same query against a tantivy-backed engine
- [x] 4.4 `recall/own_write_update`: engine warmed once, time `on_write()` for a single resolved note path

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --all-targets --all-features -- -D warnings` — bench code must be warning-free under both feature sets
- [x] 5.2 Run `cargo test` and `cargo test --all-features`; confirm the eviction test passes in both
- [x] 5.3 Run `cargo bench --bench recall` (and with `--features recall-tantivy`); record cold-start, warm-query, and own-write numbers against the informational targets (cold start < 30 s; warm query p50 < 50 ms simple, < 10 ms tantivy) for the PR description
