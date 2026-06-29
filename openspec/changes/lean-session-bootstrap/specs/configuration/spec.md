## MODIFIED Requirements

### Requirement: Configuration source
The system SHALL be configured exclusively via environment variables. CLI flags MAY be accepted as overrides, but the canonical configuration surface is the environment.

#### Scenario: Env vars are read at startup
- **WHEN** the server is launched
- **THEN** it reads `AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_SCHEME`, `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE`, `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE`, `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_HTTP_ALLOWED_HOSTS`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, and `AGENTMEM_LOG` from the process environment

#### Scenario: CLI flag overrides env var
- **WHEN** the server is launched with `--http-bind 0.0.0.0:9000` and `AGENTMEM_HTTP_BIND` is also set
- **THEN** the CLI flag wins and the bind address is `0.0.0.0:9000`

### Requirement: Required configuration variables
The system SHALL require `AGENTMEM_ROOT_DIR` to be present and valid at startup, and SHALL refuse to start otherwise. All other variables have defaults.

#### Scenario: Missing root dir
- **WHEN** `AGENTMEM_ROOT_DIR` is unset
- **THEN** the process exits non-zero with a stderr message naming the variable

#### Scenario: Root dir is not a directory
- **WHEN** `AGENTMEM_ROOT_DIR` points to a path that does not exist or is not a directory
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending value

#### Scenario: All other variables have defaults
- **WHEN** only `AGENTMEM_ROOT_DIR` is set and every other variable is unset
- **THEN** the server starts successfully with: agents folder `Agents`, scheme `<agent>.<user>`, global session-context template path `<root>/AGENT_SESSION_CONTEXT.md`, global session-bootstrap template path `<root>/AGENT_SESSION_BOOTSTRAP.md`, global memory-layout template path `<root>/AGENT_MEMORY_LAYOUT.md`, policy `namespaced`, transport `http`, bind `127.0.0.1:8000`, timezone `UTC`, ignore files honoured, hidden entries excluded

## ADDED Requirements

### Requirement: Session-bootstrap template file configuration
The system SHALL honour `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` as the filesystem path to the global session-bootstrap (lean) template document. The default value SHALL be `<root>/AGENT_SESSION_BOOTSTRAP.md`. A relative value SHALL be interpreted relative to the vault root. The configured file need not exist; when it is absent, the system SHALL fall back to the compiled-in default bootstrap template (subject to the layered resolution defined in the memory-tools capability).

#### Scenario: Default global bootstrap template path
- **WHEN** `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` is unset
- **THEN** the global session-bootstrap template path resolves to `<root>/AGENT_SESSION_BOOTSTRAP.md`

#### Scenario: Custom global bootstrap template path
- **WHEN** `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE=/etc/agentmem/bootstrap.md`
- **THEN** the server reads the global session-bootstrap template from that path

#### Scenario: Configured bootstrap file absent is not an error
- **WHEN** `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` points to a path that does not exist
- **THEN** the server starts successfully and the renderer falls back to the compiled-in default bootstrap template

### Requirement: Memory-layout template file configuration
The system SHALL honour `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` as the filesystem path to the global memory-layout template document. The default value SHALL be `<root>/AGENT_MEMORY_LAYOUT.md`. A relative value SHALL be interpreted relative to the vault root. The configured file need not exist; when it is absent, the system SHALL fall back to the compiled-in default layout content (subject to the layered resolution defined in the memory-tools capability).

#### Scenario: Default global layout template path
- **WHEN** `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` is unset
- **THEN** the global memory-layout template path resolves to `<root>/AGENT_MEMORY_LAYOUT.md`

#### Scenario: Custom global layout template path
- **WHEN** `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE=/etc/agentmem/layout.md`
- **THEN** the server reads the global memory-layout template from that path

#### Scenario: Configured layout file absent is not an error
- **WHEN** `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` points to a path that does not exist
- **THEN** the server starts successfully and the renderer falls back to the compiled-in default layout content
