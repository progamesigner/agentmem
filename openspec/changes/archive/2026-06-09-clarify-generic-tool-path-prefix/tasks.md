## 1. Layout prose

- [x] 1.1 In `src/session_context.rs`, extend the `<AGENTMEM:LAYOUT>` block of `DEFAULT_TEMPLATE` with a short addressing note: subfolder paths are relative to the agents folder, the wrapper tools (`append_diary_entry`, `evolve_core_persona`, `update_task_heartbeat`) prepend the agents-folder name, and the generic note tools (`write_memory_note`, `edit_memory_note`, `delete_memory_note`, `read_memory_note`) require the agents-folder name as the leading segment of a vault-root-relative path.
- [x] 1.2 Add one worked example contrasting a wrapper-built path with the equivalent generic-tool path, phrased name-agnostically (refer to "your agents-folder name"; the illustrative example may use the default `Agents`).

## 2. Spec scenarios

- [x] 2.1 In `openspec/specs/memory-tools/spec.md`, update the `write_memory_note`, `edit_memory_note`, and `delete_memory_note` requirements and scenarios to state `path` is vault-root-relative and use unambiguous root-relative example paths (e.g. `Agents/topics/auth/jwt.md`, `Agents/MEMORY.md`). (Done at archive time via the change delta.)

## 3. Tests

- [x] 3.1 Update/add `src/session_context.rs` renderer tests so they assert the new addressing sentence and worked example appear in the rendered `<AGENTMEM:LAYOUT>` output.
- [x] 3.2 Confirm no existing layout-substring assertions break; adjust any that do.

## 4. Verify

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; all pass.
- [x] 4.2 Run `openspec validate clarify-generic-tool-path-prefix --strict` and resolve any findings.
