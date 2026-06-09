## Context

The vault is an Obsidian-style markdown store with two regions (`src/path.rs`,
`src/storage.rs`):

- **Inside the agents folder** — scoped and isolated. A note an agent addresses
  as `Agents/topics/rust.md` is physically `Agents/coder.alice/topics/rust.coder.alice.md`.
  The rendered scope (`coder.alice`) is both a directory segment and a file-stem
  suffix. Cross-scope access is structurally impossible.
- **Outside the agents folder** — shared. Every scope reads/writes the same bytes
  at the same path; no suffix.

The suffix transform already exists for **filenames** (`apply_suffix_to_filename`
/ `strip_scope_from_filename` in `src/path.rs`). This change extends the same idea
to **link targets embedded in note content**, so agents can cross-reference notes
with `[[wikilinks]]` while keeping links Obsidian-resolvable and leak-free.

Resolution decisions already settled with the user:

- Audience is **both** a human browsing in Obsidian and the agent round-tripping
  through the server → disk must store the suffixed/resolvable form; the agent
  must see only clean shortest names.
- Ambiguity resolves to the **shortest unambiguous path**, Obsidian-faithful.
- A link in a shared file targeting the caller's own scoped note is **rejected**.
- Forms in scope: `[[t]]`, `[[t|alias]]`, `[[t#heading]]`, `![[t]]`, and relative
  markdown `[text](path.md)`.

## Goals / Non-Goals

**Goals:**

- Agents write the shortest unambiguous note name; the server rewrites to/from the
  physical form transparently on read and write.
- A human browsing the vault in Obsidian can follow every persisted link.
- An agent never sees another scope's suffix in any content it reads.
- Round-trip stability: `write([[rust]])` then `read` yields `[[rust]]`, and
  `edit_memory_note` can match link-bearing lines.

**Non-Goals:**

- Resolving links to other scopes (structurally invisible — out of the visible set).
- Maintaining a backlink graph, link autocompletion, or unresolved-link reports.
- Rewriting external (`http(s)://`) or anchor-only (`#section`) links.
- Cross-process index coherence guarantees beyond the existing last-writer-wins
  model (D5).

## Decisions

### D1: A dedicated transform module, called at the tool boundary

Add `src/wikilink.rs` with pure functions:

```
expand_links(content, scope, region, &index) -> Result<String>   // agent → disk
strip_links(content, scope) -> String                            // disk → agent
```

Hooked in `src/tools.rs`: `read_memory_note` runs `strip_links` on the content
before returning; `write_memory_note` / `append_diary_entry` run `expand_links`
before `write_atomic`. This keeps `tools.rs` thin and `storage.rs` unaware of link
semantics. **Alternative considered:** transform inside `storage.rs` — rejected
because storage has no notion of scope/region/visible-set and should stay a dumb
byte layer.

### D2: Reuse the path-layer suffix primitives

`strip_links` is `strip_scope_from_filename` applied to a link body; the
own-scope path of `expand_links` is `apply_suffix_to_filename`. Lift these to be
callable on link targets (basename for wikilinks). This guarantees the link
transform and the filename transform agree on what a suffix is — the same exact
rendered-scope match, so a clean note genuinely named `meeting.coder.alice.md`
behaves identically in both layers.

### D3: Resolution against a visible-set index, basename-keyed

`expand_links` resolves each target against an index of the caller's visible set
(own scope ∪ shared, suffixes stripped to clean names), built from
`storage::walk_files` / `list_visible`. The index maps clean basename → set of
clean paths. Resolution mirrors Obsidian:

1. exact clean-path match wins;
2. else basename match; if unique → bare name;
3. if multiple → **shortest unambiguous path** (extend the path leftward until
   unique).

`strip_links` recomputes the same shortest-name form so read output matches what
the agent would have written. **Alternative considered:** a persistent cached
index — deferred; build per-operation from the existing walk, which the listing
path already pays for. Revisit if profiling shows it matters.

### D4: Region-confined rewriting + the leak guard

The physical link form is chosen by **(region of the file being written) ×
(region the target resolves into)**:

| file ↓ / target → | own scope | shared |
|---|---|---|
| own scope | rewrite `[[rust.coder.alice]]` | leave `[[release]]` |
| shared | **REJECT** | leave `[[release]]` |

Only the own-scope→own-scope cell appends a suffix; that file is unreadable by
any other scope, so no leak. Shared→shared stays clean (correct for all readers).
Shared→own-scope is rejected: persisting `[[rust.coder.alice]]` into a file
`coder.bob` can read would leak `coder.alice`'s existence. The rejection names the
offending target and is raised before any write.

### D5: Markdown links are a two-part transform; wikilinks are stem-only

`[[basename]]` forms touch only the stem suffix. A relative markdown link
`[text](topics/rust.md)` encodes the **full relative path**, which on disk
includes both the per-scope directory and the stem suffix
(`coder.alice/topics/rust.coder.alice.md`). So markdown links round-trip through a
two-part transform (insert/strip the scope directory *and* the stem suffix), while
wikilinks only touch the stem. Markdown links are specified as a distinct
requirement and may ship in a second implementation phase to isolate the larger
parsing surface. External and anchor-only targets are passed through untouched.

### D6: `edit_memory_note` transforms `search_string`

`edit_search_replace` matches literally against the **physical** (suffixed)
content. The edit handler runs the same write-transform (`expand_links`) over the
`search_string` so a search containing `[[rust]]` matches the stored
`[[rust.coder.alice]]`. The `replace_string` is likewise expanded. Without this,
link-bearing edits silently return `edit_search_not_found`.

### D7: Dangling links are preserved verbatim

A target that resolves to nothing (note not yet created) is left as the agent
wrote it — Obsidian treats this as a valid dangling link. We do **not**
retroactively rewrite a dangling link when its target is later created; the agent
re-links if it wants resolution. This keeps the transform stateless and local to
one note's content.

## Risks / Trade-offs

- **Read/write asymmetry bugs** → The strongest mitigation is property tests:
  `strip_links(expand_links(x)) == normalize(x)` for the caller's own scope, over
  generated content with every supported link form.
- **Per-operation index cost** → Build reuses the existing walk; acceptable at
  current scale. Mitigation path is a cached index keyed by scope, invalidated on
  write — deferred until measured.
- **Shortest-name instability** → Adding a second note with a colliding basename
  changes the shortest form of an existing link on next read. Accepted: it matches
  Obsidian's own behavior and the on-disk (physical) link is unaffected.
- **Suffix-vs-real-name collision** → A note literally named `x.coder.alice.md`
  could be mis-stripped. Inherited from the path layer's exact-match behavior (D2);
  no new exposure.
- **Markdown link parsing surface** → Mitigated by phasing markdown links after
  wikilinks (D5) and by leaving any non-relative or anchor-only target untouched.

## Open Questions

- Should the per-scope **directory** segment ever appear in a rewritten markdown
  link target, or do we rely solely on Obsidian basename/shortest-path resolution
  so the directory never needs encoding? (Affects whether D5's "two-part" transform
  is truly two-part or collapses to stem-only.)
- Should a rejected shared→scoped link (D4) fail the whole write, or strip just
  that link and warn? Current decision: fail the write.
