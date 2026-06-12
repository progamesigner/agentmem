# Design: add-batch-note-writes

## Context

`write_memory_note` gates (reserved roots → policy → visibility), expands links against a `LinkIndex` built from the caller's visible set, then writes atomically and notifies recall. `rename_memory_note` already establishes the repo's two-phase shape: phase 1 validates everything and computes all new contents with zero writes; phase 2 mutates. It also establishes the index-seeding precedent: `post_rename_index` builds a hypothetical visible set so links resolve against the post-mutation world.

## Goals / Non-Goals

**Goals:**
- One call writes up to 20 notes with the exact per-entry semantics of `write_memory_note`.
- No partial batch on validation failure; intra-batch links resolve.

**Non-Goals:**
- Cross-entry transactionality against crashes (each file write stays atomic; the batch does not get a journal).
- Batch edit/delete/rename variants.
- Raising the 20-entry cap (mirrors `read_memory_notes`; revisit if real usage wants more).

## Decisions

1. **All-or-nothing validation, two-phase like `rename_memory_note`.** Phase 1 walks every entry: `VirtualPath` parse, `reject_if_root_reserved`, `gate_write`, resolve + visibility, link expansion (which runs the leak guard) — collecting the final content per entry. Phase 2 applies in request order. Alternative — best-effort per entry mirroring `read_memory_notes` — was rejected: a half-applied multi-note update (fact written, index not) is worse than a failed read, and the validate-first shape was already paid for by rename.
2. **Pre-seed the link index with the batch's own paths.** Build the normal visible-set `LinkIndex`, then insert each batch entry's clean path with its region (the `post_rename_index` pattern). This makes "create a note and link to it from its INDEX in one call" work, and makes expansion order-independent. Seeding happens before any entry expands, so entry order does not affect resolution.
3. **Duplicate virtual paths are `invalid_argument`.** With `append` in play, two entries on one path have order-dependent, surprising semantics ("replace then append"? "append twice"?); rejecting keeps every batch a function of its entry set, not its ordering. Comparison happens on the parsed virtual path, not the raw string.
4. **Append entries use the locked read-modify-write in phase 2, not precomputed bytes.** Precomputing an append's final content in phase 1 would race external editors between phases; deferring to the per-target lock keeps the single-write guarantee. Consequence: `bytes_written` for appends is known only at apply time, which is fine since results are produced in phase 2.
5. **Crash posture is documented, not engineered away.** A crash mid-phase-2 leaves a clean prefix of the batch applied — same stance as rename's documented destination-first ordering. Anything stronger needs a write-ahead journal, which the storage layer deliberately does not have.

## Risks / Trade-offs

- **[Validation-to-apply race with external editors]** → phase 1's visibility/policy checks could be stale by phase 2; identical exposure already accepted for rename, and per-target locks keep each file internally consistent.
- **[Seeded index can resolve a link to an entry that later fails to apply]** → impossible for validation failures (whole batch rejected); only a crash can leave a dangling link, covered by the documented crash posture.
- **[20-entry cap may feel arbitrary for bulk migrations]** → matches batch read; bulk vault surgery is an offline/Obsidian job, not a tool-call job.

## Open Questions

(none)
