## 1. Schema

- [x] 1.1 Add an optional `order: Option<String>` field to `ListFields` in `src/tools.rs`, documenting the `name_asc` (default) and `name_desc` values.

## 2. Handler

- [x] 2.1 In `list_memory_notes`, read the optional `order` argument and include it in the `resolve_scope` reserved-key list.
- [x] 2.2 Parse `order` into an enum; reject unrecognized values with `invalid_argument`; default to `name_asc`.
- [x] 2.3 For `name_desc`, reverse the visible-path vector before existing retains and pagination; leave `name_asc` as the current path.

## 3. Tests

- [x] 3.1 Default (unset) ordering is ascending by path.
- [x] 3.2 `name_desc` returns entries in descending path order.
- [x] 3.3 Unrecognized `order` value yields `invalid_argument`.
- [x] 3.4 Ordering is applied before pagination (page boundaries reflect the chosen order).

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`.
