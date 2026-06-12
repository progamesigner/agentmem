# Design: expand-frontmatter-property-links

## Context

`rewrite_links` (`src/wikilink.rs`) scans whole files, so the body write/read paths already transform links sitting inside the literal YAML block — `write_memory_note` expands them, `read_memory_note` strips them, `references_to` counts them as backlinks, and renames retarget them. The property tools opted out on the theory that "properties are data": `read_note_properties` returns the raw parse (leaking suffixes the body path put there) and `update_note_properties` merges values verbatim (clean targets that don't resolve in Obsidian; leak guard skippable). The two surfaces present contradictory views of the same bytes.

## Goals / Non-Goals

**Goals:**
- One consistent agent-facing view: property values show exactly what the body view shows.
- Persisted property links resolve in Obsidian; the leak guard holds on every write path.
- Agent-supplied values round-trip through the property tools unchanged.

**Non-Goals:**
- Changing the recall index's view of properties (companion change `strip-recall-index-suffixes`).
- Transforming non-string values or YAML keys.
- Obsidian's strict whole-value "link property" semantics (see Decision 1).

## Decisions

1. **Transform every link occurrence in every string value, not only whole-value links.** Obsidian's properties UI treats only a string that is exactly `"[[target]]"` as a link, but the body transform — which already processes the same block on whole-file writes — rewrites any occurrence. Choosing whole-value-only would leave the two write paths persisting different forms for identical input and leave the suffix leak half-fixed; uniformity wins.
2. **A recursive `serde_json::Value` walker beside `wikilink.rs`, reusing `rewrite_links` per string leaf.** Two thin wrappers: expand (fallible, carries the leak guard error) and strip (infallible), mirroring `expand_links`/`strip_links`. Object keys are not transformed. Alternative — serializing props to YAML and running the text transform — was rejected: it would entangle quoting/round-trip concerns with link rewriting.
3. **Expansion runs on the supplied updates inside the locked read-modify-write, before `frontmatter::merge`.** The leak guard therefore fires before any mutation, and the merge stays a pure data operation. The returned `{ properties }` is the merged set passed through the strip walker, so the response is clean even when untouched existing keys carry suffixed values.
4. **Read path: parse raw, then strip the parsed value tree.** Stripping the text and re-parsing would be equivalent but does throwaway YAML work; walking the parsed tree reuses the same walker as the write path.
5. **`serde_yaml` re-serialization handles quoting.** Values containing `[[…]]` serialize quoted (flow indicators), which is exactly the form Obsidian expects for property links; no custom emitter needed.

## Risks / Trade-offs

- **[Index mismatch until the companion change lands]** → recall's `props_json` stores suffixed values, so an `eq` filter on the clean form misses; this is the pre-existing gap, now visible. Mitigation: `strip-recall-index-suffixes` closes it; the `contains` op works meanwhile.
- **[Expansion changes stored bytes for previously-clean property values]** → only for targets that resolve in the visible set, same rule bodies follow; dangling values stay verbatim, so no retroactive rewriting surprises.
- **[Recursive walk cost on large property trees]** → property blocks are small by construction; the walker is linear in string content.

## Open Questions

(none)
