# memory-tools Specification

## Purpose
TBD - created by archiving change build-agentmem-mcp-server. Update Purpose after archive.
## Requirements
### Requirement: `list_memory_notes` tool
The system SHALL expose a `list_memory_notes` tool that returns a paginated set of virtual paths visible to a given scope, including both inside-agents-folder files belonging to that scope and outside-agents-folder files reachable under the active policy. The tool SHALL accept an optional `glob` argument that filters the visible set to entries whose clean, vault-root-relative virtual path matches the glob pattern; `glob` is applied as an in-memory filter over visible paths and SHALL NOT read note contents. When both `path_prefix` and `glob` are supplied, an entry SHALL be returned only if it satisfies both. An invalid glob pattern SHALL be rejected with `invalid_argument`. The tool SHALL accept an optional `order` argument selecting the result ordering by clean virtual path: `name_asc` (the default) or `name_desc`. Ordering SHALL be applied before pagination so `limit`/`cursor` page over the ordered set, and SHALL remain deterministic across calls with identical arguments. An unrecognized `order` value SHALL be rejected with `invalid_argument`. The tool SHALL accept an optional `view` argument selecting what the items represent: `files` (the default) returns individual note virtual paths; `dirs` returns the distinct directory virtual paths derived from the visible set — the deduplicated set of every ancestor directory of a visible note. The `dirs` view SHALL be derived purely from the visible paths without reading note contents, SHALL honor the `path_prefix` filter and pagination, and SHALL preserve deterministic ordering. An unrecognized `view` value SHALL be rejected with `invalid_argument`.

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

#### Scenario: Default ordering is ascending by path
- **WHEN** the tool is invoked with `order` unset
- **THEN** entries are returned in ascending clean-virtual-path order

#### Scenario: Descending order by path
- **WHEN** the tool is invoked with `order="name_desc"` and the visible set contains `Agents/diary/2026-01-01.md` and `Agents/diary/2026-06-10.md`
- **THEN** `Agents/diary/2026-06-10.md` is returned before `Agents/diary/2026-01-01.md`

#### Scenario: Unrecognized order value is rejected
- **WHEN** the tool is invoked with an `order` value other than `name_asc` or `name_desc`
- **THEN** the response is an MCP error with code `invalid_argument`

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

### Requirement: `read_memory_note` tool
The system SHALL expose a `read_memory_note` tool that returns the UTF-8 contents of a single file identified by its virtual path, subject to the active policy, region detection, and visibility filters. The tool SHALL accept an optional boolean `backlinks` argument; when `true`, the structured result SHALL additionally carry a `backlinks` array containing the clean virtual path of every visible note that has at least one link resolving to the target note. The array SHALL be deduplicated (one entry per referring note regardless of how many of its links resolve to the target), sorted ascending by clean virtual path, and computed over exactly the caller's visible set (own scope plus the shared region when the active policy permits reading it). When `backlinks` is absent or `false`, the response SHALL NOT contain a `backlinks` field and SHALL be unchanged from prior behavior.

#### Scenario: Read of an own-scope file inside the agents folder
- **WHEN** the tool is called with virtual path `PERSONA.md` (resolved under the agents folder) for the active scope and that file exists
- **THEN** the response contains the file's contents as a string

#### Scenario: Read outside the agents folder under namespaced policy
- **WHEN** policy is `namespaced` and the tool is called with virtual path `Actions/release.md` and that file exists
- **THEN** the response contains the file's contents as a string

#### Scenario: Read outside the agents folder under scoped policy
- **WHEN** policy is `scoped` and the tool is called with virtual path `Actions/release.md`
- **THEN** the response is an MCP error with code `path_not_permitted`

#### Scenario: Read of a missing file
- **WHEN** the tool is called with a virtual path that resolves to a non-existent file
- **THEN** the response is an MCP error with code `not_found`

#### Scenario: Read of a hidden or ignored file
- **WHEN** the tool is called with a virtual path that is excluded by hidden filtering or by an active `.gitignore`/`.obsidianignore` rule
- **THEN** the response is an MCP error with code `path_not_permitted` and the message does NOT reveal whether the file actually exists

#### Scenario: Backlinks returned on request
- **WHEN** the tool is called with `backlinks=true` for note `Agents/topics/rust.md` and the caller's visible notes `Agents/diary/2026-06-10.md` (containing `[[rust]]`) and `Agents/MEMORY.md` (containing `[[topics/rust|the Rust note]]`) both resolve those links to the target
- **THEN** the structured result carries `backlinks: ["Agents/MEMORY.md", "Agents/diary/2026-06-10.md"]` alongside the content

#### Scenario: All link forms count as backlinks
- **WHEN** a visible note references the target via an embed `![[target]]`, a heading link `[[target#section]]`, an aliased link `[[target|label]]`, or a relative markdown link `[text](path/target.md)`
- **THEN** that note appears in the target's `backlinks` array

#### Scenario: A referring note appears once
- **WHEN** a single visible note contains three distinct links that all resolve to the target
- **THEN** the `backlinks` array contains that note's clean virtual path exactly once

#### Scenario: Backlinks honor forward-resolution tie-breaks
- **WHEN** the caller's visible set contains both `Agents/topics/rust.md` and `Lang/rust.md`, and a visible note contains `[[rust]]` which forward resolution resolves to `Agents/topics/rust.md`
- **THEN** that note appears in the backlinks of `Agents/topics/rust.md` and NOT in the backlinks of `Lang/rust.md`

#### Scenario: Other scopes' links are structurally invisible
- **WHEN** the tool is called with `backlinks=true` for shared note `Actions/release.md` by scope `{agent:"jarvis", user:"tony"}`, and a note belonging to scope `jarvis.sam` links to that shared note
- **THEN** no `jarvis.sam` path appears in the `backlinks` array, because another scope's notes are never scanned

#### Scenario: scoped policy excludes the shared region from the scan
- **WHEN** policy is `scoped` and the tool is called with `backlinks=true` for an own-scope note that a shared-region note links to
- **THEN** the `backlinks` array does not contain the shared note's path

#### Scenario: Backlinks omitted by default
- **WHEN** the tool is called without the `backlinks` argument
- **THEN** the structured result contains no `backlinks` field and is unchanged from prior behavior

### Requirement: `read_memory_notes` tool
The system SHALL expose a `read_memory_notes` tool that reads multiple notes in one call. The `paths` argument SHALL be an array of 1 to 20 **vault-root-relative** virtual paths; an empty array, an array exceeding 20 entries, or any non-string entry SHALL be rejected with `invalid_argument`. The result SHALL contain a `notes` array with exactly one entry per requested path, in request order, each entry being either `{ path, content }` on success or `{ path, error: { code, message } }` on failure, where `path` echoes the requested path verbatim. Per-path semantics — policy gating, region detection, visibility filtering, suffix resolution, and own-suffix link stripping — SHALL be identical to `read_memory_note`, and a per-path failure SHALL use the same error code the single-read tool would return. A per-path failure SHALL NOT fail the call; the tool-level result is an error only for malformed arguments or invalid scope keys. Duplicate paths SHALL each produce their own entry.

#### Scenario: Batch read returns contents in request order
- **WHEN** the tool is called with `paths=["Agents/topics/rust.md", "Actions/release.md"]` under `namespaced` policy and both notes exist
- **THEN** the result's `notes` array has two entries in that order, each carrying the note's content with the caller's own link suffixes stripped

#### Scenario: Partial failure does not void the batch
- **WHEN** the tool is called with three paths of which the second resolves to a non-existent file
- **THEN** entries one and three carry content, entry two carries `error: { code: "not_found", … }`, and the tool call itself succeeds

#### Scenario: Per-path policy and visibility parity
- **WHEN** a requested path would be denied by the single read (e.g. outside the agents folder under `scoped` policy, or a hidden/ignored target)
- **THEN** that entry carries the same error code the single read would return (`path_not_permitted`), without revealing whether the file exists

#### Scenario: Batch size limits
- **WHEN** the tool is called with an empty `paths` array or with more than 20 entries
- **THEN** the response is an MCP error with code `invalid_argument` and no notes are read

#### Scenario: Duplicate paths are answered positionally
- **WHEN** the tool is called with the same path twice
- **THEN** the `notes` array contains two entries for it, one per request position

### Requirement: `write_memory_note` tool
The system SHALL expose a `write_memory_note` tool that performs an atomic full-file write to a virtual path the active policy permits writing to. The `path` argument is a **vault-root-relative** virtual path; to target a location inside the agents folder the caller MUST include the agents-folder name as the leading segment (the dedicated wrapper tools do this automatically; the generic tools do not). Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path (a path with no subfolder segment beneath the per-scope root) is reserved for the dedicated wrapper tools (`evolve_core_persona`, `update_task_heartbeat`) and SHALL be rejected.

The tool SHALL accept an optional boolean `append` argument. When `append` is `true`, `content` SHALL be appended verbatim to the existing note — exact bytes, no implicit separator — under the same per-target lock as the diary append, so concurrent appends to one note serialise without loss; when the note does not exist it SHALL be created with `content` as its full body. The appended fragment SHALL pass through the write-side link transform (including the cross-scope leak guard) exactly like full-write content. All other guards (root-level reservation, policy gates, visibility filters) apply unchanged. The returned byte count SHALL be the note's total size after the write in both modes.

#### Scenario: Write succeeds inside agents folder
- **WHEN** policy permits writes inside the agents folder (any policy other than `readonly`) and the tool is called with a vault-root-relative virtual path inside a subfolder of it (e.g. `Agents/topics/auth/jwt.md`)
- **THEN** the file is created or replaced via the atomic-write procedure and the response is a success result containing the byte count written

#### Scenario: Write to a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `Agents/MEMORY.md`, `Agents/USER.md`, or `Agents/PERSONA.md`)
- **THEN** the response is an MCP error with code `path_not_permitted`, the file on disk is unchanged, and the message names the wrapper to use (`evolve_core_persona` for foundational files, `update_task_heartbeat` for the heartbeat)

#### Scenario: Write refused outside agents folder under namespaced policy
- **WHEN** policy is `namespaced` and the tool is called with virtual path `Actions/release.md`
- **THEN** the response is an MCP error with code `write_denied` and the file on disk is unchanged

#### Scenario: Write succeeds outside agents folder under readwrite policy
- **WHEN** policy is `readwrite` and the tool is called with virtual path `Scratch/team-notes.md`
- **THEN** the file is created or replaced at `<root>/Scratch/team-notes.md` without a suffix and the response is a success result containing the byte count written

#### Scenario: Write refused under readonly policy
- **WHEN** policy is `readonly` and any write tool is invoked
- **THEN** the response is an MCP error with code `write_denied`

#### Scenario: Write refused on hidden or ignored target
- **WHEN** the tool is called against a virtual path excluded by visibility filters
- **THEN** the response is an MCP error with code `path_not_permitted` and no file is created

#### Scenario: Append extends an existing note verbatim
- **WHEN** the tool is called with `append=true` and `content="- new fact\n"` against a note ending in `"- old fact\n"`
- **THEN** the note ends with `"- old fact\n- new fact\n"` — no separator inserted — and the response reports the note's total byte count

#### Scenario: Append to a missing note creates it
- **WHEN** the tool is called with `append=true` against a virtual path with no existing file
- **THEN** the note is created with `content` as its full body

#### Scenario: Concurrent appends are not lost
- **WHEN** multiple callers append to the same note concurrently
- **THEN** every appended fragment appears exactly once in the final note (appends serialise under the per-target lock)

#### Scenario: Appended links are transformed
- **WHEN** `append=true` content contains `[[rust]]` resolving to an own-scope note
- **THEN** the persisted fragment carries the expanded suffixed form and a subsequent read returns the clean form, identical to full-write behavior

#### Scenario: Append honors the same guards as full write
- **WHEN** the tool is called with `append=true` against a root-level core file, a policy-denied region, or a visibility-excluded path
- **THEN** the response is the same error the full-write mode would produce and nothing is written

### Requirement: `edit_memory_note` tool
The system SHALL expose an `edit_memory_note` tool that takes a virtual path, a `search_string`, and a `replace_string`; replaces the unique occurrence of the search string with the replacement; and persists the result atomically. The `path` argument is a **vault-root-relative** virtual path; to target a location inside the agents folder the caller MUST include the agents-folder name as the leading segment. The search string MUST appear exactly once in the target file. Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path is reserved for the dedicated wrapper tools and SHALL be rejected.

#### Scenario: Successful edit
- **WHEN** the tool is called and the search string appears exactly once in the target file
- **THEN** the server writes the modified file atomically and returns a success result indicating the number of characters replaced

#### Scenario: Edit of a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `Agents/MEMORY.md`)
- **THEN** the response is an MCP error with code `path_not_permitted`, the file is unchanged, and the message names the wrapper to use

#### Scenario: Edit refused on read-only target
- **WHEN** the active policy denies writes to the target's region (e.g. `namespaced` on a path outside the agents folder, or `readonly` anywhere)
- **THEN** the response is an MCP error with code `write_denied` and the file is unchanged

#### Scenario: Edit refused on missing search string
- **WHEN** the search string does not appear in the target file
- **THEN** the response is an MCP error with code `edit_search_not_found` and the file is unchanged

#### Scenario: Edit refused on ambiguous search string
- **WHEN** the search string appears two or more times in the target file
- **THEN** the response is an MCP error with code `edit_search_ambiguous`, the file is unchanged, and the message tells the agent to retry with a longer, more specific snippet

### Requirement: `delete_memory_note` tool
The system SHALL expose a `delete_memory_note` tool that removes a single file at the given virtual path, subject to the active policy and own-scope rules. The `path` argument is a **vault-root-relative** virtual path; to target a location inside the agents folder the caller MUST include the agents-folder name as the leading segment. The tool SHALL NOT remove directories, and SHALL leave a parent directory in place even if it becomes empty. Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level core file SHALL NOT be deletable through this tool.

#### Scenario: Delete succeeds for own-scope file under writable policy
- **WHEN** policy permits writes in the target's region and the tool is called for an own-scope file under a subfolder that exists
- **THEN** the file is removed via `std::fs::remove_file` and the response is a success result

#### Scenario: Delete of a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `Agents/PERSONA.md`)
- **THEN** the response is an MCP error with code `path_not_permitted` and the file is unchanged

#### Scenario: Delete refused under readonly policy
- **WHEN** policy is `readonly`
- **THEN** the response is an MCP error with code `write_denied` and the file is unchanged

#### Scenario: Delete refused outside agents folder under namespaced policy
- **WHEN** policy is `namespaced` and the tool is called with a virtual path outside the agents folder
- **THEN** the response is an MCP error with code `write_denied`

#### Scenario: Delete refused outside agents folder under scoped policy
- **WHEN** policy is `scoped` and the tool is called with a virtual path outside the agents folder
- **THEN** the response is an MCP error with code `path_not_permitted`

#### Scenario: Delete of a missing file
- **WHEN** the tool is called for a path that resolves to a non-existent file
- **THEN** the response is an MCP error with code `not_found`

#### Scenario: Delete of another scope's file is unreachable
- **WHEN** the tool is called inside the agents folder for a virtual path whose own-scope resolution does not exist, even though another scope's file with a different suffix does exist at the same logical name
- **THEN** the response is `not_found` and the other scope's file is NOT removed

### Requirement: `rename_memory_note` tool
The system SHALL expose a `rename_memory_note` tool that moves a single note from `path` to `new_path` (both **vault-root-relative** virtual paths) and rewrites every visible incoming link to resolve to the new location. The destination MUST NOT already exist; a rename onto an existing note SHALL be rejected with a `destination_exists` error and no change on disk. Inside the agents folder, both `path` and `new_path` MUST be under a subfolder; an agents-folder root-level path on either end SHALL be rejected with `path_not_permitted` (core files are wrapper-managed). The active policy MUST permit writing both the source's and the destination's region, and the region of every referring note that requires rewriting; when any of these is not writable the tool SHALL refuse with the appropriate policy error before any mutation. The moved note's own content SHALL be re-run through the write-side link transform for the destination's region, including the cross-scope leak guard. All preconditions SHALL be validated before the first write; mutations are then applied in the order: write destination, rewrite referrers, delete source.

#### Scenario: Rename moves content and reports rewrites
- **WHEN** the tool is called with `path="Agents/topics/rust.md"`, `new_path="Agents/topics/rust-lang.md"` under a writable policy
- **THEN** the destination contains the source's content, the source no longer exists, and the response carries `{ renamed: true, path, new_path, notes_rewritten }`

#### Scenario: Incoming wikilinks are rewritten
- **WHEN** a visible note contains `[[rust]]`, `[[rust#install|the note]]`, and `![[rust]]` all forward-resolving to the source, and the source is renamed to `rust-lang.md`
- **THEN** after the rename the referring note's links resolve to the destination, with heading, alias, and embed decorations preserved (e.g. `[[rust-lang#install|the note]]`)

#### Scenario: Incoming markdown links are rewritten
- **WHEN** a visible note contains `[doc](topics/rust.md)` resolving to the source and the source is renamed within the same scope
- **THEN** the referring note's markdown link target is rewritten to the destination's persisted form and round-trips to the clean new path on read

#### Scenario: Self-references move with the note
- **WHEN** the source note's own content contains a link resolving to itself
- **THEN** the destination's content links to the destination (the old name neither dangles nor persists)

#### Scenario: Destination must not exist
- **WHEN** the tool is called with a `new_path` at which a visible note already exists
- **THEN** the response is an MCP error with code `destination_exists` and neither note is modified

#### Scenario: Root core files are not renamable
- **WHEN** the tool is called with `path` or `new_path` at the agents-folder root level (e.g. `Agents/MEMORY.md`)
- **THEN** the response is an MCP error with code `path_not_permitted` and nothing changes

#### Scenario: Policy gates both regions
- **WHEN** policy is `namespaced` and either `path` or `new_path` resolves outside the agents folder
- **THEN** the response is an MCP error with code `write_denied` and nothing changes

#### Scenario: Shared-to-scoped rename is refused when shared referrers exist
- **WHEN** policy is `readwrite`, a shared-region note links to shared note `Actions/release.md`, and the tool is asked to rename `Actions/release.md` to a path inside the agents folder
- **THEN** the response is an MCP error with code `write_denied` (the rewrite would persist the caller's scope suffix in a shared note) and nothing changes

#### Scenario: Leak guard applies to the moved content
- **WHEN** policy is `readwrite` and a scoped note whose content links to another of the caller's scoped notes is renamed to a destination outside the agents folder
- **THEN** the response is an MCP error with code `write_denied` and nothing changes

#### Scenario: Missing source
- **WHEN** the tool is called with a `path` that resolves to a non-existent file
- **THEN** the response is an MCP error with code `not_found`

#### Scenario: Recall reflects the rename immediately
- **WHEN** recall is enabled and a note is renamed
- **THEN** a subsequent recall in the same scope returns hits at the new path and none at the old path, without waiting for the filesystem watcher

### Requirement: `load_session_context` tool
The system SHALL expose a `load_session_context` tool that, in a single call for the active scope, returns the **rendered session-context** produced by the shared session-context renderer (see the *Session-context renderer* requirement). The tool SHALL accept only scope parameters. The response SHALL contain a `rendered` field holding the rendered markdown string and a `missing` list naming any of the five foundational files (`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `MEMORY.md`) that did not exist for the scope at render time. The tool SHALL succeed even when no foundational files and no session-context template exist.

#### Scenario: Rendered context returned
- **WHEN** the tool is called for an active scope
- **THEN** the response contains a `rendered` markdown string produced by the renderer and a `missing` list naming any absent foundational file

#### Scenario: Some files missing
- **WHEN** only `PERSONA.md` and `RULES.md` exist for the scope
- **THEN** the `rendered` output substitutes the persona and rules contents, substitutes the missing sentinel for `PROMPT.md`, `USER.md`, and `MEMORY.md`, and `missing` names `PROMPT.md`, `USER.md`, `MEMORY.md`

#### Scenario: Empty vault still succeeds
- **WHEN** the tool is called for a scope with no foundational files and no session-context template present at any layer
- **THEN** the response is a success result whose `rendered` field is the compiled-in default template with all file slots showing the missing sentinel, and `missing` names all five foundational files (`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `MEMORY.md`)

#### Scenario: No path argument
- **WHEN** a client attempts to pass a `path` or `which` argument
- **THEN** the call is rejected at schema validation because the input schema accepts only scope parameters

### Requirement: `evolve_core_persona` tool
The system SHALL expose an `evolve_core_persona` tool that performs an atomic full-file write to exactly one of the five foundational session files, selected by a required `which` parameter whose value is one of `persona`, `prompt`, `rules`, `user`, `memory`. The corresponding target file is the matching `.md` file (e.g. `which=persona` → `PERSONA.md`, `which=memory` → `MEMORY.md`) resolved relative to the agents folder for the active scope. The tool SHALL enforce a hard line-count cap on the content for the capped files: `which=user` content MUST NOT exceed 100 lines and `which=memory` content MUST NOT exceed 200 lines (counted as newline-separated lines). Content exceeding the cap SHALL be rejected before any write.

#### Scenario: Persona update
- **WHEN** the tool is called with `which="persona"` and new content for the active scope
- **THEN** the scope's `PERSONA.md` is replaced atomically and the response is a success result containing the byte count written

#### Scenario: Prompt update
- **WHEN** the tool is called with `which="prompt"` and new content
- **THEN** the scope's `PROMPT.md` is replaced atomically

#### Scenario: Rules update
- **WHEN** the tool is called with `which="rules"` and new content
- **THEN** the scope's `RULES.md` is replaced atomically

#### Scenario: User update within cap
- **WHEN** the tool is called with `which="user"` and content of 100 lines or fewer
- **THEN** the scope's `USER.md` is replaced atomically

#### Scenario: Memory update within cap
- **WHEN** the tool is called with `which="memory"` and content of 200 lines or fewer
- **THEN** the scope's `MEMORY.md` is replaced atomically

#### Scenario: User content over the line cap is rejected
- **WHEN** the tool is called with `which="user"` and content exceeding 100 lines
- **THEN** the response is an MCP error with code `invalid_argument`, the message states the 100-line limit, and `USER.md` is unchanged

#### Scenario: Memory content over the line cap is rejected
- **WHEN** the tool is called with `which="memory"` and content exceeding 200 lines
- **THEN** the response is an MCP error with code `invalid_argument`, the message states the 200-line limit, and `MEMORY.md` is unchanged

#### Scenario: Invalid `which`
- **WHEN** the tool is called with `which` set to any value other than the five accepted strings
- **THEN** the call is rejected at schema validation (the schema's `which` field is an enum)

#### Scenario: Path argument is rejected
- **WHEN** a client attempts to pass a `path` argument to override the hardcoded targets
- **THEN** the call is rejected at schema validation because the input schema does NOT include a path field

#### Scenario: Refused under readonly policy
- **WHEN** policy is `readonly` and `evolve_core_persona` is invoked with any valid `which`
- **THEN** the response is an MCP error with code `write_denied`

### Requirement: `update_task_heartbeat` tool
The system SHALL expose an `update_task_heartbeat` tool whose target is hardcoded to the conventional virtual path `HEARTBEAT.md` (resolved relative to the agents folder) for the active scope and which performs an atomic full-file write.

#### Scenario: Heartbeat is replaced atomically
- **WHEN** the tool is called with new heartbeat content for the active scope
- **THEN** the resolved physical `HEARTBEAT.md` file for that scope is replaced atomically and the response is a success result containing the byte count written

### Requirement: `append_diary_entry` tool
The system SHALL expose an `append_diary_entry` tool that appends a timestamped section to today's diary file at the virtual path `diary/<YYYY-MM-DD>.md` (resolved relative to the agents folder) for the active scope. The tool SHALL create the diary file (and its parent directories) if it does not exist, writing a `# <YYYY-MM-DD>` H1 title as the first line of a newly created file. The tool SHALL accept an optional `title` argument: when present, the entry heading is `## <HH:MM:SS> — <title>`; when absent, it is `## <HH:MM:SS>`. The append SHALL be implemented as a read-modify-write through the atomic-write procedure.

#### Scenario: Appends to existing diary
- **WHEN** the tool is called for scope `{agent:"jarvis", user:"tony"}` with `content="Picked up task #42."` and `title="Task pickup"` at local time `14:03:22` on `2026-05-25`, and the scope's diary file for that date already contains prior sections
- **THEN** the server resolves the path to the scope's physical diary file, reads its current contents, appends `\n## 14:03:22 — Task pickup\nPicked up task #42.\n`, and persists the result via the atomic-write procedure

#### Scenario: Appends without a title
- **WHEN** the tool is called with `content` but no `title` at local time `14:03:22`, and the diary file already exists
- **THEN** the server appends `\n## 14:03:22\n<content>\n` (a bare-time heading) and persists it

#### Scenario: Creates diary on first entry of the day
- **WHEN** the tool is called for an active scope with `content` and no `title` at local time `09:00:00` on `2026-05-25` and no file exists at the scope's `diary/2026-05-25.md` virtual path
- **THEN** the server creates any missing parent directories, writes a new file whose contents start with `# 2026-05-25\n\n## 09:00:00\n<content>\n`, and persists it via the atomic-write procedure

#### Scenario: Concurrent appends in the same process are serialised
- **WHEN** two concurrent `append_diary_entry` calls for the same scope target the same diary file
- **THEN** the per-target advisory lock serialises them so the final on-disk file contains both sections, each formatted correctly, with no interleaving

#### Scenario: Path argument is rejected
- **WHEN** a client attempts to pass a `path` argument to override the hardcoded target
- **THEN** the call is rejected at schema validation because the input schema does NOT accept a path field

#### Scenario: Empty content is refused
- **WHEN** the tool is called with an empty `content` string
- **THEN** the response is an MCP error with code `invalid_argument` and the diary file is unchanged

### Requirement: Common tool input contract
The system SHALL ensure every tool's input schema includes the scope parameters whose names are the placeholder idents of `AGENTMEM_VFS_SCHEME`, and SHALL reject calls whose scope arguments do not satisfy that contract.

#### Scenario: All scheme keys required
- **WHEN** scheme is `<agent>.<user>` and a tool is called with `agent` set but `user` missing
- **THEN** the call is rejected with code `missing_scope` and the message names `user`

#### Scenario: Unexpected scope parameter
- **WHEN** scheme is `<agent>` and a tool is called with both `agent` and `user`
- **THEN** the call is rejected at schema validation because the input schema does NOT include `user` under this scheme

#### Scenario: Custom scheme keys are honoured
- **WHEN** scheme is `<team>.<agent>.<env>.<user>` and a tool is called with exactly those four fields
- **THEN** the call proceeds to resolution with the rendered suffix `<team>.<agent>.<env>.<user>`

#### Scenario: Empty scheme requires no scope arguments
- **WHEN** scheme is the empty string and a tool is called with no scope fields
- **THEN** the call proceeds; if any scope field is supplied, the call is rejected at schema validation

### Requirement: Session-context renderer
The system SHALL provide a single shared renderer that produces the session-context markdown for a given scope. The renderer SHALL resolve the session-context template (see *Session-context template resolution*), read the five foundational files for the scope, substitute every recognised placeholder, and return the resulting string together with the list of foundational files that were absent. The renderer SHALL be the single source of the rendered output exposed by the `load_session_context` tool, the `session-context` resource, and the `session-context` prompt. The server-generated memory-tools guide produced for `{{tools_guide}}` SHALL name the concrete active scope as `key=value` pairs so the agent knows exactly which scope keys and values to carry on every tool call.

#### Scenario: Recognised placeholders are substituted
- **WHEN** the active template contains `{{files.persona}}`, `{{files.user}}`, `{{scope.agent}}`, and `{{tools_guide}}`
- **THEN** the renderer replaces them respectively with the contents of `PERSONA.md`, the contents of `USER.md`, the rendered value of the `agent` scope key, and the server-generated memory-tools guide

#### Scenario: Missing foundational file renders a sentinel
- **WHEN** a `{{files.*}}` placeholder names a foundational file that does not exist for the scope
- **THEN** the renderer substitutes a fixed missing sentinel (for example `(not yet recorded — set via evolve_core_persona)`) rather than omitting the placeholder or erroring, and records that file in the absent list

#### Scenario: Unknown placeholder is left literal
- **WHEN** the template contains a `{{…}}` token the renderer does not recognise
- **THEN** the token is left verbatim in the output and a single diagnostic is logged; rendering does not error

#### Scenario: Tools guide reflects the live tool set
- **WHEN** `{{tools_guide}}` is rendered
- **THEN** its content is generated from the server's live tool catalogue so that the names and usage it describes always match the tools currently advertised

#### Scenario: Tools guide names the concrete active scope
- **WHEN** `{{tools_guide}}` is rendered for a non-empty scope such as `{agent: jarvis, user: tony}`
- **THEN** the guide states that every call must carry those scope keys and lists them as `key=value` pairs in deterministic key order (for example `agent=jarvis, user=tony`), covering exactly the keys the configured scheme defines

#### Scenario: Tools guide falls back to generic phrasing for an empty scope
- **WHEN** `{{tools_guide}}` is rendered for an empty scope
- **THEN** the guide retains the generic instruction that every call must carry the scope keys defined by the server's VFS scheme, without naming any specific key

### Requirement: Session-context template
The system SHALL treat the session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `memory` and `<key>` is a scheme placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default template that delimits each section with XML-style tags rather than `##` headings, so that embedded foundational-file markdown (which typically begins at H2) does not collide with the template's own structure. The default template SHALL wrap the foundational (agent-owned) slots in bare tags — `<PERSONA>{{files.persona}}</PERSONA>`, `<RULES>{{files.rules}}</RULES>`, `<MEMORY>{{files.memory}}</MEMORY>`, `<USER>{{files.user}}</USER>`, `<PROMPT>{{files.prompt}}</PROMPT>` — and the server-generated slots in `AGENTMEM:`-namespaced tags — `<AGENTMEM:TOOLS>{{tools_guide}}</AGENTMEM:TOOLS>` and `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>` — preserving the section order `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT`, `AGENTMEM:TOOLS`, `AGENTMEM:LAYOUT`. The `<AGENTMEM:LAYOUT>` section SHALL embed a **suggested (non-enforced) memory layout** and the documented line caps as prose so the server is useful with no operator configuration. The suggested layout is illustrative guidance for the agent, not a constraint the server validates (only the wrapper-only-roots rule and the line caps are enforced). The layout prose SHALL distinguish the small set of **core files** that have special handling (changed only through the dedicated wrapper tools, and subject to the line caps) from all other paths, which behave like an ordinary filesystem the agent reads, writes, and organizes freely; it SHALL NOT expose any internal per-scope filename-suffix mechanism. The layout prose SHALL also state the path-addressing rule for the subfolder conventions: the listed subfolder paths are shown **relative to the agents folder**, the wrapper tools (`append_diary_entry`, `evolve_core_persona`, `update_task_heartbeat`) prepend the agents-folder name automatically, and the generic note tools (`write_memory_note`, `edit_memory_note`, `delete_memory_note`, `read_memory_note`) take the full **vault-root-relative** path and therefore require the agents-folder name as the leading segment; the prose SHALL convey this without hardcoding a specific agents-folder name (so it remains correct under any `AGENTMEM_AGENTS_DIR`). The default template SHALL leave the internal organization of `MEMORY.md` to the agent/user rather than prescribing a skeleton. Because the conventions live in the default template (not in `{{tools_guide}}`), an operator who supplies their own template fully controls and may override them.

#### Scenario: Default template is self-contained
- **WHEN** no session-context template file exists at any resolution layer
- **THEN** the renderer uses the compiled-in default template, which delimits its sections with XML-style tags in the order `<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`, `<PROMPT>`, `<AGENTMEM:TOOLS>`, `<AGENTMEM:LAYOUT>`, and which includes the memory-organization conventions and the documented `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 line caps

#### Scenario: Sections are delimited by tags, not H2 headings
- **WHEN** the compiled-in default template is rendered
- **THEN** each foundational file's contents are wrapped in a bare tag (`<PERSONA>…</PERSONA>`, `<RULES>…</RULES>`, `<MEMORY>…</MEMORY>`, `<USER>…</USER>`, `<PROMPT>…</PROMPT>`), the tools guide is wrapped in `<AGENTMEM:TOOLS>…</AGENTMEM:TOOLS>`, and the layout prose is wrapped in `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>`, so embedded H2 markdown in a foundational file does not collide with the template's section delimiters

#### Scenario: Default template documents the suggested layout
- **WHEN** the compiled-in default template is rendered
- **THEN** the `<AGENTMEM:LAYOUT>` section presents the suggested (non-enforced) layout with each entry's purpose: root core files `MEMORY.md` (working-memory index), `RULES.md` (safety boundaries), `PERSONA.md` (identity/soul/style), `PROMPT.md` (workflow rules, plus external-tool facts such as camera/SSH details), `USER.md` (user profile), `HEARTBEAT.md` (task heartbeat); and subfolders `diary/<YYYY-MM-DD>.md`, `workspaces/INDEX.md` + `workspaces/<project>/<item>.md`, `topics/INDEX.md` + `topics/LOG.md` + `topics/<topic>/<fact>.md`, `skills/<skill>/SKILL.md` + `skills/<skill>/references/<name>.md`, and `agents/<subagent>/PROMPT.md` + `agents/<subagent>/<context>.md`

#### Scenario: Layout distinguishes core files from free-form filesystem
- **WHEN** the compiled-in default template is rendered
- **THEN** the `<AGENTMEM:LAYOUT>` prose states that the core files are changed only through their dedicated wrapper tools and are subject to the documented line caps, while every other path behaves like an ordinary filesystem the agent may read, write, and organize freely, and it does NOT mention or rely on any internal per-scope filename suffix

#### Scenario: Layout documents the path-addressing rule for generic tools
- **WHEN** the compiled-in default template is rendered
- **THEN** the `<AGENTMEM:LAYOUT>` prose states that the listed subfolder paths are relative to the agents folder, that the wrapper tools add the agents-folder prefix automatically, and that the generic note tools (`write_memory_note`, `edit_memory_note`, `delete_memory_note`, `read_memory_note`) require the agents-folder name as the leading segment of a vault-root-relative path, including a worked example that contrasts a wrapper-built path with the equivalent generic-tool path
- **AND** the prose conveys this rule without hardcoding a specific agents-folder name, so it stays correct under any configured `AGENTMEM_AGENTS_DIR`

#### Scenario: Default template defers MEMORY.md organization to the agent
- **WHEN** the compiled-in default template is rendered
- **THEN** it does NOT prescribe an internal structure for `MEMORY.md`, leaving how to organize the index to the agent/user (subject only to the ≤ 200-line cap)

#### Scenario: Default template documents tool-managed files and caps
- **WHEN** the compiled-in default template is rendered
- **THEN** it instructs that diary entries are written with `append_diary_entry` and read with `read_memory_note` rather than hand-written, that the task heartbeat is updated via `update_task_heartbeat` to `HEARTBEAT.md`, that core root files are changed through `evolve_core_persona`, and that the documented caps are `USER.md` ≤ 100 lines and `MEMORY.md` ≤ 200 lines

#### Scenario: File and scope placeholders are distinct
- **WHEN** a template uses both `{{files.user}}` and `{{scope.user}}`
- **THEN** the former renders the contents of `USER.md` and the latter renders the `user` scope key value

#### Scenario: Memory file placeholder is recognised
- **WHEN** a template uses `{{files.memory}}`
- **THEN** the renderer substitutes the contents of `MEMORY.md` (or the missing sentinel when absent)

### Requirement: Session-context template resolution
The system SHALL resolve the active session-context template for a scope using a layered lookup, returning the first layer that exists: (1) a per-scope template file `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism inside the agents folder; (2) the global template file at the path configured by `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` (default `<root>/AGENT_SESSION_CONTEXT.md`); (3) the compiled-in default template. Absence of any layer SHALL never be an error.

#### Scenario: Per-scope template overrides global
- **WHEN** both a per-scope `AGENT_SESSION_CONTEXT.md` for the scope and a global template file exist
- **THEN** the renderer uses the per-scope template

#### Scenario: Global template used when no per-scope template
- **WHEN** no per-scope template exists for the scope but the global template file exists
- **THEN** the renderer uses the global template file

#### Scenario: Default used when nothing exists
- **WHEN** neither a per-scope template nor the global template file exists
- **THEN** the renderer uses the compiled-in default template

### Requirement: Note tools apply the link transform to content

`read_memory_note` SHALL strip the caller's own scope suffix from link targets in
returned content. `write_memory_note` and `append_diary_entry` SHALL expand link
targets in supplied content to their physical form before persisting, subject to
the cross-scope leak guard. These transforms SHALL apply the `wikilink-references`
rules and SHALL be transparent to a caller that uses only clean shortest names.

#### Scenario: Read strips the suffix from link targets
- **WHEN** `read_memory_note` returns an own-scope note whose persisted content
  contains `[[rust.jarvis.tony]]` for scope `{agent:"jarvis", user:"tony"}`
- **THEN** the returned `content` contains `[[rust]]`

#### Scenario: Write expands own-scope link targets
- **WHEN** `write_memory_note` is called for scope rendering to `jarvis.tony` with
  content containing `[[rust]]` resolving to the caller's own `rust.md`
- **THEN** the persisted file content contains `[[rust.jarvis.tony]]`

#### Scenario: Diary append expands link targets
- **WHEN** `append_diary_entry` is called with content containing `[[rust]]`
  resolving to the caller's own note
- **THEN** the appended diary content is persisted with the suffixed link form

### Requirement: Edit matches the persisted link form

`edit_memory_note` SHALL apply the write-side link transform to its
`search_string` and `replace_string` before matching against and writing the
physical file, so that a search containing a clean link target matches the
suffixed form stored on disk.

#### Scenario: Edit search containing a link matches on disk
- **WHEN** the persisted note contains `[[rust.jarvis.tony]]` and
  `edit_memory_note` is called for scope `jarvis.tony` with `search_string`
  containing `[[rust]]`
- **THEN** the search matches the stored line and the edit is applied (it does NOT
  fail with `edit_search_not_found`)

#### Scenario: Edit replacement is expanded
- **WHEN** `edit_memory_note` replaces a line with a `replace_string` containing
  `[[guide]]` resolving to the caller's own `guide.md`
- **THEN** the persisted content contains `[[guide.jarvis.tony]]`

### Requirement: Core-file tools apply the link transform

The core-file wrappers SHALL apply the same link transform as the generic note
tools. `evolve_core_persona` (PERSONA/PROMPT/RULES/USER/MEMORY) and
`update_task_heartbeat` (HEARTBEAT.md) SHALL expand link targets on write, and
`load_session_context` SHALL strip the caller's own suffix from the foundational
files it renders. Line caps SHALL be evaluated against the agent-facing content
(expansion does not change the line count).

#### Scenario: MEMORY.md index expands and renders clean
- **WHEN** `evolve_core_persona` writes `memory` content containing `[[rust]]`
  resolving to the caller's own note, for scope rendering to `jarvis.tony`
- **THEN** the persisted `MEMORY.md` contains `[[rust.jarvis.tony]]`, and a
  subsequent `load_session_context` renders the memory section with `[[rust]]`

#### Scenario: Heartbeat link expands on write
- **WHEN** `update_task_heartbeat` is called with content containing `[[rust]]`
  resolving to the caller's own note
- **THEN** the persisted `HEARTBEAT.md` contains the suffixed link form

### Requirement: `recall_memory_notes` tool registration
The system SHALL register a `recall_memory_notes` tool alongside the existing memory
tools whenever the recall backend is not `off`. Its scope extraction and visibility
semantics SHALL match `list_memory_notes` exactly: it returns only results the caller
could otherwise reach via `list_memory_notes` + `read_memory_note`, and never results
from another scope or from an ignored/hidden note.

#### Scenario: Tool is listed when recall is enabled
- **WHEN** the server starts with a recall backend other than `off`
- **THEN** `recall_memory_notes` appears in the tool listing alongside the existing
  memory tools, taking the same scope keys

#### Scenario: Visibility matches list_memory_notes
- **WHEN** `recall_memory_notes` and `list_memory_notes` are invoked for the same scope
  and policy
- **THEN** every path returned by `recall_memory_notes` is one that `list_memory_notes`
  would also return for that scope; no path outside that visible set ever appears

