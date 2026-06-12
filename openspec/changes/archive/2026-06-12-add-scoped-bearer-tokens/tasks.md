## 1. Spike

- [x] 1.1 Verify how rmcp 1.7's streamable HTTP service exposes the originating HTTP request (parts/extensions) to server handlers; confirm an axum-middleware-inserted extension is readable at `tools/call` / resource / prompt handling, or pin the fallback (per-session grant binding through the service factory). Record the outcome in this file before starting section 4.

  **Spike outcome (rmcp 1.7.0, verified in vendored source):** the primary design works; no fallback needed. `StreamableHttpService` converts each inbound HTTP request to `http::request::Parts` and inserts the whole `Parts` value into the JSON-RPC request's extensions for both requests and notifications (`transport/streamable_http_server/tower.rs:1036-1042`, and `:1101` for `initialize`). The service loop clones those extensions into `RequestContext.extensions` for every handler (`service.rs:955-961`; the struct field is public, `service.rs:659`). An axum-middleware-inserted extension therefore reaches `call_tool`/`read_resource`/`get_prompt` as `context.extensions.get::<http::request::Parts>().and_then(|p| p.extensions.get::<Grant>())` — this exact pattern is documented in rmcp's own rustdoc (`tower.rs:476-495`). `Grant` only needs `Clone + Send + Sync + 'static` (rmcp `Extensions::insert` bound). The `http` crate is reachable as `axum::http` (same crate version as rmcp's, so `TypeId`s match); no new dependency. Grants are resolved per POST (each carries its own `Authorization`), so nothing is cached per session. On the stdio path the extension is absent → `AllScopes`.

## 2. Configuration

- [x] 2.1 Add `AGENTMEM_HTTP_TOKENS_FILE` / `--http-tokens-file` to `src/config.rs`; parse the JSON grant file with `serde_json` into a `Grant` model (per-key exact-or-`*` matchers; duplicate tokens union); validate keys against the active scheme's placeholders and fail startup on any violation without echoing token values.
- [x] 2.2 Redact tokens from `Debug`/`--print-config` output (extend the existing bearer redaction); unit tests for parsing, validation failures, union semantics, and redaction.

## 3. Error code

- [x] 3.1 Add a `ScopeDenied { key }` variant with code `scope_denied` to `src/error.rs`; message names the key, never the grant set; cover in error-code tests.

## 4. Enforcement

- [x] 4.1 `src/transport/http.rs`: generalize the bearer middleware to resolve the presented token to a `Grant` (`AllScopes` for the static bearer, per-file grants otherwise; 401 for unknown bearers when any auth is configured) and attach the grant to the request; keep probes ungated.
- [x] 4.2 Thread the per-request grant into scope validation: `Toolbox::scope_map` and `render_session_context` check each requested key against the grant (absent grant context, e.g. stdio, = all scopes) and return `ScopeDenied` on mismatch, before path resolution. *(Implementation note: the tool-side check sits in `Toolbox::call` immediately before handler dispatch — the same funnel `scope_map` guards, but one frame earlier, so all fourteen tools are covered without threading the grant through every handler; `render_session_context` checks the grant after shape validation as designed.)*
- [x] 4.3 `GET /v1/context`: apply the same grant check to the bound query parameters, mapping `ScopeDenied` to HTTP 403 with the standard `{ "error": … }` body.
- [x] 4.4 Startup warning logic: unauthenticated WARN only when both `AGENTMEM_HTTP_BEARER` and the tokens file are unset.

## 5. Tests

- [x] 5.1 `tests/http_transport.rs`: scoped token calling its own scope succeeds; foreign scope → `scope_denied` tool error; unknown bearer → 401; static bearer retains all scopes; union-of-grants token; revocation honored per request.
- [x] 5.2 Resource, prompt, and `/v1/context` surfaces: grant enforced (403 path for `/v1/context`); probes reachable without auth.
- [x] 5.3 Stdio regression: no grant enforcement, all scopes usable.
- [x] 5.4 Config tests: startup failures for bad files; no token text in logs or `--print-config`.

## 6. Documentation

- [x] 6.1 Update `docs/security.md` (trust model: per-tenant auth delivered; grant file format; rotation-requires-restart), README configuration table and container examples.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-features`.
