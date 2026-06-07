## 1. Expose the renderer to HTTP handlers

- [x] 1.1 Add `pub fn scheme_placeholders(&self) -> Vec<String>` delegator to `AgentmemServer` in `src/mcp.rs`, forwarding to the inner `Toolbox`.
- [x] 1.2 Add `pub fn render_session_context(&self, scope: &BTreeMap<String, String>) -> Result<SessionContext, AgentmemError>` delegator to `AgentmemServer`, forwarding to the inner `Toolbox`.

## 2. Implement the `GET /v1/context` route

- [x] 2.1 In `src/transport/http.rs`, add the `GET /v1/context` route to the same sub-router as `/mcp` so it inherits the `require_bearer` middleware; keep `/health` outside the gated sub-router.
- [x] 2.2 Wire `AgentmemServer` into the router as `axum` handler state (clone of the existing server) so the handler can reach the renderer.
- [x] 2.3 Implement the handler: extract the raw query string, parse it into a key/value map, reject any key that is not a scheme placeholder with `400`, and bind the placeholders (in scheme order) into the scope map.
- [x] 2.4 Call `render_session_context(&scope)` and map `MissingScope` / `InvalidArgument` to `400` and other `AgentmemError` variants to `500`, all with a `{ "error": <message> }` JSON body.
- [x] 2.5 Implement `Accept`-header negotiation: return `text/markdown` with the rendered body by default; return `application/json` with `{ "rendered", "missing" }` when `Accept` prefers `application/json`.
- [x] 2.6 Handle the empty-scheme case: no query parameters required, render the single-tenant bootstrap.

## 3. Tests

- [x] 3.1 Add an integration test in `tests/http_transport.rs`: `GET /v1/context?agent=default&user=alice` returns `200` `text/markdown` with the rendered bootstrap.
- [x] 3.2 Add a test for `Accept: application/json` returning `{ rendered, missing }` with the correct `missing` list on a fresh vault.
- [x] 3.3 Add tests for scope validation: missing placeholder → `400` naming the key; empty value → `400`; unexpected parameter → `400` naming the key.
- [x] 3.4 Add bearer tests: with `AGENTMEM_HTTP_BEARER` set, the route returns `401` without the header and `200` with the matching `Authorization: Bearer` header; `/health` still reachable.
- [x] 3.5 Add a test confirming the rendered markdown body is byte-identical to the `load_session_context` tool result for the same scope.

## 4. Docs and verification

- [x] 4.1 Document `GET /v1/context` in the `README.md` HTTP section (route, query params, markdown vs. JSON negotiation, bearer behaviour).
- [x] 4.2 Run `cargo fmt`, `cargo clippy`, and `cargo test` locally; ensure all pass before committing.
