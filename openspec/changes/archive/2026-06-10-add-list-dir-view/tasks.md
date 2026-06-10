## 1. Schema

- [x] 1.1 Add an optional `view: Option<String>` field to `ListFields` in `src/tools.rs`, documenting the `files` (default) and `dirs` values.

## 2. Handler

- [x] 2.1 In `list_memory_notes`, read the optional `view` argument and include it in the `resolve_scope` reserved-key list.
- [x] 2.2 Parse `view` into an enum; reject unrecognized values with `invalid_argument`; default to `files`.
- [x] 2.3 For `dirs`, after applying `path_prefix`, derive the deduplicated set of every ancestor directory of the visible files (via a `BTreeSet` for deterministic order), then paginate that set instead of the file list.

## 3. Tests

- [x] 3.1 Default (unset) view returns file paths unchanged.
- [x] 3.2 `dirs` view returns the distinct ancestor directory paths, deduplicated and ordered.
- [x] 3.3 `dirs` view honors `path_prefix` (directories derived from the filtered subset).
- [x] 3.4 Unrecognized `view` value yields `invalid_argument`.
- [x] 3.5 `dirs` view paginates over the directory set.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`.
