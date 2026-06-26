## MODIFIED Requirements

### Requirement: Server binary lifecycle
The system SHALL ship a single Rust binary `agentmem` that, on launch, reads configuration from the environment, initialises logging to the correct sink for the selected transport, registers all tools with the `rmcp` server, and begins serving requests until terminated by a signal.

#### Scenario: Successful stdio startup
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT=stdio` and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process reads JSON-RPC frames from stdin, writes JSON-RPC responses to stdout, writes all logs and diagnostics to stderr, and continues running until stdin is closed or it receives `SIGTERM`/`SIGINT`

#### Scenario: Successful http startup (default)
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT` unset (defaults to `http`) and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process binds a TCP listener on `127.0.0.1:8000`, serves the MCP Streamable HTTP endpoint at `POST /mcp` in stateless JSON-response mode, a liveness route at `GET /healthz`, a readiness route at `GET /readyz`, and runs until receiving `SIGTERM`/`SIGINT`

#### Scenario: Misconfiguration fails fast
- **WHEN** `AGENTMEM_ROOT_DIR` is missing or invalid, or `AGENTMEM_VFS_SCHEME`/`AGENTMEM_POLICY`/`AGENTMEM_AGENTS_DIR` is set to an invalid value
- **THEN** the process writes a single human-readable line to stderr explaining which variable is wrong, exits with a non-zero status code, and does NOT begin accepting MCP requests

## ADDED Requirements

### Requirement: HTTP transport stateless JSON responses
The system SHALL, when running under `http` transport, configure the `rmcp` Streamable HTTP service in stateless mode with direct JSON responses (`stateful_mode = false`, `json_response = true`). Each `POST /mcp` request SHALL be handled independently and its JSON-RPC response returned with `Content-Type: application/json`, not `text/event-stream`. The server SHALL NOT issue an `mcp-session-id` header and SHALL NOT depend on a per-session SSE event stream for delivering responses or notifications, consistent with its advertised capabilities (no `listChanged`, no `subscribe`).

This matches the server's request→response semantics — every tool call resolves synchronously and the server never initiates messages — and avoids the SSE-on-POST response shape and `GET /mcp` resume churn that break clients which do not consume server-streamed responses.

#### Scenario: Tool call returns a JSON response
- **WHEN** a client completes the `initialize` handshake and sends a `tools/call` request to `POST /mcp` with `Accept: application/json, text/event-stream`
- **THEN** the server responds with `Content-Type: application/json` carrying the single JSON-RPC result, and the connection closes without an SSE event stream

#### Scenario: No session id is issued
- **WHEN** a client sends an `initialize` request to `POST /mcp`
- **THEN** the response does NOT include an `mcp-session-id` header and subsequent requests are accepted without one
