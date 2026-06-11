## Why

An agent reorganizing its memory today must read + write + delete, and every
incoming `[[link]]` silently dangles — the wikilink feature gives notes a graph
but offers no safe way to move a node. Renaming with incoming-link rewrite is
the missing half of that feature (it is exactly what Obsidian does on rename).

## What Changes

- New tool `rename_memory_note` taking `path` (the existing note) and
  `new_path` (the destination), both vault-root-relative virtual paths. It
  moves the note and rewrites every visible incoming link (all supported
  forms, alias/heading/embed decorations preserved) to resolve to the new
  location.
- The moved note's own content is re-run through the write-side link transform
  for the destination region, so cross-region moves keep links valid and the
  existing cross-scope leak guard applies (a move that would persist a scoped
  suffix in the shared region is refused).
- Guard rails: the destination must not already exist (new error code
  `destination_exists`); agents-folder root-level paths are rejected on both
  ends (core files are wrapper-managed); both source and destination regions
  must be policy-writable; if rewriting any referring note would require
  writing a region the policy forbids, the whole rename is refused before
  anything changes.
- Backlink scanning reuses the reverse-resolution helper introduced by the
  `add-backlink-read` change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: new `rename_memory_note` tool requirement.
- `wikilink-references`: new requirement that incoming references follow a
  rename (rewrite preserves decorations and re-derives shortest unambiguous
  names).

## Impact

- Code: `src/tools.rs` (new handler + schema struct + tool registration),
  `src/wikilink.rs` (target-rewrite helper reusing `rewrite_links`),
  `src/error.rs` (`destination_exists` code), `src/storage.rs` (no new
  primitives expected — composes read/write_atomic/delete), `tests/tools.rs`,
  `tests/wikilinks.rs`, schema snapshots, README tool table, session-context
  tools guide (auto-generated).
- Dependencies: none. Depends on the `add-backlink-read` change landing first
  (shared reverse-resolution helper).
