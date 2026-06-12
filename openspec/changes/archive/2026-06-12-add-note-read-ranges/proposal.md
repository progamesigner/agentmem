# Add line ranges to the note read tools

## Why

Long-lived notes (diaries, logs, large topic notes) grow beyond what an agent wants to pull into context at once; today `read_memory_note` and `read_memory_notes` can only return whole files, so an agent that needs one section pays the full token cost on every read.

## What Changes

- `read_memory_note` accepts optional `offset` (1-based first line) and `limit` (maximum line count) arguments and returns only that line range of the note.
- When a range is requested, the structured result additionally carries `total_lines` (the line count of the full note) so the agent knows whether more content exists.
- `read_memory_notes` entries may be either a plain string path (whole note, unchanged) or an object `{ path, offset?, limit? }` requesting a range for that entry; ranged entries carry `total_lines` in their per-entry result.
- The range is applied to the agent-facing content (after the own-suffix link strip), so line numbers are stable across reads and the suffix never leaks through a slice boundary.
- Responses without range arguments are byte-identical to today's behavior — no new fields, no contract change for existing callers.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `read_memory_note` tool requirement gains optional `offset`/`limit` arguments and `total_lines` reporting; the `read_memory_notes` tool requirement gains object-form entries with per-entry ranges.

## Impact

- `src/tools.rs`: `ReadFields`/`BatchReadFields` schemas, `read_memory_note`/`read_memory_notes` handlers, a shared line-slice helper.
- `tests/tools.rs`: range, EOF, error, and strip-interaction cases for both tools.
- `tests/schema_snapshots.rs`: both tools' input schemas change.
- Tool descriptions (and therefore the generated session-context tools guide) mention the range arguments.
