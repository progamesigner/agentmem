## Why

Only the diary can append. Every other growing note — `topics/LOG.md` from the
suggested layout, task logs, running observations — forces the agent into a
read-modify-write round trip that both costs a tool call and exposes a
lost-update window between concurrent writers. The server already owns the
race-free primitive (`read_modify_write` under the per-target lock); it is just
not reachable through the generic write tool.

## What Changes

- `write_memory_note` gains an optional boolean `append` argument. When `true`,
  `content` is appended to the existing note verbatim (exact bytes, no implicit
  separator — the agent controls layout); a missing note is created with
  `content` as its full body, so append is usable without an existence check.
- The append runs through the storage layer's existing locked
  read-modify-write, so concurrent appends to the same note serialise and none
  are lost — the same guarantee the diary already has.
- The appended fragment goes through the write-side link transform (and its
  cross-scope leak guard), exactly like full-write content.
- All existing guards unchanged: root-reserved rejection, policy gates,
  visibility filters. `bytes_written` reports the note's full size after the
  write, matching `append_diary_entry`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `write_memory_note` tool gains an optional `append` mode.

## Impact

- Code: `src/tools.rs` (`WriteFields` schema + handler branch), `tests/tools.rs`,
  schema snapshots, README tool table.
- Dependencies: none. `src/storage.rs` already provides `read_modify_write`.
