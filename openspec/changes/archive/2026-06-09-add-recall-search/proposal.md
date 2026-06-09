## Why

Agents recall memory today only through `MEMORY.md` (a hand-curated ≤200-line
index loaded each session) plus `list_memory_notes` + `read_memory_note`. To find
a note *by its content* an agent must list paths and read candidates one by one,
burning tokens and round-trips, and the curated index silently misses anything not
yet entered. At a small vault this is tolerable; at **tens of thousands of notes**
it is not — the agent has no way to ask "which notes mention X" without reading the
whole visible set.

Obsidian's own search cannot help here: it lives inside the desktop GUI, has no
headless mode and no on-disk queryable index, so an agent talking over MCP cannot
invoke it. The vault's Obsidian compatibility is a *file-format* property, not a
reusable search backend. Content recall for the agent has to be served by the
server itself.

## What Changes

- A new **`recall_memory_notes`** tool returns ranked content hits visible to the
  caller's scope. Each hit is `{ path, score, snippets }` — the clean virtual path,
  a relevance score, and matching line snippets with context.
- Three composable query facets behind one tool, via a **configurable backend**:
  - **Full-text** — BM25-ranked on the `tantivy` backend; case-insensitive
    substring on the `simple` backend.
  - **Frontmatter property filters** — Obsidian "properties" (YAML frontmatter) as
    structured predicates (`key=value`, present, list contains, numeric/date
    comparisons). **`tantivy` backend only**; rejected with `unsupported` on `simple`.
  - **Regex** — true regular-expression matching over content (the `regex` crate),
    run over the candidate set, with a bounded-scan guard when unnarrowed. Available
    on **both** backends.
- Results respect the **exact same visibility** as `list`/`read`: own-scope notes
  inside the agents folder ∪ the shared region the active policy permits; other
  scopes and ignored/hidden files never appear.
- A **fully in-memory index, the way Obsidian works** — nothing written to disk.
  Built in RAM at startup, updated synchronously on the server's own writes, and
  kept live by a **filesystem watcher** (notify) for external Obsidian/editor edits,
  with a periodic stat-diff reconcile as a backstop.
- **Per-scope indexes plus one shared-region index** (structural isolation matching
  the suffix scheme): a query opens only the caller's own-scope index and the shared
  index. A scope's content lives only in that scope's index — cross-scope recall is
  structurally impossible, not merely filtered. Cross-index results are merged by
  **per-index score normalization** (0–1), so the agent-facing `score` is normalized.
- **Configurable backend** — `AGENTMEM_RECALL_BACKEND = simple | tantivy | off`,
  **defaulting to `simple`**; `tantivy` is **opt-in** and gated behind an optional
  cargo feature (off by default), so a default build is lean and carries none of the
  heavy dependencies.
- **Kubernetes-native health** — the existing `GET /health` is renamed `GET
  /healthz` (liveness, never gated on the index) and a new `GET /readyz` reports
  readiness, flipping green only after **all scope indexes plus the shared index are
  eagerly built** at startup. Both probes stay outside the bearer gate.
- A new **frontmatter-parsing layer** extracts YAML properties at index time
  (tantivy backend). Storage stays a dumb byte layer, mirroring how `wikilink.rs`
  keeps `storage.rs` unaware of link semantics.

Phased to bound risk (see `design.md`): **P1** `simple` backend + per-scope/shared
indexes + in-memory lifecycle + watcher + `/readyz`; **P2** `tantivy` backend
(BM25 + snippets) behind the cargo feature; **P3** frontmatter property filters;
**P4** regex polish + scan guard. Regex ships with `simple` in P1.

## Capabilities

### New Capabilities
- `recall-search`: A `recall_memory_notes` tool and the in-memory, structurally
  scope-isolated index behind it — a configurable backend (`simple` substring+regex
  default; opt-in `tantivy` BM25 + property filters + snippets), per-scope + shared
  indexes merged by normalized score, a filesystem-watcher-fed live lifecycle, and
  `{path, score, snippets}` results confined to the caller's visible set.

### Modified Capabilities
- `memory-tools`: gains the `recall_memory_notes` tool alongside the existing nine;
  its visibility/scope semantics match `list_memory_notes` exactly.
- `vault-storage`: the own-scope visibility guarantee is extended to recall results
  and snippets — recall never returns content, a path, or a snippet from another
  scope, and never from an ignored/hidden note; isolation is structural (per-scope
  index), not filter-enforced.
- `context-http-api` / `mcp-server`: `GET /health` is renamed `GET /healthz`
  (liveness) and a new ungated `GET /readyz` reports index readiness on the HTTP
  transport.
- `configuration`: adds `AGENTMEM_RECALL_BACKEND` (default `simple`), the optional
  `tantivy` cargo feature, the regex scan guard, the watcher debounce window, and the
  RAM-footprint / eviction bound. No on-disk index dir.

## Impact

- **Code**: new `src/recall.rs` (the `RecallBackend` trait, query dispatch,
  per-scope/shared union + normalized merge), `src/index.rs` (in-memory lifecycle:
  eager startup build, sync write updates, notify watcher + stat-diff backstop,
  eviction, readiness state), backend impls (`SimpleBackend`; `TantivyBackend` behind
  the feature), `src/frontmatter.rs` (YAML extraction, tantivy backend);
  `recall_memory_notes` handler in `src/tools.rs`; `/healthz`+`/readyz` routes in
  `src/transport/http.rs`; index population on the existing `storage` walk so
  ignored/hidden notes never enter any index; clean-path/snippet output reusing the
  read-side suffix strip (and `wikilink::strip_links` once that lands).
- **New dependencies** (the project has been dependency-frugal — flagged
  explicitly): the `regex` crate (always) and `notify` for the watcher; **tantivy**
  and a YAML frontmatter parser **only under the opt-in cargo feature**, so the
  default build stays lean.
- **Security model**: `docs/security.md` gains the index-isolation rule — scoped
  content lives only in its own per-scope in-RAM index; a query opens only the
  caller's scope index plus the shared index, so isolation is structural.
- **Operational**: indexes are RAM-resident — nothing is written to the vault, git,
  or a cache dir. `/readyz` holds k8s traffic until the full eager build completes
  (use a `startupProbe`); steady-state RAM is bounded by eviction of idle scopes;
  external edits are reflected live via the watcher.
- **No breaking changes** to existing tool schemas; recall is additive.
