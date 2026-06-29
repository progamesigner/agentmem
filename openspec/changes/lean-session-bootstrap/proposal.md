## Why

The rendered session-context bootstrap is ~12 KB, and roughly half of it — the server-generated tools guide (`{{tools_guide}}`) and the embedded `<AGENTMEM:LAYOUT>` prose — is either redundant with the MCP tool schemas the harness already injects, or static vault-mechanics guidance that every scope renders identically. SessionStart hooks that pull this over `GET /v1/context` only reliably keep the first ~2 KB in the model's context; the tail is silently dropped. We want a small, guaranteed-in-budget bootstrap for session start, while still offering the full context and the layout guidance on demand.

## What Changes

- Add a **lean "bootstrap" render** — scope banner + `PERSONA` + `RULES` + a missing-files onboarding directive + pointers to the full context and the layout — exposed as `GET /v1/bootstrap` and the `agentmem://session-bootstrap/{…}` resource. This becomes the recommended SessionStart hook target.
- Add a dedicated **layout render** carrying the vault-mechanics guidance, exposed as `GET /v1/layout` and the `agentmem://session-layout/{…}` resource, resolved through a layered template (`AGENT_MEMORY_LAYOUT[.scope].md` → env → compiled default).
- **Remove the `{{tools_guide}}` slot and its generator** from the renderer entirely. The MCP layer already ships tool schemas; the one non-schema fact (carry the scope keys) already lives in `{{scope_directive}}`. **BREAKING**: an operator template still referencing `{{tools_guide}}` now renders the literal token.
- **Move the `<AGENTMEM:LAYOUT>` prose out of the session-context render** into the new layout surface. **BREAKING**: the full render (`/v1/context`, `load_session_context`, `session-context` resource/prompt) no longer embeds layout inline; consumers read the layout surface instead. The full render gains a one-line pointer to it.
- Add a computed `{{onboarding_directive}}` placeholder — the "foundational files are missing, interview the user and call `evolve_core_persona`" guidance lifted out of the layout prose — rendered as the empty string when no foundational files are missing, so steady-state sessions pay nothing for it.
- Add env vars `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` and `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` (defaults `<root>/AGENT_SESSION_BOOTSTRAP.md` and `<root>/AGENT_MEMORY_LAYOUT.md`).
- `GET /v1/context`, `load_session_context`, and the `session-context` resource/prompt keep their surfaces and verbosity unchanged — only their rendered *content* shrinks (tools guide and layout removed, onboarding directive and layout pointer added). No `verbosity` query parameter is introduced.

## Capabilities

### New Capabilities
<!-- none: this extends existing capabilities -->

### Modified Capabilities
- `memory-tools`: the shared session-context renderer drops `{{tools_guide}}`, gains a render *kind* (full context vs lean bootstrap) selecting between two compiled-in default templates, gains a computed `{{onboarding_directive}}` placeholder gated on missing foundational files, and gains a separate layout renderer; the `load_session_context` tool's rendered content changes accordingly.
- `context-http-api`: add `GET /v1/bootstrap` (lean) and `GET /v1/layout` alongside the unchanged `GET /v1/context`, sharing its scope binding, response negotiation, auth gate, and error mapping.
- `mcp-server`: advertise and serve two new resources, `session-bootstrap` (`agentmem://session-bootstrap/{…}`) and `session-layout` (`agentmem://session-layout/{…}`), under the existing scoped-token gate.
- `configuration`: add `AGENTMEM_SESSION_BOOTSTRAP_TEMPLATE_FILE` and `AGENTMEM_MEMORY_LAYOUT_TEMPLATE_FILE` to the recognised environment and the layered template resolution.

## Impact

- Code: `src/session_context.rs` (renderer kind, default templates, layout renderer, `{{onboarding_directive}}`, drop `tools_guide`/`tools` param), `src/template.rs` (unchanged — substitution only), `src/transport/http.rs` (two additive routes), `src/mcp.rs` (two resources; `read_resource`/`list_resource_templates` branch by URI prefix), `src/tools.rs` (caller signature), `src/config` (two env vars).
- APIs: additive HTTP routes and MCP resources; no removals. `{{tools_guide}}` placeholder removed (operator-template breaking). Full-render content shrinks for all existing full surfaces.
- Docs: `docs/session-context-hooks.md` repoints the recommended SessionStart hook to `/v1/bootstrap` and documents the layout surface.
- Verification: confirm the lean bootstrap payload (~scope + PERSONA + RULES) clears the SessionStart inline budget in practice.
