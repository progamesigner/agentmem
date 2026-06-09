## Context

The two-region permission model (`src/policy.rs`) classifies a virtual path as
inside or outside the agents folder purely by whether its leading segment is the
configured agents-folder name (`PathResolver::detect_region`, `src/path.rs:160`).
Virtual paths are vault-root-relative.

The wrapper tools (`append_diary_entry`, `evolve_core_persona`,
`update_task_heartbeat`) build their target through `agents_vpath()`
(`src/tools.rs:310`), which prepends the agents-folder name. The generic note
tools (`write_memory_note`, `edit_memory_note`, `delete_memory_note`,
`read_memory_note`) take the caller's `path` verbatim (`src/tools.rs:433` etc.).

The `<AGENTMEM:LAYOUT>` prose in the compiled-in `DEFAULT_TEMPLATE`
(`src/session_context.rs`) lists subfolder conventions as bare paths
(`topics/INDEX.md`, `diary/<YYYY-MM-DD>.md`, …). An agent that copies a bare path
into a generic write lands outside the agents folder, where the default
`namespaced` policy is read-only — surfacing as "read-only region under the
active policy" and inviting a misdiagnosis that the *policy* is the problem. The
`memory-tools` spec carries the same ambiguity (e.g. the write scenario's example
`topics/auth/jwt.md`).

## Goals / Non-Goals

**Goals:**
- Make the rendered `<AGENTMEM:LAYOUT>` prose state the path-addressing rule:
  subfolder paths are relative to the agents folder; wrappers prepend it; generic
  tools need the agents-folder name as the leading segment.
- Include one worked example contrasting a wrapper path with the generic-tool
  equivalent.
- Keep the guidance correct under any `AGENTMEM_AGENTS_DIR` (no hardcoded name).
- Remove the same bare-path ambiguity from the `memory-tools` spec scenarios.

**Non-Goals:**
- No change to path resolution, region detection, or policy gating.
- No new "forgiving" auto-prefix behavior in the generic tools (that was option 2,
  explicitly not chosen).
- No change for operators who supply their own session-context template.

## Decisions

**Decision: clarify in prose rather than auto-prefix the generic tools.**
Option 2 (resolve a bare subfolder path inside the agents folder when no leading
segment matches) was rejected: it blurs the inside/outside boundary the security
model relies on and makes a path's region depend on a fuzzy match rather than a
literal leading segment. A documentation fix is the smallest change that removes
the trap without weakening the model.

**Decision: name the agents-folder name generically, not literally.**
The `DEFAULT_TEMPLATE` is a static compiled-in string, but `AGENTMEM_AGENTS_DIR`
is configurable (default `Agents`). Two viable framings:
- (A) Pure-static prose that refers to "your agents-folder name" and shows the
  worked example with the default `Agents` as an illustration.
- (B) Interpolate the live configured agents-folder name into the prose at render
  time.

Choose **(A)**. It keeps the layout block a constant string (the renderer's
substitution surface stays limited to the documented `{{…}}` placeholders), avoids
adding a new render-time dependency on config, and the rule itself ("prefix with
your agents-folder name") is what the agent needs — the literal name is already
visible to the agent elsewhere in the resolved paths. The spec scenario is written
to require the name-agnostic phrasing so (A) is the conforming implementation.

**Decision: spec example paths become root-relative.**
Update the generic write/edit/delete scenarios to show `Agents/...` example paths
and add a sentence to each requirement stating that `path` is vault-root-relative
and must include the agents-folder segment to target inside it. This makes the
spec self-consistent with the implementation and with the corrected layout prose.

## Risks / Trade-offs

- [Worked example uses `Agents` while an operator configured a different name] →
  The prose states the *rule* ("prefix with your agents-folder name") so the
  example reads as illustrative; the agent also sees the real name in its own
  resolved paths. Acceptable for a static default template.
- [Prose grows longer, costing context budget] → Addition is a few sentences plus
  one example line; negligible against the existing layout block.
- [Existing session-context renderer tests assert on layout text] →
  `src/session_context.rs` tests that match layout substrings may need updating to
  cover the new sentence/example; handled in tasks.

## Migration Plan

No data or API migration. The change is the wording of a compiled-in default and
the spec text. On release, agents reading a freshly rendered context get the
corrected guidance immediately; operators with custom templates are unaffected.

## Open Questions

None.
