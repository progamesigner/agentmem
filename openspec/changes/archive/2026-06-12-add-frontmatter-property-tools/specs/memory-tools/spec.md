## ADDED Requirements

### Requirement: `read_note_properties` tool
The system SHALL expose a `read_note_properties` tool, available on every build, that returns the frontmatter properties of the note at the given **vault-root-relative** virtual path as a JSON object in `{ properties }`. Parsing SHALL match the recall indexer's frontmatter interpretation: a leading `---` fenced YAML block is parsed to a JSON object; absent, unterminated, or malformed frontmatter yields an empty object and is never an error. Read gating SHALL be identical to `read_memory_note` (policy, region, visibility filters), and root-level core files SHALL be readable.

#### Scenario: Properties returned as JSON
- **WHEN** the tool is called for a note beginning `---\ntags: [rust, async]\nstatus: draft\n---\n…`
- **THEN** the result is `{ properties: { "tags": ["rust", "async"], "status": "draft" } }`

#### Scenario: No frontmatter yields an empty object
- **WHEN** the tool is called for a note with no leading `---` block (or a malformed one)
- **THEN** the result is `{ properties: {} }`

#### Scenario: Read gating parity
- **WHEN** the tool is called for a missing, hidden/ignored, or policy-denied path
- **THEN** the response is the same MCP error code `read_memory_note` would return

### Requirement: `update_note_properties` tool
The system SHALL expose an `update_note_properties` tool, available on every build, that merges a JSON object `properties` into the frontmatter of the note at the given **vault-root-relative** virtual path and persists atomically under the per-target lock. Each supplied key SHALL be upserted with its JSON value (strings, numbers, booleans, arrays, and objects round-trip); a key supplied with an explicit `null` SHALL be deleted. The note body SHALL remain byte-identical; no link transform is applied inside the frontmatter block. A note without a frontmatter block SHALL gain one; a merge whose result is an empty object SHALL remove the block entirely. When the existing leading block looks like frontmatter but is not valid YAML, the tool SHALL refuse with `invalid_argument` and leave the file unchanged. The frontmatter block is re-serialized in normalized form (stable key order; comments and YAML formatting are not preserved). Write gating SHALL match the generic write tools: agents-folder root-level paths are rejected as wrapper-reserved, policy and visibility guards apply, the target must exist (`not_found` otherwise), and the recall index SHALL be updated synchronously. The result SHALL return the full post-update `{ properties }`.

#### Scenario: Upsert and delete in one call
- **WHEN** a note's frontmatter is `{ status: "draft", priority: 2 }` and the tool is called with `properties={ "status": "done", "reviewed": true, "priority": null }`
- **THEN** the persisted frontmatter parses as `{ status: "done", reviewed: true }`, the body is byte-identical, and the result echoes the merged set

#### Scenario: Block created when absent
- **WHEN** the tool is called against a note with no frontmatter
- **THEN** a `---` fenced block containing the supplied properties is added above the unchanged body

#### Scenario: Emptied block is removed
- **WHEN** the merge deletes every remaining key
- **THEN** the persisted note has no frontmatter fences and the body is unchanged

#### Scenario: Malformed existing frontmatter is refused
- **WHEN** the note begins with a `---` fence whose contents do not parse as YAML
- **THEN** the response is an MCP error with code `invalid_argument` and the file is unchanged

#### Scenario: Updated properties are immediately recallable
- **WHEN** recall runs the tantivy backend and the tool sets `status: "done"` on a note
- **THEN** a subsequent `recall_memory_notes` call with filter `{ key: "status", op: "eq", value: "done" }` returns the note without waiting for the watcher

#### Scenario: Write gating parity
- **WHEN** the tool targets an agents-folder root-level core file, a policy-denied region, a visibility-excluded path, or a missing file
- **THEN** the response carries the same error code the generic write tools would return (`path_not_permitted` naming the wrapper, the policy error, or `not_found`)
