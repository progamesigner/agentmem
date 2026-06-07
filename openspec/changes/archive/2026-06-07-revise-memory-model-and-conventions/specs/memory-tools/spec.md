## MODIFIED Requirements

### Requirement: `write_memory_note` tool
The system SHALL expose a `write_memory_note` tool that performs an atomic full-file write to a virtual path the active policy permits writing to. Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path (a path with no subfolder segment beneath the per-scope root) is reserved for the dedicated wrapper tools (`evolve_core_persona`, `update_task_heartbeat`) and SHALL be rejected.

#### Scenario: Write succeeds inside agents folder
- **WHEN** policy permits writes inside the agents folder (any policy other than `readonly`) and the tool is called with a virtual path inside a subfolder of it (e.g. `topics/auth/jwt.md`)
- **THEN** the file is created or replaced via the atomic-write procedure and the response is a success result containing the byte count written

#### Scenario: Write to a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `MEMORY.md`, `USER.md`, or `PERSONA.md`)
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

### Requirement: `edit_memory_note` tool
The system SHALL expose an `edit_memory_note` tool that takes a virtual path, a `search_string`, and a `replace_string`; replaces the unique occurrence of the search string with the replacement; and persists the result atomically. The search string MUST appear exactly once in the target file. Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path is reserved for the dedicated wrapper tools and SHALL be rejected.

#### Scenario: Successful edit
- **WHEN** the tool is called and the search string appears exactly once in the target file
- **THEN** the server writes the modified file atomically and returns a success result indicating the number of characters replaced

#### Scenario: Edit of a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `MEMORY.md`)
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
The system SHALL expose a `delete_memory_note` tool that removes a single file at the given virtual path, subject to the active policy and own-scope rules. The tool SHALL NOT remove directories, and SHALL leave a parent directory in place even if it becomes empty. Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level core file SHALL NOT be deletable through this tool.

#### Scenario: Delete succeeds for own-scope file under writable policy
- **WHEN** policy permits writes in the target's region and the tool is called for an own-scope file under a subfolder that exists
- **THEN** the file is removed via `std::fs::remove_file` and the response is a success result

#### Scenario: Delete of a root core file is rejected
- **WHEN** the tool is called with an agents-folder root-level virtual path (e.g. `PERSONA.md`)
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
- **WHEN** the tool is called for scope `{agent:"coder", user:"alice"}` with `content="Picked up task #42."` and `title="Task pickup"` at local time `14:03:22` on `2026-05-25`, and the scope's diary file for that date already contains prior sections
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

### Requirement: Session-context template
The system SHALL treat the session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `memory` and `<key>` is a scheme placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default template that orders the foundational sections as `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT` followed by the `{{tools_guide}}` slot, and that embeds a **suggested (non-enforced) memory layout** and the documented line caps as prose so the server is useful with no operator configuration. The suggested layout is illustrative guidance for the agent, not a constraint the server validates (only the wrapper-only-roots rule and the line caps are enforced). The default template SHALL leave the internal organization of `MEMORY.md` to the agent/user rather than prescribing a skeleton. Because the conventions live in the default template (not in `{{tools_guide}}`), an operator who supplies their own template fully controls and may override them.

#### Scenario: Default template is self-contained
- **WHEN** no session-context template file exists at any resolution layer
- **THEN** the renderer uses the compiled-in default template, which orders the sections `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT`, then a `{{tools_guide}}` section, and which includes the memory-organization conventions and the documented `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 line caps

#### Scenario: Default template documents the suggested layout
- **WHEN** the compiled-in default template is rendered
- **THEN** it presents the suggested (non-enforced) layout with each entry's purpose: root core files `MEMORY.md` (working-memory index), `RULES.md` (safety boundaries), `PERSONA.md` (identity/soul/style), `PROMPT.md` (workflow rules, plus external-tool facts such as camera/SSH details), `USER.md` (user profile), `HEARTBEAT.md` (task heartbeat); and subfolders `diary/<YYYY-MM-DD>.md`, `workspaces/INDEX.md` + `workspaces/<project>/<item>.md`, `topics/INDEX.md` + `topics/LOG.md` + `topics/<topic>/<fact>.md`, `skills/<skill>/SKILL.md` + `skills/<skill>/references/<name>.md`, and `agents/<subagent>/PROMPT.md` + `agents/<subagent>/<context>.md`

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
