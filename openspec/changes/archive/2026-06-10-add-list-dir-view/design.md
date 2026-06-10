## Context

`list_memory_notes` (`src/tools.rs:455`) builds a vector of visible note virtual
paths from `Storage::list_visible` and paginates it. A directory view is a pure
transform of that vector: split each path on `/`, collect every ancestor directory
into a deduplicated set. No new storage call and no file reads are required.

## Goals / Non-Goals

- **Goal:** give an agent a map of the folder structure before it drills into files.
- **Non-Goal:** a nested/tree JSON shape — the items stay a flat list of directory
  paths to match the existing response contract and pagination.
- **Non-Goal:** counts per directory or file metadata (could be a later addition).

## Decisions

- **`view` is an enum: `files` (default) | `dirs`.** Default is the current
  behavior, so the change is backward compatible.
- **`dirs` returns every ancestor directory**, not just immediate children. For
  `Agents/topics/rust.md` that yields `Agents` and `Agents/topics`. Returning all
  ancestors gives the agent the full reachable skeleton in one call rather than
  forcing repeated prefix drilling — which is the orientation problem we are solving.
- **Deduplicate via a `BTreeSet`**, which also yields the deterministic ascending
  ordering the contract requires; pagination then pages over the directory set.
- **`path_prefix` is applied first**, then directories are derived from the filtered
  files, so `view="dirs"` + `path_prefix="topics"` describes only that subtree.
- **Unknown `view` values fail fast** with `invalid_argument`.

## Risks / Trade-offs

- The item shape is overloaded: the same `items` array holds files or directories
  depending on `view`. Mitigation: the directory paths are unambiguous (they carry
  no trailing file segment), and the tool description states the distinction. An
  alternative — a separate tool — was rejected as heavier than the problem warrants.
- Interaction with a future `glob` change: glob filters files, then directories are
  derived from the filtered files. The two compose cleanly because both are
  in-memory transforms over the same path vector; ordering of landing is irrelevant.
