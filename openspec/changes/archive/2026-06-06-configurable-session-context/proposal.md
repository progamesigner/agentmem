## Why

`load_session_context` today returns a raw machine-shaped dump of five foundational files and nothing else — it does not tell the agent *how* to use the memory tools, and it is only reachable as a model-pulled tool. There is no client- or user-triggered way to inject a session bootstrap, and a brand-new vault (no foundational files yet) yields an empty, uninstructive result. Operators also have no way to shape what the bootstrap says.

## What Changes

- Introduce a **rendered session-context** produced by a single shared renderer that weaves the five foundational files together with operator-authored prose and an auto-generated memory-tools guide.
- Introduce the **session-context template**: an operator-authored markdown document containing `{{files.*}}`, `{{scope.*}}`, and `{{tools_guide}}` placeholders. The word "template" is free for this use now that the path-shaping concept is named the **scheme** (`AGENTMEM_VFS_SCHEME`, see the archived `rename-vfs-template-to-scheme` change).
- Promote a small generic **`Template`** type (`src/template.rs`, the slot left by the scheme rename) that does `{{key}}` substitution over a context map; the session-context feature builds the context and calls it.
- **Layered template resolution** (graceful, never errors): per-scope template under the agents folder → global template file → compiled-in default. Any layer may be absent.
- New env var **`AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE`** for the global template path, defaulting to `<root>/AGENT_SESSION_CONTEXT.md`. The per-scope file is `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism.
- Expose the rendered session-context through **three surfaces sharing one renderer**:
  - the existing **`load_session_context` tool** (kept) — **BREAKING**: it now returns `{ rendered, missing }` instead of the five raw fields `persona/prompt/rules/user/tools`.
  - a new **`session-context` resource** `agentmem://session-context/{…}` (registered via `resources/templates/list` as a templated URI) for client auto-attach.
  - a new **`session-context` prompt** for the user slash-command surface.
- Naming discipline (asymmetric): the prose document is the **template**; the MCP URI surface is the **resource**. We never call the URI surface a "resource template" in our own prose — "template" means the operator-authored document, and only the MCP method name `resources/templates/list` carries the word.
- Server now advertises **resources and prompts capabilities** in addition to tools.
- Missing foundational files render a sentinel (e.g. `(not yet recorded — set via evolve_core_persona)`) rather than being omitted; a missing template file falls through to the next layer. A fresh vault renders instructions-only and never errors.

## Capabilities

### New Capabilities
<!-- None — this extends existing capabilities. -->

### Modified Capabilities
- `memory-tools`: the `load_session_context` return contract changes from five raw fields to rendered prose + `missing`; new requirements for the session-context renderer, the session-context template, sentinel rendering of missing files, and layered template resolution with a compiled-in default.
- `mcp-server`: the server additionally advertises resources and prompts capabilities and implements `resources/templates/list` + `resources/read` for the `session-context` resource `agentmem://session-context/{…}` and `prompts/list` + `prompts/get` for the `session-context` prompt; scope params/arguments derive from the scheme's placeholders.
- `configuration`: new `AGENTMEM_SESSION_CONTEXT_TEMPLATE_FILE` variable (default `<root>/AGENT_SESSION_CONTEXT.md`) for the global session-context template path.

## Impact

- **Code**: a new generic `src/template.rs` (`Template`: parse/render of `{{…}}` placeholders), a new `src/session_context.rs` (the renderer: layered resolution, context assembly, `tools_guide`, `FOUNDATIONAL` reuse), `src/tools.rs` (tool return shape), `src/mcp.rs` (capabilities + resource/prompt endpoints), `src/config.rs` (new env var + CLI override), and module registration in `src/lib.rs`.
- **Specs**: deltas to `memory-tools`, `mcp-server`, `configuration`.
- **Tests/snapshots**: `tests/tools.rs` and the schema snapshots referencing `load_session_context` need updating for the new return shape; new coverage for resource/prompt surfaces and template resolution.
- **Docs**: `README.md` (env var table, new surfaces).
- **Compatibility**: BREAKING for any client relying on the tool's five raw fields; the scope contract (the scheme's placeholders) is reused unchanged across all three surfaces.
