# context-http-api Specification

## Purpose
TBD - created by archiving change add-context-http-endpoint. Update Purpose after archive.
## Requirements
### Requirement: Versioned context endpoint
The system SHALL serve a stateless, read-only HTTP route `GET /v1/context` on the
HTTP transport's `axum` router, alongside `POST/GET /mcp` and `GET /health`. The
route SHALL render the per-scope session-context bootstrap using the same
renderer as the `load_session_context` tool, the `session-context` resource, and
the `session-context` prompt. The route SHALL exist only when the binary is built
with the `transport-http` feature and the HTTP transport is selected.

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

### Requirement: Scope parameter binding
The endpoint SHALL accept exactly one query parameter per VFS-scheme placeholder,
named by the placeholder ident, and bind them into the scope map in the scheme's
order. It SHALL reject requests that omit a required placeholder, supply an empty
value, or include an unexpected parameter, with HTTP `400` and a JSON error body.
When the scheme is empty, the endpoint SHALL require no parameters.

#### Scenario: All placeholders supplied
- **WHEN** the scheme is `<team>.<agent>.<env>.<user>` and the request carries
  `?team=t&agent=a&env=prod&user=alice`
- **THEN** the server binds the scope `{team: "t", agent: "a", env: "prod", user: "alice"}`
  and responds `200 OK`

#### Scenario: Missing placeholder is rejected
- **WHEN** the scheme is `<agent>.<user>` and the request carries only `?agent=default`
- **THEN** the server responds `400 Bad Request` with a JSON body that names the
  missing scope key `user`

#### Scenario: Empty placeholder value is rejected
- **WHEN** the request carries `?agent=default&user=` (empty value)
- **THEN** the server responds `400 Bad Request` with a JSON body that names the
  offending scope key

#### Scenario: Unexpected parameter is rejected
- **WHEN** the scheme is `<agent>.<user>` and the request carries
  `?agent=default&user=alice&role=admin`
- **THEN** the server responds `400 Bad Request` with a JSON body that names the
  unexpected parameter `role`

#### Scenario: Empty scheme requires no parameters
- **WHEN** the scheme is empty (`AGENTMEM_VFS_SCHEME=`) and the request is
  `GET /v1/context` with no query parameters
- **THEN** the server responds `200 OK` with the rendered single-tenant bootstrap

### Requirement: Response negotiation
The endpoint SHALL return the rendered bootstrap as `text/markdown` by default so
it can be used directly as a system prompt. When the request's `Accept` header
prefers `application/json`, the endpoint SHALL instead return a JSON object
`{ "rendered": <string>, "missing": [<string>, …] }` mirroring the
`load_session_context` tool result, with `Content-Type: application/json`.

#### Scenario: Default is markdown
- **WHEN** `GET /v1/context?agent=default&user=alice` is sent without an `Accept`
  header (or with `Accept: text/markdown` or `*/*`)
- **THEN** the response `Content-Type` is `text/markdown` and the body is the
  rendered bootstrap text verbatim

#### Scenario: JSON negotiation
- **WHEN** `GET /v1/context?agent=default&user=alice` is sent with
  `Accept: application/json`
- **THEN** the response `Content-Type` is `application/json` and the body is an
  object with a `rendered` string field and a `missing` array of the absent
  foundational filenames

### Requirement: Authentication reuse
The endpoint SHALL sit behind the same bearer-token gate as `/mcp`. When
`AGENTMEM_HTTP_BEARER` is set, `GET /v1/context` SHALL require a matching
`Authorization: Bearer <token>` header and SHALL respond `401` otherwise. When
the bearer is unset the endpoint is unauthenticated, like `/mcp`. `GET /health`
SHALL remain reachable without authentication regardless.

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

### Requirement: Error mapping
The endpoint SHALL map invalid scope input to HTTP `400` with a JSON body of the
form `{ "error": <human-readable message> }`, and SHALL map genuine IO failures
during rendering to HTTP `500` with the same body shape. Error bodies SHALL NOT
leak resolved physical filesystem paths; messages refer to virtual paths and
scope keys only.

#### Scenario: Validation error shape
- **WHEN** a request is rejected for a missing or unexpected scope parameter
- **THEN** the response is `400` with `Content-Type: application/json` and a body
  `{ "error": "<message naming the offending key>" }`

#### Scenario: IO failure maps to 500
- **WHEN** rendering fails because of an unexpected IO error (not a missing file)
- **THEN** the response is `500` with a JSON `{ "error": … }` body that does not
  include any resolved physical path
