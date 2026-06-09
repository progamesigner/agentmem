# Tasks

Phased per `design.md`. **P1** delivers usable recall on the default `simple`
backend with the full in-memory lifecycle, watcher, and k8s health split. **P2**
adds the opt-in `tantivy` backend (BM25 + snippets). **P3** adds frontmatter
property filters. **P4** hardens regex. Regex ships with `simple` in P1.

## 1. Configuration & dependencies

- [x] 1.1 Add pinned deps: `regex` and `notify` (no open ranges); add `tantivy` and a
      YAML frontmatter parser behind a new optional cargo feature (e.g. `recall-tantivy`,
      off by default), mirroring the `transport-http` feature pattern — flag in the PR
- [x] 1.2 Add `AGENTMEM_RECALL_BACKEND` (`simple` | `tantivy` | `off`, default `simple`)
      to `src/config.rs` with the existing env+CLI override pattern
- [x] 1.3 Add config for the watcher debounce window, the regex scan guard (byte/time cap),
      and the RAM/eviction bound
- [x] 1.4 Resolve backend selection × feature: default/simple/feature-absent → simple;
      feature+tantivy → tantivy; tantivy-without-feature or init failure → fall back to
      simple (logged); `off` → tool not registered
- [x] 1.5 Unit-test config parsing, defaults, and the selection/fallback matrix

## 2. The `RecallBackend` trait + per-scope/shared model (`src/recall.rs`) — P1

- [x] 2.1 Define the `RecallBackend` trait: build/evict, `apply_write`, `apply_fs_event`,
      `query`, `ready`
- [x] 2.2 Implement per-scope + shared index registry: a query opens only the caller's
      scope index and (policy-permitting) the shared index; scoped policy omits shared
- [x] 2.3 Implement reverse path→scope/shared mapping (run `path.rs` backwards) for routing
      writes and fs events to the owning index
- [x] 2.4 Implement normalized cross-index merge (per-index scores → 0–1, merge, sort);
      `score` returned to the agent is normalized
- [x] 2.5 Property test: over a generated multi-scope corpus, no hit/path/snippet from a
      foreign scope or an ignored/hidden note ever appears (structural isolation)

## 3. In-memory lifecycle (`src/index.rs`) — P1

- [x] 3.1 Populate indexes from the existing `storage` visibility walk so ignored/hidden
      notes never enter any index
- [x] 3.2 Eager startup build: enumerate all scopes + shared, build every index, then mark
      ready (gates `/readyz`, task 5)
- [x] 3.3 Synchronous own-write updates: a server write updates the owning index in-process
- [x] 3.4 notify-based fs-watcher started before the build, queuing events; debounced;
      re-applies the ignore filter; routes each event to the owning index; idempotent via
      the `(phys_path, mtime, size)` manifest
- [x] 3.5 Periodic stat-diff reconcile as a backstop for missed watcher events
- [x] 3.6 Eviction of idle per-scope indexes under the RAM bound; shared index kept warm;
      block-until-ready (A) rebuild on access to an evicted scope
- [x] 3.7 Unit-test: eager build, own-write update, watcher add/modify/delete routing +
      debounce + idempotency, stat-diff backstop, eviction + rebuild

## 4. `SimpleBackend` (default) — P1

- [x] 4.1 In-RAM content cache; case-insensitive substring `query` and `regex` matching
- [x] 4.2 Snippet extraction (matching lines + context) and a trivial term-count score
      (normalized via task 2.4)
- [x] 4.3 Reject `filters` with `unsupported` ("property filters require the tantivy backend")
- [x] 4.4 Bounded scan guard for regex-only queries (byte/time cap) with explicit truncation
      signal in the result
- [x] 4.5 Unit + integration tests: substring hit, regex hit, filters→unsupported, truncation

## 5. Health/readiness endpoints (`src/transport/http.rs`) — P1

- [x] 5.1 Rename `GET /health` → `GET /healthz` (liveness; always OK once process up; never
      gated on the index) — breaking change, noted in proposal/README
- [x] 5.2 Add ungated `GET /readyz` (readiness) returning not-ready until all indexes built,
      ready after; both probes stay outside the bearer gate
- [x] 5.3 Integration-test: `/healthz` OK during build, `/readyz` red→green across the eager
      build, both reachable without a bearer token

## 6. Tool wiring (`src/tools.rs`) — P1

- [x] 6.1 Add the `recall_memory_notes` tool: schema (D9), scope extraction, policy gating
      consistent with the other tools
- [x] 6.2 Require at least one of `query`/`filters`/`regex`; reject empty recall with
      `invalid_argument`
- [x] 6.3 Register the tool only when the backend is not `off`
- [x] 6.4 Strip the caller's own suffix from snippets (read-side strip / `wikilink::strip_links`
      once available); `path_prefix`, `limit` 200/1000, opaque cursor matching `list_memory_notes`
- [x] 6.5 Integration tests for the P1 scenarios (full-text hit, regex hit, scope confinement,
      pagination, disabled-tool behavior)

## 7. `TantivyBackend` (opt-in, behind the cargo feature) — P2

- [x] 7.1 tantivy in a `RamDirectory`; per-index schema `path`/`phys_path`/`body`/`mtime`/`size`
      (+ `prop_<key>` from P3)
- [x] 7.2 BM25 `query` and `SnippetGenerator` snippets → `{path, score, snippets}`
- [x] 7.3 Wire into the lifecycle (build/own-write/watcher/eviction) via the trait; incompatible
      tantivy version treated as a cache miss → rebuild (no data loss; vault is source of truth)
- [x] 7.4 Tests gated on the feature: BM25 ranking, snippet correctness, lifecycle parity with simple

## 8. Frontmatter property filters — P3 (tantivy backend)

- [x] 8.1 Create `src/frontmatter.rs`: parse the leading `---` YAML block to typed properties;
      strip frontmatter from `body`; malformed → body-only + log (never fail the originating write)
- [x] 8.2 Index `prop_<key>` fields; implement predicates: exists, `==`, list-contains,
      numeric/date `>`/`<`/`>=`/`<=`
- [x] 8.3 Compose property filters with full-text and the per-scope/shared union
- [x] 8.4 Unit + integration tests: each predicate, type coercion, composition, malformed resilience

## 9. Regex hardening — P4

- [x] 9.1 On tantivy, narrow to an index-backed candidate set, then run the compiled regex over
      candidate on-disk content; compose with property filters
- [x] 9.2 Confirm the bounded-scan guard + truncation signal across both backends
- [x] 9.3 Tests: regex narrowed by text/property, truncation signal, scope confinement under regex

## 10. Docs

- [x] 10.1 Add the structural index-isolation rule to `docs/security.md`
- [x] 10.2 Document `recall_memory_notes`, `AGENTMEM_RECALL_BACKEND`, the `tantivy` cargo feature,
      and the `/health`→`/healthz` + `/readyz` change (incl. k8s `startupProbe` guidance) in README
- [x] 10.3 Note "indexes are in-memory, nothing on disk; cold start rebuilds; `/readyz` gates traffic"

## 11. Verification

- [x] 11.1 Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` — both with and
      without the `recall-tantivy` feature
- [ ] 11.2 Benchmark eager cold-start build + warm query + watcher update over a synthetic
      tens-of-thousands-note vault; confirm RAM stays within the eviction bound
- [x] 11.3 Manually verify recall visibility matches `list`/`read` for two distinct scopes sharing
      one vault, and that `/readyz` flips green only after the build completes
