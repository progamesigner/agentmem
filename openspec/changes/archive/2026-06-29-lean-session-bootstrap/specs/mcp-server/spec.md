## MODIFIED Requirements

### Requirement: Resources and prompts capability advertisement
The system SHALL advertise the resources and prompts capabilities during the MCP `initialize` handshake, in addition to tools, so that clients discover the `session-context`, `session-bootstrap`, and `session-layout` resources and the `session-context` prompt.

#### Scenario: Capabilities include resources and prompts
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the server's advertised capabilities include both resources and prompts alongside tools

#### Scenario: Resource templates list all three session resources
- **WHEN** a client calls `resources/templates/list`
- **THEN** the listed resources include `session-context`, `session-bootstrap`, and `session-layout`, each at its scheme-parameterized templated URI

## ADDED Requirements

### Requirement: `session-bootstrap` resource
The system SHALL expose a `session-bootstrap` resource at the templated URI `agentmem://session-bootstrap/{…}`, registered through `resources/templates/list`, whose URI parameters are derived, in order, from the configured scheme's placeholders. A `resources/read` of a concrete URI SHALL return the **lean bootstrap** render (the `bootstrap` render kind of the shared renderer) as the resource contents for the scope encoded in the URI.

#### Scenario: Bootstrap resource URI params follow the scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>.<user>` and a client calls `resources/templates/list`
- **THEN** the listed URI is `agentmem://session-bootstrap/{agent}/{user}`

#### Scenario: Reading the bootstrap resource renders the lean bootstrap
- **WHEN** a client calls `resources/read` for `agentmem://session-bootstrap/jarvis/tony`
- **THEN** the response contents are the lean `bootstrap` render for scope `{agent: jarvis, user: tony}`

#### Scenario: Reading an empty-vault scope succeeds
- **WHEN** a client reads the `session-bootstrap` resource for a scope with no foundational files and no template
- **THEN** the read succeeds and returns the compiled-in default bootstrap template with missing sentinels and the onboarding directive, never a not-found error

### Requirement: `session-layout` resource
The system SHALL expose a `session-layout` resource at the templated URI `agentmem://session-layout/{…}`, registered through `resources/templates/list`, whose URI parameters are derived, in order, from the configured scheme's placeholders. A `resources/read` of a concrete URI SHALL return the **layout** render for the scope encoded in the URI.

#### Scenario: Layout resource URI params follow the scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>.<user>` and a client calls `resources/templates/list`
- **THEN** the listed URI is `agentmem://session-layout/{agent}/{user}`

#### Scenario: Reading the layout resource renders the layout
- **WHEN** a client calls `resources/read` for `agentmem://session-layout/jarvis/tony`
- **THEN** the response contents are the rendered layout for scope `{agent: jarvis, user: tony}`, identical to what `GET /v1/layout` returns for the same scope

#### Scenario: Reading the layout for an empty-vault scope succeeds
- **WHEN** a client reads the `session-layout` resource for a scope with no layout template configured
- **THEN** the read succeeds and returns the compiled-in default layout content, never a not-found error

### Requirement: Scoped-token gating covers the bootstrap and layout surfaces
The system SHALL, when running under `http` transport with `AGENTMEM_HTTP_TOKENS_FILE` configured, authorize the new scope-bearing surfaces against the presenting token's scope grants exactly as it does for the `session-context` resource and `GET /v1/context`: a `session-bootstrap` resource read, a `session-layout` resource read, `GET /v1/bootstrap`, and `GET /v1/layout` SHALL each be permitted only when every requested scope key matches the token's grant, and a mismatch SHALL be rejected with a `scope_denied`/`403` error naming the offending key before any path resolution or IO. Unauthenticated bearers SHALL be rejected with `401` as for the existing surfaces.

#### Scenario: Scoped token gates the bootstrap and layout surfaces
- **WHEN** a client presenting a token granted `agent=jarvis` only requests the `session-bootstrap` resource, the `session-layout` resource, `GET /v1/bootstrap`, or `GET /v1/layout` for `agent=friday`
- **THEN** the request is rejected with a `scope_denied` error (HTTP `403` for the HTTP routes) naming the offending key `agent`, before any rendering

#### Scenario: Scoped token renders its own scope on the new surfaces
- **WHEN** the same token requests any of those surfaces for `agent=jarvis&user=tony`
- **THEN** the request succeeds and returns the rendered content for that scope
