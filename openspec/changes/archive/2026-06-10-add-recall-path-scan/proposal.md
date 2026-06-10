## Why

`recall_memory_notes` matches only note *body* — never the path. So a `query` or
`regex` that names a file (a date, a topic slug) returns nothing even when a note
with exactly that path exists, which is exactly the empty-result symptom that was
reported. Making recall also match the path closes that gap and makes the tool
behave the way agents already expect it to.

## What Changes

- The `query` (full-text/substring) and `regex` matchers in `recall_memory_notes`
  SHALL match against each note's clean virtual path in addition to its body.
- A path match contributes to the relevance score with **equal weight** to a body
  match (the score remains a match count; path matches are counted the same as body
  matches).
- When a note matches only on its path (no body match), the hit SHALL still be
  returned, with the matching path surfaced as a snippet so the agent sees why it
  matched.
- Applies to both the `simple` and `tantivy` backends. Frontmatter `filters` are
  unaffected and remain tantivy-only (unchanged).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `recall-search`: `recall_memory_notes` `query`/`regex` matching extends to the
  clean virtual path with equal scoring weight.

## Impact

- Code: `src/recall/simple.rs` (`score_doc`/`snippets_for` fold the clean path into
  the match target), `src/recall/tantivy.rs` (index and search a `path` field),
  and the snippet assembly in `src/recall/mod.rs` for path-only matches.
- No new dependencies.
