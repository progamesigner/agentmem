## Why

Two Dependabot alerts target transitive crates pulled in **only** by the opt-in
`recall-tantivy` feature (`tantivy = "0.22.0"`):

- `time` (GHSA-r6v5-fh4h-64xc, medium) — stack-exhaustion DoS; fixed in `0.3.47`.
- `lru` (GHSA-rhfx-m35p-ff5j, low) — `IterMut` violates Stacked Borrows; fixed in
  `0.16.3`, **not** backported to the `0.12.x` line `tantivy 0.22.1` pins.

The `time` alert is already resolved in the lockfile (`time 0.3.45 → 0.3.47`), but
`time 0.3.47` requires Rust 1.88, while `Cargo.toml` still declares
`rust-version = "1.85"` — the manifest now understates the true MSRV. The `lru`
alert cannot be closed within `tantivy 0.22.1`'s `lru = "^0.12.0"` constraint; the
first tantivy release requiring `lru ^0.16.3` is **0.26.1** (which also requires
`time ^0.3.47`, subsuming the time fix).

So both alerts close by upgrading tantivy `0.22.1 → 0.26.1` and bringing the
declared MSRV up to a supported toolchain. CI builds on `stable` (currently
1.95), so aligning the manifest to the latest stable removes the discrepancy and
gives headroom for the bumped dependencies.

## What Changes

- Bump `tantivy` from `0.22.0` to `0.26.1` in `Cargo.toml`, refresh `Cargo.lock`,
  and adapt `src/recall/tantivy.rs` to any breaking API changes across the four
  intervening releases (schema builder, writer/reader construction, query
  parsing, `TopDocs`/`searcher.doc`, snippet generation, stored-value access).
  The observable recall behavior — BM25 ranking, snippet generation, regex and
  frontmatter-property post-filters, upsert/remove semantics — is preserved and
  proven by the existing `recall/tantivy.rs` and `recall-search` tests.
- Raise `rust-version` in `Cargo.toml` from `1.85` to `1.95` (latest stable, ≥ the
  1.88 floor that `time 0.3.47` and the newer tantivy stack require).
- Fold in the already-applied `time 0.3.45 → 0.3.47` lockfile bump (tantivy 0.26.1
  requires it regardless), closing the medium alert, and confirm `lru` resolves to
  `≥ 0.16.3`, closing the low alert.

This is a dependency/toolchain maintenance change. No memory-tool, configuration,
or recall-search **requirement** changes — the specs reference tantivy by behavior
and never pin a version, and MSRV is not spec-governed.

## Capabilities

### New Capabilities
- `build-toolchain`: the crate's declared minimum supported Rust version (MSRV) and
  its relationship to the dependency set. This change establishes the MSRV contract
  at Rust 1.95 (raised from 1.85 to match the floor `time 0.3.47` / tantivy 0.26.1
  require).

### Modified Capabilities
<!-- None. The recall-search and configuration specs describe the tantivy backend
     behaviorally and pin no version; the upgrade preserves that behavior, so no
     requirement deltas. The tantivy version bump is implementation, not a spec
     change. -->

## Impact

- Dependencies: `tantivy 0.22.0 → 0.26.1` (optional, `recall-tantivy`); transitively
  `lru → ≥ 0.16.3` and `time → 0.3.47` (closes both Dependabot alerts); `Cargo.lock`
  refreshed.
- Manifest: `Cargo.toml` `rust-version` `1.85 → 1.95`.
- Code: `src/recall/tantivy.rs` — adapt to tantivy 0.26.1 API. Possibly
  `src/recall/mod.rs` / `src/tools.rs` if shared types shift. No changes expected to
  the `simple` backend or non-recall code.
- Build/CI: `recall-tantivy` is off by default, so default builds are unaffected;
  verification must exercise `--features recall-tantivy`. CI uses `stable`, so the
  MSRV bump needs no workflow change, but consumers on Rust < 1.95 must upgrade.
- Consumers: anyone building with `recall-tantivy` on a toolchain below 1.95 must
  update Rust; default-feature consumers are unaffected.
