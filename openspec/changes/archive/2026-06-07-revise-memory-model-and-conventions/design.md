## Context

AgentMem fronts a markdown vault as multi-tenant agent memory. Each scope has five
**foundational files** woven into a rendered session-context bootstrap, plus convenience
wrappers (`evolve_core_persona`, `update_task_heartbeat`, `append_diary_entry`) and the
generic note tools (`write/edit/delete/read/list_memory_note`).

Three problems motivate this change:

1. `TOOLS.md` (a foundational file for external-tool facts) is easily confused with the
   auto-generated `{{tools_guide}}` (which describes the memory tools). The two are
   unrelated, and the foundational set lacks a first-class working-memory index.
2. The working-memory index and user profile are meant to stay small, but nothing
   enforces that.
3. Core identity files can be overwritten by the generic `write_memory_note`, bypassing
   `evolve_core_persona` — and any cap placed only on the wrapper.

Constraints: the server is multi-tenant (scope arrives per call); enforcement is only
possible on tool writes (a human editing the vault in Obsidian bypasses any server
check); the suffix scheme is server-wide and applies only to the leaf filename.

## Goals / Non-Goals

**Goals:**
- Foundational set = `PERSONA`, `PROMPT`, `RULES`, `USER`, `MEMORY`; `TOOLS.md` removed.
- Hard, unbypassable line caps: `USER.md` ≤ 100, `MEMORY.md` ≤ 200.
- Protect all root core files: writable only through the dedicated wrappers.
- `HEARTBEAT.md` (renamed from `HEARTBEAT-STATE.md`); richer diary format.
- Move memory-organization conventions into the overridable default template; keep
  `{{tools_guide}}` as the live tool catalogue only.

**Non-Goals:**
- Prompt-prefix caching — a harness/client responsibility. The server only guarantees
  deterministic render (same scope ⇒ identical bytes), which it already satisfies.
- An auto-attach concrete session-context resource (a separate single-tenant change).
- Migrating existing vaults (rename `TOOLS.md`/`HEARTBEAT-STATE.md`) — operator task.
- Enforcing caps or layout on human Obsidian edits (out of the server's reach).

## Decisions

**1. Make caps unbypassable via the wrapper-only-roots rule, not path interception.**
Rather than special-casing `MEMORY.md`/`USER.md` inside `write_memory_note`, we forbid
*all* generic writes to agents-folder root-level paths. Root files become reachable for
writes only through `evolve_core_persona` / `update_task_heartbeat`, so the line check
lives in exactly one place (`evolve_core_persona`) and cannot be routed around.
- *Alternative considered:* per-file cap checks inside `write_memory_note`. Rejected —
  leaves multiple write paths to keep in sync and still allows generic overwrite of
  identity files.
- *Rule:* inside the agents folder, a target whose path has no subfolder segment beneath
  the per-scope root is "root-level" and is rejected for `write/edit/delete_memory_note`.
  Reads (`read_memory_note`, `list_memory_note`) are unaffected. The "outside agents
  folder" region keeps its existing policy behavior.

**2. Reuse `path_not_permitted` for rejected root writes, with a guiding message.**
The rejection carries `path_not_permitted` and a message naming the correct wrapper
(`evolve_core_persona` for foundational files, `update_task_heartbeat` for the heartbeat).
- *Alternative considered:* a new dedicated error code (e.g. `root_path_reserved`).
  Deferred to keep the error surface stable; the message disambiguates for the agent. Can
  be promoted later if telemetry shows agents need the distinct code.

**3. Line cap = newline-separated line count, checked before any write.**
Lines are cheap to count and match how the limit is communicated ("≤ 100 lines"). A byte
cap was considered earlier and dropped per decision; a single pathological long line is
an accepted edge case. Over-cap → `invalid_argument` naming the limit; file untouched.

**4. Conventions live in the default template, not `{{tools_guide}}`.**
`{{tools_guide}}` is auto-generated and not operator-overridable; baking conventions there
would force them on every operator. Putting them in the compiled-in default template lets
an operator's own template replace or drop them. `{{tools_guide}}` reverts to describing
only the live tool catalogue.

**5. Diary gains a day H1 and an optional `title`.**
New files start with `# <YYYY-MM-DD>`; each entry heading is `## <HH:MM:SS> — <title>`,
or `## <HH:MM:SS>` when `title` is omitted. Backward-compatible append semantics
otherwise (read-modify-write under the per-target advisory lock).

**6. Default template section order: PERSONA → RULES → MEMORY → USER → PROMPT → tools_guide.**
Identity and rules first, then the working-memory index and user profile, then the
operational prompt, then the tool catalogue.

**7. The default template presents a *suggested* layout, not an enforced one.**
The template lists the full recommended tree with per-file roles (`MEMORY` index,
`RULES` boundaries, `PERSONA` soul/identity/style, `PROMPT` workflow + external-tool
facts, `USER` profile, `HEARTBEAT`, and the `diary/ workspaces/ topics/ skills/ agents/`
subfolders incl. their `INDEX`/`LOG`/`SKILL` files) as guidance only. The server enforces
exactly two things — the wrapper-only-roots rule and the line caps; everything else in the
layout is advisory so agents/operators can deviate. In particular `MEMORY.md`'s internal
structure is deliberately left to the agent/user (no prescribed skeleton). Paths are shown
in virtual form; the `<agent>.<user>` suffix is applied by the server, and a subagent name
is a directory segment (the suffix does not nest).
- *Alternative considered:* prescribing a `MEMORY.md` skeleton. Rejected for now — keeps
  the index format flexible; can be a separate follow-up if a default shape proves useful.

## Risks / Trade-offs

- **Breaking change for existing vaults** (`TOOLS.md`, `HEARTBEAT-STATE.md`, core files
  written via `write_memory_note`) → Documented in proposal Impact; pre-`0.1.0` status
  means no compatibility shim. Operators rename files and route core writes through
  wrappers.
- **Wrapper-only roots forbid ad-hoc root files** → Intentional; the agents-folder root is
  now a controlled surface. New free-form notes go in subfolders.
- **Caps only bind tool writes** → A human can still create an over-long `MEMORY.md` in
  Obsidian; the renderer still reads it. Accepted — server cannot police human edits.
- **`path_not_permitted` now covers two cases** (hidden/ignored vs. reserved root) → The
  message disambiguates; a dedicated code remains a future option.
- **"Root-level" detection must be precise** → Must key off the path having no subfolder
  segment beneath the per-scope root (after suffix resolution), and must not mis-classify
  subfolder files (e.g. `diary/2026-01-01.md` is allowed). Covered by spec scenarios.
