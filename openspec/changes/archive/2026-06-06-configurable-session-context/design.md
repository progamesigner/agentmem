## Context

`load_session_context` is currently a tool that returns the raw contents of five foundational files (`PERSONA/PROMPT/RULES/USER/TOOLS.md`) plus a `missing` list. It does not instruct the agent on how to use the memory tools, and it is only reachable as a model-pulled tool — there is no client- or user-triggered bootstrap path. The server advertises only `enable_tools()`.

The path-shaping concept (dotted `<agent>.<user>` suffix/segment) is the **scheme** — `Scheme` in `src/scheme.rs`, bound to `AGENTMEM_VFS_SCHEME` (renamed from "VFS template" in the archived `rename-vfs-template-to-scheme` change). That rename freed the word "template" and the `src/template.rs` slot for this feature: the natural name for a prose document with `{{…}}` fill-in slots.

The scope contract (which keys a caller must supply) is already derived from the scheme's placeholders and merged into every tool's input schema. This change reuses that same placeholder list to derive the resource URI parameters and the prompt arguments, so scope is defined in exactly one place.

## Goals / Non-Goals

**Goals:**
- One shared renderer feeding three surfaces: the kept `load_session_context` tool, a `session-context` resource, and a `session-context` prompt.
- Operator-configurable template via a layered lookup (per-scope → global file → compiled-in default), with a sensible default that works on a fresh vault.
- Graceful degradation: missing foundational files render a sentinel; a missing template falls through to the next layer; nothing errors.
- Keep the scope contract derived from the scheme, applied uniformly to all three surfaces.

**Non-Goals:**
- A full templating language. No loops, no conditionals — placeholder substitution only.
- Per-surface divergent content — all three surfaces render the same string for a given scope.
- Caching/invalidation of template files (read per request; cheap).

## Decisions

### D1: Keep the tool, add resource + prompt — one renderer
A single function `render_session_context(scope) -> { rendered, missing }` is the source of truth. The tool returns `{ rendered, missing }`; `resources/read` returns `rendered` as resource contents; `prompts/get` returns `rendered` as a message. Keeping the tool preserves the only model-pullable path (an agent can re-pull its context mid-session); the resource serves client auto-attach; the prompt serves user slash-commands.
- *Alternative considered:* drop the tool entirely. Rejected — the model would lose any self-service refresh path.

### D2: Name the document the "template"
With the path concept now named the **scheme**, "template" is free and is the obvious word for a fill-in-the-slots document. The operator-authored document is the **session-context template**; the verb is "render the template". (This inverts the earlier defensive choice to call it a "layout" — that motivation disappeared with the scheme rename.)

### D3: Asymmetric naming — "template" is the document, "resource" is the URI surface
MCP has its own term, *resource template* (an RFC-6570 templated URI), which would collide with our document's name. To keep the two legible, we apply asymmetric naming: in our prose the operator-authored document is always the **template**, and the MCP URI surface is always the **`session-context` resource** — never a "resource template". Only the unavoidable MCP method name `resources/templates/list` carries the word, described as registering "a templated URI".
- *Alternative considered:* rename the document (e.g. *briefing*) to dodge the clash entirely. Rejected — "template" is the right word for the thing operators actually author; the clash is confined to wire-level prose and is resolved by discipline.

### D4: Promote a generic `Template` type in `src/template.rs`
The `{{key}}` substitution is a small, self-contained mini-language; it lives as a generic `Template` type (`parse` → segments, `render(context: &Map<String,String>)` → string), the lax sibling of the strict `Scheme`. `Template` knows nothing about files, scope, or "missing"; it only substitutes recognised keys and leaves unknown `{{…}}` tokens literal (reporting them so the caller can log once). The session-context renderer (`src/session_context.rs`) is the orchestrator: it resolves the template source, builds the context map, and calls `Template::render`.
- *Alternative considered:* keep substitution as a private helper inside `session_context.rs`. Rejected — promoting it mirrors `Scheme`/`scheme.rs`, makes the two configurable-string mini-languages a legible pair, and costs little.

### D5: Layered template resolution
Resolve order, first hit wins: (1) per-scope `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism inside the agents folder; (2) global file at `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` (default `<root>/AGENT_SESSION_CONTEXT.md`); (3) compiled-in default. Any layer may be absent without error.
- *Alternative considered:* single global file only. Rejected — per-scope override lets a `coder` agent and a `researcher` agent get different bootstraps, which the layered design supports at low cost.

### D6: Namespaced placeholders
`{{files.<name>}}` (persona|prompt|rules|user|tools), `{{scope.<key>}}` (any scheme placeholder), `{{tools_guide}}`. Namespacing avoids the `{{user}}` ambiguity (USER.md contents vs. the `user` scope key). Unknown `{{…}}` tokens are left literal and logged once — consistent with the graceful-degradation theme, since templates are read at request time, not validated at startup.

### D7: `{{tools_guide}}` is server-generated
The guide is built from the live tool catalogue (names + one-line usage) so it never drifts from the tools actually advertised. The template controls *where* the guide appears; the server controls *what* it says.

### D8: Missing foundational file → sentinel substitution
A `{{files.*}}` whose file is absent substitutes a fixed sentinel (e.g. `(not yet recorded — set via evolve_core_persona)`) and is recorded in `missing`. No section suppression — the template stays a dumb string, and a fresh vault renders instructions-only.

### D9: Reuse the scheme's placeholders for URI params and prompt args
`resources/templates/list` emits `agentmem://session-context/{k1}/{k2}/…` where `k*` are the scheme placeholders in order; `prompts/list` declares those same keys as required string arguments. Scope parsing for `resources/read` extracts the path segments back into the scope map.

## Risks / Trade-offs

- **BREAKING tool contract change** (five raw fields → `{ rendered, missing }`) → Mitigation: documented as BREAKING in the proposal; update `tests/tools.rs` and schema snapshots in the same change; the `missing` field is retained for machine consumers.
- **Resource scoping via URI segments** can be awkward for clients that don't support templated resource URIs → Mitigation: the prompt and tool surfaces remain available with explicit args.
- **Templates read per request** add file I/O per call → Mitigation: the reads are small and local; if profiling shows cost, add a cache later (out of scope).
- **Placeholder drift** if an operator references a renamed scope key → Mitigation: unknown tokens are left literal and logged, not fatal; the default template only uses guaranteed placeholders.
- **"template" overlaps with MCP "resource template"** → Mitigation: the asymmetric naming of D3; bare "template" always means the document.

## rmcp API surface (confirmed)

Verified against the pinned `rmcp 0.9.1`, so the resource/prompt wiring is no longer an open risk:
- `ServerHandler` provides overridable `list_resource_templates`, `read_resource`, `list_prompts`, `get_prompt` (all default to method-not-found / empty).
- `ServerCapabilities::builder().enable_resources().enable_prompts().enable_tools().build()` advertises the capabilities.
- Types: `RawResourceTemplate { uri_template, name, title, description, mime_type }` (→ `ResourceTemplate = Annotated<…>`); `ResourceContents::text(text, uri)`; `ReadResourceResult { contents }`; `Prompt::new(name, description, arguments)`; `PromptArgument { name, description, required, .. }`; `PromptMessage::new_text(role, text)`; `GetPromptResult`; request params `ReadResourceRequestParam { uri }`, `GetPromptRequestParam { name, arguments }`.

## Open Questions

- Exact wording of the compiled-in default template and the missing sentinel string (finalise during implementation).
- Whether to expose a no-op/disable switch for the resource or prompt surface (deferred unless requested).
