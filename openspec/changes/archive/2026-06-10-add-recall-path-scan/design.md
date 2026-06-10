## Context

Recall matches body only. In the `simple` backend, `score_doc` (`src/recall/simple.rs:60`)
runs `query.substring` and `query.regex` against `body`; the `clean_path` is the
`BTreeMap` key and is never a match target. The `tantivy` backend
(`src/recall/tantivy.rs`) indexes the body for BM25/regex. Both ignore the path.
The engine then normalizes the raw match-count score per index (`push_normalized`,
`src/recall/mod.rs:645`) and strips link suffixes from snippets.

## Goals / Non-Goals

- **Goal:** `query`/`regex` match the clean virtual path as well as the body.
- **Goal:** equal scoring weight for path and body matches (decision below).
- **Non-Goal:** changing `filters` (frontmatter) — those stay tantivy-only, unchanged.
- **Non-Goal:** adding a separate path-only query argument — the existing
  `query`/`regex` simply gain the path as an additional match target.

## Decisions

- **Equal weight.** The `simple` score is a match count; path matches are added to
  the same count as body matches (a path match and a body match each add 1). This is
  the simplest rule, keeps normalization unchanged, and treats "the filename says so"
  as a legitimate, comparable signal of relevance. No weighting factor is introduced.
- **simple backend:** in `score_doc`, count matches across `clean_path` *and* `body`
  for both the substring and regex matchers, summing into the existing score. The
  path must therefore be available to `score_doc` — pass it alongside the body
  (the index already keys docs by `clean_path`).
- **Path-only matches surface the path as a snippet.** `snippets_for` collects
  matching body lines today; when the path matches but no body line does, emit the
  clean path as the single snippet so the agent sees why the note matched. Path-derived
  snippets pass through the same suffix-stripping as body snippets.
- **tantivy backend:** add an indexed `path` field and include it in the query so
  BM25/regex match the path too. Because the agent-facing score is normalized per
  index (0–1), the cross-backend score shape stays consistent; exact BM25 weighting
  of the path field is tuned to approximate the "equal weight" intent without a
  separate boost.
- **Isolation is unaffected.** Path matching happens inside each per-scope/shared
  index, so structural cross-scope isolation (`src/recall/mod.rs` module docs) holds:
  a query never opens another scope's index, paths included.

## Risks / Trade-offs

- Path tokens (e.g. `Agents`, `topics`, `md`) could inflate matches for broad
  queries. Mitigation: equal-weight counting keeps the effect proportional, and
  normalization bounds scores to 0–1; broad queries already return many hits.
- BM25 on a short `path` field is not literally a "count," so tantivy's notion of
  equal weight is an approximation of the simple backend's exact count. Mitigation:
  the spec pins the *behavioral* guarantee (path-only notes are returned; path and
  body matches are comparable), not a numeric identity between backends.
