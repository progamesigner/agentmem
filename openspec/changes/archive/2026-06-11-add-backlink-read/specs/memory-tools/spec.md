## MODIFIED Requirements

### Requirement: `read_memory_note` tool
The system SHALL expose a `read_memory_note` tool that returns the UTF-8 contents of a single file identified by its virtual path, subject to the active policy, region detection, and visibility filters. The tool SHALL accept an optional boolean `backlinks` argument; when `true`, the structured result SHALL additionally carry a `backlinks` array containing the clean virtual path of every visible note that has at least one link resolving to the target note. The array SHALL be deduplicated (one entry per referring note regardless of how many of its links resolve to the target), sorted ascending by clean virtual path, and computed over exactly the caller's visible set (own scope plus the shared region when the active policy permits reading it). When `backlinks` is absent or `false`, the response SHALL NOT contain a `backlinks` field and SHALL be unchanged from prior behavior.

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
