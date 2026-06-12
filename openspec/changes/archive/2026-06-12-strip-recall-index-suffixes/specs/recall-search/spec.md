# recall-search delta: strip-recall-index-suffixes

## ADDED Requirements

### Requirement: Indexed content matches the read-path view
Each per-scope index SHALL ingest note content with that scope's own link suffixes stripped, identical to the transform `read_memory_note` applies for that scope, at every ingestion path (startup warm build, watcher reconcile, periodic stat-diff reconcile, eviction rebuild, and the synchronous own-write hook). Full-text matching, regex matching, frontmatter property filters, and snippet extraction SHALL therefore evaluate against the clean agent-facing content on every backend. The shared-region index SHALL ingest content verbatim (the cross-scope leak guard guarantees shared files carry no scope suffix). Scope suffix idents SHALL NOT be matchable as content: a query or regex matching only a stored link suffix SHALL NOT produce a hit.

#### Scenario: Regex matches the clean link form
- **WHEN** an own-scope note is persisted containing `[[rust.jarvis.tony]]` for scope `{agent:"jarvis", user:"tony"}` and the tool is invoked with `regex="\[\[rust\]\]"`
- **THEN** the note is returned as a hit, with snippets showing `[[rust]]`

#### Scenario: Scope idents are not phantom content
- **WHEN** the only occurrences of the string `tony` in a scope's notes are stored link suffixes and the tool is invoked with `query="tony"`
- **THEN** no hit is returned for those notes, on the `simple` and the `tantivy` backend alike

#### Scenario: Property filters compare agent-facing values
- **WHEN** recall runs the tantivy backend, an own-scope note's persisted frontmatter contains `related: "[[rust.jarvis.tony]]"`, and the tool is invoked with filter `{ key: "related", op: "eq", value: "[[rust]]" }`
- **THEN** the note is returned as a hit

#### Scenario: Every ingestion path strips identically
- **WHEN** the same suffixed note enters the index via the startup warm build, via the synchronous own-write hook, and via a rebuild after eviction
- **THEN** the indexed and stored content is identical in all three cases, with the scope's own suffixes stripped

#### Scenario: Shared index is verbatim
- **WHEN** a shared-region note is indexed
- **THEN** its content is ingested unchanged, and queries match it exactly as stored
