## MODIFIED Requirements

### Requirement: HTTP transport variables
The system SHALL, when the active transport is `http`, accept an optional `AGENTMEM_HTTP_BIND` socket address, an optional `AGENTMEM_HTTP_BEARER` static token, an optional `AGENTMEM_HTTP_TOKENS_FILE` path, and an optional `AGENTMEM_HTTP_ALLOWED_HOSTS` allow-list. `AGENTMEM_HTTP_BIND` SHALL default to `127.0.0.1:8000` when the variable is unset, so local development needs no CORS or auth configuration.

`AGENTMEM_HTTP_ALLOWED_HOSTS` SHALL be a comma-separated list of `Host` authorities — each a hostname or `host:port` — that the Streamable HTTP transport accepts in the inbound `Host` header. When the variable is unset (or empty after trimming), the system SHALL leave the transport's built-in loopback-only default in effect (`localhost`, `127.0.0.1`, `::1`). The single value `*` SHALL disable `Host` validation entirely. Surrounding whitespace around each entry SHALL be trimmed and empty entries SHALL be ignored. The variable SHALL be overridable by a mirroring `--http-allowed-hosts` CLI flag, with the CLI flag taking precedence over the environment variable.

`AGENTMEM_HTTP_TOKENS_FILE` SHALL name a JSON file of the form `{ "tokens": [ { "token": <string>, "scopes": { <placeholder>: <exact-or-*> , … } }, … ] }`, read once at startup and mirrored by a `--http-tokens-file` CLI flag. Validation SHALL fail startup (with a message that does not echo token values) when the file is missing or unreadable, when JSON parsing fails, when any entry's `token` is empty, when a `scopes` object names a key that is not an active scheme placeholder or omits one of the placeholders, or when a scope value is neither an exact string nor the single character `*`. Token values SHALL NOT appear in logs or in `--print-config` output. The variable SHALL be ignored under the `stdio` transport.

#### Scenario: Default bind address is loopback
- **WHEN** transport is `http` and `AGENTMEM_HTTP_BIND` is unset
- **THEN** the server binds `127.0.0.1:8000` and the chosen address is logged at startup

#### Scenario: Non-loopback bind without bearer logs a warning
- **WHEN** `AGENTMEM_HTTP_BIND=0.0.0.0:8000` is set and both `AGENTMEM_HTTP_BEARER` and `AGENTMEM_HTTP_TOKENS_FILE` are unset
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

#### Scenario: Tokens file is validated at startup
- **WHEN** `AGENTMEM_HTTP_TOKENS_FILE` points to a file whose entry grants a key that is not a scheme placeholder (e.g. `tenant` under the scheme `<agent>.<user>`), or uses a partial pattern like `"t*"`
- **THEN** the server refuses to start with an error naming the offending key or value, without echoing any token

#### Scenario: Tokens never appear in output
- **WHEN** the server starts with a valid tokens file and `--print-config` is requested or startup logs are inspected
- **THEN** no token value appears in any output

#### Scenario: Stdio ignores HTTP variables
- **WHEN** `AGENTMEM_TRANSPORT=stdio` and `AGENTMEM_HTTP_ALLOWED_HOSTS` or `AGENTMEM_HTTP_TOKENS_FILE` is set
- **THEN** no TCP listener is opened and the values of the HTTP-only variables are ignored
