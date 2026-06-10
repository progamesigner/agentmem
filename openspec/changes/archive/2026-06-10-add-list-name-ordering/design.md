## Context

`Storage::list_visible` (`src/storage.rs:337`) returns paths from a `BTreeSet`, so
the list arrives already sorted ascending by clean virtual path. The
`list_memory_notes` handler (`src/tools.rs:455`) paginates that vector directly.
Ascending order is therefore free; descending is a single reverse before pagination.

## Goals / Non-Goals

- **Goal:** let callers choose ascending or descending path order.
- **Non-Goal:** recency/mtime ordering — that would require a `stat()` per file and
  push `list` off its cheap, no-file-read path. Explicitly out of scope to keep the
  tool's identity intact.
- **Non-Goal:** glob filtering and directory view (separate changes).

## Decisions

- **`order` is an enum: `name_asc` (default) | `name_desc`.** Naming it `name_*`
  leaves room for a future `recency_*` should we ever decide to pay the stat cost,
  without overloading a bare `asc`/`desc`.
- **`name_asc` is the existing behavior**, so the default is a no-op and the change
  is backward compatible.
- **`name_desc` reverses the already-sorted vector** before applying `path_prefix`
  (and, once landed, `glob`) retains and pagination. Sort key is the clean
  virtual-path string, matching the existing deterministic order.
- **Unknown values fail fast** with `invalid_argument`, consistent with other
  argument validation in the handler.

## Risks / Trade-offs

- Minimal. The only subtlety is interaction with `cursor`: a cursor is an opaque
  offset into the ordered set, so a client must keep `order` stable across a
  paging sequence. This matches the existing contract that pagination assumes
  identical arguments across calls.
