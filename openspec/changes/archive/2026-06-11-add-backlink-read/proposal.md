## Why

Wikilinks make the vault a graph, but the graph is traversable only forward: a
note's `[[links]]` resolve outward, while "which notes reference this one?" has
no answer short of listing and reading the entire visible set. Context
gathering ("what feeds into this topic?") and safe reorganization both need the
inverse direction — and an upcoming rename tool needs the same reverse
resolution to rewrite incoming links.

## What Changes

- `read_memory_note` gains an optional boolean `backlinks` argument. When
  `true`, the structured result carries a `backlinks` array: the clean virtual
  paths of every visible note containing at least one link that resolves to the
  target note, deduplicated and deterministically ordered.
- Backlink resolution mirrors forward wikilink resolution exactly (Obsidian
  trailing-segment matching, own-scope-first tie-break) and counts every
  supported link form: `[[target]]`, `[[target|alias]]`, `[[target#heading]]`,
  `![[embeds]]`, and relative markdown links `[text](path.md)`.
- Backlinks are computed on demand by scanning the caller's visible set; no new
  persistent state, index, or configuration. Structural isolation is preserved:
  another scope's notes are never scanned, so their links to a shared note are
  invisible.
- `src/wikilink.rs` exposes its target-resolution internals to the scan
  (refactor; no behavior change to the existing read/write transforms).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `read_memory_note` tool gains an optional `backlinks`
  argument and a `backlinks` result field.
- `wikilink-references`: new requirement that backlink resolution is the exact
  inverse of forward link resolution over the caller's visible set.

## Impact

- Code: `src/tools.rs` (`read_memory_note` handler; `PathFields` grows into a
  read-specific schema struct), `src/wikilink.rs` (expose `resolve_target` /
  add a reference-collection helper), `tests/tools.rs`, `tests/wikilinks.rs`.
- Performance: a `backlinks: true` read costs one content read per visible note
  (the default read path is unchanged). Acceptable for agent-memory vault
  sizes; documented in design.
- Dependencies: none.
