## MODIFIED Requirements

### Requirement: Server binary lifecycle
The system SHALL ship a single Rust binary `agentmem` that, on launch, reads configuration from the environment, initialises logging to the correct sink for the selected transport, registers all tools with the `rmcp` server, and begins serving requests until terminated by a signal.

This requirement is restated unchanged to record that it continues to hold against the stable `rmcp` `1.x` line (upgraded from `0.9`); the observable lifecycle behavior is identical.

#### Scenario: Successful stdio startup
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT=stdio` and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process reads JSON-RPC frames from stdin, writes JSON-RPC responses to stdout, writes all logs and diagnostics to stderr, and continues running until stdin is closed or it receives `SIGTERM`/`SIGINT`

#### Scenario: Successful http startup (default)
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT` unset (defaults to `http`) and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process binds a TCP listener on `127.0.0.1:8000`, serves the MCP Streamable HTTP endpoint at `POST /mcp`, the MCP SSE endpoint at `GET /mcp`, a liveness route at `GET /health`, and runs until receiving `SIGTERM`/`SIGINT`

#### Scenario: Misconfiguration fails fast
- **WHEN** `AGENTMEM_ROOT_DIR` is missing or invalid, or `AGENTMEM_VFS_SCHEME`/`AGENTMEM_POLICY`/`AGENTMEM_AGENTS_DIR` is set to an invalid value
- **THEN** the process writes a single human-readable line to stderr explaining which variable is wrong, exits with a non-zero status code, and does NOT begin accepting MCP requests

### Requirement: Transport selection
The system SHALL select its transport based on the `AGENTMEM_TRANSPORT` environment variable, accepting the values `stdio` and `http`, and SHALL default to `http` when the variable is unset.

This requirement is restated unchanged to record that the `http` transport remains an `rmcp` Streamable HTTP transport mounted under an `axum` router, with the bearer middleware provided by `axum` (the previously declared but unused `tower`/`tower-http` direct dependencies are removed); behavior is identical.

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
