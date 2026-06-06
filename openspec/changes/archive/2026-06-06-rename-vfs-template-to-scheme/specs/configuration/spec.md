## RENAMED Requirements

- FROM: `### Requirement: VFS suffix template`
- TO: `### Requirement: VFS scheme`

## MODIFIED Requirements

### Requirement: Configuration source
The system SHALL be configured exclusively via environment variables. CLI flags MAY be accepted as overrides, but the canonical configuration surface is the environment.

#### Scenario: Env vars are read at startup
- **WHEN** the server is launched
- **THEN** it reads `AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_SCHEME`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, and `AGENTMEM_LOG` from the process environment

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
- **THEN** the server starts successfully with: agents folder `Agents`, scheme `<agent>.<user>`, policy `namespaced`, transport `http`, bind `127.0.0.1:8000`, timezone `UTC`, ignore files honoured, hidden entries excluded

### Requirement: VFS scheme
The system SHALL honour `AGENTMEM_VFS_SCHEME` as a dotted scheme string composed of literal segments and `<ident>` placeholders. The default value SHALL be `<agent>.<user>`. The scheme's placeholders define the required scope parameters on every tool call.

#### Scenario: Default scheme requires agent and user
- **WHEN** `AGENTMEM_VFS_SCHEME` is unset
- **THEN** every tool's input schema includes required string fields `agent` and `user`

#### Scenario: Single-key scheme
- **WHEN** `AGENTMEM_VFS_SCHEME=<agent>`
- **THEN** every tool's input schema includes a required string field `agent` and no `user` field

#### Scenario: Empty scheme disables suffixing
- **WHEN** `AGENTMEM_VFS_SCHEME=` (empty string)
- **THEN** tool input schemas include no scope fields, no VFS suffix is applied, and no own-scope filtering is performed inside the agents folder

#### Scenario: Custom multi-key scheme
- **WHEN** `AGENTMEM_VFS_SCHEME=<team>.<agent>.<env>.<user>`
- **THEN** every tool's input schema includes four required string fields `team`, `agent`, `env`, `user`; the rendered suffix for `{team:"platform", agent:"coder", env:"prod", user:"alice"}` is `platform.coder.prod.alice`

#### Scenario: Literal segments in scheme
- **WHEN** `AGENTMEM_VFS_SCHEME=v1.<agent>.<user>`
- **THEN** the rendered suffix for `{agent:"coder", user:"alice"}` is `v1.coder.alice` and tool schemas require only `agent` and `user`

#### Scenario: Malformed scheme
- **WHEN** `AGENTMEM_VFS_SCHEME=<agent` (unclosed bracket) or contains characters outside the grammar
- **THEN** the process exits non-zero with a stderr message naming the variable and pointing at the offending character

#### Scenario: Invalid placeholder name
- **WHEN** a placeholder ident does not match `[A-Za-z_][A-Za-z0-9_]*` (for example `<1bad>` or `<a-b>`)
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending placeholder
