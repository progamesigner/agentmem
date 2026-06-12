# memory-tools delta: add-note-read-ranges

## MODIFIED Requirements

### Requirement: `read_memory_note` tool
The system SHALL expose a `read_memory_note` tool that returns the UTF-8 contents of a single file identified by its virtual path, subject to the active policy, region detection, and visibility filters. The tool SHALL accept an optional boolean `backlinks` argument; when `true`, the structured result SHALL additionally carry a `backlinks` array containing the clean virtual path of every visible note that has at least one link resolving to the target note. The array SHALL be deduplicated (one entry per referring note regardless of how many of its links resolve to the target), sorted ascending by clean virtual path, and computed over exactly the caller's visible set (own scope plus the shared region when the active policy permits reading it). When `backlinks` is absent or `false`, the response SHALL NOT contain a `backlinks` field and SHALL be unchanged from prior behavior.

The tool SHALL accept optional `offset` and `limit` integer arguments selecting a line range of the note: `offset` is the 1-based index of the first returned line (default 1) and `limit` is the maximum number of lines returned (default: all remaining lines). The range SHALL be applied to the agent-facing content after the own-suffix link strip, with lines delimited by `\n` and delimiters preserved, so that concatenating consecutive slices reproduces the full content byte-for-byte. When at least one of `offset`/`limit` is supplied, the structured result SHALL additionally carry `total_lines`, the line count of the full agent-facing content. An `offset` past the last line SHALL return empty content (not an error) with a truthful `total_lines`. An `offset` or `limit` of `0` SHALL be rejected with `invalid_argument`. When neither `offset` nor `limit` is supplied, the response SHALL NOT contain a `total_lines` field and SHALL be unchanged from prior behavior. The `backlinks` and range arguments compose.

#### Scenario: Read of an own-scope file inside the agents folder
- **WHEN** the tool is called with virtual path `PERSONA.md` (resolved under the agents folder) for the active scope and that file exists
- **THEN** the response contains the file's contents as a string

#### Scenario: Read outside the agents folder under namespaced policy
- **WHEN** policy is `namespaced` and the tool is called with virtual path `Actions/release.md` and that file exists
- **THEN** the response contains the file's contents as a string

#### Scenario: Read outside the agents folder under scoped policy
- **WHEN** policy is `scoped` and the tool is called with virtual path `Actions/release.md`
- **THEN** the response is an MCP error with code `path_not_permitted`

#### Scenario: Read of a missing file
- **WHEN** the tool is called with a virtual path that resolves to a non-existent file
- **THEN** the response is an MCP error with code `not_found`

#### Scenario: Read of a hidden or ignored file
- **WHEN** the tool is called with a virtual path that is excluded by hidden filtering or by an active `.gitignore`/`.obsidianignore` rule
- **THEN** the response is an MCP error with code `path_not_permitted` and the message does NOT reveal whether the file actually exists

#### Scenario: Line range returns the requested slice with total_lines
- **WHEN** a note has 50 lines and the tool is called with `offset=11` and `limit=10`
- **THEN** the returned content is exactly lines 11 through 20 (line delimiters preserved) and the structured result carries `total_lines: 50`

#### Scenario: Offset alone reads to the end
- **WHEN** a note has 50 lines and the tool is called with `offset=41` and no `limit`
- **THEN** the returned content is lines 41 through 50 and `total_lines` is 50

#### Scenario: Limit alone reads from the start
- **WHEN** the tool is called with `limit=5` and no `offset`
- **THEN** the returned content is the first 5 lines and the structured result carries `total_lines`

#### Scenario: Offset past the end is empty, not an error
- **WHEN** a note has 10 lines and the tool is called with `offset=11`
- **THEN** the call succeeds with empty content and `total_lines: 10`

#### Scenario: Zero offset or limit is rejected
- **WHEN** the tool is called with `offset=0` or `limit=0`
- **THEN** the response is an MCP error with code `invalid_argument`

#### Scenario: Range slices the link-stripped view
- **WHEN** the persisted note contains `[[rust.jarvis.tony]]` on line 3 for scope `{agent:"jarvis", user:"tony"}` and the tool is called with `offset=3`, `limit=1`
- **THEN** the returned line contains `[[rust]]` — the slice is taken after the suffix strip, and line numbers match a whole-note read

#### Scenario: Default response is unchanged
- **WHEN** the tool is called without `offset` or `limit`
- **THEN** the full content is returned and the structured result contains no `total_lines` field, byte-identical to prior behavior

#### Scenario: Backlinks returned on request
- **WHEN** the tool is called with `backlinks=true` for note `Agents/topics/rust.md` and the caller's visible notes `Agents/diary/2026-06-10.md` (containing `[[rust]]`) and `Agents/MEMORY.md` (containing `[[topics/rust|the Rust note]]`) both resolve those links to the target
- **THEN** the structured result carries `backlinks: ["Agents/MEMORY.md", "Agents/diary/2026-06-10.md"]` alongside the content

#### Scenario: All link forms count as backlinks
- **WHEN** a visible note references the target via an embed `![[target]]`, a heading link `[[target#section]]`, an aliased link `[[target|label]]`, or a relative markdown link `[text](path/target.md)`
- **THEN** that note appears in the target's `backlinks` array

#### Scenario: A referring note appears once
- **WHEN** a single visible note contains three distinct links that all resolve to the target
- **THEN** the `backlinks` array contains that note's clean virtual path exactly once

#### Scenario: Backlinks honor forward-resolution tie-breaks
- **WHEN** the caller's visible set contains both `Agents/topics/rust.md` and `Lang/rust.md`, and a visible note contains `[[rust]]` which forward resolution resolves to `Agents/topics/rust.md`
- **THEN** that note appears in the backlinks of `Agents/topics/rust.md` and NOT in the backlinks of `Lang/rust.md`

#### Scenario: Other scopes' links are structurally invisible
- **WHEN** the tool is called with `backlinks=true` for shared note `Actions/release.md` by scope `{agent:"jarvis", user:"tony"}`, and a note belonging to scope `jarvis.sam` links to that shared note
- **THEN** no `jarvis.sam` path appears in the `backlinks` array, because another scope's notes are never scanned

#### Scenario: scoped policy excludes the shared region from the scan
- **WHEN** policy is `scoped` and the tool is called with `backlinks=true` for an own-scope note that a shared-region note links to
- **THEN** the `backlinks` array does not contain the shared note's path

#### Scenario: Backlinks omitted by default
- **WHEN** the tool is called without the `backlinks` argument
- **THEN** the structured result contains no `backlinks` field and is unchanged from prior behavior

### Requirement: `read_memory_notes` tool
The system SHALL expose a `read_memory_notes` tool that reads multiple notes in one call. The `paths` argument SHALL be an array of 1 to 20 entries, each entry being either a **vault-root-relative** virtual path string (the whole note) or an object `{ path, offset?, limit? }` requesting a line range of that note with the same semantics as `read_memory_note` (`offset` is the 1-based first line, `limit` the maximum line count, applied to the agent-facing content after the own-suffix link strip). An empty array, an array exceeding 20 entries, an entry that is neither a string nor such an object, or an `offset`/`limit` of `0` SHALL be rejected with `invalid_argument`. The result SHALL contain a `notes` array with exactly one entry per requested path, in request order, each entry being either `{ path, content }` on success or `{ path, error: { code, message } }` on failure, where `path` echoes the requested path verbatim; a successful entry for which a range was requested SHALL additionally carry `total_lines`, the line count of that note's full agent-facing content. Per-path semantics — policy gating, region detection, visibility filtering, suffix resolution, and own-suffix link stripping — SHALL be identical to `read_memory_note`, and a per-path failure SHALL use the same error code the single-read tool would return. A per-path failure SHALL NOT fail the call; the tool-level result is an error only for malformed arguments or invalid scope keys. Duplicate paths SHALL each produce their own entry.

#### Scenario: Batch read returns contents in request order
- **WHEN** the tool is called with `paths=["Agents/topics/rust.md", "Actions/release.md"]` under `namespaced` policy and both notes exist
- **THEN** the result's `notes` array has two entries in that order, each carrying the note's content with the caller's own link suffixes stripped

#### Scenario: Mixed string and ranged entries
- **WHEN** the tool is called with `paths=["Agents/topics/rust.md", { "path": "Agents/diary/2026-06-10.md", "offset": 1, "limit": 5 }]`
- **THEN** the first entry carries the whole note with no `total_lines` field, and the second carries only the first 5 lines plus `total_lines` for the diary note

#### Scenario: Ranged entry past the end is empty, not an error
- **WHEN** an object entry requests `offset` beyond the note's last line
- **THEN** that entry succeeds with empty content and a truthful `total_lines`, and other entries are unaffected

#### Scenario: Partial failure does not void the batch
- **WHEN** the tool is called with three paths of which the second resolves to a non-existent file
- **THEN** entries one and three carry content, entry two carries `error: { code: "not_found", … }`, and the tool call itself succeeds

#### Scenario: Per-path policy and visibility parity
- **WHEN** a requested path would be denied by the single read (e.g. outside the agents folder under `scoped` policy, or a hidden/ignored target)
- **THEN** that entry carries the same error code the single read would return (`path_not_permitted`), without revealing whether the file exists

#### Scenario: Batch size limits
- **WHEN** the tool is called with an empty `paths` array or with more than 20 entries
- **THEN** the response is an MCP error with code `invalid_argument` and no notes are read

#### Scenario: Malformed entry is rejected at the call level
- **WHEN** the tool is called with an entry that is neither a string nor an object with a string `path` (e.g. a number, or `{ "offset": 3 }` with no path), or an object entry with `offset=0` or `limit=0`
- **THEN** the response is an MCP error with code `invalid_argument` and no notes are read

#### Scenario: Duplicate paths are answered positionally
- **WHEN** the tool is called with the same path twice
- **THEN** the `notes` array contains two entries for it, one per request position
