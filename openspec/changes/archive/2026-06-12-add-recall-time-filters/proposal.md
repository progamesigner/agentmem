## Why

"What did I work on recently?" is the canonical memory question and is
currently unanswerable: `list_memory_notes` is deliberately a path-only walk,
and `recall_memory_notes` matches only content. Yet the recall engine already
stats every visible file and holds `mtime`/`size` per note in its in-memory
manifest for reconciliation — recency is sitting there unexposed.

## What Changes

- `recall_memory_notes` gains top-level `modified_after` and `modified_before`
  arguments (RFC 3339 timestamp, or a `YYYY-MM-DD` date interpreted as start of
  day in `AGENTMEM_TIMEZONE`). The interval is half-open:
  `modified_after ≤ mtime < modified_before`.
- A time filter counts as a sufficient predicate on its own — the "at least one
  of `query`, `regex`, or `filters`" rule expands to include the time bounds, so
  a pure "recent notes" query needs no content match.
- Every hit gains a `modified_at` field (RFC 3339, UTC), sourced from the
  in-memory manifest — no extra filesystem stats on the query path.
- Time-only queries (no `query`/`regex`/`filters`) are ordered by `modified_at`
  descending with a uniform score of 1.0 and empty snippets; when combined with
  a content predicate, time bounds act as a post-merge filter and the existing
  score ordering is kept.
- Works identically on both backends (`simple` and `tantivy`) — the filter
  lives in the backend-agnostic engine layer, not in either index.
- `list_memory_notes` is deliberately unchanged: it stays a stat-free readdir
  walk that works even with `AGENTMEM_RECALL_BACKEND=off`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `recall-search`: the `recall_memory_notes` tool accepts `modified_after` /
  `modified_before`, returns `modified_at` per hit, and defines time-only
  ordering.

## Impact

- Code: `src/recall/mod.rs` (`RecallQuery`, `RecallHit`, engine merge path,
  manifest lookup), `src/tools.rs` (`RecallFields` schema, argument parsing,
  predicate validation), `tests/tools.rs`, schema snapshots, README.
- Dependencies: none (`chrono` already parses RFC 3339; `chrono-tz` already
  supplies the configured timezone).
