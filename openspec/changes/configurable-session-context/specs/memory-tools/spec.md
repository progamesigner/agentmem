## MODIFIED Requirements

### Requirement: `load_session_context` tool
The system SHALL expose a `load_session_context` tool that, in a single call for the active scope, returns the **rendered session-context** produced by the shared session-context renderer (see the *Session-context renderer* requirement). The tool SHALL accept only scope parameters. The response SHALL contain a `rendered` field holding the rendered markdown string and a `missing` list naming any of the five foundational files (`PERSONA.md`, `PROMPT.md`, `RULES.md`, `USER.md`, `TOOLS.md`) that did not exist for the scope at render time. The tool SHALL succeed even when no foundational files and no session-context layout exist.

#### Scenario: Rendered context returned
- **WHEN** the tool is called for an active scope
- **THEN** the response contains a `rendered` markdown string produced by the renderer and a `missing` list naming any absent foundational file

#### Scenario: Some files missing
- **WHEN** only `PERSONA.md` and `RULES.md` exist for the scope
- **THEN** the `rendered` output substitutes the persona and rules contents, substitutes the missing sentinel for `PROMPT.md`, `USER.md`, and `TOOLS.md`, and `missing` names `PROMPT.md`, `USER.md`, `TOOLS.md`

#### Scenario: Empty vault still succeeds
- **WHEN** the tool is called for a scope with no foundational files and no session-context layout present at any layer
- **THEN** the response is a success result whose `rendered` field is the compiled-in default layout with all file slots showing the missing sentinel, and `missing` names all five foundational files

#### Scenario: No path argument
- **WHEN** a client attempts to pass a `path` or `which` argument
- **THEN** the call is rejected at schema validation because the input schema accepts only scope parameters

## ADDED Requirements

### Requirement: Session-context renderer
The system SHALL provide a single shared renderer that produces the session-context markdown for a given scope. The renderer SHALL resolve the session-context layout (see *Session-context layout resolution*), read the five foundational files for the scope, substitute every recognised placeholder, and return the resulting string together with the list of foundational files that were absent. The renderer SHALL be the single source of the rendered output exposed by the `load_session_context` tool, the `session-context` resource template, and the `session-context` prompt.

#### Scenario: Recognised placeholders are substituted
- **WHEN** the active layout contains `{{files.persona}}`, `{{files.user}}`, `{{scope.agent}}`, and `{{tools_guide}}`
- **THEN** the renderer replaces them respectively with the contents of `PERSONA.md`, the contents of `USER.md`, the rendered value of the `agent` scope key, and the server-generated memory-tools guide

#### Scenario: Missing foundational file renders a sentinel
- **WHEN** a `{{files.*}}` placeholder names a foundational file that does not exist for the scope
- **THEN** the renderer substitutes a fixed missing sentinel (for example `(not yet recorded — set via evolve_core_persona)`) rather than omitting the placeholder or erroring, and records that file in the absent list

#### Scenario: Unknown placeholder is left literal
- **WHEN** the layout contains a `{{…}}` token the renderer does not recognise
- **THEN** the token is left verbatim in the output and a single diagnostic is logged; rendering does not error

#### Scenario: Tools guide reflects the live tool set
- **WHEN** `{{tools_guide}}` is rendered
- **THEN** its content is generated from the server's live tool catalogue so that the names and usage it describes always match the tools currently advertised

### Requirement: Session-context layout
The system SHALL treat the session-context layout as an operator-authored markdown document that may contain `{{files.<name>}}`, `{{scope.<key>}}`, and `{{tools_guide}}` placeholders, where `<name>` is one of `persona`, `prompt`, `rules`, `user`, `tools` and `<key>` is a VFS template placeholder. The placeholder namespace SHALL keep file contents (`{{files.user}}`) distinct from scope values (`{{scope.user}}`). The system SHALL ship a compiled-in default layout that interleaves the foundational sections with memory-tool instructions so the server is useful with no operator configuration.

#### Scenario: Default layout is self-contained
- **WHEN** no session-context layout file exists at any resolution layer
- **THEN** the renderer uses the compiled-in default layout, which includes a `{{tools_guide}}` section and a slot for each foundational file

#### Scenario: File and scope placeholders are distinct
- **WHEN** a layout uses both `{{files.user}}` and `{{scope.user}}`
- **THEN** the former renders the contents of `USER.md` and the latter renders the `user` scope key value

### Requirement: Session-context layout resolution
The system SHALL resolve the active session-context layout for a scope using a layered lookup, returning the first layer that exists: (1) a per-scope layout file `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism inside the agents folder; (2) the global layout file at the path configured by `AGENTMEM_SESSION_CONTEXT_FILE` (default `<root>/AGENT_SESSION_CONTEXT.md`); (3) the compiled-in default layout. Absence of any layer SHALL never be an error.

#### Scenario: Per-scope layout overrides global
- **WHEN** both a per-scope `AGENT_SESSION_CONTEXT.md` for the scope and a global layout file exist
- **THEN** the renderer uses the per-scope layout

#### Scenario: Global layout used when no per-scope layout
- **WHEN** no per-scope layout exists for the scope but the global layout file exists
- **THEN** the renderer uses the global layout file

#### Scenario: Default used when nothing exists
- **WHEN** neither a per-scope layout nor the global layout file exists
- **THEN** the renderer uses the compiled-in default layout
