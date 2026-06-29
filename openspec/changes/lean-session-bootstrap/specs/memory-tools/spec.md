## MODIFIED Requirements

### Requirement: Session-context renderer
The system SHALL provide a single shared renderer that produces session-context markdown for a given scope and a **render kind** (`context` for the full render, `bootstrap` for the lean render). The renderer SHALL resolve the template for that kind (see *Session-context template resolution* and *Session-bootstrap template resolution*), read the five foundational files for the scope, substitute every recognised placeholder, and return the resulting string together with the list of foundational files that were absent. The renderer SHALL be the single source of the `context`-kind output exposed by the `load_session_context` tool, the `session-context` resource, the `session-context` prompt, and `GET /v1/context`, and of the `bootstrap`-kind output exposed by the `session-bootstrap` resource and `GET /v1/bootstrap`. The renderer SHALL NOT produce a `{{tools_guide}}` value; that slot and its generator are removed. The renderer SHALL produce a server-generated `{{scope_directive}}` value: a prominent, single-line imperative directing the agent that every memory tool call must carry the active scope. For a non-empty scope the directive SHALL name the scope keys as `key=value` pairs in deterministic key order (covering exactly the keys the configured scheme defines); for an empty scope it SHALL fall back to generic phrasing that names no specific key. The renderer SHALL also produce a server-generated `{{onboarding_directive}}` value: the empty string when no foundational file is absent, and otherwise a directive instructing the agent to interview the user and commit the foundational files via `evolve_core_persona`.

#### Scenario: Recognised placeholders are substituted
- **WHEN** the active template contains `{{files.persona}}`, `{{files.user}}`, `{{scope.agent}}`, and `{{scope_directive}}`
- **THEN** the renderer replaces them respectively with the contents of `PERSONA.md`, the contents of `USER.md`, the rendered value of the `agent` scope key, and the server-generated scope directive

#### Scenario: Missing foundational file renders a sentinel
- **WHEN** a `{{files.*}}` placeholder names a foundational file that does not exist for the scope
- **THEN** the renderer substitutes a fixed missing sentinel (for example `(not yet recorded — set via evolve_core_persona)`) rather than omitting the placeholder or erroring, and records that file in the absent list

#### Scenario: Unknown placeholder is left literal
- **WHEN** the template contains a `{{…}}` token the renderer does not recognise — including the now-removed `{{tools_guide}}`
- **THEN** the token is left verbatim in the output and a single diagnostic is logged; rendering does not error

#### Scenario: Scope directive names the concrete active scope
- **WHEN** `{{scope_directive}}` is rendered for a non-empty scope such as `{agent: default, user: swag}`
- **THEN** the value is a single-line imperative stating that every memory tool call must carry the active scope, naming the keys as `key=value` pairs in deterministic key order (for example `agent=default, user=swag`), covering exactly the keys the configured scheme defines

#### Scenario: Scope directive falls back to generic phrasing for an empty scope
- **WHEN** `{{scope_directive}}` is rendered for an empty scope
- **THEN** the value keeps the imperative that every memory tool call must carry the scope keys defined by the server's VFS scheme, without naming any specific key

#### Scenario: Onboarding directive is empty in steady state
- **WHEN** the renderer runs for a scope whose five foundational files all exist (the absent list is empty)
- **THEN** `{{onboarding_directive}}` renders as the empty string, so a steady-state session pays nothing for it

#### Scenario: Onboarding directive appears when files are missing
- **WHEN** the renderer runs for a scope with one or more absent foundational files
- **THEN** `{{onboarding_directive}}` renders the directive to interview the user and commit the foundational files via `evolve_core_persona`, in both the `context` and `bootstrap` renders

### Requirement: Session-context template
The system SHALL treat the (full) session-context template as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, `{{scope_directive}}`, and `{{onboarding_directive}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `memory` and `<key>` is a scheme placeholder. It SHALL NOT define a `{{tools_guide}}` placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default `context` template that delimits each section with XML-style tags rather than `##` headings, so that embedded foundational-file markdown (which typically begins at H2) does not collide with the template's own structure. The default template SHALL lead, directly under the `# Session Context` heading and before the first tag, with the `{{scope_directive}}` placeholder rendered as a **bare banner** (not wrapped in any XML tag). The default template SHALL wrap the foundational (agent-owned) slots in bare tags — `<PERSONA>{{files.persona}}</PERSONA>`, `<RULES>{{files.rules}}</RULES>`, `<MEMORY>{{files.memory}}</MEMORY>`, `<USER>{{files.user}}</USER>`, `<PROMPT>{{files.prompt}}</PROMPT>` — in the section order `PERSONA`, `RULES`, `MEMORY`, `USER`, `PROMPT` after the leading scope banner, followed by the `{{onboarding_directive}}` slot and a single-line pointer directing the agent to the layout surface (`agentmem://session-layout` / `GET /v1/layout`) for vault mechanics. The default `context` template SHALL NOT embed the `<AGENTMEM:TOOLS>` tools-guide section nor the `<AGENTMEM:LAYOUT>` prose; the layout guidance lives in the memory-layout capability. The default template SHALL leave the internal organization of `MEMORY.md` to the agent/user rather than prescribing a skeleton.

#### Scenario: Default context template leads with a bare scope banner
- **WHEN** the compiled-in default `context` template is rendered
- **THEN** the `{{scope_directive}}` banner appears directly under the `# Session Context` heading and before the `<PERSONA>` tag, rendered as bare markdown that is not enclosed in any XML tag

#### Scenario: Default context template no longer embeds tools guide or layout
- **WHEN** the compiled-in default `context` template is rendered
- **THEN** the output delimits its foundational sections with XML-style tags in the order `<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`, `<PROMPT>`, includes the `{{onboarding_directive}}` slot and a one-line pointer to the layout surface, and does NOT contain an `<AGENTMEM:TOOLS>` section, a `{{tools_guide}}` value, or the `<AGENTMEM:LAYOUT>` prose

#### Scenario: Sections are delimited by tags, not H2 headings
- **WHEN** the compiled-in default `context` template is rendered
- **THEN** each foundational file's contents are wrapped in a bare tag (`<PERSONA>…</PERSONA>`, `<RULES>…</RULES>`, `<MEMORY>…</MEMORY>`, `<USER>…</USER>`, `<PROMPT>…</PROMPT>`), so embedded H2 markdown in a foundational file does not collide with the template's section delimiters

#### Scenario: File and scope placeholders are distinct
- **WHEN** a template uses both `{{files.user}}` and `{{scope.user}}`
- **THEN** the former renders the contents of `USER.md` and the latter renders the `user` scope key value

#### Scenario: Memory file placeholder is recognised
- **WHEN** a template uses `{{files.memory}}`
- **THEN** the renderer substitutes the contents of `MEMORY.md` (or the missing sentinel when absent)

## ADDED Requirements

### Requirement: Session-bootstrap render and default template
The system SHALL provide a lean `bootstrap` render whose compiled-in default template contains, in order: the bare `{{scope_directive}}` banner directly under a `# Session Context` heading; the `<PERSONA>{{files.persona}}</PERSONA>` and `<RULES>{{files.rules}}</RULES>` slots; the `{{onboarding_directive}}` slot; and single-line literal pointers directing the agent to call `load_session_context` for the full context and to read the layout surface (`agentmem://session-layout` / `GET /v1/layout`) for vault mechanics. The default `bootstrap` template SHALL NOT include the `{{files.memory}}`, `{{files.user}}`, or `{{files.prompt}}` slots, the tools guide, or the layout prose. The `bootstrap` render SHALL report the same absent-foundational-files list as the `context` render for the same scope.

#### Scenario: Bootstrap render carries the lean core
- **WHEN** the `bootstrap` render is produced for a scope whose `PERSONA.md` and `RULES.md` exist
- **THEN** the output contains the bare scope banner, the persona and rules contents wrapped in `<PERSONA>` and `<RULES>` tags, and literal pointers to `load_session_context` and the layout surface

#### Scenario: Bootstrap render omits the heavier sections
- **WHEN** the compiled-in default `bootstrap` template is rendered
- **THEN** the output does NOT contain `<MEMORY>`, `<USER>`, or `<PROMPT>` slots, an `<AGENTMEM:TOOLS>` section, or the layout prose

#### Scenario: Bootstrap render surfaces the onboarding directive
- **WHEN** the `bootstrap` render is produced for a scope with one or more absent foundational files
- **THEN** the `{{onboarding_directive}}` slot renders the interview-and-`evolve_core_persona` directive; for a scope whose files all exist it renders empty

### Requirement: Session-bootstrap template resolution
The system SHALL resolve the active `bootstrap` template for a scope using a layered lookup, returning the first layer that exists: (1) a per-scope template file `AGENT_SESSION_BOOTSTRAP.md` resolved through the scope suffix mechanism inside the agents folder; (2) the global template file at the path configured by `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` (default `<root>/AGENT_SESSION_BOOTSTRAP.md`); (3) the compiled-in default `bootstrap` template. Absence of any layer SHALL never be an error.

#### Scenario: Per-scope bootstrap template overrides global
- **WHEN** both a per-scope `AGENT_SESSION_BOOTSTRAP.md` for the scope and a global bootstrap template file exist
- **THEN** the renderer uses the per-scope template

#### Scenario: Default bootstrap template when nothing exists
- **WHEN** neither a per-scope bootstrap template nor the global bootstrap template file exists
- **THEN** the renderer uses the compiled-in default `bootstrap` template

### Requirement: Memory-layout render and default content
The system SHALL provide a layout render for a scope that resolves the layout template (see *Memory-layout template resolution*) and renders it through the template engine with the scope context, so `{{scope.<key>}}` placeholders in an operator-supplied layout resolve (the compiled-in default contains none). The default layout content SHALL carry the vault-mechanics guidance formerly embedded in the session-context `<AGENTMEM:LAYOUT>` section: the suggested (non-enforced) memory layout with each entry's purpose (root core files `MEMORY.md`, `RULES.md`, `PERSONA.md`, `PROMPT.md`, `USER.md`, `HEARTBEAT.md`; and subfolders `diary/<YYYY-MM-DD>.md`, `workspaces/INDEX.md` + `workspaces/<project>/<item>.md`, `topics/INDEX.md` + `topics/LOG.md` + `topics/<topic>/<fact>.md`, `skills/<skill>/SKILL.md` + `skills/<skill>/references/<name>.md`, `agents/<subagent>/PROMPT.md` + `agents/<subagent>/<context>.md`); the distinction between **core files** (changed only through the dedicated wrapper tools and subject to the documented caps) and all other paths (an ordinary filesystem the agent reads, writes, and organizes freely), without exposing any internal per-scope filename-suffix mechanism; the path-addressing rule that wrapper tools prepend the agents-folder name automatically while the generic note tools require the agents-folder name as the leading segment of a vault-root-relative path, conveyed without hardcoding a specific agents-folder name (so it stays correct under any `AGENTMEM_AGENTS_DIR`); the tool-managed-file instructions; and the documented caps `USER.md` ≤ 100 lines and `MEMORY.md` ≤ 200 lines. The default layout content SHALL NOT include the missing-files onboarding guidance — that is the renderer's `{{onboarding_directive}}`.

#### Scenario: Layout presents the suggested layout with purposes
- **WHEN** the compiled-in default layout is rendered
- **THEN** it lists the suggested core files and subfolders with each entry's purpose, as non-enforced guidance

#### Scenario: Layout distinguishes core files from a free-form filesystem
- **WHEN** the compiled-in default layout is rendered
- **THEN** it states that core files are changed only through their dedicated wrapper tools and are subject to the documented caps, while every other path behaves like an ordinary filesystem, and it does NOT mention or rely on any internal per-scope filename suffix

#### Scenario: Layout documents the path-addressing rule without hardcoding the agents folder
- **WHEN** the compiled-in default layout is rendered
- **THEN** it states that wrapper tools add the agents-folder prefix automatically and that the generic note tools require the agents-folder name as the leading segment of a vault-root-relative path, with a worked example, and it conveys this without hardcoding a specific agents-folder name so it stays correct under any `AGENTMEM_AGENTS_DIR`

#### Scenario: Layout omits the onboarding guidance
- **WHEN** the compiled-in default layout is rendered
- **THEN** it does NOT contain the missing-files interview/`evolve_core_persona` guidance, which is rendered instead by the `{{onboarding_directive}}` in the context and bootstrap renders

### Requirement: Memory-layout template resolution
The system SHALL resolve the active layout template for a scope using a layered lookup, returning the first layer that exists: (1) a per-scope template file `AGENT_MEMORY_LAYOUT.md` resolved through the scope suffix mechanism inside the agents folder; (2) the global template file at the path configured by `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` (default `<root>/AGENT_MEMORY_LAYOUT.md`); (3) the compiled-in default layout content. Absence of any layer SHALL never be an error.

#### Scenario: Per-scope layout overrides global
- **WHEN** both a per-scope `AGENT_MEMORY_LAYOUT.md` for the scope and a global layout template file exist
- **THEN** the renderer uses the per-scope layout

#### Scenario: Default layout when nothing exists
- **WHEN** neither a per-scope layout nor the global layout template file exists
- **THEN** the renderer uses the compiled-in default layout content
