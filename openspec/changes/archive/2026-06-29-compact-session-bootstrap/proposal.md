## Why

The lean `bootstrap` render still inlines both `PERSONA.md` and `RULES.md` under a `# Session Context` heading shared with the full render. SessionStart hooks reliably keep only the first ~2 KB of the injected bootstrap, so the most load-bearing content (the scope keys) competes for budget with persona prose and an unbounded rules file. We want a compact, deterministically-bounded bootstrap that leads with the server-owned essentials, inlines only the user's own rules, and defers everything else.

## What Changes

- **Rewrite the compiled-in default `bootstrap` template** to a compact, server-owned-first layout: a `# Session Bootstrap` heading; the bare `{{scope_directive}}` banner; a single-line pointer to `load_session_context` (now also covering persona) and the layout surface; the `{{onboarding_directive}}` slot; and finally the untagged `{{files.rules}}` at the end. Memory-discipline guidance (recall/capture/diary) is intentionally NOT server-imposed — it stays the user's to express in their own `RULES.md` (which the bootstrap inlines) and `PROMPT.md`.
- **Drop the persona slot from the bootstrap.** **BREAKING**: the default bootstrap no longer inlines `PERSONA.md`; persona is delivered on demand by `load_session_context` (the full `context` render is unchanged and still inlines it).
- **Untag rules in the bootstrap** — `{{files.rules}}` is rendered without the `<RULES>` wrapper, since it is the only inlined foundational file and needs no delimiter.
- **Rename the bootstrap heading** `# Session Context` → `# Session Bootstrap`, so the lean render and the full `context` render (which keeps `# Session Context`) are visibly distinct surfaces.
- **Move rules to the end** of the bootstrap so the server-owned essentials (scope, loop, pointers, onboarding) occupy the budget-protected head and only user rules can be truncated.
- **Add a 40-line cap on `RULES.md`** enforced by `evolve_core_persona`, alongside the existing `USER.md` ≤ 100 and `MEMORY.md` ≤ 200 caps, so the inlined rules are guaranteed to fit the SessionStart budget. **BREAKING**: an `evolve_core_persona` write whose `rules` content exceeds 40 lines is now rejected; an existing oversized `RULES.md` keeps rendering until its next write.
- **Fix the stale `load_session_context` description** — it claims a "memory-tools guide" that was already removed; reword it to name what it actually returns (the five foundational files woven into the configured template, plus the layout pointer).

## Capabilities

### New Capabilities
<!-- none: this extends an existing capability -->

### Modified Capabilities
- `memory-tools`: the compiled-in default `bootstrap` template is rewritten (new `# Session Bootstrap` heading, persona slot dropped, rules untagged and moved last, pointer reworded to cover persona); `evolve_core_persona` gains a `RULES.md` ≤ 40-line cap joining the existing USER/MEMORY caps. The full `context` render and the layout render are unchanged.

## Impact

- Code: `src/session_context.rs` (`DEFAULT_BOOTSTRAP` rewrite; bootstrap tests), `src/tools.rs` (rules line-cap in `evolve_core_persona`; `load_session_context` description fix; cap test).
- APIs: no surface changes — the `session-bootstrap` resource and `GET /v1/bootstrap` keep their shape; only the rendered content shrinks. The full `context` surfaces are untouched.
- Behavior (**BREAKING**): default bootstrap no longer inlines persona; `evolve_core_persona` rejects `rules` content over 40 lines (write-time only — existing oversized `RULES.md` still renders until re-written; migration is to trim `RULES.md` to ≤ 40 lines).
- Docs: `docs/session-context-hooks.md` notes the compacted bootstrap and the `RULES.md` cap.
- Verification: confirm the rendered bootstrap leads with scope + loop + pointers and that a ≤ 40-line `RULES.md` clears the SessionStart inline budget.
