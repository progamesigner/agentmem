## Context

`render_session_context` (`src/session_context.rs`) builds a context map from the five foundational files (`files.*`), the scope values (`scope.*`), and a server-generated tools guide (`tools_guide`), then renders the active template. The tools guide is produced by `tools_guide(tools: &[Tool])`, whose intro reads:

> These memory tools are available. Every call must carry the scope keys defined by the server's VFS scheme.

The scope values exist in the context map as `{{scope.<key>}}`, but the default template references none of them, and the tools guide — the one place that *demands* the scope keys — never names them. The agent therefore can't tell from the rendered output which keys/values to pass.

Hardcoding `{{scope.agent}}`/`{{scope.user}}` into the default template was rejected: scope keys are scheme-dependent, so any scheme lacking those exact keys would leak literal `{{scope.user}}` tokens and emit an "unknown placeholder" warning.

## Goals / Non-Goals

**Goals:**
- The rendered tools guide names the concrete active scope keys/values, next to the instruction that requires them.
- Scheme-agnostic: works for any scheme's placeholder set without leaking literals.
- No change to the template, placeholder grammar, or operator-facing config.

**Non-Goals:**
- Adding a scope-identity line at the top of the session context.
- Introducing a new `{{scope_summary}}`-style placeholder.
- Changing how operator-supplied templates resolve or render.

## Decisions

- **Inject the scope into `tools_guide()`.** Change the signature to `tools_guide(tools: &[Tool], scope: &BTreeMap<String, String>)` and pass `scope` at the single call site. The identity rides the existing `{{tools_guide}}` slot, so operator templates inherit it for free.
- **Derive keys from the scope map.** Build `key=value` pairs by iterating the `BTreeMap`, which yields deterministic, sorted key order. This naturally lists exactly the configured scheme's keys.
- **Empty-scope fallback.** When the scope map is empty, keep the original generic sentence ("...the scope keys defined by the server's VFS scheme") so nothing degrades for schemes/contexts without scope keys.

## Risks / Trade-offs

- **Spec/wording coupling:** the exact intro sentence is now asserted by a unit test; future wording tweaks must update both. Accepted — the behavior (naming the keys) is the contract, and the test pins it.
- **Redundancy with `{{scope.*}}`:** the same values are available via `{{scope.<key>}}` placeholders. Acceptable: the tools guide is the actionable point of use, and the placeholders remain for templates that want them elsewhere.
