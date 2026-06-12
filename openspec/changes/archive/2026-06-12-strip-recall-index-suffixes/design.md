# Design: strip-recall-index-suffixes

## Context

Recall (`src/recall/mod.rs`) holds per-scope in-memory indexes plus one shared index; each `RegionIndex` knows its identity via `IndexRegion::Scoped(rendered_scope)` or `Shared`. Content enters an index in one funnel — the reconcile/upsert path that lists and reads files (used by warm build, watcher, stat-diff backstop, eviction rebuild) plus the synchronous `on_write` hook — and is handed to a backend (`simple` substring/regex, or `tantivy` BM25 with stored body and `props_json` for post-filtering). Today that funnel ingests raw bytes; the only clean-up is a query-time `strip_links` over outgoing snippets (`recall/mod.rs:473`).

## Goals / Non-Goals

**Goals:**
- Indexed, stored, and filtered content equals the agent-facing read view, on both backends, on every ingestion path.
- Scope idents stop being phantom-matchable content.

**Non-Goals:**
- Stripping *other* scopes' suffixes (structurally impossible to encounter: a scope's index only ingests that scope's files, and shared files carry none).
- Index persistence or migration (indexes are memory-only and rebuilt).
- Changing ranking, normalization, or pagination.

## Decisions

1. **Strip at the ingestion funnel, with the index's own scope.** The reconcile/upsert read applies `wikilink::strip_links(content, scope, resolver)` where `scope` comes from the index's `IndexRegion::Scoped`; the `Shared` region skips the call. Alternatives rejected: per-backend stripping duplicates the logic in two places and can drift; query-time stripping (status quo extended) cannot fix BM25 tokenization, which happens at ingestion.
2. **Remove the query-time snippet strip.** With stored bodies clean, the strip at `recall/mod.rs:473` is dead weight and — worse — misleading documentation of where cleanliness comes from. The existing "snippets carry no foreign scope suffix" scenario keeps passing by construction.
3. **`props_json` comes along for free.** The tantivy backend parses frontmatter from the content it is handed; clean content in, clean stored properties out, so `eq`/`contains` filters compare the agent-facing form. This is the index half of the contract whose tool half is `expand-frontmatter-property-links`; each change stands alone, together they make property reads, writes, and filters agree.
4. **Accept the per-file strip cost at reconcile.** `strip_links` is a linear scan already paid on every single-note read; paying it per ingested file during warm/reconcile is the same order as the read the indexer already does. No caching layer until profiling says otherwise.

## Risks / Trade-offs

- **[Behavior change: queries that matched suffixed text stop matching]** → intended; the only losers are queries for scope idents, which were leaking structure, not content.
- **[A future ingestion path could forget the strip]** → the strip lives in the single shared read helper used by all paths, and the "every ingestion path strips identically" scenario pins warm, own-write, and rebuild against each other.
- **[mtime/manifest interactions]** → none: stripping changes indexed text only; manifests key on paths and stat data.

## Open Questions

(none)
