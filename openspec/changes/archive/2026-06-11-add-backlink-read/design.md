## Context

Forward link resolution is fully built: `src/wikilink.rs` parses every link form
via `rewrite_links` and resolves targets Obsidian-style with `resolve_target`
against a `LinkIndex` (`src/storage.rs:65`) built per call from a names-only
directory walk. Nothing maps the other direction. The read path
(`src/tools.rs:602`) returns content only; an agent wanting referrers must list
and read the whole visible set itself, paying one tool round-trip per note.

A planned `rename_memory_note` change needs "find every note whose links resolve
to X" as its rewrite set, so this change deliberately produces a reusable
reverse-resolution helper rather than read-handler-private logic.

## Goals / Non-Goals

**Goals:**
- Answer "what links here?" in one tool call, with resolution semantics
  identical to the forward transform.
- Preserve structural scope isolation: the scan only ever touches notes the
  caller could read anyway.
- Zero new state: no reverse index, no watcher, no configuration.

**Non-Goals:**
- Link *positions* or per-link snippets in the result (paths only).
- A persistent/incremental backlink index (revisit only if scan cost is proven
  to matter).
- Rename-with-rewrite itself (separate change; it consumes the helper added
  here).

## Decisions

- **Surface = a `backlinks` flag on `read_memory_note`, not a new tool.** The
  question is always asked about a specific note the agent is looking at, and
  the small generic tool surface is a project value. The flag defaults to
  absent/false and the result field is omitted entirely unless requested, so
  existing callers see byte-identical responses. Alternative — a separate
  `list_backlinks` tool — rejected as surface growth with no added capability.
- **On-demand scan over the visible set, no reverse index.** For each note
  visible to the caller (same `policy.list_visible_regions` set as
  `list_memory_notes`), read the on-disk content and collect link targets with
  the existing `rewrite_links` parser in collector mode. This matches the
  existing pattern of building the `LinkIndex` fresh per write call, and keeps
  the feature correct under external (human) edits with no reconciliation
  machinery. Alternative — maintaining a reverse index in the recall engine —
  rejected: backlinks must work with `AGENTMEM_RECALL_BACKEND=off`, and the
  engine's lifecycle (eviction, freshness) would leak into link semantics.
- **Resolve raw on-disk targets by stripping the caller's suffix per target.**
  Stored own-scope links carry the suffixed form (`[[rust.jarvis.tony]]`).
  Rather than `strip_links`-ing each whole note (a second full rewrite pass),
  the scan strips the suffix from each collected target with the existing
  `strip_suffix_from_link_target` / markdown reversal helpers, then resolves the
  clean target with `resolve_target` and compares the resolved entry's
  `clean_path` to the read target's clean path. Exposes `resolve_target` as
  `pub(crate)`; behavior unchanged.
- **A referrer counts once.** The result is a deduplicated, ascending-sorted
  list of clean virtual paths — deterministic across calls, mirroring listing
  order guarantees. Multiple links from one note to the target do not repeat
  the path. Self-links count (a note linking to itself appears in its own
  backlinks): one uniform rule, no special case.
- **Resolution must land on the target, not merely mention it.** A link
  `[[rust]]` in a vault where `topics/rust` and `Lang/rust` both exist resolves
  per `resolve_target`'s tie-breaks to exactly one entry; it is a backlink only
  for that entry. This keeps backlinks the exact inverse of what forward
  navigation does.
- **Schema change: `PathFields` stays for `delete_memory_note`; read gets its
  own `ReadFields { path, backlinks: Option<bool> }`.** Sharing the struct
  would advertise a meaningless `backlinks` argument on delete.

## Risks / Trade-offs

- [O(visible notes) content reads per `backlinks: true` call] → Acceptable for
  agent-memory vaults (hundreds to low thousands of small markdown files); the
  default read path is untouched. If a deployment proves otherwise, an
  in-memory reverse index can be added later without changing the tool
  contract.
- [Scan reads files that may be concurrently written] → Each note read is the
  same racy-but-atomic read the rest of the server performs; a torn view is
  impossible (atomic rename) and a stale view self-corrects on the next call
  (per design decision D5, last-writer-wins).
- [Non-UTF-8 or unreadable note in the visible set] → Skip it (it cannot
  contain resolvable links); do not fail the whole read.
