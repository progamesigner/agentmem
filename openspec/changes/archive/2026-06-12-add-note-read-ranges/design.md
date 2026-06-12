# Design: add-note-read-ranges

## Context

`read_memory_note` returns whole files; `read_memory_notes` batches up to 20 whole files with per-entry errors. Both share `read_one` (`src/tools.rs`), which is `read_raw` (policy gate, suffix resolution, visibility, read) followed by the own-suffix link strip. Notes such as diaries grow without bound, and agents currently have no way to page through them.

## Goals / Non-Goals

**Goals:**
- Line-range reads on both read tools with one shared semantics.
- Byte-faithful slices: concatenating consecutive ranges reproduces the whole note.
- Zero contract change for calls that do not use ranges.

**Non-Goals:**
- Heading/section-addressed reads (more design surface; can layer on later).
- Ranges on any write or edit tool.
- Byte-offset ranges.

## Decisions

1. **`offset` (1-based) + `limit`, mirroring the Claude Code Read tool.** Agents already know this shape, and it composes (offset alone = tail-from-line, limit alone = head). Alternative `line_start`/`line_end` rejected: same power, unfamiliar shape. Schema declares both as integers with `minimum: 1`; the handler re-validates (schema-level keywords are not enforced server-side for all clients).
2. **Slice after the link strip.** `strip_links` rewrites targets inline and never adds or removes lines, so line numbers are identical between the stored and agent-facing forms; slicing the stripped content guarantees the agent never sees a suffix and keeps offsets stable across whole-note and ranged reads. The slice helper splits with `split_inclusive('\n')` so delimiters (including a missing final newline) are preserved exactly.
3. **`total_lines` only when a range is requested.** Keeps the default response byte-identical (no churn for existing callers or `schema_snapshots.rs` beyond the new input fields) while giving paging clients what they need. An empty note has `total_lines: 0`.
4. **Out-of-range offset returns empty content, not an error.** Paging loops terminate naturally on empty content + `total_lines`; erroring would force callers to pre-compute line counts. `offset`/`limit` of 0 is `invalid_argument` — silent reinterpretation as 1 would mask bugs.
5. **Batch entries become `string | { path, offset?, limit? }`.** Accepting both forms keeps existing string-array callers working. The input schema expresses the union via `anyOf`; the handler maps each entry to `(path, range)` and funnels both tools through one `read_one`-plus-slice helper so single and batch semantics cannot drift. Structural validation failures (non-string non-object entry, missing `path`, zero offset/limit) are call-level `invalid_argument`, matching the existing batch contract; per-entry errors remain reserved for path resolution and IO.

## Risks / Trade-offs

- **Line-count semantics for `\r\n` files** → splitting on `\n` keeps `\r` inside the line, which is byte-faithful and matches `total_lines` to what an agent sees; documented in the helper.
- **`anyOf` entry schema may render poorly in some clients** → the description spells out both forms; string entries keep working regardless.
- **Two tools sharing slice semantics could drift** → single shared helper used by both handlers; tests assert single-vs-batch parity on the same note.

## Open Questions

(none)
