## Context

`list_memory_notes` today lists visible paths via `Storage::list_visible`
(`src/storage.rs:337`), which returns a `BTreeSet`-derived `Vec<VirtualPath>` —
already sorted ascending, with zero per-note file reads. The handler then applies
the optional `path_prefix` retain in memory (`src/tools.rs:481`) before paginating.
The glob filter slots into exactly this spot, preserving the tool's cheap,
path-only identity.

## Goals / Non-Goals

- **Goal:** match notes by path shape without reading file contents.
- **Goal:** keep the filter purely in-memory over the already-listed paths.
- **Non-Goal:** content or frontmatter matching (that lives in `recall_memory_notes`).
- **Non-Goal:** ordering or directory-view changes (separate changes).

## Decisions

- **Match target = the clean, vault-root-relative virtual path** (the same string
  returned in `items` and accepted by `read_memory_note`), e.g.
  `Agents/diary/2026-06-10.md`. This keeps the glob consistent with what the agent
  sees and passes back, and is unambiguous about the agents-folder prefix.
- **Glob library: `globset`**, pinned to an exact version. It supports `*`, `**`,
  `?`, and character classes, and compiles a pattern once per call. There is no glob
  dependency today; this is the single new dependency and is flagged in the proposal.
- **`glob` composes with `path_prefix` via AND.** Both are independent retains; an
  entry must satisfy both. This avoids surprising precedence rules and lets an agent
  scope a broad glob to a subtree.
- **Invalid patterns fail fast** with `invalid_argument`, mirroring how
  `recall_memory_notes` rejects an invalid `regex` (`src/recall/mod.rs:629`).
- **Filter runs before pagination**, so `limit`/`cursor` page over the filtered set
  and the existing deterministic ordering is preserved.

## Risks / Trade-offs

- A new dependency (`globset`) is the only real cost; it is small, widely used, and
  pinned. Mitigation: exact version pin per project policy.
- Glob semantics differ from prefix matching (e.g. `**` crossing directory
  separators). Mitigation: scenarios pin the expected behavior, and `path_prefix`
  remains available for callers who want literal prefix semantics.
