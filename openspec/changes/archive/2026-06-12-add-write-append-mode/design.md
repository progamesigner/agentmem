## Context

`append_diary_entry` (`src/tools.rs:749`) is today's only append path; it
composes `Storage::read_modify_write` (`src/storage.rs:192`) — a locked
read-transform-write that serialises concurrent callers per target — with the
link transform on the appended fragment. Generic notes get only
`write_memory_note`'s full replace, so agents emulate append with read + write
and can lose updates between the two calls. This change routes the same proven
primitive through the generic tool.

## Goals / Non-Goals

**Goals:**
- Race-free, single-call append to any policy-writable note.
- Zero behavior change for existing callers (flag absent = today's full write).

**Non-Goals:**
- Prepend, insert-at, or any other positional write (that is `edit_memory_note`'s
  territory).
- Appending to core root files (wrapper-managed; the diary wrapper remains the
  curated append for date-keyed entries).
- Implicit formatting (separators, headings, timestamps) — the diary wrapper
  owns opinionated formatting; the generic tool stays byte-exact.

## Decisions

- **A flag on `write_memory_note`, not a new tool.** Append is a write mode,
  not a different capability; the small-surface principle holds. The schema
  documents that `append: true` changes `content` from "full new contents" to
  "bytes appended verbatim".
- **Exact-bytes semantics, missing file = create.** The server adds no
  separator: an agent appending list items sends `"- item\n"`. A missing
  target is created with `content` as the body (mirroring
  `read_modify_write`'s `None` arm), so append needs no prior existence
  check and stays idempotent-friendly for "ensure log exists and add line"
  flows.
- **Link transform applies to the fragment only.** The existing on-disk
  content is already in persisted (expanded) form; transforming the new
  fragment before concatenation matches the diary's behavior exactly and
  keeps the leak guard effective for the new bytes without re-interpreting
  old ones.
- **`bytes_written` = total note size after the write.** This is what
  `write_atomic`/`read_modify_write` return and what `append_diary_entry`
  already reports; consistency beats reporting the fragment length.

## Risks / Trade-offs

- [Unbounded note growth via repeated appends] → Same exposure as the diary
  today; curation is the agent's/human's job by design (plain-markdown vault).
- [Agents forgetting the flag and replacing instead of appending] → The
  failure mode already exists (full write is the default today); the schema
  description spells the distinction out.
