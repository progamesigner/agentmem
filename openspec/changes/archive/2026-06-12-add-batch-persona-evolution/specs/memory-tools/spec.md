# memory-tools delta: add-batch-persona-evolution

## MODIFIED Requirements

### Requirement: `evolve_core_persona` tool
The system SHALL expose an `evolve_core_persona` tool that performs atomic full-file writes to the five foundational session files, accepting exactly one of two argument forms per call. The **single form** takes a required `which` parameter whose value is one of `persona`, `prompt`, `rules`, `user`, `memory`, plus the new `content`; the **batch form** takes an `updates` array of 1 to 5 `{ which, content }` entries with the same `which` domain and no duplicate `which` values. Supplying neither form, both forms, an empty `updates` array, or a duplicate `which` SHALL be rejected with `invalid_argument`. The corresponding target file for each entry is the matching `.md` file (e.g. `which=persona` → `PERSONA.md`, `which=memory` → `MEMORY.md`) resolved relative to the agents folder for the active scope.

The tool SHALL enforce a hard line-count cap on the content for the capped files: `which=user` content MUST NOT exceed 100 lines and `which=memory` content MUST NOT exceed 200 lines (counted as newline-separated lines). In the batch form, every entry SHALL be validated — `which` domain, duplicates, line caps, and the write-side link transform — before any file is written; a failing entry SHALL reject the whole call and leave every foundational file unchanged. After validation, each selected file SHALL be replaced atomically. The single-form response is unchanged (the byte count written); the batch-form response SHALL carry a `results` array of `{ which, bytes_written }` in request order. The batch is not transactional across files: a crash mid-apply may leave a prefix of the entries applied, but never a partially written single file.

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

#### Scenario: Batch update writes several foundational files in one call
- **WHEN** the tool is called with `updates=[{which:"persona",…},{which:"user",…},{which:"memory",…}]`, every entry within its cap
- **THEN** `PERSONA.md`, `USER.md`, and `MEMORY.md` are each replaced atomically and the response carries `results` with one `{ which, bytes_written }` entry per update, in request order

#### Scenario: One over-cap entry rejects the whole batch
- **WHEN** the tool is called with `updates` containing a valid `persona` entry and a `user` entry of 101 lines
- **THEN** the response is an MCP error with code `invalid_argument` naming the 100-line limit, and neither `PERSONA.md` nor `USER.md` is changed

#### Scenario: Duplicate which in a batch is rejected
- **WHEN** the tool is called with `updates` containing two entries with `which="rules"`
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed

#### Scenario: Exactly one argument form
- **WHEN** the tool is called with both the single `which`/`content` pair and an `updates` array, or with neither
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed

#### Scenario: User content over the line cap is rejected
- **WHEN** the tool is called with `which="user"` and content exceeding 100 lines
- **THEN** the response is an MCP error with code `invalid_argument`, the message states the 100-line limit, and `USER.md` is unchanged

#### Scenario: Memory content over the line cap is rejected
- **WHEN** the tool is called with `which="memory"` and content exceeding 200 lines
- **THEN** the response is an MCP error with code `invalid_argument`, the message states the 200-line limit, and `MEMORY.md` is unchanged

#### Scenario: Invalid `which`
- **WHEN** the tool is called with `which` (in either form) set to any value other than the five accepted strings
- **THEN** the call is rejected at schema validation (the schema's `which` fields are enums)

#### Scenario: Path argument is rejected
- **WHEN** a client attempts to pass a `path` argument to override the hardcoded targets
- **THEN** the call is rejected at schema validation because the input schema does NOT include a path field

#### Scenario: Refused under readonly policy
- **WHEN** policy is `readonly` and `evolve_core_persona` is invoked with any valid form
- **THEN** the response is an MCP error with code `write_denied` and no file is changed

## ADDED Requirements

### Requirement: Persona interview guidance
The rendered session-context (the server-generated memory-tools guide and the missing-file sentinel text) SHALL direct an agent whose foundational files are missing to follow an interview-then-commit flow: ask the user as many questions as needed to understand identity, role, working style, and boundaries **before** writing; then distill the answers into the agent's own concise wording — phrased for fast comprehension by future agent sessions rather than as a verbatim transcript of the user's input — and commit all affected foundational files in a single batch `evolve_core_persona` call.

#### Scenario: Rendered guide describes the batch interview flow
- **WHEN** `load_session_context` renders for a scope with missing foundational files
- **THEN** the rendered output instructs the agent to gather answers first and write the missing files in one `evolve_core_persona` call with multiple `updates`, distilled into the agent's own words rather than the user's verbatim phrasing

#### Scenario: Tool description advertises the batch form
- **WHEN** the tool listing is requested
- **THEN** `evolve_core_persona`'s description names both the single `which`/`content` form and the `updates` batch form
