## Context

The rendered session-context is assembled by `render_session_context` in
`src/session_context.rs` from a fixed-order `DEFAULT_TEMPLATE` (when no operator
template overrides it). The scope keys an agent must carry on every tool call are
emitted only by `tools_guide`, which builds the `<AGENTMEM:TOOLS>` block near the
end of the document. The keys string is constructed inline there:

```rust
let keys = scope.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ");
```

`scope` is a `BTreeMap`, so key order is already deterministic (sorted by key).

## Goals

- Surface the active scope keys within the first few hundred bytes of the
  rendered document, outside any XML tag, so it survives both truncation and
  tag-stripping.
- Keep one source of truth for the `key=value` formatting and ordering.
- Remain correct for any scheme, including the empty scope.
- Change nothing an operator-supplied template already depends on.

## Decisions

### Bare banner above `<PERSONA>` (Option A)

The directive renders as bare markdown directly under the `# Session Context`
H1, before the first tag:

```
# Session Context

{{scope_directive}}

<PERSONA>
...
```

Rejected alternatives:

- *A dedicated `<SCOPE>` tag block.* A harness that strips or ignores unknown
  XML tags would lose it — the exact failure mode we are fixing. Bare prose is
  immune.
- *Bolding it in place inside `<AGENTMEM:TOOLS>`.* Still at byte ~4500; still
  truncated. Position is the primary problem, framing only secondary.

### Server-generated `{{scope_directive}}`, not literal `{{scope.*}}`

The template already exposes `{{scope.agent}}` / `{{scope.user}}`, so a banner
could in principle be hand-written. But the compiled-in default must work for
*any* scheme: a scheme without an `agent` key would leave a literal
`{{scope.agent}}` token in the output. A server-generated placeholder adapts to
the active scheme and degrades gracefully to generic phrasing for an empty
scope, exactly as `{{tools_guide}}` already does.

### Shared scope-keys helper

Extract the `key=value` join into a single helper (e.g.
`scope_keys_csv(scope) -> Option<String>`, `None` for an empty scope). Both
`scope_directive` and `tools_guide` call it, so the two places that name the
scope can never drift in formatting or ordering.

### Directive content

- Non-empty scope: a prominent one-liner naming the keys, e.g.
  `> **Active memory scope — \`agent=default, user=swag\`.** Every AgentMem memory
  tool call MUST carry exactly these scope arguments on every turn — otherwise it
  errors or reads/writes the wrong vault.`
- Empty scope: the same imperative without naming any key, e.g.
  `> **Active memory scope.** Every AgentMem memory tool call MUST carry the scope
  keys defined by the server's VFS scheme on every turn — otherwise it errors or
  reads/writes the wrong vault.`

A blockquote (`>`) makes the line visually distinct without introducing a heading
that could collide with embedded foundational-file markdown.

### Keep the `{{tools_guide}}` mention

The in-`<AGENTMEM:TOOLS>` scope sentence stays. The top banner beats truncation;
the point-of-use mention beats "read the tool list but skipped the preamble."
Duplication is intentional defense in depth, and the shared helper keeps both
consistent.

## Risks / Trade-offs

- *Redundancy* — two scope mentions. Accepted: they serve different failure
  modes and share a formatting source.
- *Default-render byte layout changes* — a leading banner shifts every
  subsequent byte offset. Only downstream parsers keying on exact offsets of the
  default render are affected; operator templates and tag-based parsers are not.
