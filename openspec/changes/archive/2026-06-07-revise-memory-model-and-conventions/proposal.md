## Why

The foundational-file model carries a `TOOLS.md` slot whose name collides with the
auto-generated memory-tools guide, while there is no first-class home for an agent's
own working-memory index. There is also no guard against the two files that must stay
small (the working-memory index and the user profile) growing without bound, and the
core identity files can be silently overwritten through the generic note tools,
bypassing the dedicated evolution path. This change tightens the core memory model so
the foundational set matches how agents actually bootstrap, makes the small files
stay small, and protects the identity files from accidental generic writes.

## What Changes

- **BREAKING**: Replace `TOOLS.md` with `MEMORY.md` in the five foundational files.
  External-tool facts (camera/SSH/etc.) now belong in `PROMPT.md`. The new
  foundational set is `PERSONA`, `PROMPT`, `RULES`, `USER`, `MEMORY`.
- **BREAKING**: `evolve_core_persona`'s `which` enum drops `tools` and gains `memory`.
- **BREAKING**: `update_task_heartbeat` targets `HEARTBEAT.md` instead of
  `HEARTBEAT-STATE.md`.
- Enforce hard line caps on tool writes: `USER.md` ≤ 100 lines, `MEMORY.md` ≤ 200
  lines. Over-cap writes are rejected with `invalid_argument`.
- **BREAKING**: Inside the agents folder, `write_memory_note` / `edit_memory_note` /
  `delete_memory_note` may only target paths under a subfolder. Root-level core files
  are wrapper-only (foundational files via `evolve_core_persona`, `HEARTBEAT.md` via
  `update_task_heartbeat`). Reads of root files remain allowed.
- Revise `append_diary_entry` output: write a `# YYYY-MM-DD` H1 when the diary file is
  created, and accept an optional `title` so each entry heading reads
  `## HH:MM:SS — title` (falling back to `## HH:MM:SS` when no title is given).
- Move the memory-organization conventions (subfolder layout, diary-via-tool,
  heartbeat usage, `agents/<subagent>/` layout, external-tool-info → `PROMPT.md`, and
  the documented line caps) into the **compiled-in default session-context template**
  so operators can override them by supplying their own template. The auto-generated
  `{{tools_guide}}` reverts to describing only the live tool catalogue.
- Update the default template section order to
  `PERSONA → RULES → MEMORY → USER → PROMPT → {{tools_guide}}`.

Out of scope: prompt-prefix caching (a harness/client responsibility — the server only
guarantees deterministic render) and an auto-attach concrete session-context resource
(a separate single-tenant change).

## Capabilities

### New Capabilities
<!-- None: this revises existing behavior rather than introducing a new capability. -->

### Modified Capabilities
- `memory-tools`: foundational set swaps `TOOLS.md` for `MEMORY.md`; `evolve_core_persona`
  `which` enum changes and enforces line caps for `user`/`memory`; `update_task_heartbeat`
  retargets to `HEARTBEAT.md`; `append_diary_entry` gains a day H1 and optional `title`;
  generic write/edit/delete are restricted to subfolders inside the agents folder; the
  session-context template placeholder set, default-template section order, and embedded
  conventions change. (`configuration` is unaffected — it governs env vars and paths
  only; the default-template content lives in `memory-tools`.)

## Impact

- Code: `src/session_context.rs` (FOUNDATIONAL set, DEFAULT_TEMPLATE with embedded
  conventions, `tools_guide` left as the tool catalogue), `src/tools.rs`
  (`evolve_core_persona` enum + caps, `update_task_heartbeat` target,
  `append_diary_entry` format + `title` arg, root-write restriction in
  write/edit/delete).
- Specs/tests: `openspec/specs/memory-tools` and the corresponding unit tests in
  `session_context.rs` and `tools.rs`.
- Consumers: vaults using `TOOLS.md` or `HEARTBEAT-STATE.md`, and any client that wrote
  core files via `write_memory_note`, must migrate (rename files; route core writes
  through the wrappers).
