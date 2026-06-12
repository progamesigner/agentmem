## MODIFIED Requirements

### Requirement: Authentication reuse
The endpoint SHALL sit behind the same authentication gate as `/mcp`. When
`AGENTMEM_HTTP_BEARER` is set, `GET /v1/context` SHALL require a matching
`Authorization: Bearer <token>` header and SHALL respond `401` otherwise. When
`AGENTMEM_HTTP_TOKENS_FILE` is configured, a request presenting a configured
scoped token SHALL additionally have its query-parameter scope checked against
that token's grant: a mismatch SHALL be rejected with `403` and a JSON
`{ "error": … }` body naming the offending key, before any rendering. When
neither variable is set the endpoint is unauthenticated, like `/mcp`. The probe
routes `GET /healthz` and `GET /readyz` SHALL remain reachable without
authentication regardless.

#### Scenario: Missing bearer is rejected when configured
- **WHEN** the server is started with `AGENTMEM_HTTP_BEARER=secret` and
  `GET /v1/context?agent=jarvis&user=tony` is sent without an `Authorization`
  header
- **THEN** the server responds `401 Unauthorized` and does not render any context

#### Scenario: Matching bearer is accepted
- **WHEN** the server is started with `AGENTMEM_HTTP_BEARER=secret` and the
  request carries `Authorization: Bearer secret`
- **THEN** the server responds `200 OK` with the rendered context

#### Scenario: Unauthenticated when bearer unset
- **WHEN** both `AGENTMEM_HTTP_BEARER` and `AGENTMEM_HTTP_TOKENS_FILE` are unset
  and `GET /v1/context?agent=jarvis&user=tony` is sent without an
  `Authorization` header
- **THEN** the server responds `200 OK` with the rendered context

#### Scenario: Scoped token renders only its own scope
- **WHEN** the tokens file grants the presented token
  `{ "agent": "jarvis", "user": "*" }` and the request is
  `GET /v1/context?agent=jarvis&user=tony`
- **THEN** the server responds `200 OK` with the rendered context

#### Scenario: Scope mismatch yields 403
- **WHEN** the same token requests `GET /v1/context?agent=friday&user=tony`
- **THEN** the server responds `403` with a JSON `{ "error": … }` body naming
  `agent`, and no context is rendered
