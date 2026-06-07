## Context

The session-context renderer (`src/session_context.rs`) is the single source of
the per-scope bootstrap. It is reached today through three MCP surfaces, all of
which funnel into `Toolbox::render_session_context` (`src/tools.rs`):

- the `load_session_context` tool (returns `{ rendered, missing }`),
- the `session-context` resource (`agentmem://session-context/{k1}/{k2}/…`),
- the `session-context` prompt (scope keys as prompt arguments).

The HTTP transport (`src/transport/http.rs`) already builds an `axum` router that
mounts the `rmcp` Streamable HTTP service at `/mcp`, a `GET /health` liveness
route, and an optional `require_bearer` middleware layered onto the `/mcp`
sub-router. `AgentmemServer` (`src/mcp.rs`) holds an `Arc<Toolbox>` and is `Clone`
and `Send + Sync`, so it can be shared into `axum` handler state cheaply.

A harness that only wants the rendered system prompt currently has to run the
full MCP handshake. This change adds a plain HTTP `GET` that returns the same
rendered text.

## Goals / Non-Goals

**Goals:**
- A versioned, stateless, read-only `GET /v1/context` that renders the bootstrap
  for a scope passed as query parameters.
- Reuse the existing renderer verbatim — byte-identical output to
  `load_session_context` for the same scope.
- Reuse the existing bearer middleware so the new route inherits `/mcp`'s auth.
- Return markdown by default (drop-in system prompt) with opt-in JSON.

**Non-Goals:**
- No new configuration variables, no new dependencies.
- No write operations, no other `/v1/*` routes (this change adds only
  `/v1/context`; the `/v1` prefix simply leaves room to version later).
- No change to the MCP protocol surface, tools, resource, or prompt.
- No exposure over the stdio transport.
- No per-scope authorization (the bearer is server-wide, exactly as `/mcp`
  is today; scoping callers to specific agents/users is out of scope).

## Decisions

### Decision: New capability rather than modifying `mcp-server`
The new endpoint is plain HTTP, not MCP. It does not change any existing MCP
requirement; it reuses the renderer and the bearer middleware additively. Framing
it as the new capability `context-http-api` keeps the MCP spec stable and gives
the HTTP read API its own home for future `/v1/*` growth.
- *Alternative considered:* a delta on `mcp-server`'s "Successful http startup"
  scenario. Rejected — that scenario describes MCP transport startup, and the new
  route is a separate concern that warrants its own spec.

### Decision: Route placement and shared state
Mount `GET /v1/context` on the same router as `/mcp`, using `axum`'s
`State<AgentmemServer>` (clone of the existing server). The handler calls a small
new public accessor on `AgentmemServer` exposing `render_session_context(&scope)`
and `scheme_placeholders()` (both already exist on `Toolbox` but the `toolbox`
field is private). Add thin `pub fn` delegators on `AgentmemServer` rather than
making the field public, keeping the toolbox encapsulated.
- *Alternative considered:* threading a separate `Arc<Toolbox>` into the router.
  Rejected — `AgentmemServer` is already the shared handle and is `Clone`; adding
  two delegators is less surface area than a second shared type.

### Decision: Scope binding from raw query string
Parse the query string into a map and bind exactly the scheme placeholders, in
scheme order. Validation (missing, empty, unexpected keys) is delegated to the
existing checks inside `Toolbox::render_session_context`, which already returns
`MissingScope` / `InvalidArgument` for those cases — the handler maps those
variants to `400`. The handler additionally rejects query keys that are not
scheme placeholders before calling the renderer, producing a precise "unexpected
parameter" message.
- *Why not `axum::extract::Query<T>` into a typed struct:* the parameter set is
  dynamic (it depends on the runtime scheme), so a static struct cannot model it.
  Use `axum::extract::RawQuery` (or `Query<HashMap<String,String>>`) and bind
  dynamically.

### Decision: Content negotiation
Inspect the `Accept` header. If it prefers `application/json`, serialize
`{ rendered, missing }` (a `#[derive(Serialize)]` struct or `serde_json::json!`);
otherwise return the raw `rendered` string with `Content-Type: text/markdown`.
Default (no `Accept`, `*/*`, or `text/markdown`) is markdown.
- *Alternative considered:* a `?format=json` query parameter. Rejected — `Accept`
  is the conventional HTTP mechanism and avoids colliding with scope parameter
  names; a query flag could also be mistaken for a scope key.

### Decision: Error body shape
All endpoint errors return `{ "error": <message> }` as `application/json`.
`AgentmemError` already formats messages that reference virtual paths and scope
keys (never resolved physical paths), so mapping its `Display` into the body is
safe. Validation errors → `400`; unexpected IO errors → `500`.

## Risks / Trade-offs

- **Unauthenticated exposure of memory content** → The endpoint renders
  potentially sensitive persona/memory/user files. Mitigation: it inherits the
  exact `/mcp` bearer gate; the existing startup `WARN` for an unset bearer on a
  non-loopback bind already covers this surface. Documented in the README.
- **Scope enumeration** → Any caller past the bearer can request any scope's
  context (same trust model as `/mcp`, which can already call
  `load_session_context` for any scope). No new exposure beyond MCP; per-scope
  authorization remains a deliberate non-goal.
- **`Accept` parsing brittleness** → Full RFC 7231 q-value parsing is overkill.
  Mitigation: a simple substring/`contains("application/json")` check is
  sufficient for the two supported representations; default to markdown on
  anything ambiguous.
- **Feature/transport gating** → The handler lives in `transport/http.rs`, which
  is already behind `#[cfg(feature = "transport-http")]`, so no extra gating is
  needed; stdio never mounts a router.

## Migration Plan

Purely additive. No config, schema, or protocol changes; no rollback steps beyond
reverting the commit. Existing clients are unaffected.

## Open Questions

- None blocking. (If finer-grained auth is wanted later, it would be a separate
  change layered on top of the shared bearer.)
