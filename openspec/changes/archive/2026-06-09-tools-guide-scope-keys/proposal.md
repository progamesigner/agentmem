## Why

The server-generated memory-tools guide tells the agent that "every call must carry the scope keys defined by the server's VFS scheme" but never names the concrete keys or their values. The agent is left to infer its own scope (e.g. `agent`, `user`) from elsewhere, which is exactly the identity it must pass on every tool call — so the guide's central instruction is unactionable on its own.

## What Changes

- The server-generated tools guide (`{{tools_guide}}`) names the concrete active scope as `key=value` pairs (e.g. `agent=coder, user=alice`) in the sentence that requires them, derived from the live scope map rather than hardcoded.
- The wording is scheme-agnostic: it lists whatever keys the configured scheme defines, in deterministic order, and falls back to the existing generic phrasing when the scope is empty.
- No template or placeholder change: the identity flows through the existing `{{tools_guide}}` slot, so operator-supplied templates inherit it automatically.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the *Session-context renderer* requirement's tools-guide behavior is extended so the generated guide names the concrete active scope keys/values, not only the live tool set.

## Impact

- Code: `src/session_context.rs` — `tools_guide()` gains the scope map; one call site updated.
- Behavior: every rendered session-context (via `load_session_context` tool, `session-context` resource, and `session-context` prompt) now states the active scope in the tools guide.
- No API, dependency, or configuration changes; no breaking changes.
