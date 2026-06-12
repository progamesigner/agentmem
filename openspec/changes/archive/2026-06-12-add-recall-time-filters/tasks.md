## 1. Query model

- [x] 1.1 Add `modified_after: Option<SystemTime>` and `modified_before: Option<SystemTime>` to `RecallQuery`, and `modified_at` to `RecallHit`, in `src/recall/mod.rs`.
- [x] 1.2 Add a timestamp parser in `src/tools.rs`: RFC 3339 via `chrono`, or bare `YYYY-MM-DD` resolved to start of day in the configured `AGENTMEM_TIMEZONE`; anything else → `invalid_argument`. Unit-test both forms and the rejection.

## 2. Tool surface

- [x] 2.1 Extend `RecallFields` with `modified_after`/`modified_before` (schema descriptions covering accepted formats, half-open semantics, and the mtime caveat) and add both to the `resolve_scope` tool-fields list.
- [x] 2.2 Relax the empty-predicate rejection to require at least one of `query`, `regex`, `filters`, `modified_after`, `modified_before`.

## 3. Engine

- [x] 3.1 Build a `clean_path → mtime` lookup from the opened indexes' manifests during `recall`; attach `modified_at` (RFC 3339 UTC) to every merged hit, omitting it if the manifest entry vanished mid-query.
- [x] 3.2 Content-predicate path: apply the time bounds as a post-merge `retain` before sorting/pagination; score ordering unchanged.
- [x] 3.3 Time-only path: skip the backend scan entirely; enumerate manifest entries within bounds as hits with `score: 1.0` and empty snippets; order by mtime descending then path ascending; reuse the existing pagination.

## 4. Tests

- [x] 4.1 Time-only query returns the bounded set in recency order on both backends (`cargo test` with and without `--features recall-tantivy`).
- [x] 4.2 Combined query+time filtering keeps score order and drops out-of-range hits; half-open boundary cases (mtime == after included, mtime == before excluded).
- [x] 4.3 Date-only input respects `AGENTMEM_TIMEZONE`; invalid input rejected; empty-predicate rejection updated; `modified_at` present on ordinary content hits.
- [x] 4.4 Update schema snapshots and the README recall section.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
