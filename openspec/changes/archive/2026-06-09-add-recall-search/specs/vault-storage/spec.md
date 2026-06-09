## ADDED Requirements

### Requirement: Own-scope strictness extends to recall results
The own-scope strictness guarantee SHALL extend to content recall: a
`recall_memory_notes` result — its hit paths, scores, and snippets — SHALL only ever
derive from notes in the caller's own scope or the shared region the active policy
permits. Because recall is backed by per-scope in-memory indexes, content from
another scope SHALL be structurally unreachable by a recall query, not merely
filtered out, and ignored/hidden notes SHALL never enter any index.

#### Scenario: Recall cannot cross a scope boundary
- **WHEN** a recall is issued for scope `{agent:"coder", user:"alice"}` and the vault
  contains matching notes for `coder.bob`
- **THEN** the query opens only alice's index (and the shared index when permitted),
  so no byte of `coder.bob`'s content can appear in any hit, path, or snippet

#### Scenario: Ignored content stays out of the index
- **WHEN** a note is excluded by an active `.gitignore`/`.obsidianignore`/`.ignore`
  rule or by hidden filtering
- **THEN** it is never indexed and never returned by recall, consistent with how it is
  hidden from `list_memory_notes` and `read_memory_note`
