# vault-storage Specification

## Purpose
TBD - created by archiving change build-agentmem-mcp-server. Update Purpose after archive.
## Requirements
### Requirement: Vault root containment
The system SHALL canonicalise every virtual path against the configured vault root and SHALL reject any resolution whose canonical absolute path is not a descendant of that root.

#### Scenario: Traversal attempt is rejected
- **WHEN** a tool is called with virtual path `../../etc/passwd`
- **THEN** the operation is refused with a structured error of code `path_escapes_root` before any filesystem call is issued

#### Scenario: Symlink escape is rejected
- **WHEN** a symlink inside the vault points to a path outside the vault root, and a tool resolves to that symlink
- **THEN** the operation is refused with code `path_escapes_root`

#### Scenario: Legitimate path inside root is accepted
- **WHEN** a tool is called with a virtual path that resolves under `AGENTMEM_ROOT_DIR`
- **THEN** the operation proceeds to template resolution and policy enforcement

### Requirement: VFS template resolution
The system SHALL, on every tool call, validate that the supplied scope arguments exactly match the placeholder idents of the configured `AGENTMEM_VFS_TEMPLATE`, and SHALL render the template into a single string used as both the per-scope directory segment under the agents folder and the dotted suffix appended to the file stem inside the agents folder.

#### Scenario: Default template resolves agent and user
- **WHEN** template is `<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/coder.alice/tasks/plan.coder.alice.md`

#### Scenario: Single-key template
- **WHEN** template is `<agent>`, scope is `{agent:"coder"}`, agents folder is `Agents`, and virtual path is `HEARTBEAT-STATE.md`
- **THEN** the resolved physical path is `<root>/Agents/coder/HEARTBEAT-STATE.coder.md`

#### Scenario: Multi-key template
- **WHEN** template is `<team>.<agent>.<env>.<user>`, scope is `{team:"platform", agent:"coder", env:"prod", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/platform.coder.prod.alice/tasks/plan.platform.coder.prod.alice.md`

#### Scenario: Template with literal segment
- **WHEN** template is `v1.<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/v1.coder.alice/tasks/plan.v1.coder.alice.md`

#### Scenario: Empty template applies no suffix
- **WHEN** template is the empty string and virtual path is `notes.md`
- **THEN** the resolved physical path is `<root>/<agents_dir>/notes.md` with no per-scope directory and no suffix

#### Scenario: Vault root as agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=.`, template is `<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/coder.alice/tasks/plan.coder.alice.md` and the "outside the agents folder" region is empty

#### Scenario: Missing required scope key
- **WHEN** template is `<agent>.<user>` and a tool is called with `agent` set but `user` missing
- **THEN** the call is rejected with code `missing_scope` and a message naming `user`

#### Scenario: Extra scope key
- **WHEN** template is `<agent>` and a tool is called with both `agent` and `user`
- **THEN** the call is rejected at schema validation because the input schema does NOT include `user` under this template

### Requirement: Region detection
The system SHALL, for every virtual path that passes vault-root containment, classify it as either *inside the agents folder* or *outside the agents folder but inside the vault root*. The agents folder is determined entirely by `AGENTMEM_AGENTS_DIR`; no globs are involved.

#### Scenario: Path under agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=Agents` and virtual path is `Agents/topics/rust.md`
- **THEN** the region is `inside-agents-folder`

#### Scenario: Path outside agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=Agents` and virtual path is `Actions/release.md`
- **THEN** the region is `outside-agents-folder`

#### Scenario: Vault root is agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=.` and virtual path is `anything.md`
- **THEN** the region is `inside-agents-folder` and the `outside-agents-folder` region is empty

### Requirement: Policy enforcement
The system SHALL enforce permissions according to `AGENTMEM_POLICY` and the region classification, as follows:

| Policy | Inside agents folder | Outside agents folder |
|---|---|---|
| `scoped` | own-scope read & write (suffix applied) | denied |
| `namespaced` | own-scope read & write (suffix applied) | read-only (no suffix) |
| `readonly` | own-scope read-only (suffix applied) | read-only (no suffix) |
| `readwrite` | own-scope read & write (suffix applied) | read & write (no suffix) |

#### Scenario: scoped denies outside region
- **WHEN** policy is `scoped` and any tool targets a path outside the agents folder
- **THEN** the operation is refused with code `path_not_permitted`

#### Scenario: namespaced permits reads outside
- **WHEN** policy is `namespaced` and an agent reads `Actions/release.md`
- **THEN** the read succeeds, the same physical file `<root>/Actions/release.md` is served to every scope, and no VFS suffix is applied

#### Scenario: namespaced denies writes outside
- **WHEN** policy is `namespaced` and an agent writes to `Actions/release.md`
- **THEN** the write is refused with code `write_denied` and the file is unchanged

#### Scenario: readonly denies writes inside agents folder
- **WHEN** policy is `readonly` and an agent writes to its own scope's file inside the agents folder
- **THEN** the write is refused with code `write_denied` and the file is unchanged

#### Scenario: readwrite permits writes outside
- **WHEN** policy is `readwrite` and an agent writes to `Scratch/team-notes.md`
- **THEN** the write succeeds, the file is created or replaced at `<root>/Scratch/team-notes.md` without a suffix, and every other agent can read it at the same virtual path

### Requirement: Own-scope strictness inside the agents folder
Inside the agents folder, when the template is non-empty, the system SHALL only allow read, write, edit, and list operations on files whose physical path's rendered suffix matches the caller's rendered suffix. Files belonging to other scopes SHALL be invisible (absent from listings) AND inaccessible (any direct attempt to address them resolves to `not_found`).

#### Scenario: Other scope's file is unreachable
- **WHEN** the resolver is invoked for scope `{agent:"coder", user:"alice"}` on virtual path `tasks/plan.md` and the only file on disk in that directory is `plan.coder.bob.md`
- **THEN** the operation is refused with code `not_found` (the resolved file for the caller is `plan.coder.alice.md`, which does not exist) and `plan.coder.bob.md` is NOT read

#### Scenario: Crafted virtual path cannot reach other scope
- **WHEN** an agent in scope `{agent:"coder", user:"alice"}` attempts to address another scope's physical file by passing a virtual path that includes the other scope's suffix in the stem (e.g. `tasks/plan.coder.bob.md`)
- **THEN** the resolver applies the caller's own suffix on top, producing `plan.coder.bob.md.coder.alice.md` which does not exist and is reported as `not_found`; under no input does the resolver ever open another scope's file

#### Scenario: Listing only shows own scope
- **WHEN** `list_workspace_files` is called for scope `{agent:"coder", user:"alice"}` and the disk contains files for `coder.alice`, `coder.bob`, and `writer.alice` under the agents folder
- **THEN** only the `coder.alice` files appear in the result, with suffixes stripped

#### Scenario: Empty template removes own-scope filtering
- **WHEN** template is the empty string, policy is `namespaced`, and `list_workspace_files` is called
- **THEN** all files inside the agents folder are listed (there are no per-scope subdirectories or suffixes to filter by)

### Requirement: Visibility filters
The system SHALL, on every list / read / write / edit / delete operation, apply visibility filters that exclude (a) any path whose any segment begins with `.` when `AGENTMEM_INCLUDE_HIDDEN=false` (the default), and (b) any path matched by an applicable `.gitignore` or `.obsidianignore` rule inside the vault when `AGENTMEM_HONOR_IGNORE_FILES=true` (the default). The walker semantics SHALL match the `ignore` crate's `WalkBuilder` so per-directory ignore files compose as in `ripgrep` and Obsidian's own search.

#### Scenario: Hidden file excluded from listing
- **WHEN** defaults are in effect and the vault contains `Agents/<scope>/notes.md` and `Agents/<scope>/.tmp.md`
- **THEN** `list_memory_notes` returns only `notes.md`; `.tmp.md` is absent

#### Scenario: Hidden file inaccessible by direct read
- **WHEN** defaults are in effect and `read_memory_note` is called with virtual path `Agents/<scope>/.tmp.md`
- **THEN** the response is an MCP error with code `path_not_permitted`

#### Scenario: gitignore-matched file excluded
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=true` and the vault contains a `.gitignore` line `drafts/*.md` plus the file `Agents/<scope>/drafts/wip.md`
- **THEN** `list_memory_notes` does not include `drafts/wip.md` and a direct `read_memory_note` for it returns `path_not_permitted`

#### Scenario: Including hidden files
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=true`
- **THEN** dotfiles appear in listings and are directly readable (still subject to ignore-file rules unless also disabled)

#### Scenario: Disabling ignore-file enforcement
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=false`
- **THEN** `.gitignore` and `.obsidianignore` patterns are not consulted; the visible set is widened accordingly

#### Scenario: Agents folder itself never filtered out
- **WHEN** `AGENTMEM_AGENTS_DIR=.agents` (begins with `.`) and `AGENTMEM_INCLUDE_HIDDEN=false`
- **THEN** the agents folder is still recognised as the scoped/suffixed region and its contents remain visible to and writable by the owning scope; hidden filtering does NOT exclude the agents folder

### Requirement: Atomic full-file writes
The system SHALL implement every full-file write as: create a temp file in the same directory as the target, write contents to the temp file, fsync, then rename the temp file over the target.

#### Scenario: Crash during write leaves target intact
- **WHEN** the server is killed after writing the temp file but before the rename completes
- **THEN** the target file on disk is unchanged from its prior contents (or absent if it never existed)

#### Scenario: Successful write replaces target atomically
- **WHEN** `write_workspace_file` succeeds
- **THEN** the target file at the resolved physical path contains exactly the bytes supplied by the caller and no other file is created in the parent directory

### Requirement: Edit precondition uniqueness
The system SHALL refuse an `edit_workspace_file` call whose `search_string` occurs zero times or more than once in the current target file.

#### Scenario: Search string occurs once
- **WHEN** the search string appears exactly once in the file
- **THEN** the server replaces that single occurrence and persists the result via the atomic write procedure

#### Scenario: Search string is missing
- **WHEN** the search string does not appear in the file
- **THEN** the call is rejected with code `edit_search_not_found` and the file is unchanged

#### Scenario: Search string is ambiguous
- **WHEN** the search string appears two or more times in the file
- **THEN** the call is rejected with code `edit_search_ambiguous`, the file is unchanged, and the error message advises the client to provide a longer, unique snippet

### Requirement: Auto-create parent directories on writes
The system SHALL create any missing parent directories along the physical path during a write inside the agents folder before opening the temp file. For writes outside the agents folder (only possible under `readwrite` policy), parent directories SHALL likewise be auto-created.

#### Scenario: First write into a new scope
- **WHEN** `write_workspace_file` is called for a virtual path inside the agents folder and no directory for the caller's rendered suffix yet exists on disk
- **THEN** the server creates the per-scope directory tree and then performs the atomic write

#### Scenario: First write outside the agents folder under readwrite
- **WHEN** policy is `readwrite` and `write_workspace_file` is called for `Scratch/team/notes.md` where `Scratch/team/` does not yet exist
- **THEN** the server creates the directory tree and then performs the atomic write

