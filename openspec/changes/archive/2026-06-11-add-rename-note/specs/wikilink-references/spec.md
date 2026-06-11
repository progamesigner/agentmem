## ADDED Requirements

### Requirement: Incoming references follow a rename
When a note is renamed, the system SHALL rewrite, in every visible referring note, exactly those link targets whose forward resolution selects the renamed note, leaving all other bytes of the referring note untouched. The rewritten target SHALL be derived the same way the write-side transform derives link targets: the shortest unambiguous name for the destination against the post-rename visible set, suffixed for own-scope wikilinks, the vault-root-relative physical path for own-scope markdown links, and the clean form for shared targets. Aliases, headings, and embed markers SHALL be preserved. Links that resolve to other notes — including other notes sharing the old basename — SHALL NOT be modified.

#### Scenario: Only links resolving to the renamed note are touched
- **WHEN** the visible set contains `Agents/topics/rust.md` and `Lang/rust.md`, a referring note links `[[topics/rust]]` and `[[Lang/rust]]`, and `Agents/topics/rust.md` is renamed
- **THEN** only the `[[topics/rust]]` target is rewritten; `[[Lang/rust]]` is byte-identical

#### Scenario: Rewritten names are shortest unambiguous post-rename
- **WHEN** a note is renamed such that its new basename is unique in the visible set
- **THEN** referring wikilinks are rewritten to the bare basename form (with the caller's suffix when the destination is own-scope), not a longer qualified path

#### Scenario: Decorations survive the rewrite
- **WHEN** a referring note links to the renamed note as `![[old]]`, `[[old#h]]`, and `[[old|alias]]`
- **THEN** the rewritten links are `![[new]]`, `[[new#h]]`, and `[[new|alias]]` respectively (with the applicable suffix form on disk)

#### Scenario: Round-trip after rename
- **WHEN** a referring note is rewritten by a rename and subsequently read through `read_memory_note`
- **THEN** the reader sees the clean shortest-name form of the new target, exactly as if the link had been written fresh against the post-rename vault
