## ADDED Requirements

### Requirement: Liveness and readiness probes
The HTTP transport SHALL serve two ungated probe routes: `GET /healthz` (liveness)
and `GET /readyz` (readiness). `GET /healthz` SHALL report success as soon as the
process is up and SHALL NOT depend on recall index state. `GET /readyz` SHALL report
not-ready until every recall scope index and the shared index have been eagerly built
at startup, and ready thereafter. Both routes SHALL remain reachable without
authentication regardless of `AGENTMEM_HTTP_BEARER`. When recall is `off`, `GET
/readyz` SHALL report ready once the process is up.

#### Scenario: Liveness is up during the index build
- **WHEN** the server is still building recall indexes at startup and `GET /healthz`
  is requested
- **THEN** the response is `200 OK`, so an orchestrator's liveness probe does not kill
  the process mid-build

#### Scenario: Readiness flips only after the build completes
- **WHEN** `GET /readyz` is requested before the eager index build has finished
- **THEN** the response indicates not-ready (HTTP `503`); once all scope indexes and
  the shared index are built, `GET /readyz` responds `200 OK`

#### Scenario: Probes need no bearer token
- **WHEN** `AGENTMEM_HTTP_BEARER` is set and `GET /healthz` or `GET /readyz` is
  requested without an `Authorization` header
- **THEN** the response is the normal probe result, not `401`

## MODIFIED Requirements

### Requirement: Versioned context endpoint
The system SHALL serve a stateless, read-only HTTP route `GET /v1/context` on the
HTTP transport's `axum` router, alongside `POST/GET /mcp`, `GET /healthz`, and
`GET /readyz`. The route SHALL render the per-scope session-context bootstrap using
the same renderer as the `load_session_context` tool, the `session-context`
resource, and the `session-context` prompt. The route SHALL exist only when the
binary is built with the `transport-http` feature and the HTTP transport is selected.

#### Scenario: Endpoint renders the bootstrap
- **WHEN** a client issues `GET /v1/context?agent=default&user=alice` against a
  server whose scheme is `<agent>.<user>`
- **THEN** the server responds `200 OK` with the rendered session-context for
  the scope `{agent: "default", user: "alice"}`, identical to what
  `load_session_context` would return for the same scope

#### Scenario: Rendering never errors on absent files
- **WHEN** `GET /v1/context` is called for a scope whose foundational files do
  not yet exist
- **THEN** the server responds `200 OK` with the bootstrap rendered from the
  compiled-in default template, with the missing files substituted by their
  sentinel — absence is never an error

#### Scenario: Endpoint is absent without the HTTP transport
- **WHEN** the server is running under the `stdio` transport
- **THEN** no TCP listener is opened and `GET /v1/context` is not served

### Requirement: Authentication reuse
The endpoint SHALL sit behind the same bearer-token gate as `/mcp`. When
`AGENTMEM_HTTP_BEARER` is set, `GET /v1/context` SHALL require a matching
`Authorization: Bearer <token>` header and SHALL respond `401` otherwise. When
the bearer is unset the endpoint is unauthenticated, like `/mcp`. The probe routes
`GET /healthz` and `GET /readyz` SHALL remain reachable without authentication
regardless.

#### Scenario: Missing bearer is rejected when configured
- **WHEN** the server is started with `AGENTMEM_HTTP_BEARER=secret` and
  `GET /v1/context?agent=default&user=alice` is sent without an `Authorization`
  header
- **THEN** the server responds `401 Unauthorized` and does not render any context

#### Scenario: Matching bearer is accepted
- **WHEN** the server is started with `AGENTMEM_HTTP_BEARER=secret` and the
  request carries `Authorization: Bearer secret`
- **THEN** the server responds `200 OK` with the rendered context

#### Scenario: Unauthenticated when bearer unset
- **WHEN** `AGENTMEM_HTTP_BEARER` is unset and `GET /v1/context?agent=default&user=alice`
  is sent without an `Authorization` header
- **THEN** the server responds `200 OK` with the rendered context
