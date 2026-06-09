## ADDED Requirements

### Requirement: Recall configuration variables
The system SHALL read recall configuration from the environment, with CLI flag
overrides consistent with the other configuration variables. `AGENTMEM_RECALL_BACKEND`
SHALL select the backend, accepting `simple`, `tantivy`, and `off`, and SHALL default
to `simple`. The system SHALL additionally accept configuration for the filesystem
watcher debounce window, the regex scan guard (a byte and/or time cap), and the
recall memory/eviction bound. The `tantivy` backend SHALL be compiled in only under
its optional cargo feature; no recall configuration SHALL require an on-disk index
directory, because indexes are held in memory only.

#### Scenario: Default backend
- **WHEN** `AGENTMEM_RECALL_BACKEND` is unset
- **THEN** the configured backend is `simple`

#### Scenario: Invalid backend value fails fast
- **WHEN** `AGENTMEM_RECALL_BACKEND` is set to a value other than `simple`, `tantivy`,
  or `off`
- **THEN** the process writes a human-readable line to stderr naming the variable and
  exits with a non-zero status, consistent with other misconfiguration handling

#### Scenario: No on-disk index directory is configured
- **WHEN** recall is enabled under any backend
- **THEN** the server requires no index directory configuration and writes no index
  data to disk
