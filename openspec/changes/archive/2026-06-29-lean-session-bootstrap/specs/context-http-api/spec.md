## ADDED Requirements

### Requirement: Versioned bootstrap endpoint
The system SHALL serve a stateless, read-only HTTP route `GET /v1/bootstrap` on the HTTP transport's `axum` router, alongside `GET /v1/context`, `POST/GET /mcp`, `GET /healthz`, and `GET /readyz`. The route SHALL render the per-scope **lean bootstrap** (the `bootstrap` render kind of the shared renderer). It SHALL reuse the same scope-parameter binding, response negotiation (`text/markdown` by default, `{ rendered, missing }` JSON when `Accept` prefers `application/json`), authentication gate, and error mapping defined for `GET /v1/context`. The route SHALL exist only when the binary is built with the `transport-http` feature and the HTTP transport is selected. Adding this route SHALL NOT change the behavior or output shape of `GET /v1/context`.

#### Scenario: Bootstrap endpoint renders the lean bootstrap
- **WHEN** a client issues `GET /v1/bootstrap?agent=jarvis&user=tony` against a server whose scheme is `<agent>.<user>`
- **THEN** the server responds `200 OK` with the lean `bootstrap` render for the scope `{agent: "jarvis", user: "tony"}` — the scope banner, persona and rules, the onboarding directive (empty when no files are missing), and pointers to the full context and layout surfaces

#### Scenario: Bootstrap rendering never errors on absent files
- **WHEN** `GET /v1/bootstrap` is called for a scope whose foundational files do not yet exist
- **THEN** the server responds `200 OK` with the bootstrap rendered from the compiled-in default template, the missing files substituted by their sentinel and the onboarding directive present — absence is never an error

#### Scenario: Bootstrap endpoint reuses scope binding and auth
- **WHEN** `GET /v1/bootstrap` is called with a missing or unexpected scope parameter, or without a required `Authorization` bearer when one is configured
- **THEN** the server responds with the same `400`/`401`/`403` outcomes and JSON error shape as `GET /v1/context` under the identical conditions

#### Scenario: Bootstrap endpoint is absent without the HTTP transport
- **WHEN** the server is running under the `stdio` transport
- **THEN** no TCP listener is opened and `GET /v1/bootstrap` is not served

### Requirement: Versioned layout endpoint
The system SHALL serve a stateless, read-only HTTP route `GET /v1/layout` on the HTTP transport's `axum` router. The route SHALL render the per-scope **layout** content (the layout renderer) carrying the vault-mechanics guidance. It SHALL reuse the same scope-parameter binding, response negotiation, authentication gate, and error mapping defined for `GET /v1/context`. The route SHALL exist only when the binary is built with the `transport-http` feature and the HTTP transport is selected.

#### Scenario: Layout endpoint renders the layout
- **WHEN** a client issues `GET /v1/layout?agent=jarvis&user=tony` against a server whose scheme is `<agent>.<user>`
- **THEN** the server responds `200 OK` with the rendered layout content for that scope, identical to what the `agentmem://session-layout/{…}` resource returns for the same scope

#### Scenario: Layout endpoint reuses scope binding and auth
- **WHEN** `GET /v1/layout` is called with a missing or unexpected scope parameter, or without a required `Authorization` bearer when one is configured
- **THEN** the server responds with the same `400`/`401`/`403` outcomes and JSON error shape as `GET /v1/context` under the identical conditions

#### Scenario: Layout endpoint is absent without the HTTP transport
- **WHEN** the server is running under the `stdio` transport
- **THEN** no TCP listener is opened and `GET /v1/layout` is not served
