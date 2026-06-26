## Context

`src/transport/http.rs::serve` builds its transport config from `StreamableHttpServerConfig::default()`, whose defaults are `stateful_mode: true, json_response: false` (`rmcp-1.7.0/.../streamable_http_server/tower.rs:106-113`). Under those defaults, every `POST /mcp` request result is delivered as an SSE stream on the POST response, and clients open a `GET /mcp` stream that the server tracks per session and replays on reconnect.

AgentMem uses none of that. `AgentmemServer::call_tool` (`src/mcp.rs:243`) resolves synchronously and returns a single result; capabilities are built with `.enable_tools().enable_resources().enable_prompts()` (`src/mcp.rs:220`) with no `listChanged` or `subscribe`, so the server never sends a server-initiated message. The stateful default therefore contributes only cost: recurring `Resume failed (Session error: Channel closed)` warnings from `GET /mcp` reconnects, and an SSE-framed POST response that clients such as Raycast fail to consume — they hang on tool calls until interrupted. A standard client (curl) against the live endpoint returns `tools/call` results in ~0.1s, confirming the server is correct and the failure is the response shape, not the logic.

## Goals / Non-Goals

**Goals:**
- Serve `POST /mcp` results as `Content-Type: application/json` (no SSE framing).
- Stop issuing session ids and stop relying on the `GET /mcp` notification stream.
- Eliminate the `Resume failed` log churn.
- Bump the crate to `0.7.0`.

**Non-Goals:**
- No new configuration variable or runtime toggle — the change is unconditional (stateful served no purpose here).
- No change to tool behavior, scopes, auth (`AGENTMEM_HTTP_BEARER`/tokens), or Host validation.
- No change to the stdio transport.
- No change to `/v1/context`, `/healthz`, `/readyz`.

## Decisions

**Decision: Switch to stateless + json_response via `StreamableHttpServerConfig`.**
Build the config with `.with_stateful_mode(false).with_json_response(true)` so the request path takes rmcp's stateless `serve_directly` + `OneshotTransport` branch (`tower.rs:1165-1213`), awaiting the single response and returning it as `application/json`. Applied to all three current branches in `serve` (default, `disable_allowed_hosts`, `with_allowed_hosts`) so Host-validation behavior is preserved unchanged.

- *Alternative — `json_response` only:* rejected; `json_response` is honored only when `stateful_mode` is false (the JSON-direct branch lives inside the stateless `else`, `tower.rs:1187`). Setting it alone does nothing.
- *Alternative — add an env toggle (`AGENTMEM_HTTP_STATELESS`):* rejected per the chosen scope; stateful has no use for this server, so an unconditional switch is simpler and removes a config surface rather than adding one.

**Decision: Keep `LocalSessionManager` in the `StreamableHttpService::new` call.**
The constructor still requires a session manager argument, but in stateless mode the request path bypasses it entirely. Keeping the existing `Arc::new(LocalSessionManager::default())` is the smallest change. If rmcp exposes a no-op manager (`session::never`) cleanly, it may be substituted, but that is optional polish, not required for correctness.

**Decision: Version bump to 0.7.0 (minor, not patch).**
The externally observable transport contract changes (no session ids, JSON instead of SSE on POST), so a patch bump understates it. Per `CLAUDE.md`, refresh `Cargo.lock` via `cargo check` as part of the bump; the release commit/tag are a separate, later step (not part of this change).

## Risks / Trade-offs

- **A client that depends on session ids or the SSE notification stream breaks.** → AgentMem advertises no notification capabilities, so a spec-compliant client has nothing to lose; Claude Code and curl both work statelessly. Documented as BREAKING in the proposal.
- **Future need for server-initiated notifications (e.g. `resources/listChanged`) would require stateful mode again.** → Out of scope today; revisiting would be a deliberate new change that re-introduces sessions alongside the capability that needs them.
- **Behavioral regression in the three config branches.** → Covered by updating `tests/http_transport.rs` to assert `application/json` responses and absence of `mcp-session-id`, exercising the live router.

## Migration Plan

1. Update `serve` to apply `.with_stateful_mode(false).with_json_response(true)` to the config in all branches.
2. Update/extend `tests/http_transport.rs`.
3. Bump `Cargo.toml` to `0.7.0`; `cargo check` to refresh `Cargo.lock`.
4. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.

Rollback: revert the `serve` change; no persisted state or schema is involved.

## Open Questions

- None blocking. Substituting a no-op session manager for `LocalSessionManager` is optional and can be decided during implementation.
