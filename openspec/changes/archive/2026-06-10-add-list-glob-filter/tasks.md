## 1. Dependency

- [x] 1.1 Add `globset` to `Cargo.toml` pinned to an exact version; refresh `Cargo.lock`.

## 2. Schema

- [x] 2.1 Add an optional `glob: Option<String>` field to `ListFields` in `src/tools.rs` with a description covering virtual-path glob semantics and composition with `path_prefix`.

## 3. Handler

- [x] 3.1 In `list_memory_notes`, read the optional `glob` argument and include it in the `resolve_scope` reserved-key list.
- [x] 3.2 Compile the glob with `globset`; on a compile error return `invalid_argument`.
- [x] 3.3 Apply the compiled glob as an in-memory retain over the visible paths, alongside the existing `path_prefix` retain (AND semantics), before pagination.

## 4. Tests

- [x] 4.1 Glob filter returns only matching virtual paths (e.g. `Agents/diary/2026-*`).
- [x] 4.2 `glob` and `path_prefix` together require both to match.
- [x] 4.3 Invalid glob pattern yields `invalid_argument`.
- [x] 4.4 Glob filtering preserves deterministic ordering and pagination.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`.
