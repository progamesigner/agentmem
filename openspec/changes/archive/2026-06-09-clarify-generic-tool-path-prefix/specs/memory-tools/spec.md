## MODIFIED Requirements

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

### Requirement: `write_memory_note` tool
The system SHALL expose a `write_memory_note` tool that performs an atomic full-file write to a virtual path the active policy permits writing to. The `path` argument is a **vault-root-relative** virtual path; to target a location inside the agents folder the caller MUST include the agents-folder name as the leading segment (the dedicated wrapper tools do this automatically; the generic tools do not). Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path (a path with no subfolder segment beneath the per-scope root) is reserved for the dedicated wrapper tools (`evolve_core_persona`, `update_task_heartbeat`) and SHALL be rejected.

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
