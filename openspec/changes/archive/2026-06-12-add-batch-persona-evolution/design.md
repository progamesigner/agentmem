# Design: add-batch-persona-evolution

## Context

`evolve_core_persona` (`src/tools.rs`) writes one of PERSONA/PROMPT/RULES/USER/MEMORY per call, with line caps on USER (100) and MEMORY (200) and the write-side link transform applied before the gated atomic write. The session-context renderer (`src/session_context.rs`) substitutes a missing-file sentinel (`(not yet recorded — set via evolve_core_persona)`) and emits a compiled-in tools guide; this guide is the only channel through which the server can shape how a fresh agent conducts persona bootstrap.

## Goals / Non-Goals

**Goals:**
- One call can replace any subset of the five foundational files, validated as a unit.
- Existing single-form callers keep working with byte-identical responses.
- The rendered guidance produces interview-first, distill-then-commit behavior.

**Non-Goals:**
- MCP elicitation (server-initiated questions to the client) — the interview stays the calling agent's conversation; the server only shapes it through guidance text.
- Changing the caps, the five-file set, or the wrapper-only rule for core files.
- Cross-file transactionality against crashes.

## Decisions

1. **One tool, two argument forms, exactly-one enforced in the handler.** `which`/`content` become optional in the schema and `updates` is added as an optional array; the handler rejects neither/both. Alternative — a separate `evolve_core_personas` tool — was rejected: it doubles the guidance surface and the tool list for what is one operation. The merged scope-field schema (with `additionalProperties: false`) cannot express the exclusive-or, so the handler owns it; the schema docs state it.
2. **Validate everything, then write.** All entries pass `which` parsing, duplicate detection, line caps, and link expansion before the first write — same two-phase stance as `rename_memory_note` and the batch write tool. Caps are evaluated on agent-facing content (expansion never changes line counts), preserving the existing rule.
3. **Duplicate `which` is rejected rather than last-wins.** Five-entry batches are small enough that a duplicate is always an authoring bug; silently dropping an earlier persona draft would destroy user input.
4. **Guidance lands in two places: the tool description and the compiled-in session-context guide.** The guide already nudges via the missing-file sentinel; it gains explicit instructions — ask the questions needed for identity/role/style/boundaries first; write once, batched. The distillation rule is phrased as the deliverable being *the agent's own working language*: concise, structured for machine re-reading at session start, never a transcript of the user's raw answers. Rationale: these files are read by future agent sessions, not by the user, and verbatim transcription both wastes the line caps and embeds ambiguity the agent already resolved during the interview.
5. **Batch response shape mirrors the batch write tool** (`results: [{which, bytes_written}]`), keeping batch result conventions uniform across the surface.

## Risks / Trade-offs

- **[Crash mid-batch leaves some files updated]** → each file write is atomic and the set is re-runnable (full-file replaces are idempotent); documented, consistent with rename and batch write.
- **[Guidance text is advisory — agents may still transcribe verbatim]** → the spec scenario pins the instruction's presence, not agent compliance; wording is the lever we own.
- **[Two argument forms complicate the schema]** → both forms documented in the field docs; exactly-one validation gives a crisp `invalid_argument` rather than a confusing schema error.

## Open Questions

(none)
