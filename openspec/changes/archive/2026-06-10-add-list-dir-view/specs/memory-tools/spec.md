## MODIFIED Requirements

### Requirement: `list_memory_notes` tool
The system SHALL expose a `list_memory_notes` tool that returns a paginated set of virtual paths visible to a given scope, including both inside-agents-folder files belonging to that scope and outside-agents-folder files reachable under the active policy. The tool SHALL accept an optional `view` argument selecting what the items represent: `files` (the default) returns individual note virtual paths; `dirs` returns the distinct directory virtual paths derived from the visible set — the deduplicated set of every ancestor directory of a visible note. The `dirs` view SHALL be derived purely from the visible paths without reading note contents, SHALL honor the `path_prefix` filter and pagination, and SHALL preserve deterministic ordering. An unrecognized `view` value SHALL be rejected with `invalid_argument`.

#### Scenario: Lists own-scope and outside files under namespaced policy
- **WHEN** the tool is invoked with the active scope, policy is `namespaced`, and the vault contains scope-owned files inside the agents folder plus human-authored files outside it
- **THEN** the response contains both sets, each entry represented as the clean virtual path the agent would use in subsequent calls

#### Scenario: Optional path prefix filter
- **WHEN** the tool is invoked with `path_prefix="topics"` and the agents folder is `Agents`
- **THEN** only entries whose virtual path begins with `topics` (under the agents folder) are returned

#### Scenario: Default view lists files
- **WHEN** the tool is invoked with `view` unset
- **THEN** the items are individual note virtual paths, as before

#### Scenario: Directory view lists distinct directories
- **WHEN** the tool is invoked with `view="dirs"` and the visible set contains `Agents/diary/2026-06-10.md`, `Agents/topics/rust.md`, and `Agents/topics/python.md`
- **THEN** the items are the distinct directory paths `Agents`, `Agents/diary`, and `Agents/topics` (no individual file paths), deduplicated and deterministically ordered

#### Scenario: Unrecognized view value is rejected
- **WHEN** the tool is invoked with a `view` value other than `files` or `dirs`
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
