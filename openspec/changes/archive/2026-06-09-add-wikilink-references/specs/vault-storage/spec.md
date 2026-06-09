## ADDED Requirements

### Requirement: Own-scope strictness extends to link targets in content

The own-scope strictness guarantee SHALL extend from filenames to link targets
embedded in note content. Inside the agents folder with a non-empty scheme, a
suffixed link target persisted in a note SHALL carry only the owning scope's
rendered suffix, and the system SHALL never persist a link bearing another scope's
suffix in a file readable by a different scope.

#### Scenario: Persisted own-scope link carries only the owner's suffix
- **WHEN** scope `{agent:"coder", user:"alice"}` writes an own-scope note linking
  to its own `rust.md`
- **THEN** the persisted link target is `rust.coder.alice` and contains no other
  scope's suffix

#### Scenario: A scoped suffix is never persisted in a shared file
- **WHEN** any caller writes a file in the shared region whose content links to a
  note in that caller's own scope
- **THEN** the write is refused before any bytes are written, so no scoped suffix
  is ever persisted in a shared file

### Requirement: Link transform respects visibility filters

Link resolution SHALL only consider notes that pass the existing visibility
filters (hidden-segment and ignore-file rules). A note excluded by those filters
SHALL NOT be a resolution candidate and SHALL be treated as outside the caller's
visible set.

#### Scenario: Ignored note is not a link target
- **WHEN** a `.gitignore` rule excludes `drafts/wip.md` and the caller writes a
  note containing `[[wip]]` with no other matching note
- **THEN** `wip` does not resolve (the excluded note is not a candidate) and the
  link is left verbatim as dangling
