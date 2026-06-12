# Tasks: add-note-read-ranges

## 1. Schema & argument parsing

- [x] 1.1 Add optional `offset`/`limit` (integers, `minimum: 1`) to `ReadFields` in `src/tools.rs` with doc comments describing 1-based lines, `total_lines`, and the empty-past-EOF behavior
- [x] 1.2 Change `BatchReadFields::paths` entries to the `string | { path, offset?, limit? }` union (`anyOf` in the generated schema) and update the field doc comment
- [x] 1.3 Add argument helpers: extract an optional 1-based range from args (rejecting 0 with `invalid_argument`) and parse a batch entry into `(path, Option<Range>)` with call-level `invalid_argument` for malformed entries
- [x] 1.4 Update tool descriptions for `read_memory_note` and `read_memory_notes` to mention ranges, and refresh `tests/schema_snapshots.rs` expectations

## 2. Handlers

- [x] 2.1 Add a line-slice helper (split via `split_inclusive('\n')`, returns sliced content + total line count) with unit tests covering empty notes, missing final newline, and `\r\n` content
- [x] 2.2 Wire the range path into `read_memory_note`: slice after `read_one`, include `total_lines` in the structured result only when a range was requested, keep `backlinks` composing
- [x] 2.3 Wire ranged entries into `read_memory_notes`: per-entry slice + `total_lines`, per-entry errors unchanged for path/IO failures

## 3. Tests

- [x] 3.1 Integration tests in `tests/tools.rs`: mid-file range, offset alone, limit alone, offset past EOF (empty + `total_lines`), `offset=0`/`limit=0` rejected, default response carries no `total_lines`
- [x] 3.2 Strip-interaction test: a suffixed link on a known line is returned clean when that single line is ranged, and line numbers match a whole-note read
- [x] 3.3 Batch tests: mixed string/object entries, ranged entry past EOF succeeds, malformed entry rejects the whole call, single-vs-batch parity on the same note and range

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; fix anything they surface
