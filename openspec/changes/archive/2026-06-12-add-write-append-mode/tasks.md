## 1. Schema

- [x] 1.1 Add `append: Option<bool>` to `WriteFields` in `src/tools.rs`, with a description distinguishing full-replace from verbatim append (exact bytes, no separator, missing note created).

## 2. Handler

- [x] 2.1 In `write_memory_note`, accept `append` in the tool-fields list; when `true`, route through `gated_write` with `storage.read_modify_write` concatenating existing content (or creating from `content` when absent); the link transform applies to the fragment before concatenation; full-write path untouched.

## 3. Tests

- [x] 3.1 `tests/tools.rs`: append extends verbatim (no separator), creates a missing note, reports total byte count, and round-trips appended `[[links]]` through expand/strip.
- [x] 3.2 Guard parity: append rejected on root core files, policy-denied regions, and visibility-excluded targets with the same codes as full write.
- [x] 3.3 Concurrency: parallel appends to one note all land exactly once (mirror the existing `concurrent_appends_are_serialised` storage test at the tool layer).
- [x] 3.4 Update schema snapshots and the README tool table.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
