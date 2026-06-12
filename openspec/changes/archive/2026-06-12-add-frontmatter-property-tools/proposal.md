## Why

Frontmatter properties are queryable (tantivy `filters`) but not ergonomically
writable: setting `status: done` on a note means a full-file rewrite or a
brittle string edit against YAML the agent must format itself. Properties are
structured data and deserve a structured surface — both to make the existing
filters practically usable and to keep agent-written frontmatter
Obsidian-valid.

## What Changes

- New tool `read_note_properties` (`path`) returning the note's frontmatter as
  a JSON object (`{ properties }`); absent frontmatter yields an empty object.
- New tool `update_note_properties` (`path`, `properties`) that merges the
  given keys into the note's frontmatter atomically: each key upserts, an
  explicit `null` deletes a key, the body is untouched byte-for-byte, and the
  result returns the full post-update property set. Updating all keys away
  removes the frontmatter block entirely; updating a note with no block
  creates one. Existing frontmatter that is not valid YAML is refused with
  `invalid_argument` rather than clobbered.
- YAML parsing moves into the default build: `serde_yaml` and
  `src/frontmatter.rs` lose their `recall-tantivy` feature gate, so the
  property tools exist on every build. (The tantivy backend keeps its gate;
  only the YAML layer becomes unconditional.)
- Property update guards match writes: root core files stay wrapper-only,
  policy gates and visibility filters apply, the recall index is updated
  synchronously so tantivy `filters` see fresh values.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: two new tool requirements (`read_note_properties`,
  `update_note_properties`).

## Impact

- Code: `Cargo.toml` (`serde_yaml` becomes a required dependency — already
  shipped in tantivy builds, now unconditional; it is in maintenance mode,
  which we accept as the status quo the project already depends on),
  `src/frontmatter.rs` (un-gate; add serialization), `src/tools.rs` (two
  schema structs + handlers + registration), `tests/tools.rs`, schema
  snapshots, README.
- Build: the default binary grows by the YAML parser; the
  `recall-tantivy`-only size argument in `Cargo.toml`'s feature comment needs
  updating.
