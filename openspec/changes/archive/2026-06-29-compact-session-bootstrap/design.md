## Context

The `bootstrap` render is injected at SessionStart, where hooks reliably retain only the first ~2 KB. Today it inlines `PERSONA.md` and `RULES.md` (both tagged) under a `# Session Context` heading shared with the full render. `RULES.md` is uncapped, so a verbose rules file can push the load-bearing content (the scope keys) out of budget. The full `context` render and the layout render are healthy and out of scope here.

## Goals / Non-Goals

**Goals:**
- A compact bootstrap that leads with server-owned essentials (scope, pointers) and bounds its single user-content slot.
- Keep the bootstrap and the full `context` render visibly distinct surfaces.

**Non-Goals:**
- Imposing a server-defined memory loop — recall/capture/diary discipline is left to the user's own `RULES.md` (inlined in the bootstrap) and `PROMPT.md` (delivered by `load_session_context`).
- Changing the full `context` render, the layout render, or any HTTP/MCP/resource surface shape.
- Introducing a `verbosity` parameter or a new computed placeholder.
- Capping `PERSONA.md` or `PROMPT.md` (only `RULES.md` joins the cap family here).

## Decisions

**Drop persona from the bootstrap; keep rules.** Safety rules have no other always-in-context home — no agentmem tool description states git/shell safety boundaries — and `RULES.md` is also where the user expresses their own working discipline, so inlining it keeps that present at SessionStart. Persona is identity/voice, not safety-critical, and is the more deferrable of the two. The full `context` render still inlines persona, and `load_session_context` re-delivers it on demand. *Alternative — remove rules too (pure-pointer bootstrap):* rejected because it re-introduces the "trust the agent to load context before acting" bet that inlining rules was meant to avoid, with no fallback for safety boundaries.

**Leave the memory loop to the user.** The bootstrap imposes no server-defined recall/capture/diary loop; that discipline lives in the user's own `RULES.md` — which the bootstrap inlines — and `PROMPT.md`, delivered on demand by `load_session_context`. *Alternative — bake a server-owned loop overview into the bootstrap and teach it across the tool descriptions:* rejected so the memory loop stays the user's to define rather than a server default.

**Rules last, untagged.** Server-owned essentials occupy the budget-protected head; rules — the only slot that can vary in size — is the truncatable tail. With persona gone, rules is the only inlined foundational file, so the `<RULES>` wrapper (a collision-avoidance delimiter for back-to-back tagged sections) is unnecessary; it renders bare, consistent with the deliberately-bare scope banner.

**Cap `RULES.md` ≤ 40 lines, enforced on `evolve_core_persona` writes.** 40 (vs. 100) guarantees rules fits whole within the ~2 KB budget rather than merely bounding it; it joins the existing `USER.md` ≤ 100 / `MEMORY.md` ≤ 200 cap family using the same validation path. The cap doubles as a forcing function keeping `RULES.md` a tight list of real safety boundaries. *Alternative — ≤ 100:* bounds size but a 100-line rules file (~4 KB) can still partially truncate; we chose the stronger guarantee.

**Rename the bootstrap heading to `# Session Bootstrap`.** The lean and full renders are different documents serving different roles; distinct titles remove the ambiguity of two surfaces sharing `# Session Context`. The change is contained to the bootstrap requirement — the `context` template requirement keeps `# Session Context`.

## Risks / Trade-offs

- **[BREAKING: persona no longer in the default bootstrap]** → The full `context` render still inlines it and the bootstrap explicitly points the agent to `load_session_context` for persona; operators who need persona at SessionStart can author a per-scope/global bootstrap template.
- **[BREAKING: `evolve_core_persona` rejects `rules` > 40 lines]** → Write-time only; an existing oversized `RULES.md` keeps rendering until its next write. Migration is to trim `RULES.md` to ≤ 40 lines. This repo's own `RULES.md` sits near the boundary and may need trimming — documented as expected.
- **[Rules can still clip if a harness budget is below the compact head + 40 lines]** → Acceptable: the server-owned head is protected by ordering, and `load_session_context` re-delivers the full rules on demand.
- **[A user who never authors recall/diary discipline gets none]** → By design — the server no longer imposes a loop. Users who want one express it in `RULES.md` (inlined) or `PROMPT.md`; the cap keeps that affordable.
