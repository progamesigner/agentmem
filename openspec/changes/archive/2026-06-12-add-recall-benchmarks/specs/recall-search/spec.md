## MODIFIED Requirements

### Requirement: In-memory index lifecycle
Recall indexes SHALL be held entirely in memory; the system SHALL NOT write any
index data to disk. At startup the system SHALL eagerly build every scope index and
the shared index. The system SHALL update the owning index synchronously on its own
note writes, reconcile external edits via a filesystem watcher (debounced and
ignore-filtered, routing each event to the owning index idempotently by file
metadata), and run a periodic stat-diff reconcile as a backstop for missed watcher
events. Idle per-scope indexes SHALL be evicted least-recently-accessed-first so
that after a recall completes the number of resident per-scope indexes does not
exceed the configured `max_resident_scopes` bound (a configured value of 0 is
treated as 1), and evicted indexes SHALL be rebuilt on next access. The engine
SHALL expose the current resident per-scope index count so the eviction bound is
verifiable by tests and benchmarks.

#### Scenario: Server write is reflected immediately
- **WHEN** `write_memory_note` creates or replaces a note in the caller's scope
- **THEN** a subsequent recall in that scope reflects the new content without any
  external trigger

#### Scenario: External edit is picked up
- **WHEN** a human edits a note directly in Obsidian while the server is running
- **THEN** the watcher updates the owning index and a subsequent recall reflects the
  edit; if the watcher event is missed, the periodic stat-diff reconcile corrects it

#### Scenario: Evicted scope is rebuilt on access
- **WHEN** a per-scope index has been evicted under the memory bound and a recall for
  that scope arrives
- **THEN** the call blocks until the index is rebuilt and then returns correct results

#### Scenario: Resident indexes stay within the eviction bound
- **WHEN** the engine is configured with `max_resident_scopes` smaller than the
  number of scopes in the vault and recalls are issued against each scope in turn
- **THEN** after every recall completes, the resident per-scope index count reported
  by the engine is at most `max_resident_scopes`
