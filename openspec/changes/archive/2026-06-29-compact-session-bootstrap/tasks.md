## 1. Rewrite the default bootstrap template

- [x] 1.1 Rewrite `DEFAULT_BOOTSTRAP` in `src/session_context.rs`: `# Session Bootstrap` heading; bare `{{scope_directive}}`; a single-line pointer to `load_session_context` (persona, working memory, user profile, workflow prompt) and the layout surface; `{{onboarding_directive}}`; then `{{files.rules}}` untagged as the final content
- [x] 1.2 Confirm the bootstrap template omits `{{files.persona}}`/`{{files.memory}}`/`{{files.user}}`/`{{files.prompt}}`, the `<PERSONA>`/`<RULES>` tags, the tools guide, the layout prose, and any server-defined memory-loop/recall/diary directive; leave `DEFAULT_CONTEXT` (full render) and `DEFAULT_LAYOUT` unchanged

## 2. Enforce the RULES cap

- [x] 2.1 In `src/tools.rs` `evolve_core_persona`, add `which=rules` content ≤ 40 lines to the line-cap validation, alongside the existing USER ≤ 100 / MEMORY ≤ 200 caps, reusing the same newline-count path and rejecting with `invalid_argument` naming the 40-line limit (single and batch forms)

## 3. Fix stale tool description

- [x] 3.1 Fix the `load_session_context` description in `src/tools.rs`: remove the stale "memory-tools guide" clause and name what it actually returns (the five foundational files woven into the configured template, plus the layout pointer)

## 4. Tests

- [x] 4.1 Update `bootstrap_render_is_lean` in `src/session_context.rs`: assert `# Session Bootstrap` heading, no `<PERSONA>`/`<RULES>` tags, no persona contents, no server-defined memory-loop directive, the `load_session_context`/layout pointer present, and rules rendered last and untagged
- [x] 4.2 Update `default_templates_lead_with_bare_scope_banner` to branch by kind — bootstrap leads with `# Session Bootstrap` (no `<PERSONA>` landmark), context still leads with `# Session Context` and `<PERSONA>`
- [x] 4.3 Add a rules-cap test for `evolve_core_persona`: a 40-line `rules` write succeeds, a 41-line write is rejected with `invalid_argument` and leaves `RULES.md` unchanged; update the batch over-cap test to use a 41-line `rules` entry
- [x] 4.4 Confirm `onboarding_directive_gated_on_missing` still passes for the bootstrap kind

## 5. Docs & verification

- [x] 5.1 Update `docs/session-context-hooks.md`: note the compacted bootstrap layout (server-owned essentials first, rules last) and the `RULES.md` ≤ 40-line cap with its migration note
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; manually render the default bootstrap and confirm it leads with scope + loop + pointers and that a ≤ 40-line `RULES.md` clears the SessionStart inline budget
