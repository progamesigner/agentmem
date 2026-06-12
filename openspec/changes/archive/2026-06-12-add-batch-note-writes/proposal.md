# Add a batch note write tool

## Why

Routine memory maintenance writes several notes at once — a topic fact plus its `INDEX.md` and `LOG.md`, or a workspace note plus `workspaces/INDEX.md` — and today each write is a separate `write_memory_note` round-trip with no way to validate the set together. A batch tool cuts the round-trips and lets a coherent multi-note update be checked as a unit before anything lands.

## What Changes

- New `write_memory_notes` tool: 1 to 20 entries `{ path, content, append? }`, mirroring `read_memory_notes`'s batch shape on the write side.
- All-or-nothing validation: every entry is fully validated (path, wrapper-reserved roots, policy, visibility, link expansion including the cross-scope leak guard) before any file is touched; any failure rejects the whole call with no writes.
- Intra-batch link resolution: the link index used for expansion is pre-seeded with the batch's own paths, so one entry can `[[link]]` to a note created by another entry in the same call.
- Entries resolving to the same virtual path are rejected (`invalid_argument`).
- Per-entry results `{ path, bytes_written }` in request order; each individual write is atomic and updates the recall index synchronously, but the batch as a whole is not transactional (documented crash posture, same as `rename_memory_note`).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: a new `write_memory_notes` tool requirement is added (the existing single-write requirements are unchanged).

## Impact

- `src/tools.rs`: new entry in `TOOL_NAMES` and `build_tools`, new schema struct, new handler with the two-phase validate/mutate shape already used by `rename_memory_note`; module doc tool count.
- `src/storage.rs` or `src/wikilink.rs`: a small helper to seed a `LinkIndex` with additional pending paths (the `post_rename_index` pattern generalized).
- `tests/tools.rs`, `tests/wikilinks.rs`, `tests/schema_snapshots.rs`.
- The generated session-context tools guide picks up the new tool automatically from the tool list.
