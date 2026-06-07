## 1. Foundational set: swap TOOLS.md → MEMORY.md

- [x] 1.1 In `src/session_context.rs`, change the `FOUNDATIONAL` array: remove `("tools", "TOOLS.md")`, add `("memory", "MEMORY.md")`.
- [x] 1.2 Update the compiled-in `DEFAULT_TEMPLATE` section order to `PERSONA → RULES → MEMORY → USER → PROMPT → {{tools_guide}}`; replace the `## Tools / {{files.tools}}` slot with `## Memory / {{files.memory}}`.
- [x] 1.3 Update unit tests in `session_context.rs` that reference `TOOLS.md`/`{{files.tools}}` to use `MEMORY.md`/`{{files.memory}}`, including the missing-list assertions.

## 2. Conventions in the default template; tools_guide reduced

- [x] 2.1 Add a **suggested-layout** block to `DEFAULT_TEMPLATE` (illustrative, not enforced), listing each entry with its purpose: root files `MEMORY.md` (working-memory index), `RULES.md` (safety boundaries), `PERSONA.md` (identity/soul/style), `PROMPT.md` (workflow rules + external-tool facts like camera/SSH), `USER.md` (user profile), `HEARTBEAT.md`; subfolders `diary/<YYYY-MM-DD>.md`, `workspaces/INDEX.md` + `workspaces/<project>/<item>.md`, `topics/INDEX.md` + `topics/LOG.md` + `topics/<topic>/<fact>.md`, `skills/<skill>/SKILL.md` + `skills/<skill>/references/<name>.md`, `agents/<subagent>/PROMPT.md` + `agents/<subagent>/<context>.md`. Note paths are shown virtually (the `<agent>.<user>` suffix is applied by the server). State that diary is written via `append_diary_entry`/read via `read_memory_note`, heartbeat via `update_task_heartbeat`, core root files via `evolve_core_persona`; document the `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 caps; and explicitly leave `MEMORY.md` internal organization to the agent/user (no prescribed skeleton).
- [x] 2.2 Confirm `tools_guide()` emits only the live tool catalogue (no conventions); adjust if any convention prose currently lives there.
- [x] 2.3 Add/adjust a test asserting the default template documents the conventions and the cap limits.

## 3. evolve_core_persona: which enum + line caps

- [x] 3.1 In `src/tools.rs`, change the `evolve_core_persona` `which` enum: drop `tools`, add `memory` (→ `MEMORY.md`).
- [x] 3.2 Enforce line caps before write: reject `which=user` content > 100 lines and `which=memory` content > 200 lines with `invalid_argument`, message stating the limit; file left unchanged.
- [x] 3.3 Update the tool's input-schema enum and description text.
- [x] 3.4 Replace the "Tools update" test with "Memory update"; add over-cap rejection tests for `user` (100) and `memory` (200) and within-cap success tests.

## 4. update_task_heartbeat: rename target

- [x] 4.1 In `src/tools.rs:486`, change the hardcoded target from `HEARTBEAT-STATE.md` to `HEARTBEAT.md`; update the tool description.
- [x] 4.2 Update heartbeat tests to assert the `HEARTBEAT.md` target.

## 5. append_diary_entry: day H1 + optional title

- [x] 5.1 Add an optional `title` string to the tool input schema.
- [x] 5.2 On file creation, write `# <YYYY-MM-DD>\n\n` as the first lines; build the entry heading as `## <HH:MM:SS> — <title>` when `title` is present, else `## <HH:MM:SS>`.
- [x] 5.3 Preserve append-to-existing and per-target advisory-lock serialisation semantics.
- [x] 5.4 Update diary tests: new-file H1, with-title heading, without-title heading, concurrent appends.

## 6. Wrapper-only root files (restrict generic writes to subfolders)

- [x] 6.1 Add a helper that detects an agents-folder root-level virtual path (no subfolder segment beneath the per-scope root, after suffix resolution; must not mis-classify subfolder files like `diary/2026-01-01.md`).
- [x] 6.2 In `write_memory_note`, `edit_memory_note`, and `delete_memory_note`, reject root-level agents-folder targets with `path_not_permitted` and a message naming the correct wrapper (`evolve_core_persona` / `update_task_heartbeat`); leave the file unchanged. Reads stay allowed; outside-agents-folder behavior unchanged.
- [x] 6.3 Add tests: rejected root write/edit/delete (e.g. `MEMORY.md`, `USER.md`, `PERSONA.md`), allowed subfolder write/edit/delete, and that outside-region policy behavior is unaffected.

## 7. Validation

- [x] 7.1 Run `openspec validate revise-memory-model-and-conventions --strict` (or repo equivalent) and resolve any issues.
- [x] 7.2 Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`; all green before commit.
- [x] 7.3 Update `README.md` (Tools table, foundational-files / session-context sections, worked layouts) to reflect `MEMORY.md`, `HEARTBEAT.md`, the caps, and the wrapper-only-roots rule.
