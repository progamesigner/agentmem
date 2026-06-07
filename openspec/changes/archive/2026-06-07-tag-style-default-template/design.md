## Context

`render_session_context` (in `src/session_context.rs`) splices each foundational
file's body into the active template via `{{files.*}}` placeholders. The compiled-in
`DEFAULT_TEMPLATE` currently frames each slot with an `##` heading (`## Persona`,
`## Rules`, …). Foundational files are operator/agent-authored markdown that, by the
project's own conventions, open at H2. Splicing an H2-rooted body beneath a template
H2 produces a flat, ambiguous outline: a reader (human or model) cannot tell the
server's framing apart from the agent's recorded content.

The `## Memory Layout (suggested)` block additionally documents that "the server
applies the per-scope `<agent>.<user>` suffix to leaf filenames automatically." That
suffix is an internal storage detail of the path resolver; surfacing it in the
agent-facing bootstrap invites agents to reason about (or hard-code) a mechanism that
is meant to be transparent.

The template engine (`crate::template::Template`) only substitutes `{{…}}` tokens; it
treats all other text — including XML-style tags — as literal passthrough. So changing
the delimiters is a pure content edit to one string constant plus its tests; no engine
change is required.

## Goals / Non-Goals

**Goals:**
- Delimit default-template sections with tags that cannot collide with embedded
  markdown headings, regardless of what heading level a foundational file uses.
- Keep a clean visual/structural separation between agent-owned content and
  server-generated content.
- Stop exposing the per-scope filename-suffix mechanism in agent-facing prose; reframe
  the layout around "wrapper-managed core files vs. ordinary filesystem."

**Non-Goals:**
- No change to layered template resolution, the missing sentinel, the placeholder
  namespace, the wrapper-only-roots rule, or the line caps.
- No change to operator-supplied templates or the `{{tools_guide}}` content.
- No change to section ordering.

## Decisions

**1. XML-style tags instead of headings.** Use `<PERSONA>…</PERSONA>` etc. rather than
`##` headings. Tags occupy a delimiter namespace disjoint from markdown headings, so an
embedded body may freely use H1–H6 without colliding with the template frame. The
template engine passes tags through untouched.
- *Alternative — deeper headings (H4/H5 in the template, require H1 in files):*
  rejected. It only pushes the collision down a level and forces a heading-level
  contract on every foundational file; agents author free-form markdown.
- *Alternative — fenced code blocks per section:* rejected. It would render the
  foundational markdown as literal/monospace rather than as prose.

**2. Bare tags for agent content, `AGENTMEM:`-namespaced tags for server content.**
Foundational slots use bare tags (`<PERSONA>`, `<RULES>`, `<MEMORY>`, `<USER>`,
`<PROMPT>`); the server-generated sections use `<AGENTMEM:TOOLS>` and
`<AGENTMEM:LAYOUT>`. The namespace prefix signals "this is server framing, not your
recorded content," which is exactly the line the old H2 headings blurred.

**3. Keep the outer `# Session Context` H1 title.** A single document title does not
collide with embedded H2 bodies and remains a useful anchor; existing tests assert it.
Only the per-section `##` delimiters change.

**4. Reframe the layout prose; drop the suffix sentence.** Replace the "the server
applies the per-scope `<agent>.<user>` suffix…" sentence with framing that a small set
of **core files** (`MEMORY.md`, `RULES.md`, `PERSONA.md`, `PROMPT.md`, `USER.md`,
`HEARTBEAT.md`) are special — changed only through their dedicated wrapper tools and
bounded by the line caps — while every other path behaves like an ordinary filesystem
the agent reads, writes, and organizes freely. This tells the agent the one thing it
needs (which paths are restricted and why) without leaking the resolver's internals.

## Risks / Trade-offs

- **Downstream parsers keyed on the old `## Persona` headings break.** → This is the
  default template only; the change is called out in the proposal's Impact, and any
  operator who needs the old shape can supply their own template (layered resolution is
  unchanged). The repo's own tests are updated in lockstep.
- **Tags inside an embedded foundational body could confuse a naive reader** (e.g. a
  `USER.md` that literally contains `</USER>`). → Acceptable: the same ambiguity
  existed with `##` headings, the rendered output is advisory text for a model rather
  than a parsed format, and the `AGENTMEM:` namespace keeps the server's own tags
  distinct from anything an agent is likely to write.

## Migration Plan

Single-commit content edit to `DEFAULT_TEMPLATE` plus its unit tests in
`src/session_context.rs`. No data migration, no config change, no API surface change.
Rollback is reverting the commit.

## Open Questions

None.
