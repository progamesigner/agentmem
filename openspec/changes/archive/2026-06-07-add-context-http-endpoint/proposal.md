## Why

The rendered session-context bootstrap is currently reachable only through the
MCP protocol (the `load_session_context` tool, the `session-context` resource,
and the `session-context` prompt). A harness or client that just wants to fetch
a scope's system prompt must speak full MCP (initialize handshake, JSON-RPC,
session management) even though the operation is a stateless, read-only lookup.
A plain versioned HTTP `GET` makes the same rendered context trivially
fetchable by any harness, shell script, or orchestration layer.

## What Changes

- Add a new plain-HTTP read endpoint **`GET /v1/context`** to the HTTP
  transport's `axum` router, served alongside `/mcp` and `/health`.
- The endpoint takes one query parameter per VFS-scheme placeholder
  (e.g. `?agent=default&user=alice`), validates them against the active scheme,
  renders the session-context, and returns it.
- Default response is the rendered bootstrap as `text/markdown` (ready to drop
  straight into a system prompt). `Accept: application/json` returns
  `{ "rendered": "...", "missing": [...] }`, mirroring the `load_session_context`
  tool result.
- The endpoint sits **behind the same `AGENTMEM_HTTP_BEARER` gate as `/mcp`**:
  when a bearer is configured it requires `Authorization: Bearer <token>`;
  `/health` remains the only always-reachable route.
- Invalid requests (missing, empty, or unexpected scope parameters) return a
  `400` with a JSON error body; genuine IO failures return `500`. Absent
  foundational files are never errors — they render as sentinels, exactly as
  the existing surfaces behave.
- The endpoint is available only when the `transport-http` feature is built and
  the HTTP transport is selected (it is part of the HTTP router).

## Capabilities

### New Capabilities
- `context-http-api`: A versioned, stateless, read-only HTTP API
  (`GET /v1/context`) that renders the per-scope session-context bootstrap for a
  harness or client to fetch directly, without the MCP protocol. Covers scope
  parameter binding, response negotiation (markdown vs. JSON), authentication
  reuse, and error mapping.

### Modified Capabilities
- (none — the MCP surfaces and configuration are unchanged; the new endpoint is
  additive and reuses the existing renderer and bearer middleware)

## Impact

- **Code**: `src/transport/http.rs` (new route + handler + query/scope parsing
  and Accept negotiation); a small public accessor on `AgentmemServer`
  (`src/mcp.rs`) so the handler can reach `Toolbox::render_session_context` and
  `scheme_placeholders`. No new dependencies — `axum`, `serde`, and
  `serde_json` are already present.
- **APIs**: new outward HTTP surface `GET /v1/context`; no change to the MCP
  protocol, tools, resource, or prompt.
- **Config**: no new variables. Reuses `AGENTMEM_HTTP_BIND` and
  `AGENTMEM_HTTP_BEARER`.
- **Tests**: new integration coverage in `tests/http_transport.rs` (rendered
  markdown, JSON negotiation, scope validation, bearer enforcement).
- **Docs**: `README.md` HTTP section gains a `/v1/context` entry.
