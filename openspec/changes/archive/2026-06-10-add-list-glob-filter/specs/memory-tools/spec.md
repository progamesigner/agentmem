## MODIFIED Requirements

### Requirement: `list_memory_notes` tool
The system SHALL expose a `list_memory_notes` tool that returns a paginated set of virtual paths visible to a given scope, including both inside-agents-folder files belonging to that scope and outside-agents-folder files reachable under the active policy. The tool SHALL accept an optional `glob` argument that filters the visible set to entries whose clean, vault-root-relative virtual path matches the glob pattern; `glob` is applied as an in-memory filter over visible paths and SHALL NOT read note contents. When both `path_prefix` and `glob` are supplied, an entry SHALL be returned only if it satisfies both. An invalid glob pattern SHALL be rejected with `invalid_argument`.

#### Scenario: Lists own-scope and outside files under namespaced policy
- **WHEN** the tool is invoked with the active scope, policy is `namespaced`, and the vault contains scope-owned files inside the agents folder plus human-authored files outside it
- **THEN** the response contains both sets, each entry represented as the clean virtual path the agent would use in subsequent calls

#### Scenario: Optional path prefix filter
- **WHEN** the tool is invoked with `path_prefix="topics"` and the agents folder is `Agents`
- **THEN** only entries whose virtual path begins with `topics` (under the agents folder) are returned

#### Scenario: Optional glob filter over the virtual path
- **WHEN** the tool is invoked with `glob="Agents/diary/2026-*"` and the visible set contains `Agents/diary/2026-06-10.md` and `Agents/topics/rust.md`
- **THEN** only `Agents/diary/2026-06-10.md` is returned

#### Scenario: glob composes with path_prefix
- **WHEN** the tool is invoked with `path_prefix="topics"` and `glob="**/*.md"`
- **THEN** only entries that both fall under `topics` (within the agents folder) and match `**/*.md` are returned

#### Scenario: Invalid glob is rejected
- **WHEN** the tool is invoked with a `glob` argument that is not a valid glob pattern
- **THEN** the response is an MCP error with code `invalid_argument`

#### Scenario: Other scopes' files are hidden
- **WHEN** the tool is invoked with scope `{agent:"jarvis", user:"tony"}` and the vault also contains files for `jarvis.sam`
- **THEN** the `jarvis.sam` files do NOT appear in the response

#### Scenario: scoped policy hides everything outside agents folder
- **WHEN** the tool is invoked under policy `scoped`
- **THEN** the response contains only the caller's own-scope files inside the agents folder and no entries from outside it

#### Scenario: Pagination via limit and cursor
- **WHEN** the tool is invoked with `limit=50` and the visible set contains more than 50 entries
- **THEN** the response contains exactly 50 entries and a non-null `next_cursor` opaque string; passing that `next_cursor` back in a follow-up call returns the next page; the final page's response has `next_cursor: null`

#### Scenario: Default page size
- **WHEN** the tool is invoked with `limit` unset
- **THEN** the server applies a default page size of 200 and caps `limit` at 1000; values above 1000 are rejected with `invalid_argument`

#### Scenario: Stable ordering across pages
- **WHEN** the tool is called twice in a row with the same arguments and no concurrent writes occur between the calls
- **THEN** the entries appear in the same deterministic order in both responses
