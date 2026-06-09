## Why

Agents store memory in an Obsidian-style vault but cannot link notes together: a
note in one scope has no way to reference another note by name. Obsidian's
`[[wikilink]]` syntax is the natural cross-note reference, but the vault's
per-scope suffix transform (`rust.md` on disk is `rust.coder.alice.md`) means a
link an agent writes will not resolve in Obsidian, and a link rewritten to
resolve in Obsidian would expose another scope's existence. We need a transform
that lets agents write the shortest possible name while keeping links resolvable
for a human browsing the vault and structurally leak-free across scopes.

## What Changes

- Agents may write `[[wikilink]]` references using the **shortest unambiguous
  note name**; the server resolves them against the caller's visible set (own
  scope ∪ shared region), exactly as Obsidian resolves by basename.
- On **write**, link targets that resolve into the caller's own scope are
  rewritten to the suffixed physical name (`[[rust]]` → `[[rust.coder.alice]]`)
  so a human browsing the vault in Obsidian can follow them.
- On **read**, the caller's own scope suffix is stripped from every link target,
  so the agent sees only clean shortest names and never another scope's suffix.
- Ambiguous targets are qualified to the **shortest unambiguous path**
  (`[[topics/rust]]`) on both write and read, mirroring Obsidian.
- A link in a **shared** file that targets the caller's **own scoped** note is
  **rejected** (`write_denied`-class error), because persisting its suffixed form
  would leak the scope's existence to every other reader of that shared file.
- The transform handles `[[target]]`, `[[target|alias]]`, `[[target#heading]]`,
  embeds `![[target]]`, and relative markdown links `[text](path.md)`. External
  (`http(s)://`) and anchor-only (`#section`) markdown links are left untouched.
- `edit_memory_note` applies the write-transform to its `search_string` so edits
  match the suffixed link form stored on disk.
- The transform applies to **every note-writing surface**, including the core-file
  wrappers `evolve_core_persona` (PERSONA/PROMPT/RULES/USER/MEMORY) and
  `update_task_heartbeat` (HEARTBEAT.md), so a MEMORY.md index of `[[notes]]`
  resolves in Obsidian. `load_session_context` strips the suffix from the
  foundational files it renders, so the agent sees clean names there too.

## Capabilities

### New Capabilities
- `wikilink-references`: Bidirectional rewriting of `[[wikilink]]` and relative
  markdown link targets between the agent-facing clean shortest-name form and the
  on-disk suffixed/Obsidian-resolvable form, including resolution against the
  caller's visible set, shortest-unambiguous-path qualification, and the
  cross-scope leak guard.

### Modified Capabilities
- `memory-tools`: `read_memory_note`, `write_memory_note`, `edit_memory_note`,
  `append_diary_entry`, the core-file wrappers (`evolve_core_persona`,
  `update_task_heartbeat`), and `load_session_context` gain link-transform
  behavior on their content (and, for edit, on `search_string`).
- `vault-storage`: The own-scope strictness guarantee is extended to link targets
  embedded in note content, not only to filenames.

## Impact

- **Code**: new `src/wikilink.rs` transform module; hooks in `src/tools.rs`
  (read/write/edit/diary handlers); a visible-set resolution index built on
  `src/storage.rs` (`walk_files`/`list_visible`); reuse of the suffix primitives
  in `src/path.rs`.
- **Security model**: `docs/security.md` gains a row for cross-scope link leakage
  and the shared→scoped rejection rule.
- **No new dependencies**; no breaking changes to existing tool schemas (behavior
  is additive over note content).
