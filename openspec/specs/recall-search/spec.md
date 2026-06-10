# recall-search Specification

## Purpose
Content-ranked recall over a caller's visible memory notes, backed by per-scope in-memory indexes with structural scope isolation.

## Requirements
### Requirement: `recall_memory_notes` tool
The system SHALL expose a `recall_memory_notes` tool that returns content-ranked
hits for a query over the caller's visible set. Each hit SHALL be an object
`{ path, score, snippets }` where `path` is the clean virtual path, `score` is a
relevance value normalized to the range 0–1, and `snippets` are matching text
fragments with surrounding context. The tool SHALL accept the active scope keys
plus optional `query` (full-text), `filters` (frontmatter property predicates),
`regex`, `path_prefix`, `limit`, and `cursor` arguments. At least one of `query`,
`filters`, or `regex` MUST be supplied. The `query` and `regex` matchers SHALL be
evaluated against each note's clean virtual path in addition to its body; a path
match SHALL contribute to the relevance score with equal weight to a body match.
When a note matches only on its path, it SHALL still be returned as a hit, with the
matching path surfaced as a snippet.

#### Scenario: Full-text recall returns ranked hits
- **WHEN** the tool is invoked with `query="borrow checker"` for the active scope and
  the scope owns notes whose content matches
- **THEN** the response contains a `hits` array of `{ path, score, snippets }` objects
  ordered by descending `score`, each `path` being a clean virtual path the agent can
  pass to `read_memory_note`

#### Scenario: Recall matches the virtual path
- **WHEN** the tool is invoked with `regex="2026-06-10"` (or `query="2026-06-10"`) and
  the scope owns a note at `Agents/diary/2026-06-10.md` whose body does not contain
  that string
- **THEN** the note is returned as a hit, with the matching path surfaced as a snippet

#### Scenario: Path and body matches are weighted equally
- **WHEN** a query matches one note in its body and another note only in its path
- **THEN** each occurrence counts the same toward the match-count score, so a single
  path match and a single body match yield comparable raw scores before normalization

#### Scenario: Empty recall is rejected
- **WHEN** the tool is invoked with none of `query`, `filters`, or `regex` supplied
- **THEN** the response is an MCP error with code `invalid_argument` and no full dump
  of the vault is returned

#### Scenario: Pagination mirrors list_memory_notes
- **WHEN** the tool is invoked with `limit=50` and more than 50 hits match
- **THEN** the response contains at most 50 hits and a non-null `next_cursor`; passing
  that cursor back returns the next page; the final page returns `next_cursor: null`;
  `limit` defaults to 200 and is capped at 1000

#### Scenario: Snippets carry no foreign scope suffix
- **WHEN** a hit's underlying note content contains the caller's own-scope link suffix
- **THEN** the returned snippet has the caller's own suffix stripped, identical to the
  read-path transform, and never exposes another scope's suffix

### Requirement: Structural per-scope index isolation
Recall SHALL be backed by per-scope in-memory indexes plus a single shared-region
in-memory index. A recall query SHALL open only the caller's own-scope index and,
when the active policy permits reading the shared region, the shared index. A
scope's content SHALL reside only in that scope's index, so a recall can never
return a hit, path, or snippet belonging to another scope — isolation is structural,
not a query-time filter.

#### Scenario: Other scopes' notes are unreachable
- **WHEN** the tool is invoked for scope `{agent:"jarvis", user:"tony"}` and the vault
  also contains matching notes for `jarvis.sam`
- **THEN** no `jarvis.sam` hit, path, or snippet appears in the response, because the
  query never opens `jarvis.sam`'s index

#### Scenario: scoped policy omits the shared index
- **WHEN** the tool is invoked under policy `scoped`
- **THEN** the query opens only the caller's own-scope index and returns no hits from
  the shared region

#### Scenario: Ignored and hidden notes never match
- **WHEN** a note is excluded by hidden filtering or by an active
  `.gitignore`/`.obsidianignore`/`.ignore` rule
- **THEN** that note is absent from every index and can never appear as a recall hit

### Requirement: Configurable search backend with a simple fallback
The system SHALL select a recall backend via `AGENTMEM_RECALL_BACKEND`, accepting
`simple`, `tantivy`, and `off`, defaulting to `simple`. The `tantivy` backend SHALL
be available only when the binary is built with its optional cargo feature; when the
feature is absent, or initialization fails, or `simple` is selected, the system SHALL
use the `simple` backend. When `off` is selected the `recall_memory_notes` tool SHALL
NOT be registered. The `simple` backend SHALL support `query` (case-insensitive
substring) and `regex`, and SHALL NOT support frontmatter property `filters`.

#### Scenario: Default build uses the simple backend
- **WHEN** the server starts with `AGENTMEM_RECALL_BACKEND` unset and the `tantivy`
  feature not compiled in
- **THEN** the `simple` backend serves recall and full-text + regex queries succeed

#### Scenario: Property filters require tantivy
- **WHEN** a recall carries `filters` while the active backend is `simple`
- **THEN** the response is an MCP error with code `unsupported` whose message states
  that property filters require the tantivy backend

#### Scenario: tantivy selected without the feature falls back
- **WHEN** `AGENTMEM_RECALL_BACKEND=tantivy` but the binary was built without the
  tantivy feature
- **THEN** the server logs the fallback and serves recall with the `simple` backend

#### Scenario: Recall disabled
- **WHEN** `AGENTMEM_RECALL_BACKEND=off`
- **THEN** the `recall_memory_notes` tool is not present in the tool listing

### Requirement: In-memory index lifecycle
Recall indexes SHALL be held entirely in memory; the system SHALL NOT write any
index data to disk. At startup the system SHALL eagerly build every scope index and
the shared index. The system SHALL update the owning index synchronously on its own
note writes, reconcile external edits via a filesystem watcher (debounced and
ignore-filtered, routing each event to the owning index idempotently by file
metadata), and run a periodic stat-diff reconcile as a backstop for missed watcher
events. Idle per-scope indexes MAY be evicted under a configured memory bound and
SHALL be rebuilt on next access.

#### Scenario: Server write is reflected immediately
- **WHEN** `write_memory_note` creates or replaces a note in the caller's scope
- **THEN** a subsequent recall in that scope reflects the new content without any
  external trigger

#### Scenario: External edit is picked up
- **WHEN** a human edits a note directly in Obsidian while the server is running
- **THEN** the watcher updates the owning index and a subsequent recall reflects the
  edit; if the watcher event is missed, the periodic stat-diff reconcile corrects it

#### Scenario: Evicted scope is rebuilt on access
- **WHEN** a per-scope index has been evicted under the memory bound and a recall for
  that scope arrives
- **THEN** the call blocks until the index is rebuilt and then returns correct results

### Requirement: Normalized cross-index ranking
The system SHALL normalize result scores per index before merging across indexes.
When a recall opens both the caller's scope index and the shared index, each index's
result scores SHALL be normalized to the range 0–1 within that index's result set
before merging and sorting, and the normalized value SHALL be returned as the hit
`score`.

#### Scenario: Scores from two indexes are comparable
- **WHEN** matching hits come from both the caller's scope index and the shared index
- **THEN** every returned `score` is in 0–1 and the merged result is ordered by the
  normalized score regardless of the differing corpus statistics of the two indexes
