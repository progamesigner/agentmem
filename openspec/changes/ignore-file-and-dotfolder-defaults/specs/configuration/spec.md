## MODIFIED Requirements

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
