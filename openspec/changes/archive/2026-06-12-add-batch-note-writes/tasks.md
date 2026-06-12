# Tasks: add-batch-note-writes

## 1. Schema & registration

- [x] 1.1 Add a `BatchWriteFields` schema struct (`notes: Vec<{path, content, append?}>`, 1–20) with doc comments covering all-or-nothing validation, intra-batch links, and the crash posture
- [x] 1.2 Register `write_memory_notes` in `TOOL_NAMES`, `build_tools`, and the `call` dispatcher; bump the tool count in the `src/tools.rs` module doc
- [x] 1.3 Refresh `tests/schema_snapshots.rs` for the new tool

## 2. Link-index seeding

- [x] 2.1 Add a helper that extends a built `LinkIndex` with additional pending clean paths + regions (generalizing the `post_rename_index` pattern); unit-test that seeded entries resolve and participate in shortest-name disambiguation

## 3. Handler

- [x] 3.1 Implement phase 1: parse entries, reject empty/oversized/malformed/duplicate-path batches, then per entry run reserved-root rejection, write gating, visibility, and link expansion against the seeded index, collecting final contents with no writes
- [x] 3.2 Implement phase 2: apply in request order (atomic replace, locked read-modify-write for appends), notify recall per entry, return `{ results: [{path, bytes_written}] }` in request order

## 4. Tests

- [x] 4.1 Happy path: multi-entry batch lands in order with correct bytes and recall visibility
- [x] 4.2 Intra-batch link test: entry B links to entry A's new note and persists the suffixed form (and resolution is independent of entry order)
- [x] 4.3 All-or-nothing tests: reserved-root entry, policy-denied entry, and leak-guard entry each reject the whole batch leaving the vault byte-identical
- [x] 4.4 Contract tests: duplicate paths rejected, size limits, append + replace mixed batch, per-entry error-code parity with `write_memory_note`

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; fix anything they surface
