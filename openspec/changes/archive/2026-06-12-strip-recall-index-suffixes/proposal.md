# Strip own-scope link suffixes at recall index time

## Why

Recall indexes ingest raw on-disk bytes, so every own-scope index holds content the agent never sees: BM25 tokenizes scope suffixes (`[[rust.jarvis.tony]]` indexes `jarvis` and `tony`, so querying your own agent or user name matches every note containing a link), the regex and simple-backend substring matchers run over suffixed text (a pattern like `\[\[rust\]\]` misses stored content), and the stored frontmatter properties carry suffixed values that an `eq` filter on the agent-facing form can never match. Snippets are currently repaired at query time — the only symptom patched, not the cause.

## What Changes

- Each per-scope index ingests note content with that scope's own link suffixes stripped — the exact `read_memory_note` view — at the single point where content is read for indexing (warm build, watcher reconcile, stat-diff backstop, and the synchronous own-write hook alike).
- Full-text (BM25 and simple substring), regex, frontmatter property filters, and snippet extraction therefore all evaluate against clean agent-facing content on both backends.
- The shared-region index ingests verbatim: the cross-scope leak guard guarantees shared files contain no scope suffixes, so there is nothing to strip.
- The now-redundant query-time snippet strip is removed; stored bodies are already clean.
- No migration: indexes are in-memory only and rebuilt on startup, eviction, and reconcile.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `recall-search`: a new requirement pins that indexed content matches the read-path view; the existing tool requirement's snippet scenario keeps holding, now by construction.

## Impact

- `src/recall/mod.rs`: the ingestion read path (reconcile/upsert) gains the strip using the index's own `IndexRegion::Scoped` scope; the query-time snippet strip is removed.
- `src/recall/simple.rs` / `src/recall/tantivy.rs`: untouched — they receive already-clean content.
- `tests/` recall coverage for both backends.
- Related change: `expand-frontmatter-property-links` makes the property tools present the same clean view; together the tool surface and the index agree.
