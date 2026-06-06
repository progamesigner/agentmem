## MODIFIED Requirements

### Requirement: Server binary lifecycle
The system SHALL ship a single Rust binary `agentmem` that, on launch, reads configuration from the environment, initialises logging to the correct sink for the selected transport, registers all tools with the `rmcp` server, and begins serving requests until terminated by a signal.

#### Scenario: Successful stdio startup
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT=stdio` and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process reads JSON-RPC frames from stdin, writes JSON-RPC responses to stdout, writes all logs and diagnostics to stderr, and continues running until stdin is closed or it receives `SIGTERM`/`SIGINT`

#### Scenario: Successful http startup (default)
- **WHEN** `agentmem` is launched with `AGENTMEM_TRANSPORT` unset (defaults to `http`) and a valid `AGENTMEM_ROOT_DIR`
- **THEN** the process binds a TCP listener on `127.0.0.1:8000`, serves the MCP Streamable HTTP endpoint at `POST /mcp`, the MCP SSE endpoint at `GET /mcp`, a liveness route at `GET /health`, and runs until receiving `SIGTERM`/`SIGINT`

#### Scenario: Misconfiguration fails fast
- **WHEN** `AGENTMEM_ROOT_DIR` is missing or invalid, or `AGENTMEM_VFS_SCHEME`/`AGENTMEM_POLICY`/`AGENTMEM_AGENTS_DIR` is set to an invalid value
- **THEN** the process writes a single human-readable line to stderr explaining which variable is wrong, exits with a non-zero status code, and does NOT begin accepting MCP requests

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
