## ADDED Requirements

### Requirement: Note tools apply the link transform to content

`read_memory_note` SHALL strip the caller's own scope suffix from link targets in
returned content. `write_memory_note` and `append_diary_entry` SHALL expand link
targets in supplied content to their physical form before persisting, subject to
the cross-scope leak guard. These transforms SHALL apply the `wikilink-references`
rules and SHALL be transparent to a caller that uses only clean shortest names.

#### Scenario: Read strips the suffix from link targets
- **WHEN** `read_memory_note` returns an own-scope note whose persisted content
  contains `[[rust.coder.alice]]` for scope `{agent:"coder", user:"alice"}`
- **THEN** the returned `content` contains `[[rust]]`

#### Scenario: Write expands own-scope link targets
- **WHEN** `write_memory_note` is called for scope rendering to `coder.alice` with
  content containing `[[rust]]` resolving to the caller's own `rust.md`
- **THEN** the persisted file content contains `[[rust.coder.alice]]`

#### Scenario: Diary append expands link targets
- **WHEN** `append_diary_entry` is called with content containing `[[rust]]`
  resolving to the caller's own note
- **THEN** the appended diary content is persisted with the suffixed link form

### Requirement: Edit matches the persisted link form

`edit_memory_note` SHALL apply the write-side link transform to its
`search_string` and `replace_string` before matching against and writing the
physical file, so that a search containing a clean link target matches the
suffixed form stored on disk.

#### Scenario: Edit search containing a link matches on disk
- **WHEN** the persisted note contains `[[rust.coder.alice]]` and
  `edit_memory_note` is called for scope `coder.alice` with `search_string`
  containing `[[rust]]`
- **THEN** the search matches the stored line and the edit is applied (it does NOT
  fail with `edit_search_not_found`)

#### Scenario: Edit replacement is expanded
- **WHEN** `edit_memory_note` replaces a line with a `replace_string` containing
  `[[guide]]` resolving to the caller's own `guide.md`
- **THEN** the persisted content contains `[[guide.coder.alice]]`

### Requirement: Core-file tools apply the link transform

The core-file wrappers SHALL apply the same link transform as the generic note
tools. `evolve_core_persona` (PERSONA/PROMPT/RULES/USER/MEMORY) and
`update_task_heartbeat` (HEARTBEAT.md) SHALL expand link targets on write, and
`load_session_context` SHALL strip the caller's own suffix from the foundational
files it renders. Line caps SHALL be evaluated against the agent-facing content
(expansion does not change the line count).

#### Scenario: MEMORY.md index expands and renders clean
- **WHEN** `evolve_core_persona` writes `memory` content containing `[[rust]]`
  resolving to the caller's own note, for scope rendering to `coder.alice`
- **THEN** the persisted `MEMORY.md` contains `[[rust.coder.alice]]`, and a
  subsequent `load_session_context` renders the memory section with `[[rust]]`

#### Scenario: Heartbeat link expands on write
- **WHEN** `update_task_heartbeat` is called with content containing `[[rust]]`
  resolving to the caller's own note
- **THEN** the persisted `HEARTBEAT.md` contains the suffixed link form
