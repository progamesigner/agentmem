## MODIFIED Requirements

### Requirement: HTTP transport static authentication
The system SHALL, when running under `http` transport, optionally require an `Authorization: Bearer <token>` header matching `AGENTMEM_HTTP_BEARER`. The static bearer SHALL carry an all-scopes grant: requests presenting it may name any scope. When both `AGENTMEM_HTTP_BEARER` and `AGENTMEM_HTTP_TOKENS_FILE` are unset, no authentication is enforced and a startup warning is logged; when either is set, requests without an acceptable bearer SHALL be rejected.

#### Scenario: Bearer token accepted
- **WHEN** `AGENTMEM_HTTP_BEARER=secret` is set and a client sends a request to `POST /mcp` with header `Authorization: Bearer secret`
- **THEN** the request is processed normally

#### Scenario: Bearer token rejected
- **WHEN** `AGENTMEM_HTTP_BEARER=secret` is set and a client sends a request without the header or with the wrong token
- **THEN** the server responds with HTTP 401 and an MCP error body, and the request never reaches a tool handler

#### Scenario: Auth disabled emits warning
- **WHEN** the server starts in `http` mode with both `AGENTMEM_HTTP_BEARER` and `AGENTMEM_HTTP_TOKENS_FILE` unset
- **THEN** a single `WARN`-level log line is emitted indicating that the HTTP endpoint is unauthenticated

## ADDED Requirements

### Requirement: HTTP per-tenant scoped tokens
The system SHALL, when running under `http` transport with `AGENTMEM_HTTP_TOKENS_FILE` configured, authenticate each request to `/mcp` and `/v1/context` against the configured token set and authorize every scope-bearing operation against the presenting token's scope grants. A bearer that is neither a configured token nor the static `AGENTMEM_HTTP_BEARER` SHALL be rejected with HTTP 401. For an authenticated scoped token, any operation naming scope keys — a `tools/call`, a `session-context` resource read, a `session-context` prompt request, or `GET /v1/context` — SHALL be permitted only when every requested scope key matches the token's grant (exact value or `*` per key, the union of grants when a token has several entries); a mismatch SHALL be rejected with a `scope_denied` error before any path resolution or IO, and the error message SHALL name the offending key without enumerating valid grants. Operations carrying no scope keys (e.g. `tools/list`) SHALL require only authentication. Tokens SHALL NOT appear in logs. The `stdio` transport SHALL be unaffected. Grants SHALL be resolved per request, so a token removed from the file no longer authorizes new operations after a restart-reload, including on already-open sessions.

#### Scenario: Scoped token confined to its grant
- **WHEN** the tokens file grants token `t1` `{ "agent": "jarvis", "user": "*" }` and a client presenting `t1` calls a tool with `agent=jarvis, user=tony`
- **THEN** the call proceeds normally

#### Scenario: Scope mismatch is denied before IO
- **WHEN** the same client presenting `t1` calls a tool with `agent=friday, user=tony`
- **THEN** the response is an MCP error with code `scope_denied` naming `agent`, and no vault path is resolved or read

#### Scenario: Unknown bearer is unauthenticated
- **WHEN** `AGENTMEM_HTTP_TOKENS_FILE` is configured and a client presents a bearer that appears in neither the tokens file nor `AGENTMEM_HTTP_BEARER`
- **THEN** the server responds with HTTP 401

#### Scenario: Static bearer retains all scopes
- **WHEN** both `AGENTMEM_HTTP_TOKENS_FILE` and `AGENTMEM_HTTP_BEARER=admin` are configured and a client presenting `admin` calls a tool with any valid scope
- **THEN** the call proceeds normally

#### Scenario: Scoped token gates the session-context surfaces
- **WHEN** a client presenting `t1` (granted `agent=jarvis` only) requests the `session-context` resource or prompt for `agent=friday`
- **THEN** the request is rejected with `scope_denied` and no context is rendered

#### Scenario: Union of grants for a repeated token
- **WHEN** the tokens file lists token `t2` twice, once granting `{ "agent": "jarvis", "user": "tony" }` and once `{ "agent": "friday", "user": "tony" }`
- **THEN** `t2` may name either agent with `user=tony` and no other combination
