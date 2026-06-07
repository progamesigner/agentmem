## MODIFIED Requirements

### Requirement: Session-context template
The system SHALL treat the session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `memory` and `<key>` is a scheme placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default template that delimits each section with XML-style tags rather than `##` headings, so that embedded foundational-file markdown (which typically begins at H2) does not collide with the template's own structure. The default template SHALL wrap the foundational (agent-owned) slots in bare tags — `<PERSONA>{{files.persona}}</PERSONA>`, `<RULES>{{files.rules}}</RULES>`, `<MEMORY>{{files.memory}}</MEMORY>`, `<USER>{{files.user}}</USER>`, `<PROMPT>{{files.prompt}}</PROMPT>` — and the server-generated slots in `AGENTMEM:`-namespaced tags — `<AGENTMEM:TOOLS>{{tools_guide}}</AGENTMEM:TOOLS>` and `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>` — preserving the section order `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT`, `AGENTMEM:TOOLS`, `AGENTMEM:LAYOUT`. The `<AGENTMEM:LAYOUT>` section SHALL embed a **suggested (non-enforced) memory layout** and the documented line caps as prose so the server is useful with no operator configuration. The suggested layout is illustrative guidance for the agent, not a constraint the server validates (only the wrapper-only-roots rule and the line caps are enforced). The layout prose SHALL distinguish the small set of **core files** that have special handling (changed only through the dedicated wrapper tools, and subject to the line caps) from all other paths, which behave like an ordinary filesystem the agent reads, writes, and organizes freely; it SHALL NOT expose any internal per-scope filename-suffix mechanism. The default template SHALL leave the internal organization of `MEMORY.md` to the agent/user rather than prescribing a skeleton. Because the conventions live in the default template (not in `{{tools_guide}}`), an operator who supplies their own template fully controls and may override them.

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
