## MODIFIED Requirements

### Requirement: Configuration source
The system SHALL be configured exclusively via environment variables. CLI flags MAY be accepted as overrides, but the canonical configuration surface is the environment.

#### Scenario: Env vars are read at startup
- **WHEN** the server is launched
- **THEN** it reads `AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_SCHEME`, `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_HTTP_ALLOWED_HOSTS`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, and `AGENTMEM_LOG` from the process environment

#### Scenario: CLI flag overrides env var
- **WHEN** the server is launched with `--http-bind 0.0.0.0:9000` and `AGENTMEM_HTTP_BIND` is also set
- **THEN** the CLI flag wins and the bind address is `0.0.0.0:9000`

### Requirement: HTTP transport variables
The system SHALL, when the active transport is `http`, accept an optional `AGENTMEM_HTTP_BIND` socket address, an optional `AGENTMEM_HTTP_BEARER` static token, and an optional `AGENTMEM_HTTP_ALLOWED_HOSTS` allow-list. `AGENTMEM_HTTP_BIND` SHALL default to `127.0.0.1:8000` when the variable is unset, so local development needs no CORS or auth configuration.

`AGENTMEM_HTTP_ALLOWED_HOSTS` SHALL be a comma-separated list of `Host` authorities — each a hostname or `host:port` — that the Streamable HTTP transport accepts in the inbound `Host` header. When the variable is unset (or empty after trimming), the system SHALL leave the transport's built-in loopback-only default in effect (`localhost`, `127.0.0.1`, `::1`). The single value `*` SHALL disable `Host` validation entirely. Surrounding whitespace around each entry SHALL be trimmed and empty entries SHALL be ignored. The variable SHALL be overridable by a mirroring `--http-allowed-hosts` CLI flag, with the CLI flag taking precedence over the environment variable.

#### Scenario: Default bind address is loopback
- **WHEN** transport is `http` and `AGENTMEM_HTTP_BIND` is unset
- **THEN** the server binds `127.0.0.1:8000` and the chosen address is logged at startup

#### Scenario: Non-loopback bind without bearer logs a warning
- **WHEN** `AGENTMEM_HTTP_BIND=0.0.0.0:8000` is set and `AGENTMEM_HTTP_BEARER` is unset
- **THEN** the server starts and emits a single `WARN`-level log line indicating the endpoint is reachable from outside the host and is unauthenticated

#### Scenario: Allowed hosts default to loopback only
- **WHEN** transport is `http` and `AGENTMEM_HTTP_ALLOWED_HOSTS` is unset
- **THEN** the transport accepts the `Host` values `localhost`, `127.0.0.1`, and `::1` and rejects all others, matching the prior default

#### Scenario: Configured allowed hosts are accepted
- **WHEN** `AGENTMEM_HTTP_ALLOWED_HOSTS=agentmem.svc.cluster.local,agentmem.example.com:8000` is set
- **THEN** the parsed list is applied so that the trimmed authorities `agentmem.svc.cluster.local` and `agentmem.example.com:8000` are accepted in the inbound `Host` header

#### Scenario: Wildcard disables Host validation
- **WHEN** `AGENTMEM_HTTP_ALLOWED_HOSTS=*` is set
- **THEN** the transport accepts any `Host` header value and the server emits a single `WARN`-level log line noting that `Host` validation is disabled

#### Scenario: Stdio ignores HTTP variables
- **WHEN** `AGENTMEM_TRANSPORT=stdio` and `AGENTMEM_HTTP_ALLOWED_HOSTS` is set
- **THEN** no TCP listener is opened and the value of `AGENTMEM_HTTP_ALLOWED_HOSTS` is ignored
