## MODIFIED Requirements

### Requirement: Visibility filters
The system SHALL, on every list / read / write / edit / delete operation, apply visibility filters that exclude (a) any path whose any segment begins with `.` when `AGENTMEM_INCLUDE_HIDDEN=false` (the default) AND the path is not exempted by the include-hidden glob list, and (b) any path matched by an applicable `.ignore`, `.gitignore`, or `.obsidianignore` rule inside the vault when `AGENTMEM_HONOR_IGNORE_FILES=true` (the default). Ignore files SHALL be honoured **per-directory and nested**, exactly as `git` treats `.gitignore`: a file in any subfolder applies to that subtree and composes with files in ancestor directories, with the rules assembled from the vault root down to the target's parent directory. This composition SHALL apply to all three ignore-file kinds on both the listing path and the direct-access path. The walker semantics SHALL match the `ignore` crate's `WalkBuilder` so per-directory ignore files compose as in `ripgrep` and Obsidian's own search. The set of files excluded by direct read/write/edit/delete checks SHALL be identical to the set the walker hides from listings (the visible set and the addressable set agree for all three ignore-file kinds).

An include-hidden glob list (configured via `AGENTMEM_INCLUDE_HIDDEN_GLOBS`) SHALL exempt matching dot-paths from hidden filtering. A path is exempt when the path itself OR any of its parent directories (relative to the vault root) matches an include glob; thus matching a directory un-hides that directory and its entire subtree, including nested dot-segments. The list is empty by default, in which case no exemption applies and all dot-segments are excluded as before. Ignore-file rules continue to apply to exempted paths unless `AGENTMEM_HONOR_IGNORE_FILES=false`. The agents-folder exemption (below) is independent of and unaffected by this glob list.

#### Scenario: Hidden file excluded from listing
- **WHEN** defaults are in effect and the vault contains `Agents/<scope>/notes.md` and `Agents/<scope>/.tmp.md`
- **THEN** `list_memory_notes` returns only `notes.md`; `.tmp.md` is absent

#### Scenario: Hidden file inaccessible by direct read
- **WHEN** defaults are in effect and `read_memory_note` is called with virtual path `Agents/<scope>/.tmp.md`
- **THEN** the response is an MCP error with code `path_not_permitted`

#### Scenario: gitignore-matched file excluded
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=true` and the vault contains a `.gitignore` line `drafts/*.md` plus the file `Agents/<scope>/drafts/wip.md`
- **THEN** `list_memory_notes` does not include `drafts/wip.md` and a direct `read_memory_note` for it returns `path_not_permitted`

#### Scenario: generic .ignore file excludes consistently across listing and direct access
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=true` and the vault contains a `.ignore` line `scratch/*.md` plus the file `Agents/<scope>/scratch/wip.md`, with no matching `.gitignore` or `.obsidianignore` rule
- **THEN** `list_memory_notes` does not include `scratch/wip.md` AND a direct `read_memory_note`, `write_memory_note`, `edit`, or `delete` targeting it returns `path_not_permitted`

#### Scenario: Nested ignore file in a subfolder is honoured
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=true` and the vault contains `Agents/<scope>/drafts/.gitignore` with the line `*.tmp.md`, plus the files `Agents/<scope>/drafts/wip.tmp.md` and `Agents/<scope>/keep.tmp.md`
- **THEN** `list_memory_notes` excludes `drafts/wip.tmp.md` (the nested rule applies to its own subtree) and a direct access to it returns `path_not_permitted`, while `keep.tmp.md` outside that subtree remains visible and accessible
- **AND** the same exclusion holds when the nested file is `.ignore` or `.obsidianignore` instead of `.gitignore`

#### Scenario: Including hidden files globally
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=true`
- **THEN** dotfiles appear in listings and are directly readable (still subject to ignore-file rules unless also disabled), and the include-hidden glob list has no further effect

#### Scenario: Include-glob un-hides a dot-directory subtree
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=false`, `AGENTMEM_INCLUDE_HIDDEN_GLOBS=.obsidian/**`, and the vault contains `.obsidian/app.json`, `.obsidian/plugins/x/data.json`, and an unrelated `.cache/tmp.md`
- **THEN** `list_memory_notes` includes `.obsidian/app.json` and `.obsidian/plugins/x/data.json` and they are directly readable/writable, while `.cache/tmp.md` remains hidden and returns `path_not_permitted` on direct access

#### Scenario: Include-glob does not widen beyond its match
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=false`, `AGENTMEM_INCLUDE_HIDDEN_GLOBS=.obsidian/**`, and the vault contains `.obsidian/app.json` and a sibling `.git/config`
- **THEN** `.obsidian/app.json` is visible while `.git/config` remains excluded and returns `path_not_permitted` on direct access

#### Scenario: Disabling ignore-file enforcement
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=false`
- **THEN** `.ignore`, `.gitignore`, and `.obsidianignore` patterns are not consulted; the visible set is widened accordingly

#### Scenario: Agents folder itself never filtered out
- **WHEN** `AGENTMEM_AGENTS_DIR=.agents` (begins with `.`) and `AGENTMEM_INCLUDE_HIDDEN=false`
- **THEN** the agents folder is still recognised as the scoped/suffixed region and its contents remain visible to and writable by the owning scope; hidden filtering does NOT exclude the agents folder, independent of any include-hidden glob list
