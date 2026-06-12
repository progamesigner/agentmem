# wikilink-references delta: expand-frontmatter-property-links

## ADDED Requirements

### Requirement: Frontmatter property values participate in the link transform
The bidirectional link transform SHALL cover link occurrences inside frontmatter property string values uniformly with body content: the same resolution rules (visible-set matching, shortest unambiguous names, own-scope-preferred tie-breaks), the same supported link forms, the same dangling-link preservation, and the same cross-scope leak guard. This SHALL hold regardless of the write path: whole-file writes already scan the literal frontmatter block as content; the property tools (`read_note_properties`, `update_note_properties`) SHALL apply the transform to string values — recursing into arrays and nested objects — so the property surface and the content surface present one consistent agent-facing view in which the caller never observes its own scope suffix.

#### Scenario: Property round-trip matches the body round-trip
- **WHEN** scope `jarvis.tony` writes `related: "[[rust]]"` (resolving to its own `topics/rust.md`) once via `write_memory_note` frontmatter and once via `update_note_properties`
- **THEN** both persist `"[[rust.jarvis.tony]]"` on disk, and both `read_memory_note` (showing the block as content) and `read_note_properties` (showing the parsed value) return the clean `[[rust]]` form

#### Scenario: Obsidian resolvability of property links
- **WHEN** an own-scope note's persisted frontmatter contains `related: "[[rust.jarvis.tony]]"`
- **THEN** the target names the suffixed physical basename, so Obsidian resolves the property link to the caller's physical file

#### Scenario: Property links count toward backlinks
- **WHEN** a visible note's frontmatter contains a property value whose link resolves to a target note
- **THEN** the referring note appears in the target's backlinks, identical to a body link
