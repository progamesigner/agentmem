# Expand wikilinks in frontmatter property values

## Why

The link transform already covers frontmatter *by accident of position* — `write_memory_note` scans the whole file, so a property like `related: "[[rust]]"` is persisted suffixed — but the property tools bypass it entirely: `read_note_properties` returns raw stored bytes, **leaking the caller's scope suffix** (violating the wikilink-references rule that an agent never observes a suffix), and `update_note_properties` persists clean targets that don't resolve in Obsidian and skips the cross-scope leak guard.

## What Changes

- `update_note_properties` applies the write-side link transform to every string value in the supplied properties (recursing into arrays and nested objects): own-scope targets are expanded to the suffixed form, shared targets stay clean, and the cross-scope leak guard refuses a shared-region note whose property links into the caller's own scope.
- `read_note_properties` strips the caller's own scope suffix from every string value in the returned properties (same recursion), so the property surface matches the read-path view of the body.
- Property values round-trip: what an agent writes through the property tool is what it reads back, while the persisted form resolves in Obsidian.
- Note bodies remain untouched by the property tools; non-string values (numbers, booleans) are untouched data.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `read_note_properties` and `update_note_properties` requirements change from "no link transform" to the strip/expand behavior above.
- `wikilink-references`: a new requirement extends the transform's coverage to frontmatter property string values.

## Impact

- `src/tools.rs`: `read_note_properties` and `update_note_properties` handlers.
- `src/wikilink.rs` (or a small helper beside it): recursive expand/strip over a `serde_json::Value`'s string leaves, reusing `rewrite_links`.
- `tests/tools.rs`, `tests/wikilinks.rs`.
- Related change: `strip-recall-index-suffixes` aligns the recall index with this clean-form view so property filters compare what agents see; the two changes are independent to implement but complementary (this one fixes the tool surface, that one the index).
