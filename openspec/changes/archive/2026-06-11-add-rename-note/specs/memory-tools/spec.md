## ADDED Requirements

### Requirement: `rename_memory_note` tool
The system SHALL expose a `rename_memory_note` tool that moves a single note from `path` to `new_path` (both **vault-root-relative** virtual paths) and rewrites every visible incoming link to resolve to the new location. The destination MUST NOT already exist; a rename onto an existing note SHALL be rejected with a `destination_exists` error and no change on disk. Inside the agents folder, both `path` and `new_path` MUST be under a subfolder; an agents-folder root-level path on either end SHALL be rejected with `path_not_permitted` (core files are wrapper-managed). The active policy MUST permit writing both the source's and the destination's region, and the region of every referring note that requires rewriting; when any of these is not writable the tool SHALL refuse with the appropriate policy error before any mutation. The moved note's own content SHALL be re-run through the write-side link transform for the destination's region, including the cross-scope leak guard. All preconditions SHALL be validated before the first write; mutations are then applied in the order: write destination, rewrite referrers, delete source.

#### Scenario: Rename moves content and reports rewrites
- **WHEN** the tool is called with `path="Agents/topics/rust.md"`, `new_path="Agents/topics/rust-lang.md"` under a writable policy
- **THEN** the destination contains the source's content, the source no longer exists, and the response carries `{ renamed: true, path, new_path, notes_rewritten }`

#### Scenario: Incoming wikilinks are rewritten
- **WHEN** a visible note contains `[[rust]]`, `[[rust#install|the note]]`, and `![[rust]]` all forward-resolving to the source, and the source is renamed to `rust-lang.md`
- **THEN** after the rename the referring note's links resolve to the destination, with heading, alias, and embed decorations preserved (e.g. `[[rust-lang#install|the note]]`)

#### Scenario: Incoming markdown links are rewritten
- **WHEN** a visible note contains `[doc](topics/rust.md)` resolving to the source and the source is renamed within the same scope
- **THEN** the referring note's markdown link target is rewritten to the destination's persisted form and round-trips to the clean new path on read

#### Scenario: Self-references move with the note
- **WHEN** the source note's own content contains a link resolving to itself
- **THEN** the destination's content links to the destination (the old name neither dangles nor persists)

#### Scenario: Destination must not exist
- **WHEN** the tool is called with a `new_path` at which a visible note already exists
- **THEN** the response is an MCP error with code `destination_exists` and neither note is modified

#### Scenario: Root core files are not renamable
- **WHEN** the tool is called with `path` or `new_path` at the agents-folder root level (e.g. `Agents/MEMORY.md`)
- **THEN** the response is an MCP error with code `path_not_permitted` and nothing changes

#### Scenario: Policy gates both regions
- **WHEN** policy is `namespaced` and either `path` or `new_path` resolves outside the agents folder
- **THEN** the response is an MCP error with code `write_denied` and nothing changes

#### Scenario: Shared-to-scoped rename is refused when shared referrers exist
- **WHEN** policy is `readwrite`, a shared-region note links to shared note `Actions/release.md`, and the tool is asked to rename `Actions/release.md` to a path inside the agents folder
- **THEN** the response is an MCP error with code `write_denied` (the rewrite would persist the caller's scope suffix in a shared note) and nothing changes

#### Scenario: Leak guard applies to the moved content
- **WHEN** policy is `readwrite` and a scoped note whose content links to another of the caller's scoped notes is renamed to a destination outside the agents folder
- **THEN** the response is an MCP error with code `write_denied` and nothing changes

#### Scenario: Missing source
- **WHEN** the tool is called with a `path` that resolves to a non-existent file
- **THEN** the response is an MCP error with code `not_found`

#### Scenario: Recall reflects the rename immediately
- **WHEN** recall is enabled and a note is renamed
- **THEN** a subsequent recall in the same scope returns hits at the new path and none at the old path, without waiting for the filesystem watcher
