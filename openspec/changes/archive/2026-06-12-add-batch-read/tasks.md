## 1. Refactor

- [x] 1.1 Extract the body of `read_memory_note` into a private `read_one(&self, scope, vpath) -> Result<String, AgentmemError>` in `src/tools.rs`; the single-read handler delegates to it (no behavior change).

## 2. Tool

- [x] 2.1 Add a `BatchReadFields { paths: Vec<String> }` schemars struct (description: 1–20 vault-root-relative paths, per-path error envelope), register `read_memory_notes` in `TOOL_NAMES`/`build_tools`, and dispatch in `Toolbox::call`.
- [x] 2.2 Handler: validate the array (non-empty, ≤ 20, string entries) with `invalid_argument`; map each path through `read_one`, emitting `{ path, content }` or `{ path, error: { code, message } }` in request order; duplicates processed independently.

## 3. Tests

- [x] 3.1 `tests/tools.rs`: ordered batch with own-scope and shared notes; link stripping parity with single read; duplicate paths answered positionally.
- [x] 3.2 Partial failure: missing file mid-batch yields `not_found` in place, others succeed; hidden/ignored and policy-denied entries carry `path_not_permitted` parity.
- [x] 3.3 Argument validation: empty array, 21 entries, non-string entry → `invalid_argument`.
- [x] 3.4 Update schema snapshots and the README tool table.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
