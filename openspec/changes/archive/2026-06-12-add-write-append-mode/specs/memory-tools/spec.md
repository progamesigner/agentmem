## MODIFIED Requirements

### Requirement: `write_memory_note` tool
The system SHALL expose a `write_memory_note` tool that performs an atomic full-file write to a virtual path the active policy permits writing to. The `path` argument is a **vault-root-relative** virtual path; to target a location inside the agents folder the caller MUST include the agents-folder name as the leading segment (the dedicated wrapper tools do this automatically; the generic tools do not). Inside the agents folder, the target virtual path MUST be under a subfolder; a root-level path (a path with no subfolder segment beneath the per-scope root) is reserved for the dedicated wrapper tools (`evolve_core_persona`, `update_task_heartbeat`) and SHALL be rejected.

The tool SHALL accept an optional boolean `append` argument. When `append` is `true`, `content` SHALL be appended verbatim to the existing note — exact bytes, no implicit separator — under the same per-target lock as the diary append, so concurrent appends to one note serialise without loss; when the note does not exist it SHALL be created with `content` as its full body. The appended fragment SHALL pass through the write-side link transform (including the cross-scope leak guard) exactly like full-write content. All other guards (root-level reservation, policy gates, visibility filters) apply unchanged. The returned byte count SHALL be the note's total size after the write in both modes.

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

#### Scenario: Append extends an existing note verbatim
- **WHEN** the tool is called with `append=true` and `content="- new fact\n"` against a note ending in `"- old fact\n"`
- **THEN** the note ends with `"- old fact\n- new fact\n"` — no separator inserted — and the response reports the note's total byte count

#### Scenario: Append to a missing note creates it
- **WHEN** the tool is called with `append=true` against a virtual path with no existing file
- **THEN** the note is created with `content` as its full body

#### Scenario: Concurrent appends are not lost
- **WHEN** multiple callers append to the same note concurrently
- **THEN** every appended fragment appears exactly once in the final note (appends serialise under the per-target lock)

#### Scenario: Appended links are transformed
- **WHEN** `append=true` content contains `[[rust]]` resolving to an own-scope note
- **THEN** the persisted fragment carries the expanded suffixed form and a subsequent read returns the clean form, identical to full-write behavior

#### Scenario: Append honors the same guards as full write
- **WHEN** the tool is called with `append=true` against a root-level core file, a policy-denied region, or a visibility-excluded path
- **THEN** the response is the same error the full-write mode would produce and nothing is written
