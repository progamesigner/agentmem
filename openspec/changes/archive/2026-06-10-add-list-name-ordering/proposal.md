## Why

`list_memory_notes` always returns paths in ascending path order. Agents paging
through a large vault often want the reverse — e.g. the most recent date-named
diary entries first (`Agents/diary/2026-06-10.md` before `…2026-01-01.md`) — but
have no way to ask for it and must page to the end instead.

## What Changes

- Add an optional `order` argument to `list_memory_notes` accepting `name_asc`
  (default, current behavior) and `name_desc`.
- Ordering is by the clean virtual path string; `name_desc` reverses the existing
  deterministic order. Pagination via `limit`/`cursor` operates over the ordered set.
- `list_memory_notes` stays path-only and cheap: ordering is a sort of the
  already-listed paths and reads no file contents.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `list_memory_notes` tool gains an optional `order` argument.

## Impact

- Code: `src/tools.rs` (`list_memory_notes` handler and `ListFields` schema).
- No new dependencies.
