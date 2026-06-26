## Why

The HTTP transport inherits `rmcp`'s default `stateful_mode: true`, which issues session ids, keeps a per-session SSE event cache, and returns every tool result framed as `text/event-stream`. None of this serves AgentMem: the server advertises no `listChanged`/`subscribe` capabilities and `call_tool` is a pure synchronous request→response, so it never pushes server-initiated messages. The stateful machinery only adds cost — recurring `Resume failed` log churn from clients reconnecting the `GET /mcp` stream, and a SSE-on-POST response shape that some MCP clients (e.g. Raycast) fail to consume, hanging indefinitely on tool calls. Stateless + JSON-response mode matches the server's actual semantics and maximizes client compatibility.

## What Changes

- The HTTP transport SHALL run in stateless mode (`stateful_mode = false`) with direct JSON responses (`json_response = true`).
- Each `POST /mcp` is handled independently and its result is returned as `Content-Type: application/json`, not `text/event-stream`.
- No `mcp-session-id` is issued and the `GET /mcp` SSE stream is no longer used for server-initiated notifications (the server advertises none). **BREAKING**: clients relying on session ids, SSE notification streams, or SSE-framed POST responses change behavior.
- Bump crate version to `0.7.0` (`Cargo.toml` + `Cargo.lock`) to reflect the transport behavior change.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `mcp-server`: the HTTP transport's response/session behavior changes — stateless, JSON-direct responses, no session ids, and the `GET /mcp` SSE endpoint no longer carries notifications.

## Impact

- Code: `src/transport/http.rs` (`serve` — build `StreamableHttpServerConfig` with stateless + json_response).
- Version: `Cargo.toml`, `Cargo.lock` → `0.7.0`.
- Tests: `tests/http_transport.rs` — assert tool-call responses are `application/json` and no `mcp-session-id` is issued.
- Clients: any consumer that depended on session ids or SSE notification streams; the `Resume failed` warnings disappear.
- No new configuration variables; the change is unconditional.
