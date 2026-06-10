## Why

Faced with a large vault, an agent gets a long flat list of file paths from
`list_memory_notes` and often can't tell what folders even exist — so it guesses
prefixes (`Journal`, `Daily`) that don't match the real layout (`diary/`, `topics/`)
and gives up on the tool. A directory-structure view lets the agent orient first,
then drill in.

## What Changes

- Add an optional `view` argument to `list_memory_notes` accepting `files` (default,
  current behavior) and `dirs`.
- In `dirs` view, the response items are the distinct directory paths derived from
  the visible set (every ancestor directory of a visible note), deduplicated and in
  the same deterministic order — instead of individual file paths.
- The `dirs` view honors the existing `path_prefix` filter and pagination, and
  reads no file contents: directories are derived purely from the listed paths.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `list_memory_notes` tool gains an optional `view` argument with
  a directory-structure mode.

## Impact

- Code: `src/tools.rs` (`list_memory_notes` handler and `ListFields` schema).
- No new dependencies.
