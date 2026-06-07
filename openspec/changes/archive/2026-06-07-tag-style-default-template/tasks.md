## 1. Re-delimit DEFAULT_TEMPLATE with tags

- [x] 1.1 In `src/session_context.rs`, edit the `DEFAULT_TEMPLATE` constant: keep the `# Session Context` H1 title, then replace each `## <Heading>` + `{{files.*}}` slot with a tag-wrapped slot — `<PERSONA>\n{{files.persona}}\n</PERSONA>`, `<RULES>\n{{files.rules}}\n</RULES>`, `<MEMORY>\n{{files.memory}}\n</MEMORY>`, `<USER>\n{{files.user}}\n</USER>`, `<PROMPT>\n{{files.prompt}}\n</PROMPT>` — preserving the order PERSONA → RULES → MEMORY → USER → PROMPT.
- [x] 1.2 Wrap the tools guide as `<AGENTMEM:TOOLS>\n{{tools_guide}}\n</AGENTMEM:TOOLS>` (replacing the `## Memory Tools` heading).
- [x] 1.3 Wrap the layout block as `<AGENTMEM:LAYOUT>…</AGENTMEM:LAYOUT>` (replacing the `## Memory Layout (suggested)` heading), keeping the entry list, tool-managed-files notes, and the documented `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 caps.

## 2. Reframe the layout prose (drop the suffix detail)

- [x] 2.1 Remove the sentence describing the per-scope `<agent>.<user>` suffix from the `<AGENTMEM:LAYOUT>` prose (and any other suffix/"subagent name is a directory segment" wording).
- [x] 2.2 Add framing that the core files (`MEMORY.md`, `RULES.md`, `PERSONA.md`, `PROMPT.md`, `USER.md`, `HEARTBEAT.md`) are special — changed only through their dedicated wrapper tools and bounded by the line caps — while all other paths behave like an ordinary filesystem the agent reads, writes, and organizes freely.
- [x] 2.3 Sweep the surrounding doc comments in `src/session_context.rs` for any remaining agent-facing mention of the suffix mechanism that would surface in rendered output (internal `//!`/`///` comments describing resolution may stay).

## 3. Update tests

- [x] 3.1 Update `default_template_documents_conventions_and_caps` to assert the tag delimiters (e.g. `<PERSONA>`, `<AGENTMEM:TOOLS>`, `<AGENTMEM:LAYOUT>`) instead of `## Memory Layout (suggested)`, while keeping the existing assertions for `HEARTBEAT.md`, `diary/<YYYY-MM-DD>.md`, `agents/<subagent>/PROMPT.md`, the tool names, and the cap strings.
- [x] 3.2 Add an assertion that the rendered default template does NOT contain the per-scope suffix wording (e.g. does not contain `<agent>.<user>` suffix prose) and DOES contain the wrapper-vs-filesystem framing.
- [x] 3.3 Update `layered_resolution_prefers_per_scope_then_global_then_default` and any other test still matching the old `## Persona`/`## Memory Tools` headings to match the new tag delimiters; keep the `# Session Context` H1 assertion.

## 4. Validation

- [x] 4.1 Run `cargo fmt`, `cargo clippy`, and `cargo test` locally and resolve any failures.
- [x] 4.2 Run `openspec validate tag-style-default-template --strict` and resolve any issues.
