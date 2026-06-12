## Context

`read_memory_note` (`src/tools.rs:602`) performs: policy gate by region →
suffix resolution → visibility check → read → own-suffix link strip. Agents
that just listed or recalled a set of paths repeat that call serially; each MCP
round trip costs network latency (HTTP transport) plus per-call protocol and
schema tokens. The per-path logic is already a self-contained sequence, so a
batch tool is composition, not new semantics.

## Goals / Non-Goals

**Goals:**
- Fetch a known set of notes in one call with per-path success/failure.
- Byte-identical per-note semantics to the single read (same errors, same link
  stripping).

**Non-Goals:**
- Glob/prefix expansion inside the tool (compose with `list_memory_notes` /
  `recall_memory_notes`, which already filter; the batch takes explicit paths).
- Unbounded batches or whole-vault dumps (the cap exists to keep one response
  inside sane context/token bounds).
- Batch variants of write/edit/delete (mutation batching has transactional
  questions this change deliberately avoids).

## Decisions

- **A new `read_memory_notes` tool rather than overloading `read_memory_note`.**
  Overloading would make `path` optional-but-required-unless-`paths`, an
  awkward schema for agents; a plural tool keeps both schemas honest. The name
  follows the existing list/read naming line.
- **Cap = 20 paths.** Reading is content-heavy; 20 full notes is already a
  large response for a model context. Larger sweeps paginate naturally by
  issuing further calls. Empty arrays and >20 arrays are `invalid_argument`.
- **Per-path result envelope, request order preserved.** Each entry is
  `{ path, content }` or `{ path, error: { code, message } }` with `path`
  echoing the requested string verbatim, so callers can zip results to
  requests positionally or by value. Duplicates are processed independently
  (no implicit dedup — the response shape stays positionally predictable).
- **Partial failure is success.** The tool result is a success whenever the
  arguments were well-formed; per-path domain failures (`not_found`,
  `path_not_permitted`, `write_denied`-class policy codes) ride inside the
  entries using the same `code` strings the single read would surface. This
  is the only practical batch contract: all-or-nothing would make one stale
  path poison a 20-note fetch.
- **Implementation = extract-and-reuse.** The single-read handler's body moves
  into a private `read_one(scope, vpath) -> Result<String, AgentmemError>`
  used by both tools, so the two can never drift.

## Risks / Trade-offs

- [Large aggregate responses (20 × big notes)] → Bounded by the cap; agents
  control batch composition and can split. No server-side truncation magic —
  predictability over cleverness.
- [Error-in-success envelope means clients must check entries] → Documented in
  the tool description and result schema; this mirrors how every bulk API
  behaves and is what MCP structured content is for.
