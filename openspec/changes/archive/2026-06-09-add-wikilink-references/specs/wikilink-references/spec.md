## ADDED Requirements

### Requirement: Visible-set link resolution

The system SHALL resolve a `[[wikilink]]` target against the caller's visible set
— the union of the caller's own scope (inside the agents folder, suffix stripped)
and the shared region (outside the agents folder) — using Obsidian's basename
matching. The system SHALL NOT resolve a target to any file outside the caller's
visible set.

#### Scenario: Bare name resolves to a unique visible note
- **WHEN** scope is `{agent:"coder", user:"alice"}`, the caller's own scope
  contains `topics/rust.md`, and the caller writes a note containing `[[rust]]`
- **THEN** the link resolves to `topics/rust.md` in the caller's own scope

#### Scenario: Target in another scope does not resolve
- **WHEN** the only note with basename `rust` on disk belongs to scope
  `{agent:"coder", user:"bob"}` and the caller `{agent:"coder", user:"alice"}`
  writes `[[rust]]`
- **THEN** the target does not resolve (it is outside the caller's visible set)
  and the link is treated as dangling and left verbatim

### Requirement: Shortest unambiguous link names

The system SHALL present and accept link targets as the shortest unambiguous note
name. When a bare basename is unique within the caller's visible set, the system
SHALL use the bare basename; when a basename is shared by two or more visible
notes, the system SHALL qualify the target with the shortest leading path segments
that make it unambiguous.

#### Scenario: Unique basename uses the bare name
- **WHEN** the caller's visible set contains exactly one note with basename `rust`
  and the caller reads a note linking to it
- **THEN** the rendered link target is `rust`

#### Scenario: Colliding basename is qualified to the shortest unambiguous path
- **WHEN** the caller's visible set contains both `topics/rust.md` (own scope) and
  `lang/rust.md` (shared) and the caller reads a note linking to the first
- **THEN** the rendered link target is `topics/rust`, not the bare `rust`

### Requirement: Own-scope link expansion on write

On write, for a note inside the caller's own scope, the system SHALL rewrite every
`[[wikilink]]` whose target resolves into the caller's own scope to the suffixed
physical basename, so the persisted link resolves in Obsidian. The system SHALL
leave targets that resolve into the shared region unchanged (no suffix).

#### Scenario: Own-scope target is suffixed on disk
- **WHEN** scope renders to `coder.alice`, the caller writes a note inside its own
  scope containing `[[rust]]`, and `rust` resolves to the caller's own
  `topics/rust.md`
- **THEN** the persisted file content contains `[[rust.coder.alice]]`

#### Scenario: Shared target is left clean on disk
- **WHEN** the caller writes an own-scope note containing `[[release]]` and
  `release` resolves to the shared `Actions/release.md`
- **THEN** the persisted file content contains `[[release]]` with no suffix

### Requirement: Own-scope suffix stripping on read

On read, the system SHALL strip the caller's own rendered scope suffix from every
`[[wikilink]]` target in the returned content, so the agent sees only clean
shortest names. The agent SHALL never observe a scope suffix in returned content.

#### Scenario: Suffix is stripped on read
- **WHEN** the persisted own-scope note content contains `[[rust.coder.alice]]`
  and scope `{agent:"coder", user:"alice"}` reads it
- **THEN** the returned content contains `[[rust]]`

#### Scenario: Round-trip is stable
- **WHEN** scope `coder.alice` writes content containing `[[rust]]` (resolving in
  its own scope) and then reads the same note
- **THEN** the returned content contains `[[rust]]`, identical to what was written

### Requirement: Cross-scope link leak prevention

The system SHALL reject a write whose content, in a file in the shared region,
contains a `[[wikilink]]` that resolves into the caller's own scope, because the
suffixed physical form would expose the caller's scope to other readers of the
shared file. The error SHALL name the offending target and the file SHALL be left
unchanged.

#### Scenario: Shared file linking to a scoped note is rejected
- **WHEN** policy permits writing the shared file `Actions/release.md`, and the
  caller writes content into it containing `[[rust]]` where `rust` resolves only
  into the caller's own scope
- **THEN** the write is refused with a `write_denied`-class error naming `rust`
  and `Actions/release.md` is unchanged

#### Scenario: Shared file linking to a shared note is allowed
- **WHEN** the caller writes `Actions/release.md` containing `[[changelog]]` where
  `changelog` resolves to the shared `Actions/changelog.md`
- **THEN** the write succeeds and the persisted content contains `[[changelog]]`

### Requirement: Supported link forms

The system SHALL apply the link transform to plain wikilinks `[[target]]`, aliased
wikilinks `[[target|alias]]`, heading links `[[target#heading]]`, embeds
`![[target]]`, and relative markdown links `[text](path.md)`. The system SHALL
rewrite only the target portion and SHALL preserve the alias text, heading anchor,
embed prefix, and markdown link text. The system SHALL leave external
(`http://`, `https://`) and anchor-only (`#section`) markdown link targets
unchanged.

#### Scenario: Alias and heading are preserved
- **WHEN** scope renders to `coder.alice` and the caller writes an own-scope note
  containing `[[rust|the Rust note]]` and `[[rust#install]]` resolving to its own
  `rust.md`
- **THEN** the persisted content contains `[[rust.coder.alice|the Rust note]]` and
  `[[rust.coder.alice#install]]`

#### Scenario: Embed target is rewritten with the prefix preserved
- **WHEN** the caller writes an own-scope note containing `![[rust]]` resolving to
  its own `rust.md` and scope renders to `coder.alice`
- **THEN** the persisted content contains `![[rust.coder.alice]]`

#### Scenario: Relative markdown link round-trips
- **WHEN** scope renders to `coder.alice` and the caller writes an own-scope note
  containing `[see Rust](topics/rust.md)` resolving to its own `topics/rust.md`
- **THEN** the persisted link resolves in Obsidian to the caller's physical file
  and a subsequent read returns `[see Rust](topics/rust.md)`

#### Scenario: External markdown link is untouched
- **WHEN** the caller writes a note containing `[docs](https://example.com)` and
  `[top](#summary)`
- **THEN** both links are persisted and returned verbatim

### Requirement: Dangling links are preserved

The system SHALL leave a `[[wikilink]]` or relative markdown link whose target
does not resolve in the caller's visible set unchanged on write and on read, and
SHALL NOT retroactively rewrite it if a matching note is later created.

#### Scenario: Unresolved link is preserved verbatim
- **WHEN** the caller writes a note containing `[[not-yet-created]]` with no
  matching note in the visible set
- **THEN** the persisted content contains `[[not-yet-created]]` and a later read
  returns it unchanged, even after a note named `not-yet-created` is created
