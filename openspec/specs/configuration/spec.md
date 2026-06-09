# configuration Specification

## Purpose
TBD - created by archiving change build-agentmem-mcp-server. Update Purpose after archive.
## Requirements
### Requirement: Configuration source
The system SHALL be configured exclusively via environment variables. CLI flags MAY be accepted as overrides, but the canonical configuration surface is the environment.

#### Scenario: Env vars are read at startup
- **WHEN** the server is launched
- **THEN** it reads `AGENTMEM_ROOT_DIR`, `AGENTMEM_AGENTS_DIR`, `AGENTMEM_VFS_SCHEME`, `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE`, `AGENTMEM_POLICY`, `AGENTMEM_TRANSPORT`, `AGENTMEM_HTTP_BIND`, `AGENTMEM_HTTP_BEARER`, `AGENTMEM_HTTP_ALLOWED_HOSTS`, `AGENTMEM_TIMEZONE`, `AGENTMEM_HONOR_IGNORE_FILES`, `AGENTMEM_INCLUDE_HIDDEN`, and `AGENTMEM_LOG` from the process environment

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
- **THEN** the server starts successfully with: agents folder `Agents`, scheme `<agent>.<user>`, global session-context template path `<root>/AGENT_SESSION_CONTEXT.md`, policy `namespaced`, transport `http`, bind `127.0.0.1:8000`, timezone `UTC`, ignore files honoured, hidden entries excluded

### Requirement: Session-context template file configuration
The system SHALL honour `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` as the filesystem path to the global session-context template document. The default value SHALL be `<root>/AGENT_SESSION_CONTEXT.md`. A relative value SHALL be interpreted relative to the vault root. The configured file need not exist; when it is absent, the system SHALL fall back to the compiled-in default template (subject to the layered resolution defined in the memory-tools capability).

#### Scenario: Default global template path
- **WHEN** `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` is unset
- **THEN** the global session-context template path resolves to `<root>/AGENT_SESSION_CONTEXT.md`

#### Scenario: Custom global template path
- **WHEN** `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE=/etc/agentmem/bootstrap.md`
- **THEN** the server reads the global session-context template from that path

#### Scenario: Configured file absent is not an error
- **WHEN** `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` points to a path that does not exist
- **THEN** the server starts successfully and the renderer falls back to the compiled-in default template

### Requirement: Agents folder configuration
The system SHALL honour `AGENTMEM_AGENTS_DIR` as the relative folder name under the vault root that delimits the scoped/suffixed region. The default value SHALL be `Agents`. A value of `.` or the empty string SHALL be interpreted as "the agents folder IS the vault root".

#### Scenario: Default agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR` is unset
- **THEN** the agents folder resolves to `<root>/Agents/`

#### Scenario: Custom subdirectory
- **WHEN** `AGENTMEM_AGENTS_DIR=memory`
- **THEN** the agents folder resolves to `<root>/memory/` and any virtual path under `memory/` is treated as inside the agents region

#### Scenario: Vault root is the agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=.` (or empty)
- **THEN** the agents folder resolves to the vault root itself; every virtual path inside the vault is inside the agents region and the "outside the agents folder" region is empty

#### Scenario: Path traversal in agents dir is rejected
- **WHEN** `AGENTMEM_AGENTS_DIR=../escape`
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending value

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

### Requirement: Server-wide policy
The system SHALL honour `AGENTMEM_POLICY` as exactly one of `scoped`, `namespaced`, `readonly`, `readwrite`. The default value SHALL be `namespaced`. The policy governs read/write permissions across the whole vault, in concert with the agents-folder boundary.

#### Scenario: Default policy
- **WHEN** `AGENTMEM_POLICY` is unset
- **THEN** the effective policy is `namespaced`: inside the agents folder, own-scope read/write with suffix; outside the agents folder but inside the vault root, read-only with no suffix; outside the vault root, denied

#### Scenario: scoped policy denies outside agents folder
- **WHEN** `AGENTMEM_POLICY=scoped` and an agent attempts to read a path outside the agents folder but inside the vault root
- **THEN** the operation is refused with code `path_not_permitted`

#### Scenario: readonly forbids writes everywhere
- **WHEN** `AGENTMEM_POLICY=readonly` and any tool that performs a write is invoked
- **THEN** the operation is refused with code `write_denied`, regardless of whether the target is inside or outside the agents folder

#### Scenario: readwrite permits writes outside agents folder
- **WHEN** `AGENTMEM_POLICY=readwrite` and an agent writes to a path outside the agents folder but inside the vault root
- **THEN** the write succeeds, no VFS suffix is applied, and every other agent sees the resulting file at the same virtual path

#### Scenario: Invalid policy
- **WHEN** `AGENTMEM_POLICY` is set to any value other than the four accepted strings
- **THEN** the process exits non-zero with a stderr message listing the accepted values

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

### Requirement: Visibility filter variables
The system SHALL honour `AGENTMEM_HONOR_IGNORE_FILES` and `AGENTMEM_INCLUDE_HIDDEN` as strict booleans (`true`/`false`) that control which files are visible to and addressable by agents. The defaults SHALL be `AGENTMEM_HONOR_IGNORE_FILES=true` and `AGENTMEM_INCLUDE_HIDDEN=false`. When `AGENTMEM_HONOR_IGNORE_FILES=true`, the system SHALL consult a generic `.ignore` file in addition to `.gitignore` and `.obsidianignore`.

The system SHALL additionally accept `AGENTMEM_INCLUDE_HIDDEN_GLOBS`, a comma-separated list of gitignore-style glob patterns evaluated relative to the vault root. Each pattern exempts matching dot-paths — and their entire subtree — from hidden-segment exclusion, so that a specific dotfile or dot-directory (e.g. `.obsidian/**`) can be exposed while other dotfiles stay excluded. The default SHALL be empty (no exemptions). Each of the boolean variables and the glob list SHALL be overridable by a mirroring CLI flag (`--honor-ignore-files`, `--include-hidden`, `--include-hidden-globs`), with the CLI flag taking precedence over the environment variable.

#### Scenario: Defaults exclude hidden files and honour ignore files
- **WHEN** neither variable is set
- **THEN** any path whose any segment begins with `.` is excluded from all tools, and any path matched by a `.ignore`, `.gitignore`, or `.obsidianignore` rule inside the vault is also excluded

#### Scenario: Including hidden files
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=true`
- **THEN** dotfiles and dotdirectories (excluding ignored ones, unless ignore is also disabled) are visible to and addressable by agents

#### Scenario: Include-hidden glob list exposes selected dot-paths
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN=false` and `AGENTMEM_INCLUDE_HIDDEN_GLOBS=.obsidian/**,**/.config`
- **THEN** dot-paths matching either glob (and everything beneath them) are visible to and addressable by agents, while all other dot-paths remain excluded

#### Scenario: Empty include-hidden glob list is the default
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN_GLOBS` is unset or empty
- **THEN** no dot-path exemption applies and hidden filtering behaves exactly as when only `AGENTMEM_INCLUDE_HIDDEN` is considered

#### Scenario: CLI flag overrides environment for the glob list
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN_GLOBS=.cache/**` is set in the environment and the process is started with `--include-hidden-globs .obsidian/**`
- **THEN** the effective include-hidden glob list is `.obsidian/**` and `.cache/**` is not applied

#### Scenario: Disabling ignore-file enforcement
- **WHEN** `AGENTMEM_HONOR_IGNORE_FILES=false`
- **THEN** `.ignore`, `.gitignore`, and `.obsidianignore` patterns are not consulted; hidden filtering still applies according to `AGENTMEM_INCLUDE_HIDDEN` and `AGENTMEM_INCLUDE_HIDDEN_GLOBS`

#### Scenario: Invalid boolean
- **WHEN** either boolean variable is set to a value other than `true` or `false`
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending value

#### Scenario: Invalid glob pattern fails fast
- **WHEN** `AGENTMEM_INCLUDE_HIDDEN_GLOBS` contains an entry that is not a valid gitignore-style glob
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending pattern

### Requirement: Timezone for date-derived tools
The system SHALL honour `AGENTMEM_TIMEZONE` as an IANA timezone identifier (e.g. `Asia/Taipei`, `UTC`). The default value SHALL be `UTC`. The timezone SHALL be used by any tool that derives a date or time from "now" (notably `append_daily_entry`).

#### Scenario: Default timezone is UTC
- **WHEN** `AGENTMEM_TIMEZONE` is unset and `append_daily_entry` is called at `2026-05-25T23:30:00Z`
- **THEN** the resolved virtual path is `<agents_dir>/diary/2026-05-25.md`

#### Scenario: Custom timezone shifts the date boundary
- **WHEN** `AGENTMEM_TIMEZONE=Asia/Taipei` and `append_daily_entry` is called at `2026-05-25T23:30:00Z` (07:30 next day in Taipei)
- **THEN** the resolved virtual path is `<agents_dir>/diary/2026-05-26.md`

#### Scenario: Invalid timezone fails fast
- **WHEN** `AGENTMEM_TIMEZONE` is set to a string that is not a valid IANA timezone
- **THEN** the process exits non-zero with a stderr message naming the variable and the offending value

### Requirement: Logging configuration
The system SHALL honour `AGENTMEM_LOG` as a `tracing_subscriber::EnvFilter` directive string. The default level SHALL be `info` for the `agentmem` crate and `warn` for everything else.

#### Scenario: Default filter
- **WHEN** `AGENTMEM_LOG` is unset
- **THEN** the active filter is `warn,agentmem=info`

#### Scenario: Custom filter applied
- **WHEN** `AGENTMEM_LOG=debug,agentmem=trace`
- **THEN** the active filter is exactly that string and is logged once at startup

