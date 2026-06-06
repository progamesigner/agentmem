# memory-tools Specification

## Purpose
TBD - created by archiving change build-agentmem-mcp-server. Update Purpose after archive.
## Requirements
### Requirement: `list_memory_notes` tool
The system SHALL expose a `list_memory_notes` tool that returns a paginated set of virtual paths visible to a given scope, including both inside-agents-folder files belonging to that scope and outside-agents-folder files reachable under the active policy.

#### Scenario: Lists own-scope and outside files under namespaced policy
- **WHEN** the tool is invoked with the active scope, policy is `namespaced`, and the vault contains scope-owned files inside the agents folder plus human-authored files outside it
- **THEN** the response contains both sets, each entry represented as the clean virtual path the agent would use in subsequent calls

#### Scenario: Optional path prefix filter
- **WHEN** the tool is invoked with `path_prefix="topics"` and the agents folder is `Agents`
- **THEN** only entries whose virtual path begins with `topics` (under the agents folder) are returned

#### Scenario: Other scopes' files are hidden
- **WHEN** the tool is invoked with scope `{agent:"coder", user:"alice"}` and the vault also contains files for `coder.bob`
- **THEN** the `coder.bob` files do NOT appear in the response

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
The system SHALL expose a `read_memory_note` tool that returns the UTF-8 contents of a single file identified by its virtual path, subject to the active policy, region detection, and visibility filters.

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

### Requirement: `write_memory_note` tool
The system SHALL expose a `write_memory_note` tool that performs an atomic full-file write to a virtual path the active policy permits writing to.

#### Scenario: Write succeeds inside agents folder
- **WHEN** policy permits writes inside the agents folder (any policy other than `readonly`) and the tool is called with a virtual path inside it
- **THEN** the file is created or replaced via the atomic-write procedure and the response is a success result containing the byte count written

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

### Requirement: `edit_memory_note` tool
The system SHALL expose an `edit_memory_note` tool that takes a virtual path, a `search_string`, and a `replace_string`; replaces the unique occurrence of the search string with the replacement; and persists the result atomically. The search string MUST appear exactly once in the target file.

#### Scenario: Successful edit
- **WHEN** the tool is called and the search string appears exactly once in the target file
- **THEN** the server writes the modified file atomically and returns a success result indicating the number of characters replaced

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
The system SHALL expose a `delete_memory_note` tool that removes a single file at the given virtual path, subject to the active policy and own-scope rules. The tool SHALL NOT remove directories, and SHALL leave a parent directory in place even if it becomes empty.

#### Scenario: Delete succeeds for own-scope file under writable policy
- **WHEN** policy permits writes in the target's region and the tool is called for an own-scope file that exists
- **THEN** the file is removed via `std::fs::remove_file` and the response is a success result

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

### Requirement: `load_session_context` tool
The system SHALL expose a `load_session_context` tool that, in a single call for the active scope, returns the **rendered session-context** produced by the shared session-context renderer (see the *Session-context renderer* requirement). The tool SHALL accept only scope parameters. The response SHALL contain a `rendered` field holding the rendered markdown string and a `missing` list naming any of the five foundational files (`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md`) that did not exist for the scope at render time. The tool SHALL succeed even when no foundational files and no session-context template exist.

#### Scenario: Rendered context returned
- **WHEN** the tool is called for an active scope
- **THEN** the response contains a `rendered` markdown string produced by the renderer and a `missing` list naming any absent foundational file

#### Scenario: Some files missing
- **WHEN** only `PERSONA.md` and `RULES.md` exist for the scope
- **THEN** the `rendered` output substitutes the persona and rules contents, substitutes the missing sentinel for `PROMPT.md`, `USER.md`, and `TOOLS.md`, and `missing` names `PROMPT.md`, `USER.md`, `TOOLS.md`

#### Scenario: Empty vault still succeeds
- **WHEN** the tool is called for a scope with no foundational files and no session-context template present at any layer
- **THEN** the response is a success result whose `rendered` field is the compiled-in default template with all file slots showing the missing sentinel, and `missing` names all five foundational files

#### Scenario: No path argument
- **WHEN** a client attempts to pass a `path` or `which` argument
- **THEN** the call is rejected at schema validation because the input schema accepts only scope parameters

### Requirement: `evolve_core_persona` tool
The system SHALL expose an `evolve_core_persona` tool that performs an atomic full-file write to exactly one of the five foundational session files, selected by a required `which` parameter whose value is one of `persona`, `prompt`, `rules`, `user`, `tools`. The corresponding target file is the matching `.md` file (e.g. `which=persona` → `PERSONA.md`) resolved relative to the agents folder for the active scope.

#### Scenario: Persona update
- **WHEN** the tool is called with `which="persona"` and new content for the active scope
- **THEN** the scope's `PERSONA.md` is replaced atomically and the response is a success result containing the byte count written

#### Scenario: Prompt update
- **WHEN** the tool is called with `which="prompt"` and new content
- **THEN** the scope's `PROMPT.md` is replaced atomically

#### Scenario: Rules update
- **WHEN** the tool is called with `which="rules"` and new content
- **THEN** the scope's `RULES.md` is replaced atomically

#### Scenario: User update
- **WHEN** the tool is called with `which="user"` and new content
- **THEN** the scope's `USER.md` is replaced atomically

#### Scenario: Tools update
- **WHEN** the tool is called with `which="tools"` and new content
- **THEN** the scope's `TOOLS.md` is replaced atomically

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
The system SHALL expose an `update_task_heartbeat` tool whose target is hardcoded to the conventional virtual path `HEARTBEAT-STATE.md` (resolved relative to the agents folder) for the active scope and which performs an atomic full-file write.

#### Scenario: Heartbeat is replaced atomically
- **WHEN** the tool is called with new heartbeat content for the active scope
- **THEN** the resolved physical heartbeat file for that scope is replaced atomically and the response is a success result containing the byte count written

### Requirement: `append_diary_entry` tool
The system SHALL expose an `append_diary_entry` tool that appends a timestamped section to today's diary file at the virtual path `diary/<YYYY-MM-DD>.md` (resolved relative to the agents folder) for the active scope. The tool SHALL create the diary file (and its parent directories) if it does not exist. The append SHALL be implemented as a read-modify-write through the atomic-write procedure.

#### Scenario: Appends to existing diary
- **WHEN** the tool is called for scope `{agent:"coder", user:"alice"}` with `content="Picked up task #42."` at local time `14:03:22` on `2026-05-25`, and the scope's diary file for that date already contains prior sections
- **THEN** the server resolves the path to the scope's physical diary file, reads its current contents, appends `\n## 14:03:22\nPicked up task #42.\n`, and persists the result via the atomic-write procedure

#### Scenario: Creates diary on first entry of the day
- **WHEN** the tool is called for an active scope at local time `09:00:00` on `2026-05-25` and no file exists at the scope's `diary/2026-05-25.md` virtual path
- **THEN** the server creates any missing parent directories, writes a new file whose contents start with `## 09:00:00\n<content>\n`, and persists it via the atomic-write procedure

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
The system SHALL provide a single shared renderer that produces the session-context markdown for a given scope. The renderer SHALL resolve the session-context template (see *Session-context template resolution*), read the five foundational files for the scope, substitute every recognised placeholder, and return the resulting string together with the list of foundational files that were absent. The renderer SHALL be the single source of the rendered output exposed by the `load_session_context` tool, the `session-context` resource, and the `session-context` prompt.

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

### Requirement: Session-context template
The system SHALL treat the session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `tools` and `<key>` is a scheme placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default template that interleaves the foundational sections with memory-tool instructions so the server is useful with no operator configuration.

#### Scenario: Default template is self-contained
- **WHEN** no session-context template file exists at any resolution layer
- **THEN** the renderer uses the compiled-in default template, which includes a `{{tools_guide}}` section and a slot for each foundational file

#### Scenario: File and scope placeholders are distinct
- **WHEN** a template uses both `{{files.user}}` and `{{scope.user}}`
- **THEN** the former renders the contents of `USER.md` and the latter renders the `user` scope key value

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

