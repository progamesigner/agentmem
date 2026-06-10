## Why

Agents cannot find a note by its name or path shape. `list_memory_notes` filters
only by exact `path_prefix`, and `recall_memory_notes` matches note *content*, not
paths — so a lookup like "the diary note for 2026-06-10" silently returns nothing.
This was the reported symptom: a date regex through recall and prefix guesses
(`Journal`, `Daily`) all came back empty.

## What Changes

- Add an optional `glob` argument to `list_memory_notes` that filters the visible
  set by glob pattern over each entry's clean, vault-root-relative virtual path
  (e.g. `Agents/diary/2026-*`, `**/release.md`, `Agents/topics/**/*.md`).
- `glob` composes with `path_prefix` (both apply; AND semantics) and with
  pagination, preserving the existing deterministic ordering.
- `list_memory_notes` remains a cheap, path-only tool: the glob is an in-memory
  filter over the already-listed paths and reads no file contents.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `list_memory_notes` tool gains an optional `glob` path filter.

## Impact

- Code: `src/tools.rs` (`list_memory_notes` handler and `ListFields` schema).
- Dependencies: adds a glob-matching crate (`globset`), pinned to an exact version —
  there is no glob dependency today.
