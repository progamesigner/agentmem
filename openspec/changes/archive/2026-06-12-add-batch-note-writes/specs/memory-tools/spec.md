# memory-tools delta: add-batch-note-writes

## ADDED Requirements

### Requirement: `write_memory_notes` tool
The system SHALL expose a `write_memory_notes` tool that writes multiple notes in one call. The `notes` argument SHALL be an array of 1 to 20 entries, each an object `{ path, content, append? }` with the same per-entry semantics as `write_memory_note`: `path` is a **vault-root-relative** virtual path, `content` the full new contents (or, with `append: true`, the bytes to append verbatim with a missing note created from `content`). An empty array, an array exceeding 20 entries, a malformed entry, or two entries resolving to the same virtual path SHALL be rejected with `invalid_argument`.

The tool SHALL validate every entry before modifying any file: virtual-path validity, the wrapper-reserved rejection for agents-folder root-level core files, policy write gating by region, visibility filtering, and the write-side link transform including the cross-scope leak guard. Link expansion SHALL resolve against the caller's visible set extended with the batch's own paths, so a link whose target is created by another entry of the same batch resolves exactly as it would after the batch lands. Any validation failure SHALL reject the whole call with the error code the failing entry would have received from `write_memory_note`, naming the offending path, and SHALL leave every file unchanged.

After validation, entries SHALL be applied in request order: a full replace persists through the atomic-write procedure, an append through the locked read-modify-write, and each applied entry SHALL update the recall index synchronously. The result SHALL contain a `results` array with exactly one `{ path, bytes_written }` entry per requested note, in request order, where `path` echoes the requested path verbatim. The batch is not transactional across entries: a crash mid-apply may leave a prefix of the batch applied, but never a partially written single file.

#### Scenario: Batch write lands all entries in order
- **WHEN** the tool is called with three valid entries for the active scope
- **THEN** all three notes are persisted, the recall index reflects each, and the result carries three `{ path, bytes_written }` entries in request order

#### Scenario: Intra-batch link resolves to a note created in the same batch
- **WHEN** scope renders to `jarvis.tony` and one batch creates `Agents/topics/rust/facts.md` and a second entry whose content contains `[[facts]]` resolving to it
- **THEN** the second entry's persisted content contains `[[facts.jarvis.tony]]`, exactly as if the notes had been written sequentially

#### Scenario: Any validation failure leaves the vault untouched
- **WHEN** the tool is called with three entries of which the third targets the wrapper-reserved `Agents/MEMORY.md`
- **THEN** the response is an MCP error with code `path_not_permitted` naming the reserved path and the wrapper tool, and none of the three notes — including the two valid ones — is created or modified

#### Scenario: Cross-scope leak guard rejects the whole batch
- **WHEN** one entry targets a shared-region file and its content contains a `[[wikilink]]` resolving into the caller's own scope
- **THEN** the whole call is refused with the `write_denied`-class cross-scope error naming the offending target and no file is changed

#### Scenario: Append entries append
- **WHEN** an entry carries `append: true` for an existing note
- **THEN** its `content` is appended verbatim (no implicit separator) under the per-target lock, while full-replace entries in the same batch replace their targets

#### Scenario: Duplicate target paths are rejected
- **WHEN** two entries resolve to the same virtual path
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed

#### Scenario: Batch size limits
- **WHEN** the tool is called with an empty `notes` array or with more than 20 entries
- **THEN** the response is an MCP error with code `invalid_argument` and no file is changed

#### Scenario: Per-entry semantics match the single write
- **WHEN** an entry targets a policy-denied region or a visibility-excluded path
- **THEN** the call fails with the same error code `write_memory_note` would return for that entry

#### Scenario: Writes are immediately recallable
- **WHEN** a batch creates notes in the caller's scope and recall is enabled
- **THEN** a subsequent `recall_memory_notes` call matches the new content without waiting for the watcher
