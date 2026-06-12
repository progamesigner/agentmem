## Context

The recall engine's stat-diff reconcile (`src/recall/mod.rs:520`) maintains a
per-region manifest `BTreeMap<PathBuf, FileMeta { clean_path, mtime, size }>`,
kept fresh by the filesystem watcher plus the freshness window. Both backends
share it; neither index stores time. The listing walk (`src/storage.rs:419`)
never stats files, and keeping it that way is a design value: list works with
recall off and costs one readdir per directory. Time-awareness therefore
belongs in recall, where the data is already resident.

Today `recall_memory_notes` requires at least one of `query`/`regex`/`filters`
(`src/tools.rs:833`), and `filters` are frontmatter predicates supported only
by tantivy — recency must not inherit that restriction.

## Goals / Non-Goals

**Goals:**
- Answer "what changed since X?" on both backends, with or without a content
  predicate, from data already in memory.
- Expose each hit's `modified_at` so agents can reason about staleness without
  extra calls.

**Non-Goals:**
- Exposing mtime through `list_memory_notes` (would add a per-entry stat to a
  deliberately stat-free tool, and would silently vanish as a concept when
  recall is off).
- Created-at / accessed-at times (not reliably available across platforms;
  mtime is the honest signal).
- Modeling time as a frontmatter pseudo-property in `filters` (would make
  recency tantivy-only and conflate file metadata with note properties).

## Decisions

- **Top-level `modified_after` / `modified_before` parameters, not `filters`
  entries.** They are implemented once in the engine's backend-agnostic merge
  path, so `simple` and `tantivy` behave identically. The half-open interval
  `after ≤ mtime < before` makes day-range queries compose without overlap;
  an empty intersection returns an empty page, not an error.
- **Timestamp parsing: RFC 3339, plus bare `YYYY-MM-DD` resolved to start of
  day in `AGENTMEM_TIMEZONE`.** Agents speak in dates; the configured timezone
  already defines what "a day" means for this vault (it dates diary files —
  `configuration` spec, "Timezone for date-derived tools"). Anything else is
  `invalid_argument`.
- **Time bounds are a sufficient predicate.** The empty-recall rejection
  expands to "at least one of `query`, `regex`, `filters`, `modified_after`,
  `modified_before`". A time-only query enumerates the opened indexes'
  manifests directly (no backend scan at all): every manifest entry inside the
  bounds becomes a hit. This is strictly cheaper than any content scan.
- **Ordering: score when there is a content predicate, recency when there is
  not.** With a content predicate, time bounds are a post-merge `retain` (via
  a `clean_path → mtime` lookup built from the opened manifests) and the
  existing normalized-score ordering stands. Time-only hits carry uniform
  `score: 1.0` (the field is contractually 0–1; inventing a recency-derived
  score would falsely imply relevance ranking) and are ordered
  `modified_at` descending, then path ascending for determinism. Pagination
  is unchanged on both paths.
- **`modified_at` on every hit, RFC 3339 in UTC.** Sourced from the manifest at
  merge time. UTC keeps the field machine-comparable regardless of the vault's
  display timezone (the timezone only interprets *date-only inputs*). A hit
  whose manifest entry vanished mid-query (deleted between scan and merge)
  omits the field rather than failing the page.
- **Snippets for time-only hits are empty.** There is nothing matched to
  excerpt; fabricating a content preview would re-read files and break the
  "no extra IO" property.

## Risks / Trade-offs

- [mtime is the filesystem's claim, not the agent's edit history — restores,
  `cp -p`, and sync tools can carry old timestamps] → Acceptable and honest;
  this is the same signal Obsidian sorts by. Documented in the tool
  description.
- [Manifest freshness lags external edits by up to the freshness window /
  watcher debounce] → Identical staleness contract as content recall today;
  the existing reconcile machinery applies unchanged.
- [Time-only queries over an evicted scope force an index rebuild (content
  reads) even though only stats are needed] → Accepted for simplicity: the
  rebuild path is the established residency mechanism, and the subsequent
  content queries benefit from the warm index. Optimizing a stat-only
  residency mode is premature.
