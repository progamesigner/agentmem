## Why

Session bootstrap and recall follow-up both end with a list of paths the agent
then reads one round-trip at a time — over HTTP that is N sequential tool
calls' worth of latency and per-call token overhead for what is logically one
operation: "give me these notes".

## What Changes

- New tool `read_memory_notes` taking `paths`, an array of vault-root-relative
  virtual paths (1 to 20 entries). It returns one entry per requested path, in
  request order: `{ path, content }` on success or
  `{ path, error: { code, message } }` on failure.
- Failures are per-path, not per-call: each path is independently resolved,
  policy-gated, visibility-checked, and link-stripped with semantics identical
  to `read_memory_note`, so one missing or denied note never voids the rest of
  the batch. The call itself errors only for malformed arguments (empty array,
  more than 20 entries, non-string entries) or invalid scope keys.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `memory-tools`: new `read_memory_notes` batch-read tool requirement.

## Impact

- Code: `src/tools.rs` (new schema struct, handler delegating per path to the
  single-read logic, tool registration), `tests/tools.rs`, schema snapshots,
  README tool table.
- Dependencies: none.
