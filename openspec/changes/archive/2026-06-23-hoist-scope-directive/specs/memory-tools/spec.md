## MODIFIED Requirements

### Requirement: Session-context renderer
The system SHALL provide a single shared renderer that produces the session-context markdown for a given scope. The renderer SHALL resolve the session-context template (see *Session-context template resolution*), read the five foundational files for the scope, substitute every recognised placeholder, and return the resulting string together with the list of foundational files that were absent. The renderer SHALL be the single source of the rendered output exposed by the `load_session_context` tool, the `session-context` resource, and the `session-context` prompt. The server-generated memory-tools guide produced for `{{tools_guide}}` SHALL name the concrete active scope as `key=value` pairs so the agent knows exactly which scope keys and values to carry on every tool call. The renderer SHALL also produce a server-generated `{{scope_directive}}` value: a prominent, single-line imperative directing the agent that every memory tool call must carry the active scope. For a non-empty scope the directive SHALL name the scope keys as `key=value` pairs in deterministic key order (covering exactly the keys the configured scheme defines); for an empty scope it SHALL fall back to generic phrasing that names no specific key. The `key=value` formatting and ordering used by `{{scope_directive}}` and `{{tools_guide}}` SHALL derive from a single shared source so the two cannot drift.

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

#### Scenario: Scope directive names the concrete active scope
- **WHEN** `{{scope_directive}}` is rendered for a non-empty scope such as `{agent: default, user: swag}`
- **THEN** the value is a single-line imperative stating that every memory tool call must carry the active scope, naming the keys as `key=value` pairs in deterministic key order (for example `agent=default, user=swag`), covering exactly the keys the configured scheme defines

#### Scenario: Scope directive falls back to generic phrasing for an empty scope
- **WHEN** `{{scope_directive}}` is rendered for an empty scope
- **THEN** the value keeps the imperative that every memory tool call must carry the scope keys defined by the server's VFS scheme, without naming any specific key

#### Scenario: Scope directive and tools guide share one keys source
- **WHEN** both `{{scope_directive}}` and `{{tools_guide}}` are rendered for the same non-empty scope
- **THEN** the `key=value` list each emits is identical in content and ordering, because both derive it from a single shared helper

### Requirement: Session-context template
The system SHALL treat the session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, `{{scope_directive}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `memory` and `<key>` is a scheme placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default template that delimits each section with XML-style tags rather than `##` headings, so that embedded foundational-file markdown (which typically begins at H2) does not collide with the template's own structure. The default template SHALL lead, directly under the `# Session Context` heading and before the first tag, with the `{{scope_directive}}` placeholder rendered as a **bare banner** (not wrapped in any XML tag), so the active scope keys appear in the first portion of the document and survive both byte-budget truncation and tag-stripping by a consuming harness. The default template SHALL wrap the foundational (agent-owned) slots in bare tags — `<PERSONA>{{files.persona}}</PERSONA>`, `<RULES>{{files.rules}}</RULES>`, `<MEMORY>{{files.memory}}</MEMORY>`, `<USER>{{files.user}}</USER>`, `<PROMPT>{{files.prompt}}</PROMPT>` — and the server-generated slots in `AGENTMEM:`-namespaced tags — `<AGENTMEM:TOOLS>{{tools_guide}}</AGENTMEM:TOOLS>` and `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>` — preserving the section order `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT`, `AGENTMEM:TOOLS`, `AGENTMEM:LAYOUT` after the leading scope banner. The `<AGENTMEM:LAYOUT>` section SHALL embed a **suggested (non-enforced) memory layout** and the documented line caps as prose so the server is useful with no operator configuration. The suggested layout is illustrative guidance for the agent, not a constraint the server validates (only the wrapper-only-roots rule and the line caps are enforced). The layout prose SHALL distinguish the small set of **core files** that have special handling (changed only through the dedicated wrapper tools, and subject to the line caps) from all other paths, which behave like an ordinary filesystem the agent reads, writes, and organizes freely; it SHALL NOT expose any internal per-scope filename-suffix mechanism. The layout prose SHALL also state the path-addressing rule for the subfolder conventions: the listed subfolder paths are shown **relative to the agents folder**, the wrapper tools (`append_diary_entry`, `evolve_core_persona`, `update_task_heartbeat`) prepend the agents-folder name automatically, and the generic note tools (`write_memory_note`, `edit_memory_note`, `delete_memory_note`, `read_memory_note`) take the full **vault-root-relative** path and therefore require the agents-folder name as the leading segment; the prose SHALL convey this without hardcoding a specific agents-folder name (so it remains correct under any `AGENTMEM_AGENTS_DIR`). The default template SHALL leave the internal organization of `MEMORY.md` to the agent/user rather than prescribing a skeleton. Because the conventions live in the default template (not in `{{tools_guide}}`), an operator who supplies their own template fully controls and may override them.

#### Scenario: Default template leads with a bare scope banner
- **WHEN** the compiled-in default template is rendered
- **THEN** the `{{scope_directive}}` banner appears directly under the `# Session Context` heading and before the `<PERSONA>` tag, rendered as bare markdown that is not enclosed in any XML tag

#### Scenario: Default template is self-contained
- **WHEN** no session-context template file exists at any resolution layer
- **THEN** the renderer uses the compiled-in default template, which leads with the bare `{{scope_directive}}` banner and then delimits its sections with XML-style tags in the order `<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`, `<PROMPT>`, `<AGENTMEM:TOOLS>`, `<AGENTMEM:LAYOUT>`, and which includes the memory-organization conventions and the documented `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 line caps

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
