## Why

`load_session_context` today returns a raw machine-shaped dump of five foundational files and nothing else — it does not tell the agent *how* to use the memory tools, and it is only reachable as a model-pulled tool. There is no client- or user-triggered way to inject a session bootstrap, and a brand-new vault (no foundational files yet) yields an empty, uninstructive result. Operators also have no way to shape what the bootstrap says.

## What Changes

- Introduce a **rendered session-context** produced by a single shared renderer that weaves the five foundational files together with operator-authored prose and an auto-generated memory-tools guide.
- Introduce the **session-context layout**: an operator-authored markdown document containing `{{files.*}}`, `{{scope.*}}`, and `{{tools_guide}}` placeholders. Named "layout" (not "template") to avoid collision with the existing `AGENTMEM_VFS_TEMPLATE` / `Template`, which shapes *paths*.
- **Layered layout resolution** (graceful, never errors): per-scope layout under the agents folder → global layout file → compiled-in default. Any layer may be absent.
- New env var **`AGENTMEM_SESSION_CONTEXT_FILE`** for the global layout path, defaulting to `<root>/AGENT_SESSION_CONTEXT.md`. The per-scope file is `AGENT_SESSION_CONTEXT.md` resolved through the scope suffix mechanism.
- Expose the rendered session-context through **three surfaces sharing one renderer**:
  - the existing **`load_session_context` tool** (kept) — **BREAKING**: it now returns `{ rendered, missing }` instead of the five raw fields `persona/prompt/rules/user/tools`.
  - a new **resource template** `agentmem://session-context/{…}` whose URI params are derived from the VFS template placeholders (client auto-attach).
  - a new **prompt** `session-context` whose arguments are the VFS scope keys (user slash-command).
- Server now advertises **resources and prompts capabilities** in addition to tools.
- Missing foundational files render a sentinel (e.g. `(not yet recorded — set via evolve_core_persona)`) rather than being omitted; missing layout file falls through to the next layer. A fresh vault renders instructions-only and never errors.

## Capabilities

### New Capabilities
<!-- None — this extends existing capabilities. -->

### Modified Capabilities
- `memory-tools`: the `load_session_context` return contract changes from five raw fields to rendered prose + `missing`; new requirements for the session-context renderer, the session-context layout, sentinel rendering of missing files, and layered layout resolution with a compiled-in default.
- `mcp-server`: the server additionally advertises resources and prompts capabilities and implements `resources/templates/list` + `resources/read` for `agentmem://session-context/{…}` and `prompts/list` + `prompts/get` for `session-context`; scope params/arguments derive from the VFS template placeholders.
- `configuration`: new `AGENTMEM_SESSION_CONTEXT_FILE` variable (default `<root>/AGENT_SESSION_CONTEXT.md`) for the global session-context layout path.

## Impact

- **Code**: `src/tools.rs` (renderer, tool return shape, `FOUNDATIONAL` reuse), `src/mcp.rs` (capabilities + resource/prompt endpoints), `src/config.rs` (new env var + CLI override), and a new module for the session-context layout (parse/render of `{{…}}` placeholders, layered resolution).
- **Specs**: deltas to `memory-tools`, `mcp-server`, `configuration`.
- **Tests/snapshots**: `tests/tools.rs` and the schema snapshots referencing `load_session_context` need updating for the new return shape; new coverage for resource/prompt surfaces and layout resolution.
- **Docs**: `README.md` (env var table, new surfaces).
- **Compatibility**: BREAKING for any client relying on the tool's five raw fields; the scope contract (VFS placeholders) is reused unchanged across all three surfaces.
