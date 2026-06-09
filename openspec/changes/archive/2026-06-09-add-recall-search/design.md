## Context

The vault is a plain-markdown, Obsidian-compatible store with two regions
(`src/path.rs`, `src/storage.rs`): scoped-and-isolated **inside** the agents folder
(a note an agent addresses as `Agents/topics/rust.md` is physically
`Agents/coder.alice/topics/rust.coder.alice.md` — the rendered scope is both a
directory segment and a file-stem suffix, making cross-scope access structurally
impossible) and **shared** outside it. Visibility today is served by directory
walks (`storage::walk_files` / `list_visible`) that already honor scope, policy,
hidden filtering, and `.gitignore`/`.obsidianignore`/`.ignore`. There is **no
content search and no frontmatter parsing** — markdown is opaque bytes. The HTTP
transport (`src/transport/http.rs`) currently exposes an ungated `GET /health`
liveness route alongside `/mcp` and `/v1/context`.

Requirements settled with the user:

- Scale is **tens of thousands of notes**; recall must rank by content.
- Match facets: **ranked full-text (BM25)**, **regex**, and **Obsidian property
  (frontmatter) filters**, composable; each hit returns **path + score + snippet**.
- The index is **fully in memory, like Obsidian** — nothing written to disk.
- **Per-scope indexes + one shared index** (structural isolation, not a filter).
- **Cold start is acceptable**; the server must be **healthy only after the index is
  ready** — and it is deployed to **Kubernetes with health checks**.
- A recall call against a not-yet-built scope **blocks until that scope is ready**.
- Cross-index results are merged by **per-index score normalization**.
- The search backend is **configurable**, falling back to a **simpler full-text +
  regex** implementation only.

## Goals / Non-Goals

**Goals:**

- An agent finds notes by content at tens-of-thousands scale in one tool call,
  getting ranked `{path, score, snippets}` results.
- Results are *exactly* as visible as `list_memory_notes` — own scope ∪ permitted
  shared region; never another scope, never an ignored/hidden note.
- Scoped content is isolated **structurally** (its own in-RAM index), not by a
  filter that must not be gotten wrong.
- The index reflects on-disk truth: server writes update it synchronously; external
  Obsidian/editor edits are picked up live by a watcher, with a stat-diff backstop.
- Clean Kubernetes semantics: liveness never blocks on the index; readiness gates
  traffic until the index can serve.
- A lean build path exists: the heavy engine is optional, with a simple
  full-text + regex backend as the floor.

**Non-Goals:**

- Semantic / embedding / vector recall (no model runtime). Lexical only.
- Any on-disk index, cache dir, or index file format.
- Cross-scope or global search for operators.
- Indexing binary/attachment files — markdown notes only.
- Reproducing Obsidian's full Dataview query language; property filters are a
  bounded predicate set (tantivy backend only).
- True cross-corpus BM25 across the scope/shared boundary (traded away by the
  two-index choice — see D8).

## Decisions

### D1: Backend is a trait; tantivy (in a RAM directory) is the default impl

Define a `RecallBackend` trait — `build/evict` lifecycle, `apply_write`,
`apply_fs_event`, `query`, `ready` — so the lifecycle machinery (cold-start,
readiness, watcher, eviction, merge) lives *around* the backend, not inside it. Two
implementations:

- **`SimpleBackend`** — an in-RAM content cache scanned per query with
  substring/`regex` matching. **No BM25 ranking, no property filters.** The
  **default** backend and the floor.
- **`TantivyBackend`** — tantivy in a **`RamDirectory`** (nothing on disk). Native
  **BM25** ranking, per-doc **stored/indexed fields** for frontmatter properties,
  built-in **SnippetGenerator**. **Opt-in.**

tantivy is pure Rust (consistent with avoiding C deps; SQLite FTS5 rejected for the
C dependency) and is gated behind an **optional cargo feature** (mirroring the
existing `transport-http` feature), **off by default**. Built without the feature,
the dependency is absent and `SimpleBackend` is the only backend. See D3.

### D2: Per-scope indexes + one shared index — the index *is* the boundary

There is no combined index and no scope/region filter field. Each scope owns an
in-RAM index holding only that scope's notes; one shared-region index holds the
shared notes. A query opens **only**:

```
recall(scope = coder.alice, policy)
   ├──▶ index[coder.alice]            // always
   └──▶ index[shared]                 // iff policy permits shared reads
                                       // (scoped policy → omitted entirely)
```

coder.bob's notes live in a different in-RAM index alice's query never opens —
cross-scope recall is **structurally impossible**, matching the suffix scheme's
ethos. Per-index schema is therefore minimal: `path` (clean, stored), `phys_path`
(stored, for snippet re-read + stat-diff), `body` (BM25), `prop_<key>` (P2),
`mtime`, `size`. No `scope`/`region` fields are needed.

Indexes are seeded from the **same `storage` visibility walk** that backs
`list_memory_notes`, so hidden filtering and `.gitignore`/`.obsidianignore`/`.ignore`
exclusion are inherited and an ignored note never enters any index.

### D3: Configurable backend + simple fallback, with an honest capability gap

Selection via `AGENTMEM_RECALL_BACKEND = simple | tantivy | off` (CLI override per
the existing pattern), **defaulting to `simple`**, composed with the cargo feature:

- default / `simple` selected / feature absent → `SimpleBackend`;
- feature present **and** `tantivy` explicitly selected → `TantivyBackend`;
- `tantivy` selected but the feature is absent or init fails → **fall back to
  `simple`** (logged);
- `off` → the `recall_memory_notes` tool is not registered.

So the rich engine is strictly opt-in: a default build is lean and runs `simple`.

**Capability gap is explicit, never silent.** `SimpleBackend` supports `query`
(substring, case-insensitive) and `regex`, returns `{path, score, snippets}` with a
trivial term-count score (normalized like everything else), but has **no property
filters**. A recall carrying `filters` against `SimpleBackend` is rejected with
`unsupported` ("property filters require the tantivy backend") — an agent must never
believe a filter applied when it did not.

### D4: In-memory lifecycle — cold build, watcher, sync writes, stat-diff backstop, eviction

```
STARTUP (cold build — /readyz stays red throughout)
  1. start the fs-watcher FIRST and queue events   // nothing edited mid-build is lost
  2. enumerate all scopes from the vault + the shared region
  3. eagerly build EVERY scope index and the shared index
  4. drain queued events onto each; mark ALL ready → /readyz flips green (D5)
STEADY STATE
  server's own write       → update the owning index synchronously (in-process)
  external edit (Obsidian)  → watcher event → reverse-map path→scope/shared (path.rs)
                            → update that index; idempotent via the mtime/size manifest
  periodic reconcile        → stat-only walk diff vs manifest (BACKSTOP for missed
                              watcher events: buffer overflow, network FS)
```

- **Watcher** uses the `notify` crate, **debounced** (editors save via
  write-temp-then-rename, firing several events), re-applying the same ignore filter
  as the walk, reverse-mapping each physical path to its scope/shared index via
  `path.rs` run backwards. Events for an **evicted** scope are dropped (it rebuilds
  on next use).
- **Idempotency:** the server's own synchronous write and the subsequent watcher
  event both observe the change; the `(phys_path, mtime, size)` manifest makes the
  watcher's second pass a no-op.
- **Eviction:** all scopes are built eagerly at startup (D5), but idle per-scope
  indexes may still be dropped post-readiness under a configurable RAM bound and
  rebuilt on next use (the vault is the source of truth). The **shared index stays
  warm** (every query touches it).
- **Block-until-ready (A):** applies to a recall that hits a scope index not
  currently resident — i.e. one **evicted** after startup (initial build is eager,
  so the cold-start window is covered by `/readyz`, not by per-call blocking). The
  call **awaits** the rebuild, then returns correct results; the tool contract stays
  "results in, results out."

### D5: Kubernetes health model — `/healthz` liveness, `/readyz` readiness (renames `/health`)

The existing ungated `GET /health` is **renamed `GET /healthz`** (breaking; called
out in the proposal) and a new ungated `GET /readyz` is added. Both stay **outside
the bearer gate** — k8s probes send no `Authorization`.

| route | meaning | gates on index? |
|---|---|---|
| `GET /healthz` | liveness — process is alive | **NO.** Gating here makes k8s kill the pod mid-build. Always OK once the process is up. |
| `GET /readyz` | readiness — safe to route traffic | **YES.** Not ready until **every scope index and the shared index** are built. |

Readiness gates on a **full eager build of all scopes** at startup: the server
enumerates every scope from the vault, builds all indexes, and only then flips
`/readyz` green. Simple mental model — once a pod is Ready, *every* recall serves
immediately with no per-call build wait. Cost is a longer one-time cold start (a
`startupProbe` is the documented fit for it) and higher steady-state RAM, mitigated
by post-readiness eviction of idle scopes (D4). (Alternative considered: gate on the
shared index only and build scopes lazily — faster cold start, but a first recall
per scope blocks. Rejected: the user wants "ready" to mean *fully* ready.)

### D6: Frontmatter parsing is a read-side indexer concern (tantivy backend only)

Add `src/frontmatter.rs`: parse a leading `---\n … \n---` YAML block into typed
properties (text, list, number, checkbox/bool, date, datetime per Obsidian), used
**only** by `TantivyBackend` to populate `prop_<key>` fields and to strip
frontmatter from `body`. `storage.rs` read/write stay byte-exact and
frontmatter-agnostic — same separation as `wikilink.rs`. Malformed frontmatter is
non-fatal: index body-only and log; never fail the originating write (writes don't
touch the indexer). Predicates (P2): `key` exists, `key == value`, list contains,
and `>`/`<`/`>=`/`<=` for numeric/date values.

### D7: Regex semantics — true regex over an index-narrowed candidate set

tantivy's term-dictionary `RegexQuery` can't honor an arbitrary multiline/cross-token
pattern, so regex uses the `regex` crate over content:

1. Narrow to a candidate set via the query's index-backed facets (full-text and/or
   property filters).
2. Run the compiled regex over the **on-disk content of the candidates only**,
   producing real matches + snippets.
3. Regex-**only** (no narrowing) degrades to a bounded scan over the visible set,
   guarded by a configurable **byte/time cap** with an explicit truncation signal in
   the result — never an unbounded blocking scan.

`SimpleBackend`'s entire query path is this scan (its content cache *is* the
candidate set), so regex is first-class there even though ranking is not.

### D8: Cross-index merge — per-index normalization

BM25 scores from the scope index and the shared index are computed over different
corpus statistics (different `N`, different doc-freq) and are **not comparable**.
Merge by: take each index's hits, **normalize to 0–1 within that index's result
set**, then merge and sort. The agent-facing `score` is this **normalized** value —
raw BM25 across a union is meaningless, so a 0–1 contract is the defensible one.
Accepted trade-off: this is approximate relevance, not true cross-corpus BM25 (which
the two-index isolation choice precludes). Documented so it is not "fixed" later
without understanding the trade.

### D9: Tool surface — `recall_memory_notes`, consistent with `list_memory_notes`

```
recall_memory_notes(
  <scope keys>,                     // same as every tool
  query?: string,                   // full-text (BM25 on tantivy; substring on simple)
  filters?: [{key, op, value}],     // frontmatter predicates — tantivy only (D3)
  regex?: string,                   // true regex over candidates (D7)
  path_prefix?: string,             // same semantics as list_memory_notes
  limit?: number,                   // default 200 / cap 1000, mirroring list
  cursor?: string                   // opaque pagination cursor
) -> {
  hits: [{ path: string, score: number /* 0–1 */, snippets: [string] }],
  next_cursor: string | null
}
```

At least one of `query`/`filters`/`regex` is required (empty recall →
`invalid_argument`, not a full dump). Snippets pass through the read-side suffix
strip (and `wikilink::strip_links` once it lands); since results are confined to own
scope ∪ shared, no foreign suffix can appear.

## Risks / Trade-offs

- **Cold-start latency / readiness** → a full eager build of all scopes is a longer
  one-time cost; `/readyz` holds traffic until it finishes and `/healthz` never
  blocks, so k8s won't route early or kill a building pod (use a `startupProbe`).
  Post-startup, (A) blocks only a recall to a scope evicted after readiness.
- **RAM footprint at scale** → bounded by eviction of idle per-scope indexes
  (rebuildable); shared index kept warm. Snippets re-read from disk via `phys_path`
  so full bodies need not be retained.
- **Watcher misses events** → stat-diff reconcile backstop (D4); watcher is the live
  path, not the only path.
- **Approximate cross-index ranking (D8)** → accepted and documented; normalized 0–1
  score is the contract.
- **Capability gap on `simple` (D3)** → `filters` rejected with `unsupported` rather
  than silently dropped.
- **`/health` rename is breaking** → existing probes pointing at `/health` must move
  to `/healthz`; called out in the proposal and README.
- **Large optional dep (tantivy) + notify + YAML + regex** → tantivy and the watcher
  are feature-gateable; a lean build runs `simple` with neither.
- **Frontmatter misparse** → non-fatal, body-only, logged.

## Open Questions

- Should frontmatter property **values** also be full-text searchable (a `query`
  matching a tag word), or stay strictly in the `filters` facet?
- Watcher **debounce window** and the regex **scan-guard cap** — pick defaults during
  implementation and expose as config.
- Eviction policy detail — LRU by last-query time, with a max-resident-scopes and/or
  max-bytes bound? (Eager startup build means a very large vault could exceed the RAM
  bound *during* the build, before eviction can act — does the bound need to apply
  mid-build too?)
