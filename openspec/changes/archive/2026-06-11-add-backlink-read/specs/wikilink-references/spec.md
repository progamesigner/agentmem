## ADDED Requirements

### Requirement: Backlink resolution is the inverse of forward resolution
The system SHALL be able to compute, for a given target note, the set of visible notes containing at least one link that resolves to that target, using the same resolution rules as the forward link transform: trailing-segment matching against the caller's visible set, own-scope-preferred then lexicographic tie-breaking, and all supported link forms (`[[wikilink]]`, `[[wikilink|alias]]`, `[[wikilink#heading]]`, `![[embed]]`, and relative markdown links ending in `.md`). A link SHALL count toward a target's backlinks only when forward resolution selects exactly that target. Dangling links SHALL count toward no target. Stored own-scope (suffixed) link forms SHALL be interpreted identically to their clean agent-facing forms.

#### Scenario: Suffixed on-disk form resolves like its clean form
- **WHEN** a stored own-scope note contains the on-disk link `[[rust.jarvis.tony]]` (written by the expansion transform for clean target `[[rust]]`)
- **THEN** backlink computation for the note that `[[rust]]` forward-resolves to includes the referring note

#### Scenario: Ambiguous basenames follow the forward tie-break
- **WHEN** two visible notes share the basename `rust` and a note links `[[rust]]`
- **THEN** the link counts as a backlink only for the entry forward resolution selects (own scope preferred, then the lexicographically smallest clean path)

#### Scenario: Dangling links produce no backlinks
- **WHEN** a visible note contains `[[ghost]]` and no visible note resolves that target
- **THEN** the link contributes to no note's backlinks
