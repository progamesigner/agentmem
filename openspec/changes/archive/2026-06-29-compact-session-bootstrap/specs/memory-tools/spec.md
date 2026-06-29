## MODIFIED Requirements

### Requirement: Session-bootstrap render and default template
The system SHALL provide a lean `bootstrap` render whose compiled-in default template is compact and ordered server-owned-content-first, containing, in order: a `# Session Bootstrap` heading (distinct from the full `context` render's `# Session Context` heading, so the two surfaces are visibly different documents); the bare `{{scope_directive}}` banner; a single-line pointer directing the agent to call `load_session_context` for the rest of the foundational context (persona, working memory, user profile, workflow prompt) and to read the layout surface (`agentmem://session-layout` / `GET /v1/layout`) for vault mechanics; the `{{onboarding_directive}}` slot; and finally the `{{files.rules}}` slot rendered without any wrapping tag, as the last content in the document. The default `bootstrap` template SHALL NOT include the `{{files.persona}}`, `{{files.memory}}`, `{{files.user}}`, or `{{files.prompt}}` slots, any `<PERSONA>`/`<RULES>` wrapper tag, the tools guide, the layout prose, or any server-defined memory-loop or recall/diary directive. The `bootstrap` render SHALL report the same absent-foundational-files list as the `context` render for the same scope.

#### Scenario: Bootstrap render carries the compact core in order
- **WHEN** the `bootstrap` render is produced for a scope whose `RULES.md` exists
- **THEN** the output begins with the `# Session Bootstrap` heading, followed by the bare scope banner and a single-line pointer to `load_session_context` and the layout surface, and the `RULES.md` contents appear last with no wrapping tag

#### Scenario: Bootstrap render omits persona and the heavier sections
- **WHEN** the compiled-in default `bootstrap` template is rendered
- **THEN** the output does NOT contain a `<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`, or `<PROMPT>` tag, does NOT contain the persona contents, and does NOT contain an `<AGENTMEM:TOOLS>` section or the layout prose

#### Scenario: Bootstrap heading is distinct from the full context render
- **WHEN** the compiled-in default `bootstrap` template and the compiled-in default `context` template are each rendered for the same scope
- **THEN** the `bootstrap` render leads with `# Session Bootstrap` and the `context` render leads with `# Session Context`

#### Scenario: Rules are the final content
- **WHEN** the `bootstrap` render is produced for a scope whose `RULES.md` exists
- **THEN** the `RULES.md` contents appear after the scope banner, the `load_session_context`/layout pointer, and the `{{onboarding_directive}}` slot, with no template content following them

#### Scenario: Bootstrap carries no server-defined memory loop
- **WHEN** the compiled-in default `bootstrap` template is rendered for any scope
- **THEN** the output contains no server-authored recall/capture/diary directive — any such memory-discipline guidance present comes solely from the inlined `RULES.md` contents

#### Scenario: Bootstrap render surfaces the onboarding directive
- **WHEN** the `bootstrap` render is produced for a scope with one or more absent foundational files
- **THEN** the `{{onboarding_directive}}` slot renders the interview-and-`evolve_core_persona` directive; for a scope whose files all exist it renders empty

### Requirement: `evolve_core_persona` tool
The system SHALL expose an `evolve_core_persona` tool that performs atomic full-file writes to the five foundational session files, accepting exactly one of two argument forms per call. The **single form** takes a required `which` parameter whose value is one of `persona`, `prompt`, `rules`, `user`, `memory`, plus the new `content`; the **batch form** takes an `updates` array of 1 to 5 `{ which, content }` entries with the same `which` domain and no duplicate `which` values. Supplying neither form, both forms, an empty `updates` array, or a duplicate `which` SHALL be rejected with `invalid_argument`. The corresponding target file for each entry is the matching `.md` file (e.g. `which=persona` → `PERSONA.md`, `which=memory` → `MEMORY.md`) resolved relative to the agents folder for the active scope.

The tool SHALL enforce a hard line-count cap on the content for the capped files: `which=rules` content MUST NOT exceed 40 lines, `which=user` content MUST NOT exceed 100 lines, and `which=memory` content MUST NOT exceed 200 lines (counted as newline-separated lines). In the batch form, every entry SHALL be validated — `which` domain, duplicates, line caps, and the write-side link transform — before any file is written; a failing entry SHALL reject the whole call and leave every foundational file unchanged. After validation, each selected file SHALL be replaced atomically. The single-form response is unchanged (the byte count written); the batch-form response SHALL carry a `results` array of `{ which, bytes_written }` in request order. The batch is not transactional across files: a crash mid-apply may leave a prefix of the entries applied, but never a partially written single file.

#### Scenario: Persona update
- **WHEN** the tool is called with `which="persona"` and new content for the active scope
- **THEN** the scope's `PERSONA.md` is replaced atomically and the response is a success result containing the byte count written

#### Scenario: Prompt update
- **WHEN** the tool is called with `which="prompt"` and new content
- **THEN** the scope's `PROMPT.md` is replaced atomically

#### Scenario: Rules update within cap
- **WHEN** the tool is called with `which="rules"` and content of 40 lines or fewer
- **THEN** the scope's `RULES.md` is replaced atomically

#### Scenario: Rules over cap rejected
- **WHEN** the tool is called with `which="rules"` and content of 41 lines
- **THEN** the response is an MCP error with code `invalid_argument` naming the 40-line limit, and `RULES.md` is not changed

#### Scenario: User update within cap
- **WHEN** the tool is called with `which="user"` and content of 100 lines or fewer
- **THEN** the scope's `USER.md` is replaced atomically

#### Scenario: Memory update within cap
- **WHEN** the tool is called with `which="memory"` and content of 200 lines or fewer
- **THEN** the scope's `MEMORY.md` is replaced atomically

#### Scenario: Batch update writes several foundational files in one call
- **WHEN** the tool is called with `updates=[{which:"persona",…},{which:"user",…},{which:"memory",…}]`, every entry within its cap
- **THEN** `PERSONA.md`, `USER.md`, and `MEMORY.md` are each replaced atomically and the response carries `results` with one `{ which, bytes_written }` entry per update, in request order

#### Scenario: One over-cap entry rejects the whole batch
- **WHEN** the tool is called with `updates` containing a valid `persona` entry and a `rules` entry of 41 lines
- **THEN** the response is an MCP error with code `invalid_argument` naming the 40-line limit, and neither `PERSONA.md` nor `RULES.md` is changed

#### Scenario: Duplicate which in a batch is rejected
- **WHEN** the tool is called with `updates` containing two entries with `which="rules"`
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed

#### Scenario: Exactly one argument form
- **WHEN** the tool is called with both the single `which`/`content` pair and an `updates` array, or with neither
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed
