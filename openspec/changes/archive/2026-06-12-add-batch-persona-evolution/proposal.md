# Batch persona evolution with an interview-then-commit flow

## Why

Bootstrapping or reshaping an agent's identity touches several foundational files at once (PERSONA, PROMPT, RULES, USER, MEMORY), but `evolve_core_persona` writes exactly one file per call — so agents write after every user answer, producing five round-trips and half-finished personas when a session is cut short. The bootstrap guidance also says nothing about *how* to gather the material, so agents tend to transcribe raw user answers instead of distilling them.

## What Changes

- `evolve_core_persona` additionally accepts an `updates` array of `{ which, content }` entries (1–5, duplicate `which` rejected), validating every entry — including the line caps and link expansion — before writing any file, then writing each selected file atomically.
- The existing single `which`/`content` form keeps working unchanged (existing callers and already-rendered guides depend on it); exactly one of the two forms must be supplied per call.
- Batch-form result: `{ results: [{ which, bytes_written }] }` in request order; single-form result unchanged.
- The tool description and the rendered session-context guidance describe an interview-then-commit flow: when foundational files are missing, ask the user as many questions as needed first, then distill the answers into the agent's own concise wording — written for fast comprehension by future agent sessions, not a verbatim transcript of the user's words — and commit all affected files in one batch call.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: the `evolve_core_persona` tool requirement gains the batch `updates` form with all-or-nothing validation; a new requirement covers the persona interview guidance in the rendered session-context.

## Impact

- `src/tools.rs`: `EvolveFields` schema (optional `which`/`content` + optional `updates`, exactly-one-form enforced in the handler), `evolve_core_persona` handler, tool description.
- `src/session_context.rs`: the compiled-in guide text gains the interview-then-commit and distillation guidance.
- `tests/tools.rs`, `tests/session_context.rs`, `tests/schema_snapshots.rs`.
