## Context

`load_session_context` is currently a tool that returns the raw contents of five foundational files (`PERSONA/PROMPT/RULES/USER/TOOLS.md`) plus a `missing` list. It does not instruct the agent on how to use the memory tools, and it is only reachable as a model-pulled tool — there is no client- or user-triggered bootstrap path. The server advertises only `enable_tools()`.

The codebase already has a `Template` type (`src/template.rs`) bound to `AGENTMEM_VFS_TEMPLATE` that shapes scope **paths** (dotted `<agent>.<user>` suffix/segment). This change introduces a second, unrelated concept — a prose document with `{{…}}` placeholders that renders into the session bootstrap. To avoid overloading the word "template," the new concept is named the **session-context layout**.

The scope contract (which keys a caller must supply) is already derived from the VFS template's placeholders and merged into every tool's input schema. This change reuses that same placeholder list to derive the resource-template URI parameters and the prompt arguments, so scope is defined in exactly one place.

## Goals / Non-Goals

**Goals:**
- One shared renderer feeding three surfaces: the kept `load_session_context` tool, a `session-context` resource template, and a `session-context` prompt.
- Operator-configurable layout via a layered lookup (per-scope → global file → compiled-in default), with a sensible default that works on a fresh vault.
- Graceful degradation: missing foundational files render a sentinel; a missing layout falls through to the next layer; nothing errors.
- Keep the scope contract derived from the VFS template, applied uniformly to all three surfaces.

**Non-Goals:**
- A full templating language. No loops, no conditionals — placeholder substitution only.
- Renaming or changing the existing `AGENTMEM_VFS_TEMPLATE` / `Template` (path) concept.
- Per-surface divergent content — all three surfaces render the same string for a given scope.
- Caching/invalidation of layout files (read per request; cheap).

## Decisions

### D1: Keep the tool, add resource template + prompt — one renderer
A single function `render_session_context(scope) -> { rendered, missing }` is the source of truth. The tool returns `{ rendered, missing }`; `resources/read` returns `rendered` as resource contents; `prompts/get` returns `rendered` as a message. Keeping the tool preserves the only model-pullable path (an agent can re-pull its context mid-session); the resource template serves client auto-attach; the prompt serves user slash-commands.
- *Alternative considered:* drop the tool entirely. Rejected — the model would lose any self-service refresh path.

### D2: Name the new concept "layout," not "template"
"Template" is already taken by the path-shaping `AGENTMEM_VFS_TEMPLATE`. The new document is named the **session-context layout**; the verb is "render the layout." (Runner-up names: *scaffold*, *briefing*.)

### D3: Layered layout resolution
Resolve order, first hit wins: (1) per-scope `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism inside the agents folder; (2) global file at `AGENTMEM_SESSION_CONTEXT_FILE` (default `<root>/AGENT_SESSION_CONTEXT.md`); (3) compiled-in default. Any layer may be absent without error.
- *Alternative considered:* single global file only. Rejected — per-scope override lets a `coder` agent and a `researcher` agent get different bootstraps, which the layered design supports at low cost.

### D4: Namespaced placeholders
`{{files.<name>}}` (persona|prompt|rules|user|tools), `{{scope.<key>}}` (any VFS placeholder), `{{tools_guide}}`. Namespacing avoids the `{{user}}` ambiguity (USER.md contents vs. the `user` scope key). Unknown `{{…}}` tokens are left literal and logged once — consistent with the graceful-degradation theme, since layouts are read at request time, not validated at startup.

### D5: `{{tools_guide}}` is server-generated
The guide is built from the live tool catalogue (names + one-line usage) so it never drifts from the tools actually advertised. The layout controls *where* the guide appears; the server controls *what* it says.

### D6: Missing foundational file → sentinel substitution
A `{{files.*}}` whose file is absent substitutes a fixed sentinel (e.g. `(not yet recorded — set via evolve_core_persona)`) and is recorded in `missing`. No section suppression — the layout stays a dumb string, and a fresh vault renders instructions-only.

### D7: Reuse VFS placeholders for URI params and prompt args
`resources/templates/list` emits `agentmem://session-context/{k1}/{k2}/…` where `k*` are the VFS placeholders in order; `prompts/list` declares those same keys as required string arguments. Scope parsing for `resources/read` extracts the path segments back into the scope map.

## Risks / Trade-offs

- **BREAKING tool contract change** (five raw fields → `{ rendered, missing }`) → Mitigation: documented as BREAKING in the proposal; update `tests/tools.rs` and schema snapshots in the same change; the `missing` field is retained for machine consumers.
- **Resource scoping via URI segments** can be awkward for clients that don't support resource templates → Mitigation: the prompt and tool surfaces remain available with explicit args.
- **Layouts read per request** add file I/O per call → Mitigation: the reads are small and local; if profiling shows cost, add a cache later (out of scope).
- **Placeholder drift** if an operator references a renamed scope key → Mitigation: unknown tokens are left literal and logged, not fatal; the default layout only uses guaranteed placeholders.
- **rmcp API surface** for resource templates and prompts must be confirmed against the pinned `rmcp` version → Mitigation: verify `ServerHandler` hooks (`list_resource_templates`/`read_resource`/`list_prompts`/`get_prompt`) before wiring.

## Open Questions

- Exact wording of the compiled-in default layout and the missing sentinel string (finalise during implementation).
- Whether to expose a no-op/disable switch for the resource or prompt surface (deferred unless requested).
