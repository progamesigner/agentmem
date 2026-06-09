## MODIFIED Requirements

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
