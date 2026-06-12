# Add Recall Benchmarks

## Why

Task 11.2 of the archived `2026-06-09-add-recall-search` change was deferred: the recall engine's startup build, query latency, and write-update cost have never been measured at scale, and the `max_resident_scopes` eviction bound has no test proving it actually caps resident indexes. Without these, regressions in index build time or memory residency would go unnoticed.

## What Changes

- Add a Criterion benchmark suite at `benches/recall.rs` driven by a synthetic large vault (10 000 notes across 10 scopes) covering: eager cold-start build (`RecallEngine::warm`), warm query latency on the `simple` backend (and `tantivy` when the `recall-tantivy` feature is enabled), and the synchronous own-write index update (`RecallEngine::on_write`).
- Add a public `RecallEngine::resident_scope_count()` accessor so tests and benchmarks can observe how many per-scope indexes are resident.
- Add an eviction-bound integration test: with `max_resident_scopes` smaller than the number of scopes, sequential cross-scope queries never leave more than the cap resident after a query completes.
- Wire `criterion` in as a pinned dev-dependency with a `[[bench]]` section in `Cargo.toml`.

## Capabilities

### New Capabilities

None — benchmarks and tests verify existing behavior; no new agent-facing capability.

### Modified Capabilities

- `recall-search`: the "In-memory index lifecycle" requirement is strengthened from "idle per-scope indexes MAY be evicted under a configured memory bound" to a verifiable invariant — after a recall completes, the number of resident per-scope indexes SHALL NOT exceed `max_resident_scopes`, and the engine SHALL expose the resident count for verification.

## Impact

- `Cargo.toml`: new dev-dependency `criterion` (pinned) and a `[[bench]]` entry; no runtime dependencies change.
- `src/recall/mod.rs`: one new public read-only method (`resident_scope_count`); no behavior change.
- New files: `benches/recall.rs`, `tests/recall_eviction.rs`.
- CI/dev workflow: `cargo bench --bench recall` is informational (no hard timing asserts); the eviction test runs under `cargo test` and is a hard gate.
