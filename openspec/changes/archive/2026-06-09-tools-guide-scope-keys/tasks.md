## 1. Implementation

- [x] 1.1 Change `tools_guide()` in `src/session_context.rs` to take `scope: &BTreeMap<String, String>` and emit `key=value` pairs (sorted, deterministic) in the intro sentence, with the generic fallback for an empty scope
- [x] 1.2 Update the single call site in `render_session_context` to pass `scope` into `tools_guide()`

## 2. Tests

- [x] 2.1 Add a unit test asserting the rendered tools guide names the concrete scope (`agent=coder, user=alice`) and still lists the live tools
- [x] 2.2 Add a unit test asserting the empty-scope fallback retains the generic phrasing

## 3. Verification

- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` all pass
- [x] 3.2 Confirm no spec/integration test asserts on the old intro wording
