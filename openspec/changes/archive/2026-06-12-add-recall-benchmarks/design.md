# Design: Add Recall Benchmarks

## Context

The recall engine (`src/recall/mod.rs`) builds per-scope in-memory indexes eagerly at startup (`RecallEngine::warm`), updates them synchronously on the server's own writes (`RecallEngine::on_write`), and evicts the least-recently-accessed scope indexes past `RecallConfig::max_resident_scopes` (`evict_if_needed`, called after each query). None of this has ever been measured at scale, and the eviction bound has no test. The deferred task 11.2 of `2026-06-09-add-recall-search` (planned in `bench.md`) calls for Criterion benchmarks over a synthetic ~10 000-note vault plus a hard RAM-bound assertion.

Note the plan in `bench.md` predates the final API: the build entry point is `warm()` (not `build_all()`) and the engine-level update hook is `on_write()` (`recall_on_write` is the `tools.rs` wrapper). `resident_scope_count()` does not exist yet and must be added.

## Goals / Non-Goals

**Goals:**

- Measure cold-start build, warm query (both backends), and own-write update latency with Criterion, reproducibly, over a deterministic synthetic vault.
- Prove the eviction invariant: after a recall completes, resident per-scope indexes never exceed `max_resident_scopes`.
- Keep CI green without slowing it down: benches compile under `clippy --all-targets` but only the eviction test runs under `cargo test`.

**Non-Goals:**

- No hard timing assertions in CI — bench numbers are informational (the `bench.md` targets: cold start < 30 s, warm query p50 < 50 ms simple / < 10 ms tantivy, are recorded, not asserted).
- No benchmarking of the real filesystem watcher (`notify`) — debounce and OS event delivery dominate and are not our code; the synchronous `on_write` path is what we own and measure.
- No changes to eviction policy or index behavior.

## Decisions

**Criterion `0.5.1` pinned, `harness = false`.** Standard stable-Rust benchmarking with statistical sampling; matches the `bench.md` plan. Dev-dependency only, so the runtime dependency tree is untouched. `[[bench]] name = "recall"` with `harness = false` as Criterion requires.

**Reuse `assert_fs` for the synthetic vault instead of adding `tempfile`.** `assert_fs::TempDir` is already a dev-dependency and is what the unit tests in `src/recall/mod.rs` use; adding `tempfile` directly would be a redundant new dependency.

**Vault shape mirrors the unit-test fixture.** Scheme `<agent>.<user>`, agents dir `Agents`, notes at `Agents/<scope>/topics/note-<i>.<scope>.md` with a short lorem-like body embedding a known keyword (e.g. `borrow`) in a deterministic subset so query benches have predictable hit counts. Default parameters `(notes, scopes) = (10_000, 10)`, generated once per benchmark group outside the timing loop.

**Bench scenarios:**

- `recall/cold_start/10k_notes` — `iter_batched`: setup constructs a fresh `RecallEngine` over the pre-generated vault (cheap), routine calls `warm()` (the eager build). `sample_size(10)` because each iteration reads 10 000 files.
- `recall/warm_query/simple` and (under `cfg(feature = "recall-tantivy")`) `recall/warm_query/tantivy` — engine built and warmed once; routine runs `engine.recall(scope, BOTH_REGIONS, &query)` with a text query matching the seeded keyword. Default sample size.
- `recall/own_write_update` — engine warmed once; routine calls `engine.on_write(scope, Region::InsideAgentsFolder, &physical)` for one resolved note path. `on_write` re-reads the file and upserts it, which is exactly the cost of the server's synchronous post-write index update.

**Set `freshness` high (e.g. 1 hour) in bench configs.** The unit tests use `freshness: 0`, which forces a full stat-diff reconcile on every query — over 10 000 files that would measure `reconcile`, not query latency. A large freshness window isolates the backend scan. `watch_debounce` is irrelevant (the watcher is never started in benches or the eviction test).

**`resident_scope_count()` as a thin public accessor.** `self.state.lock().scopes.len()` — read-only, no behavior change. It locks the same mutex the query path uses, which is fine for tests and benches (called between queries, not inside the timed routine).

**Eviction test as an integration test (`tests/recall_eviction.rs`).** Small vault (10 scopes × a handful of notes — eviction correctness does not need 10 000 files, keeping `cargo test` fast), `max_resident_scopes = 3`. Query each scope in turn through the public `recall()` API and assert `resident_scope_count() <= 3` after every query. Runs under both `cargo test` and `cargo test --all-features` in the existing CI matrix — no workflow changes needed.

**No CI workflow changes.** `clippy --all-targets` already compiles bench targets (so the bench code is gated for warnings under both feature sets), and `cargo test` picks up the new integration test automatically. `cargo bench --bench recall` stays a local/dev command, documented in the tasks; recording results in the PR description satisfies the informational acceptance criteria.

## Risks / Trade-offs

- [Cold-start bench is slow (~10 iterations × 10 000 file reads)] → `sample_size(10)` keeps a full `cargo bench` run in the low minutes; the vault is generated once and reused across iterations since `warm()` never mutates the vault.
- [Timing acceptance criteria are hardware-dependent] → Recorded by Criterion as informational numbers, never asserted; the only hard assert is the structural eviction invariant, which is hardware-independent.
- [Transient over-cap residency: `evict_if_needed` runs after the query, so the count can exceed the cap momentarily mid-query] → The invariant is specified and asserted at query completion ("after a recall completes"), matching the implementation's intent; the spec delta words it that way.
- [`html_reports` feature pulls plotters into the dev tree] → Dev-dependencies only; release builds and the container image are unaffected.
