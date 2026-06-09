## MODIFIED Requirements

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
- **WHEN** `{{tools_guide}}` is rendered for a non-empty scope such as `{agent: coder, user: alice}`
- **THEN** the guide states that every call must carry those scope keys and lists them as `key=value` pairs in deterministic key order (for example `agent=coder, user=alice`), covering exactly the keys the configured scheme defines

#### Scenario: Tools guide falls back to generic phrasing for an empty scope
- **WHEN** `{{tools_guide}}` is rendered for an empty scope
- **THEN** the guide retains the generic instruction that every call must carry the scope keys defined by the server's VFS scheme, without naming any specific key
