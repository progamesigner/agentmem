## ADDED Requirements

### Requirement: Resources and prompts capability advertisement
The system SHALL advertise the resources and prompts capabilities during the MCP `initialize` handshake, in addition to tools, so that clients discover the `session-context` resource and the `session-context` prompt.

#### Scenario: Capabilities include resources and prompts
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the server's advertised capabilities include both resources and prompts alongside tools

### Requirement: `session-context` resource
The system SHALL expose a `session-context` resource at the templated URI `agentmem://session-context/{…}`, registered through `resources/templates/list`, whose URI parameters are derived, in order, from the configured scheme's placeholders. A `resources/read` of a concrete URI SHALL return the rendered session-context (produced by the shared renderer) as the resource contents for the scope encoded in the URI.

#### Scenario: Resource URI params follow the scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>.<user>` and a client calls `resources/templates/list`
- **THEN** the listed URI is `agentmem://session-context/{agent}/{user}`

#### Scenario: Reading a resource renders the context
- **WHEN** a client calls `resources/read` for `agentmem://session-context/coder/alice`
- **THEN** the response contents are the rendered session-context for scope `{agent: coder, user: alice}`

#### Scenario: Reading an empty-vault scope succeeds
- **WHEN** a client reads the session-context resource for a scope with no foundational files and no template
- **THEN** the read succeeds and returns the compiled-in default template with missing sentinels, never a not-found error

### Requirement: `session-context` prompt
The system SHALL expose a prompt named `session-context` through `prompts/list`, whose arguments are derived from the configured scheme's placeholders. A `prompts/get` SHALL return a message whose content is the rendered session-context (produced by the shared renderer) for the scope supplied in the arguments.

#### Scenario: Prompt arguments follow the scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>.<user>` and a client calls `prompts/list`
- **THEN** the `session-context` prompt declares required string arguments `agent` and `user`

#### Scenario: Getting the prompt renders the context
- **WHEN** a client calls `prompts/get` for `session-context` with `{agent: coder, user: alice}`
- **THEN** the returned message content is the rendered session-context for that scope

#### Scenario: Missing required argument is rejected
- **WHEN** a client calls `prompts/get` for `session-context` omitting a required scope argument
- **THEN** the server returns an error naming the missing argument and does not render
