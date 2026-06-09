## Why

The `<AGENTMEM:LAYOUT>` guide lists subfolder conventions as bare paths
(`topics/INDEX.md`, `workspaces/<project>/<item>.md`, `diary/<YYYY-MM-DD>.md`),
but the generic note tools (`write_memory_note`, `edit_memory_note`,
`delete_memory_note`) take the `path` argument verbatim as a **vault-root-relative**
path. Only the wrapper tools prepend the agents-folder name. An agent that copies
a bare subfolder path into a generic write therefore lands *outside* the agents
folder, where the default `namespaced` policy is read-only, and gets a misleading
"read-only region under the active policy" rejection — diagnosing it as a policy
restriction rather than a missing path prefix.

## What Changes

- Clarify in the `<AGENTMEM:LAYOUT>` prose that the subfolder paths shown are
  **relative to the agents folder**, and that the generic note tools require the
  agents-folder name as the leading segment (the wrapper tools add it for you).
- Add a worked example contrasting a wrapper-built path with the equivalent
  generic-tool path, using an agents-folder-name placeholder so it stays correct
  for any configured `AGENTMEM_AGENTS_DIR`.
- Align the `memory-tools` spec's write/edit/delete scenarios so their example
  paths are unambiguously root-relative (i.e. include the agents-folder segment),
  removing the same bare-path ambiguity from the spec itself.
- No behavior change to path resolution, region detection, or policy gating.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the requirement governing the `<AGENTMEM:LAYOUT>` prose gains a
  path-addressing clarification (subfolder paths are relative to the agents
  folder; generic note tools need the agents-folder prefix), and the generic
  write/edit/delete scenarios use unambiguous root-relative example paths.

## Impact

- `src/session_context.rs` — the compiled-in `DEFAULT_TEMPLATE`'s
  `<AGENTMEM:LAYOUT>` block.
- `openspec/specs/memory-tools/spec.md` — wording of the layout-prose requirement
  and the generic-tool scenarios.
- Documentation/guidance only; no change to tool semantics, the policy model, or
  path resolution. Operators who supply a custom session-context template are
  unaffected (they already control this prose).
