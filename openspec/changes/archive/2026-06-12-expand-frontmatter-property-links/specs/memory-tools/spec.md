# memory-tools delta: expand-frontmatter-property-links

## MODIFIED Requirements

### Requirement: `read_note_properties` tool
The system SHALL expose a `read_note_properties` tool, available on every build, that returns the frontmatter properties of the note at the given **vault-root-relative** virtual path as a JSON object in `{ properties }`. Parsing SHALL match the recall indexer's frontmatter interpretation: a leading `---` fenced YAML block is parsed to a JSON object; absent, unterminated, or malformed frontmatter yields an empty object and is never an error. The caller's own scope suffix SHALL be stripped from link targets in every string value of the returned properties, recursing into arrays and nested objects, applying the same read-path transform as `read_memory_note`; the agent SHALL never observe its own scope suffix in a returned property value. Read gating SHALL be identical to `read_memory_note` (policy, region, visibility filters), and root-level core files SHALL be readable.

#### Scenario: Properties returned as JSON
- **WHEN** the tool is called for a note beginning `---\ntags: [rust, async]\nstatus: draft\n---\n…`
- **THEN** the result is `{ properties: { "tags": ["rust", "async"], "status": "draft" } }`

#### Scenario: Suffixed link values are returned clean
- **WHEN** scope `{agent:"jarvis", user:"tony"}` calls the tool for a note whose persisted frontmatter contains `related: "[[rust.jarvis.tony]]"`
- **THEN** the result contains `related: "[[rust]]"`

#### Scenario: Nested string values are stripped
- **WHEN** the persisted frontmatter contains `links: ["[[a.jarvis.tony]]", { "see": "[[b.jarvis.tony]]" }]` for the caller's own scope
- **THEN** the returned values are `["[[a]]", { "see": "[[b]]" }]`

#### Scenario: No frontmatter yields an empty object
- **WHEN** the tool is called for a note with no leading `---` block (or a malformed one)
- **THEN** the result is `{ properties: {} }`

#### Scenario: Read gating parity
- **WHEN** the tool is called for a missing, hidden/ignored, or policy-denied path
- **THEN** the response is the same MCP error code `read_memory_note` would return

### Requirement: `update_note_properties` tool
The system SHALL expose an `update_note_properties` tool, available on every build, that merges a JSON object `properties` into the frontmatter of the note at the given **vault-root-relative** virtual path and persists atomically under the per-target lock. Each supplied key SHALL be upserted with its JSON value (strings, numbers, booleans, arrays, and objects round-trip); a key supplied with an explicit `null` SHALL be deleted. The write-side link transform SHALL be applied to every string value of the supplied properties, recursing into arrays and nested objects: link targets resolving into the caller's own scope are expanded to the suffixed physical form, shared targets are left clean, dangling targets are left verbatim, and a supplied property whose link target resolves into the caller's own scope while the note lives in the shared region SHALL be refused with the cross-scope leak-guard error, leaving the file unchanged. Non-string values SHALL NOT be transformed. The note body SHALL remain byte-identical. A note without a frontmatter block SHALL gain one; a merge whose result is an empty object SHALL remove the block entirely. When the existing leading block looks like frontmatter but is not valid YAML, the tool SHALL refuse with `invalid_argument` and leave the file unchanged. The frontmatter block is re-serialized in normalized form (stable key order; comments and YAML formatting are not preserved). Write gating SHALL match the generic write tools: agents-folder root-level paths are rejected as wrapper-reserved, policy and visibility guards apply, the target must exist (`not_found` otherwise), and the recall index SHALL be updated synchronously. The result SHALL return the full post-update `{ properties }` in the agent-facing clean form (own suffixes stripped).

#### Scenario: Upsert and delete in one call
- **WHEN** a note's frontmatter is `{ status: "draft", priority: 2 }` and the tool is called with `properties={ "status": "done", "reviewed": true, "priority": null }`
- **THEN** the persisted frontmatter parses as `{ status: "done", reviewed: true }`, the body is byte-identical, and the result echoes the merged set

#### Scenario: Own-scope link value is expanded on disk and returned clean
- **WHEN** scope renders to `jarvis.tony` and the tool sets `related: "[[rust]]"` where `rust` resolves to the caller's own `topics/rust.md`
- **THEN** the persisted frontmatter value is `"[[rust.jarvis.tony]]"`, the result echoes `related: "[[rust]]"`, and a subsequent `read_note_properties` returns `"[[rust]]"`

#### Scenario: Shared link value stays clean
- **WHEN** the tool sets `related: "[[release]]"` where `release` resolves to the shared `Actions/release.md`
- **THEN** the persisted value is `"[[release]]"` with no suffix

#### Scenario: Leak guard applies to property values
- **WHEN** policy permits writing the shared note `Actions/release.md` and the tool sets a property containing `[[rust]]` that resolves only into the caller's own scope
- **THEN** the call is refused with the `write_denied`-class cross-scope error naming the target and `Actions/release.md` is unchanged

#### Scenario: Dangling and non-string values are untouched
- **WHEN** the tool sets `related: "[[not-yet-created]]"` (resolving to nothing) and `priority: 2`
- **THEN** both are persisted verbatim

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
