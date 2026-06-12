## Why

`docs/security.md` is explicit that scope keys are trusted claims in v1: any
client holding the single static bearer (or none) may name any
`agent`/`user` scope and read or write that scope's memory. That is fine for a
loopback sidecar but dishonest for the deployment the container image
advertises — one shared server reachable over the network by multiple agents.
Per-tenant authentication is the deferred follow-up this change delivers.

## What Changes

- New optional `AGENTMEM_HTTP_TOKENS_FILE` (HTTP transport only): a JSON file
  mapping bearer tokens to scope grants, e.g.
  `{ "tokens": [ { "token": "…", "scopes": { "agent": "jarvis", "user": "*" } } ] }`.
  Grant keys MUST be the active scheme's placeholders; each value is an exact
  string or `*`. A token may appear in multiple entries (grants union). The
  file is read at startup; tokens never appear in logs.
- Authentication: when the file is configured, a request to `/mcp` or
  `/v1/context` must present a bearer that is either a configured scoped token
  or the static `AGENTMEM_HTTP_BEARER` (which remains supported and grants all
  scopes); anything else is 401.
- Authorization: every scope-bearing surface — `tools/call`, the
  `session-context` resource and prompt, and `GET /v1/context` — validates the
  requested scope keys against the presenting token's grants. A mismatch is a
  new `scope_denied` domain error (HTTP 403 on `/v1/context`); it never reaches
  storage. Surfaces without scope arguments (`tools/list`, probes) behave as
  today.
- Stdio transport is unchanged (process-level trust by design).
- `docs/security.md` drops "per-tenant authentication" from deferred work and
  documents the grant model.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `mcp-server`: new requirement for scoped-token authentication/authorization
  on the HTTP transport (the static-bearer requirement stands).
- `configuration`: the HTTP transport variables requirement gains
  `AGENTMEM_HTTP_TOKENS_FILE` (with validation rules).
- `context-http-api`: the authentication-reuse requirement covers scoped
  tokens (401 unknown token, 403 scope mismatch).

## Impact

- Code: `src/config.rs` (tokens-file loading/validation), `src/transport/http.rs`
  (auth middleware resolves a grant and attaches it to the request),
  `src/mcp.rs` + `src/tools.rs` (grant check where scope maps are validated),
  `src/error.rs` (`scope_denied`), `docs/security.md`, `tests/http_transport.rs`,
  README.
- Integration risk: carrying the per-request grant from the axum middleware
  into rmcp tool handlers depends on rmcp's propagation of HTTP request parts
  into the request context — verified by an early spike task (fallback design
  documented).
- Dependencies: none (`serde_json` parses the tokens file).
