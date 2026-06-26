## 1. Transport change

- [x] 1.1 In `src/transport/http.rs::serve`, apply `.with_stateful_mode(false).with_json_response(true)` to the `StreamableHttpServerConfig` in all three branches (default, `disable_allowed_hosts`, `with_allowed_hosts`) so Host-validation behavior is preserved.
- [x] 1.2 Update the surrounding doc comments in `http.rs` to describe stateless JSON-response mode (drop references to the SSE-on-POST / session behavior).

## 2. Tests

- [x] 2.1 In `tests/http_transport.rs`, assert a `tools/call` over `POST /mcp` returns `Content-Type: application/json` (not `text/event-stream`).
- [x] 2.2 Assert the `initialize` response carries no `mcp-session-id` header and that a follow-up request succeeds without one.
- [x] 2.3 Confirm existing auth and Host-validation tests still pass under the new config.

## 3. Version bump

- [x] 3.1 Bump `version` in `Cargo.toml` to `0.7.0`.
- [x] 3.2 Run `cargo check` to refresh `Cargo.lock` for `0.7.0`.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` — all must pass.
- [x] 4.2 Manually verify with curl against a local instance that `tools/call` returns `application/json` and the server log shows no `Resume failed` warnings.
