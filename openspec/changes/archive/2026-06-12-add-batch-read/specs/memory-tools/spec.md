## ADDED Requirements

### Requirement: `read_memory_notes` tool
The system SHALL expose a `read_memory_notes` tool that reads multiple notes in one call. The `paths` argument SHALL be an array of 1 to 20 **vault-root-relative** virtual paths; an empty array, an array exceeding 20 entries, or any non-string entry SHALL be rejected with `invalid_argument`. The result SHALL contain a `notes` array with exactly one entry per requested path, in request order, each entry being either `{ path, content }` on success or `{ path, error: { code, message } }` on failure, where `path` echoes the requested path verbatim. Per-path semantics — policy gating, region detection, visibility filtering, suffix resolution, and own-suffix link stripping — SHALL be identical to `read_memory_note`, and a per-path failure SHALL use the same error code the single-read tool would return. A per-path failure SHALL NOT fail the call; the tool-level result is an error only for malformed arguments or invalid scope keys. Duplicate paths SHALL each produce their own entry.

#### Scenario: Batch read returns contents in request order
- **WHEN** the tool is called with `paths=["Agents/topics/rust.md", "Actions/release.md"]` under `namespaced` policy and both notes exist
- **THEN** the result's `notes` array has two entries in that order, each carrying the note's content with the caller's own link suffixes stripped

#### Scenario: Partial failure does not void the batch
- **WHEN** the tool is called with three paths of which the second resolves to a non-existent file
- **THEN** entries one and three carry content, entry two carries `error: { code: "not_found", … }`, and the tool call itself succeeds

#### Scenario: Per-path policy and visibility parity
- **WHEN** a requested path would be denied by the single read (e.g. outside the agents folder under `scoped` policy, or a hidden/ignored target)
- **THEN** that entry carries the same error code the single read would return (`path_not_permitted`), without revealing whether the file exists

#### Scenario: Batch size limits
- **WHEN** the tool is called with an empty `paths` array or with more than 20 entries
- **THEN** the response is an MCP error with code `invalid_argument` and no notes are read

#### Scenario: Duplicate paths are answered positionally
- **WHEN** the tool is called with the same path twice
- **THEN** the `notes` array contains two entries for it, one per request position
