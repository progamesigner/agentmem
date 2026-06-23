## Why

The single most operationally load-bearing instruction in the rendered
session-context — *which scope keys every memory tool call must carry* (e.g.
`agent=default, user=swag`) — currently appears only inside the
`<AGENTMEM:TOOLS>` block, the 7th of 8 sections. In a representative render it
lands at byte ~4500 of an ~11.7 KB document.

That position is past two real boundaries:

- **Truncation.** A `SessionStart` hook observed in practice ingests the rendered
  context but surfaces only the first ~2 KB inline (persisting the rest to a
  file the model may never read). The scope directive at byte ~4500 is more than
  2× past that cutoff — a harness consuming only the preview never sees the scope
  keys at all.
- **Recency/primacy.** Even when the full document is present, the directive is
  buried mid-sentence and immediately followed by a long bulleted tool list that
  visually swallows it.

Get the scope keys wrong and every tool call errors or reads/writes the wrong
vault. The most critical instruction should not be the one most reliably lost.

## What Changes

- Add a server-generated `{{scope_directive}}` placeholder to the session-context
  renderer: a prominent one-line imperative that names the concrete active scope
  as `key=value` pairs (e.g. `agent=default, user=swag`) in deterministic key
  order, robust across **all** schemes — for an empty scope it falls back to
  generic phrasing that names no specific key, mirroring `{{tools_guide}}`.
- Lead the compiled-in `DEFAULT_TEMPLATE` with `{{scope_directive}}` as a **bare
  banner directly under the `# Session Context` H1, above `<PERSONA>`** — outside
  any XML tag, so neither byte-budget truncation nor tag-stripping can drop it.
  This places the scope keys in the first ~200 bytes of the document.
- Refactor the `key=value` scope-join (currently inline in `tools_guide`) into a
  shared helper so `{{scope_directive}}` and `{{tools_guide}}` derive the keys
  string from one source of truth with identical formatting and ordering.
- Keep the existing scope mention inside `{{tools_guide}}` unchanged (defense in
  depth at the point of use). Section order, layered template resolution, the
  missing sentinel, the wrapper-only-roots rule, and the line caps are all
  unchanged.

## Capabilities

### New Capabilities
<!-- None: this extends an existing capability. -->

### Modified Capabilities
- `memory-tools`: the session-context renderer additionally produces a
  `{{scope_directive}}` value naming the active scope keys (or a generic
  fallback for an empty scope), and the compiled-in default template leads with
  that directive as a bare banner above `<PERSONA>`. The `{{tools_guide}}` scope
  mention is retained. (`configuration` is unaffected — it governs env vars and
  paths only.)

## Impact

- Code: `src/session_context.rs` — a new shared scope-keys helper, a
  `scope_directive` builder, insertion of `{{scope_directive}}` into the renderer
  context map, the `DEFAULT_TEMPLATE` constant (new leading banner), and the unit
  tests asserting renderer output and default-template content.
- Specs: `openspec/specs/memory-tools` — the *Session-context renderer* and
  *Session-context template* requirements and their scenarios.
- Consumers: operators who supply their own template are unaffected (the banner
  lives only in the compiled-in default), but they may adopt `{{scope_directive}}`
  in their own templates. Downstream parsers keying on the default render's exact
  byte layout should expect the new leading banner before `<PERSONA>`.
