## Context

`src/frontmatter.rs` already parses a leading `---` YAML block into a
`serde_json::Value` object plus the remaining body, treating malformed or
absent frontmatter as "no properties". It is feature-gated behind
`recall-tantivy` solely because the tantivy indexer was its only consumer; the
user-decision for this change is to accept YAML in the default build. The
storage layer is byte-exact and frontmatter-agnostic, and stays that way —
property tools are a tools-layer composition of parse → mutate → serialize →
`write_atomic`.

## Goals / Non-Goals

**Goals:**
- Read and mutate note properties as structured data, on every build, with
  Obsidian-valid output.
- Keep the note body byte-identical across property updates.
- Keep tantivy `filters` coherent: a property update is immediately queryable.

**Non-Goals:**
- Preserving YAML comments, key order quirks, anchors, or flow-style
  formatting inside the frontmatter block (it is rewritten as data; Obsidian
  does the same when editing properties).
- Property *schema* enforcement (types per key, allowed keys) — vault
  convention, not server policy.
- Supporting frontmatter filters on the simple recall backend (unchanged;
  separate concern).

## Decisions

- **Two tools, `read_note_properties` / `update_note_properties`, not one.**
  Read is policy-read-gated, update is write-gated; fusing them would blur the
  gate and the schemas. Verbs follow the existing tool family
  (`read_memory_note`, `update_task_heartbeat`).
- **Merge semantics with `null`-deletes, not whole-object replace.** The
  common operation is touching one key (`status`, `reviewed`); replace
  semantics would force read-modify-write back onto the agent and reintroduce
  the lost-update window. The whole merge runs under the storage layer's
  per-target lock via a read-modify-write, so concurrent updates to different
  keys both land. An empty post-merge object removes the `---` block entirely
  (no stub fences left behind).
- **Values are arbitrary JSON.** `properties` is a JSON object; strings,
  numbers, booleans, arrays, and nested objects round-trip through
  `serde_yaml` exactly as the parser already maps them for the indexer, so
  what `filters` queries is what the tools wrote.
- **Malformed existing YAML is refused, not clobbered.** `parse` treats a
  malformed block as body text; blindly prepending a fresh block would leave
  two `---` fences and silently demote the human's (broken) metadata. The
  update tool detects "looks like a fence but does not parse" and returns
  `invalid_argument` naming the problem — a human fixes it in an editor; reads
  keep returning `{}` for it (unchanged parser behavior).
- **Serialization is normalized.** The block is re-emitted as
  `---\n<yaml>\n---\n` with keys in stable sorted order (the
  `serde_json::Value` object representation the parser already produces).
  Normalization is the price of treating properties as data; the body below
  the closing fence is never re-encoded.
- **No link transform inside frontmatter.** Property values are data, not
  prose; expanding `[[links]]` inside YAML strings would corrupt values and
  Obsidian does not resolve them there. (The body is untouched, so this tool
  cannot leak suffixes — the leak guard is not in play.)
- **Core root files: readable, not updatable.** `read_note_properties` works
  anywhere reads work; `update_note_properties` rejects agents-folder
  root-level paths with the existing wrapper-naming error, exactly like
  `write_memory_note`/`edit_memory_note`.
- **Un-gating = deleting cfg attributes, not restructuring.** `serde_yaml`
  moves out of `[features]`' optional set; `frontmatter.rs` loses its gate;
  the tantivy backend's own gate is untouched. `serde_yaml`'s maintenance-mode
  status is unchanged risk the project already carries for its published
  (tantivy) image; swapping parsers later is a contained follow-up since
  `frontmatter.rs` is the single YAML chokepoint.

## Risks / Trade-offs

- [Frontmatter comments/order are not preserved on update] → Inherent to
  data-level editing and matches Obsidian's own properties editor; documented
  in the tool description so agents and humans expect normalization.
- [`serde_yaml` is in maintenance mode and now ships in every build] →
  Accepted by explicit decision; single-chokepoint design keeps a future
  parser swap small.
- [YAML type coercion surprises (e.g. `no` → boolean) when humans hand-edit] →
  Read tool reports what the indexer sees — surfacing the coercion is a
  feature (agents observe the effective value, mismatches become visible).
