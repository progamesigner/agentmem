# Tasks: add-batch-persona-evolution

## 1. Schema

- [x] 1.1 Make `which`/`content` optional in `EvolveFields`, add the optional `updates` array of `{ which, content }` (enum `which`, 1–5 entries), and document the exactly-one-form contract and batch validation in the field docs
- [x] 1.2 Update the `evolve_core_persona` tool description to name both forms and the interview-then-commit flow; refresh `tests/schema_snapshots.rs`

## 2. Handler

- [x] 2.1 Enforce exactly-one-form (`invalid_argument` for neither/both) and parse batch entries, rejecting empty arrays and duplicate `which`
- [x] 2.2 Phase 1: validate every entry — `which` domain, line caps on agent-facing content, link expansion — collecting final contents with no writes
- [x] 2.3 Phase 2: write each selected file atomically via the gated write path; return the legacy single-form response unchanged and `{ results: [{which, bytes_written}] }` for the batch form

## 3. Guidance text

- [x] 3.1 Extend the compiled-in session-context guide in `src/session_context.rs`: when foundational files are missing, interview the user first (identity, role, working style, boundaries), distill answers into the agent's own concise wording — written for future agent sessions, not a verbatim transcript — and commit all affected files in one batch call
- [x] 3.2 Update `tests/session_context.rs` to assert the rendered guide carries the interview/batch/distillation instructions

## 4. Tests

- [x] 4.1 Batch happy path: persona+user+memory in one call, files land atomically, `results` in request order
- [x] 4.2 All-or-nothing: an over-cap `user` entry rejects the batch leaving every file unchanged; duplicate `which` rejected; neither/both forms rejected
- [x] 4.3 Back-compat: single-form calls produce byte-identical responses to today; readonly policy refuses both forms
- [x] 4.4 Link-transform parity: a batch `memory` entry containing `[[rust]]` persists suffixed and renders clean via `load_session_context`

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`; fix anything they surface
