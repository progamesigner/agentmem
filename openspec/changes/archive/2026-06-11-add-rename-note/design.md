## Context

Moving a note is currently three tool calls (`read`, `write`, `delete`) and
breaks every incoming reference: forward links are expanded/stripped by
`src/wikilink.rs`, but nothing updates the *referrers* when a target moves. The
`add-backlink-read` change introduces reverse resolution (`references_to`,
`resolve_target` exposed `pub(crate)`); this change builds rename on top of it.
Storage already provides the needed primitives: atomic full-file writes with
per-target advisory locks, `delete`, auto-created parent directories, and
recall's `on_write` hook (whose `apply_path` also handles a vanished file by
removing it from the index manifest).

## Goals / Non-Goals

**Goals:**
- One-call move that leaves the visible link graph exactly as consistent as it
  was before the move.
- All-or-nothing *validation*: every precondition (destination free, policy,
  leak guard, rewritability of every referrer) is checked before the first
  byte is written.
- Deterministic rewrite output (shortest unambiguous names, decorations
  preserved) identical to what the forward transform would produce if the
  links were written fresh against the post-move vault.

**Non-Goals:**
- Directory renames or bulk moves (single note only, mirroring every other
  tool).
- Transactional multi-file atomicity (see Risks — the filesystem has no
  multi-file rename; we order operations to make the non-atomic window
  harmless).
- Renaming agents-folder root core files (wrapper-managed, fixed names).

## Decisions

- **Tool shape: `rename_memory_note { path, new_path }`.** Mirrors
  `read`/`delete` naming and the vault-root-relative path convention.
  Returns `{ renamed: true, path, new_path, notes_rewritten }`.
- **Validate everything, then mutate, in a fixed order.** Phase 1 (no writes):
  resolve + visibility-check the source; check destination resolves, is
  policy-writable, is not root-reserved, and does not exist
  (`destination_exists`, a new error code — agents need to distinguish "pick
  another name" from "bad argument"); strip the source content to its clean
  form and re-expand it for the destination's region (surfacing the leak guard
  `write_denied` now, before any mutation); compute the referrer set via the
  backlink scan and verify every referrer's region is policy-writable; compute
  each referrer's rewritten content. Phase 2 (mutations): write the
  destination, rewrite each referrer atomically, delete the source last.
  Deleting last means a crash mid-flight leaves both copies present and every
  link resolvable to at least one of them — never a dangling reference.
- **Rewrite = re-target, not regenerate.** Referring notes are transformed by
  the existing `rewrite_links` walker: only link targets that resolve to the
  source are replaced; aliases, headings, embed markers, link text, and all
  other content bytes are untouched. The new target text is derived the same
  way the forward transform derives it: shortest unambiguous name against the
  post-move `LinkIndex` (source entry replaced by destination entry), suffixed
  for own-scope wikilinks, physical path for own-scope markdown links, clean
  for shared targets. This guarantees a subsequent read round-trips cleanly.
- **The moved note rewrites its own self-references.** The source note's
  content may link to itself; those targets are re-pointed at the destination
  during the Phase 1 content computation, so they neither dangle nor
  resurrect the old name.
- **Cross-scope safety falls out of existing guards, made explicit for one
  case.** A shared note can never reference a scoped note (existing leak-guard
  invariant), so renaming a scoped note never needs to touch shared referrers'
  *suffixes*. Renaming a *shared* note to a *scoped* destination would force
  shared referrers to embed the caller's suffix — Phase 1 detects this and
  refuses the whole rename with `write_denied`, the same code the forward leak
  guard uses.
- **Recall stays synchronous.** `on_write` is invoked for the destination and
  every rewritten referrer; it is also invoked for the deleted source path,
  which `apply_path_with` already treats as a removal (metadata read fails →
  manifest + index entry dropped). No reliance on the watcher for the
  server's own rename.
- **Concurrency: per-target locks only, last-writer-wins (design decision
  D5).** A concurrent external edit to a referrer between Phase 1 and its
  Phase 2 write can be overwritten, exactly as any two concurrent writers can
  race today. We accept this rather than introduce a global vault lock;
  per-note writes remain individually atomic.

## Risks / Trade-offs

- [No multi-file atomicity: a crash between phases leaves the note at both
  paths] → Operation order makes the window benign (no dangling links; the
  duplicate is visible and trivially deleted); each individual write is
  atomic + fsynced. Documented behavior, not silent corruption.
- [Referrer scan cost: one content read per visible note] → Same cost profile
  as a `backlinks: true` read, accepted there; rename is a rare,
  deliberate operation.
- [Phase-1 validation races a concurrent writer creating the destination] →
  The destination write uses the same per-target lock as any write; the racing
  writer's content is replaced last-writer-wins, consistent with D5. The
  existence check still catches the overwhelmingly common case (agent retrying
  or picking an occupied name).
- [Rewritten referrers may collapse to a *different* shortest name than the
  author originally wrote (e.g. a qualified `[[topics/rust]]` becoming
  unambiguous `[[rust-async]]`)] → The rewrite always emits the canonical
  shortest unambiguous form — the same normalization the write transform
  already applies — so round-trip behavior stays consistent.
