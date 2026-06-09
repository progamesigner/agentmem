# mcp-server Specification

## Purpose
TBD - created by archiving change build-agentmem-mcp-server. Update Purpose after archive.
## Requirements
### Requirement: Server binary lifecycle
The system SHALL ship a single Rust binary `agentmem` that, on launch, reads configuration from the environment, initialises logging to the correct sink for the selected transport, registers all tools with the `rmcp` server, and begins serving requests until terminated by a signal.

#### Scenario: Successful stdio startup
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT=stdio` and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process reads JSON-RPC frames from stdin, writes JSON-RPC responses to stdout, writes all logs and diagnostics to stderr, and continues running until stdin is closed or it receives `SIGTERM`/`SIGINT`

#### Scenario: Successful http startup (default)
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT` unset (defaults to `http`) and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process binds a TCP listener on `127.0.0.1:8000`, serves the MCP Streamable HTTP endpoint at `POST /mcp`, the MCP SSE endpoint at `GET /mcp`, a liveness route at `GET /healthz`, a readiness route at `GET /readyz`, and runs until receiving `SIGTERM`/`SIGINT`

#### Scenario: Misconfiguration fails fast
- **WHEN** `AGENTMEM_ROOT_DIR` is missing or invalid, or `AGENTMEM_VFS_SCHEME`/`AGENTMEM_POLICY`/`AGENTMEM_AGENTS_DIR` is set to an invalid value
- **THEN** the process writes a single human-readable line to stderr explaining which variable is wrong, exits with a non-zero status code, and does NOT begin accepting MCP requests

### Requirement: Transport selection
The system SHALL select its transport based on the `AGENTMEM_TRANSPORT` environment variable, accepting the values `stdio` and `http`, and SHALL default to `http` when the variable is unset.

#### Scenario: http is the default transport
- **WHEN** `AGENTMEM_TRANSPORT` is unset
- **THEN** the server uses the `rmcp` Streamable HTTP transport mounted under an `axum` router

#### Scenario: stdio is selectable
- **WHEN** `AGENTMEM_TRANSPORT` is set to `stdio`
- **THEN** the server uses the `rmcp` stdio transport and no TCP listener is opened

#### Scenario: http is selectable explicitly
- **WHEN** `AGENTMEM_TRANSPORT` is set to `http`
- **THEN** the server uses the `rmcp` Streamable HTTP transport and binds the listener address from `AGENTMEM_HTTP_BIND` (default `127.0.0.1:8000`)

#### Scenario: Unknown transport value
- **WHEN** `AGENTMEM_TRANSPORT` is set to any value other than `stdio` or `http`
- **THEN** the server exits with a non-zero status and writes a stderr message that names the accepted values

### Requirement: Stdio output discipline
The system SHALL guarantee that, when running under stdio transport, no byte that is not a valid JSON-RPC frame is ever written to stdout.

#### Scenario: Logs go to stderr under stdio
- **WHEN** the server is running under stdio and emits a log line at any level
- **THEN** the line is written to stderr and stdout receives only the bytes that constitute MCP JSON-RPC frames

#### Scenario: Panics do not corrupt stdout
- **WHEN** an internal panic occurs in a tool handler
- **THEN** the panic message is written to stderr, the JSON-RPC response sent on stdout is a well-formed MCP error, and the server continues serving subsequent requests

### Requirement: Tool registration and listing
The system SHALL register the following nine tools with the MCP server and advertise them through `tools/list`: `list_memory_notes`, `read_memory_note`, `write_memory_note`, `edit_memory_note`, `delete_memory_note`, `load_session_context`, `evolve_core_persona`, `update_task_heartbeat`, `append_diary_entry`.

#### Scenario: tools/list returns the full set
- **WHEN** an MCP client calls `tools/list` after the `initialize` handshake
- **THEN** the response contains exactly the nine tool entries above, each with a JSON Schema generated from its Rust input struct via `schemars` and merged with the scheme-derived scope fields

#### Scenario: Schema reflects the configured VFS scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>` and a client calls `tools/list`
- **THEN** the input schemas for every tool include a required string `agent` parameter and do NOT include a `user` parameter

#### Scenario: Schema includes custom scheme keys
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<team>.<agent>.<env>.<user>` and a client calls `tools/list`
- **THEN** the input schemas for every tool include required string parameters `team`, `agent`, `env`, `user` in that order

#### Scenario: Empty scheme removes scope fields from schemas
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=` (empty) and a client calls `tools/list`
- **THEN** the input schemas for every tool include no scope parameters at all

### Requirement: Error reporting at the MCP boundary
The system SHALL map every internal error into an MCP tool result that contains a human-readable `text` message and a structured `code` discriminator. Raw OS error messages SHALL NOT be passed through verbatim.

#### Scenario: Policy violation returns a structured error
- **WHEN** a tool call is rejected because it tries to write to a `shared_readonly` path
- **THEN** the tool result is an MCP error whose text is of the form "write denied: path '...' is in a read-only region" and whose `code` field is `write_denied`

#### Scenario: Missing file
- **WHEN** `read_workspace_file` is called with a virtual path that resolves to a non-existent file
- **THEN** the tool result is an MCP error with code `not_found` and a message that includes the virtual path the client supplied (never the resolved physical path)

### Requirement: HTTP transport static authentication
The system SHALL, when running under `http` transport, optionally require an `Authorization: Bearer <token>` header matching `AGENTMEM_HTTP_BEARER`. When the variable is unset, no authentication is enforced and a startup warning is logged.

#### Scenario: Bearer token accepted
- **WHEN** `AGENTMEM_HTTP_BEARER=secret` is set and a client sends a request to `POST /mcp` with header `Authorization: Bearer secret`
- **THEN** the request is processed normally

#### Scenario: Bearer token rejected
- **WHEN** `AGENTMEM_HTTP_BEARER=secret` is set and a client sends a request without the header or with the wrong token
- **THEN** the server responds with HTTP 401 and an MCP error body, and the request never reaches a tool handler

#### Scenario: Auth disabled emits warning
- **WHEN** the server starts in `http` mode with `AGENTMEM_HTTP_BEARER` unset
- **THEN** a single `WARN`-level log line is emitted naming the variable and indicating that the HTTP endpoint is unauthenticated

### Requirement: HTTP transport Host validation
The system SHALL, when running under `http` transport, configure the `rmcp` Streamable HTTP service's inbound `Host` validation from the resolved allowed-hosts list (`AGENTMEM_HTTP_ALLOWED_HOSTS` / `--http-allowed-hosts`). When the list is non-empty, requests whose `Host` header authority matches an entry SHALL be accepted and all others SHALL be rejected by the transport. When the list is unset, the transport SHALL retain its loopback-only default (`localhost`, `127.0.0.1`, `::1`). The single value `*` SHALL disable `Host` validation so requests with any `Host` header are accepted.

This makes the HTTP transport usable behind a Kubernetes Service or ingress, where clients address the server by a cluster DNS name or external hostname rather than a loopback address.

#### Scenario: Cluster hostname accepted when allow-listed
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.svc.cluster.local` and a client sends `POST /mcp` carrying `Host: agentmem.svc.cluster.local`
- **THEN** the transport accepts the request and processes the MCP call

#### Scenario: Non-listed host rejected
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.example.com` and a client sends a request carrying `Host: evil.example.net`
- **THEN** the transport rejects the request

#### Scenario: Loopback default preserved when unset
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS` unset and a client on the same host sends `POST /mcp` carrying `Host: 127.0.0.1:8000`
- **THEN** the transport accepts the request, unchanged from prior behavior

#### Scenario: Validation disabled by wildcard
- **WHEN** the server runs under `http` transport with `AGENTMEM_HTTP_ALLOWED_HOSTS=*` and a client sends a request carrying any `Host` header
- **THEN** the transport accepts the request without `Host` validation

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
- **WHEN** a client calls `resources/read` for `agentmem://session-context/jarvis/tony`
- **THEN** the response contents are the rendered session-context for scope `{agent: jarvis, user: tony}`

#### Scenario: Reading an empty-vault scope succeeds
- **WHEN** a client reads the session-context resource for a scope with no foundational files and no template
- **THEN** the read succeeds and returns the compiled-in default template with missing sentinels, never a not-found error

### Requirement: `session-context` prompt
The system SHALL expose a prompt named `session-context` through `prompts/list`, whose arguments are derived from the configured scheme's placeholders. A `prompts/get` SHALL return a message whose content is the rendered session-context (produced by the shared renderer) for the scope supplied in the arguments.

#### Scenario: Prompt arguments follow the scheme
- **WHEN** the server is started with `AGENTMEM_VFS_SCHEME=<agent>.<user>` and a client calls `prompts/list`
- **THEN** the `session-context` prompt declares required string arguments `agent` and `user`

#### Scenario: Getting the prompt renders the context
- **WHEN** a client calls `prompts/get` for `session-context` with `{agent: jarvis, user: tony}`
- **THEN** the returned message content is the rendered session-context for that scope

#### Scenario: Missing required argument is rejected
- **WHEN** a client calls `prompts/get` for `session-context` omitting a required scope argument
- **THEN** the server returns an error naming the missing argument and does not render

