## Why

The compiled-in default session-context template delimits its sections with `##`
(H2) headings, but the foundational files it embeds (`PERSONA.md`, `RULES.md`,
`MEMORY.md`, `USER.md`, `PROMPT.md`) are themselves authored as markdown that
typically opens at H2. When a foundational file's body is spliced under a template
H2, the two heading levels collide: the document's outline becomes ambiguous and an
agent cannot reliably tell where the server's framing ends and its own recorded
content begins. The layout prose also leaks an internal storage detail — the
per-scope `<agent>.<user>` suffix — that agents should never reason about.

## What Changes

- Re-delimit every section of the compiled-in `DEFAULT_TEMPLATE` with XML-style tags
  instead of `##` headings, so embedded foundational markdown (which starts at H2)
  no longer collides with the template's own structure. The sections become
  `<PERSONA>{{files.persona}}</PERSONA>`, `<RULES>{{files.rules}}</RULES>`,
  `<MEMORY>{{files.memory}}</MEMORY>`, `<USER>{{files.user}}</USER>`,
  `<PROMPT>{{files.prompt}}</PROMPT>`, `<AGENTMEM:TOOLS>{{tools_guide}}</AGENTMEM:TOOLS>`,
  and `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>`. Foundational (agent-owned) slots use bare
  tags; server-generated sections use the `AGENTMEM:` namespace prefix. Section order
  is unchanged: PERSONA → RULES → MEMORY → USER → PROMPT → TOOLS → LAYOUT.
- Remove the per-scope `<agent>.<user>` suffix explanation from the `<AGENTMEM:LAYOUT>`
  prose. Suffixing is an internal, transparent mechanism agents must not depend on.
  Replace it with framing that a small set of **core files** have special handling
  (wrapper-only edits via the dedicated tools, plus the line caps), while everything
  else behaves like an ordinary filesystem the agent reads, writes, and organizes
  freely.
- Keep all enforced behavior identical: layered template resolution, the missing
  sentinel, the wrapper-only-roots rule, and the `USER.md` ≤ 100 / `MEMORY.md` ≤ 200
  line caps. This change is presentational and content-only within the default
  template.

## Capabilities

### New Capabilities
<!-- None: this revises the presentation of an existing capability. -->

### Modified Capabilities
- `memory-tools`: the compiled-in default session-context template wraps its sections
  in XML-style tags (`<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`, `<PROMPT>`,
  `<AGENTMEM:TOOLS>`, `<AGENTMEM:LAYOUT>`) rather than `##` headings, and its embedded
  layout prose no longer mentions the per-scope suffix — instead distinguishing
  wrapper-managed core files from free-form filesystem paths. Section order, layered
  resolution, the missing sentinel, and the documented caps are unchanged.
  (`configuration` is unaffected — it governs env vars and paths only.)

## Impact

- Code: `src/session_context.rs` — the `DEFAULT_TEMPLATE` constant (tag delimiters and
  revised `<AGENTMEM:LAYOUT>` prose) and the unit tests asserting template content
  (`default_template_documents_conventions_and_caps`,
  `layered_resolution_prefers_per_scope_then_global_then_default`, and any test
  matching on the old `## …` headings).
- Specs: `openspec/specs/memory-tools` — the *Session-context template* requirement and
  its scenarios describing the default template's structure.
- Consumers: operators relying on the old H2 section headings in the default template
  output (e.g. downstream parsers keying on `## Persona`) must update to the tag
  delimiters. Operators who supply their own template are unaffected.
