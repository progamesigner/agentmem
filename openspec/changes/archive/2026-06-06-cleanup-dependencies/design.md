## Context

`agentmem` is a Rust MCP server (edition 2024, MSRV 1.85) fronting an Obsidian-style markdown vault. Its `Cargo.toml` has accumulated:

- **Unused runtime deps**: `tower` and `tower-http` are declared and wired into the `transport-http` feature, but a grep across `src/` shows the bearer-token middleware is built with `axum::middleware::from_fn_with_state` (not `tower`), and `TraceLayer` (the only reason for `tower-http`'s `trace` feature) is never instantiated. Both survive only in a doc comment in `src/transport/http.rs`.
- **Unused dev-deps**: `predicates`, `rstest`, and `assert_cmd` have zero references in `tests/`. The tests use `assert_fs::TempDir` for fixtures and the `rmcp` client harness + `reqwest` for transport assertions; no `predicate::`, `#[rstest]`, or `Command::cargo_bin` usage exists.
- **A pinned pre-1.0 core dep**: `rmcp` is at `0.9` while the stable line is now `1.x`. The dev-dependency entry also pins `0.9`.
- **A behind-major dev dep**: `reqwest` is at `0.12` while `0.13` is current.

All other direct dependencies are already on their current major version and resolve to the latest in-range release; `cargo update --dry-run` shows only trivial transitive patch bumps.

**Security baseline (`cargo audit`, RustSec advisory DB, 302 crates scanned):** exactly one advisory, a *warning* (not a CVE-level vulnerability): `paste 1.0.15` is unmaintained (RUSTSEC-2024-0436). `cargo tree -i paste` shows it is pulled in **solely by `rmcp 0.9.1`**. The `rmcp 0.9 → 1.x` upgrade is therefore expected to drop `paste` and clear the advisory. No high/critical vulnerabilities are present today; the broad upgrade and a standing `cargo audit` gate are preventative — keeping the graph clean as advisories land in future.

## Goals / Non-Goals

**Goals:**
- Remove every dependency with zero source/test references, plus their feature wiring.
- Move `rmcp` to the stable `1.x` line and `reqwest` (dev) to `0.13`, adapting any changed APIs.
- Keep behavior identical: the full test suite and schema snapshots pass unchanged.
- Regenerate `Cargo.lock` so it reflects the trimmed, upgraded graph.

**Non-Goals:**
- No changes to the MCP tool contract, transport behavior, storage semantics, or any spec-level requirement.
- No proactive major bumps of deps already on their current major (caret ranges already track the latest compatible release).
- No new functionality (e.g., this does not add the request tracing that `tower-http` was presumably once intended for — if tracing is wanted later, it is a separate change).

## Decisions

**Decision 1 — Remove `tower` and `tower-http` rather than start using them.**
The bearer middleware already works via `axum`; `axum 0.8` brings `tower` transitively so `.layer()` keeps compiling. Re-introducing a direct `tower-http` `TraceLayer` would be scope creep (new behavior). Rationale: delete dead deps; defer any HTTP tracing to a dedicated change. *Alternative considered*: wire up `TraceLayer` to justify the dep — rejected as out of scope and behavior-changing.

**Decision 2 — Drop `predicates`/`rstest`/`assert_cmd` instead of adopting them.**
They are unused dev-deps. The existing test style (`assert_fs` + `rmcp` client + `insta` snapshots) is sufficient and consistent. Rationale: smaller dev graph, faster `cargo test` builds.

**Decision 3 — Upgrade `rmcp` 0.9 → 1.x as the one substantive code change, isolated in its own task.**
This is the only upgrade likely to touch source. Approach: bump both the runtime and dev-dependency `rmcp` entries to `1`, then `cargo build`/`cargo test --all-features` and fix compile errors at the rmcp call sites (server construction in `src/mcp.rs`, transport setup in `src/transport/`, and the client harness in `tests/`). Keep the feature flags currently requested (`server`, `transport-io`, `transport-streamable-http-server`, and the client/transport features in dev). *Alternative considered*: stay on `0.9` — rejected; the user explicitly asked to upgrade used deps and `0.9` is unsupported pre-stable.

**Decision 4 — Raise every direct dependency's version floor to its latest release, then refresh the lockfile.**
For deps already on the current major, bump the `Cargo.toml` requirement to the latest published version (e.g. `tokio = "1.52"`, `axum = "0.8.9"`, `serde = "1.0.228"`) and run `cargo update`. Rationale: the user explicitly wants every dependency on its latest release for security reasons; raising the floor makes "latest" explicit in the manifest and prevents the resolver from later selecting an older, potentially-vulnerable in-range version. *Alternative considered*: leave caret requirements untouched and rely solely on the lockfile — rejected because it does not raise the minimum-supported floor and leaves the manifest silent about the intended versions. Trim to minor precision (not exact patch) where a crate publishes frequent patches, to avoid needless churn.

**Decision 5 — Treat `cargo audit` as a required, repeatable gate, not a one-off.**
Run `cargo audit` after the upgrades and resolve every reported advisory. Today that means confirming the `paste` advisory clears once `rmcp` no longer depends on it; if any new advisory surfaces against a transitive crate that a direct upgrade does not fix, address it with an explicit transitive bump (`cargo update -p <crate> --precise <ver>`) or a `[patch]` entry. Rationale: the user's motivation is security, so the change should leave behind a clean `cargo audit` and a documented way to keep it clean. *Alternative considered*: a one-time manual check — rejected; the gate belongs in the verification step and ideally CI (out of scope here but noted).

**Decision 6 — No behavioral spec deltas; restate the affected `mcp-server` requirements.**
This change alters only the dependency graph and internal call sites; no capability's requirements or observable behavior change. Because OpenSpec requires every change to carry at least one delta, the `mcp-server` `Server binary lifecycle` and `Transport selection` requirements (whose normative text names `rmcp`/`axum`) are restated **verbatim** under `## MODIFIED Requirements`, recording that they continue to hold under `rmcp 1.x` and after the `tower`/`tower-http` removal. Correctness is otherwise asserted purely by the existing test suite and schema snapshots passing unchanged.

## Risks / Trade-offs

- **`rmcp` 1.x has breaking API changes** → Isolate the bump in its own task; rely on `cargo build` + the stdio/http integration tests and schema snapshots to surface and confirm any required adaptations before moving on. If the migration proves large, it can be split into its own change without blocking the removals.
- **Removing `tower`/`tower-http` breaks the `transport-http` build** → Build with `--features transport-http` (the default) and `--all-features` after removal to confirm `axum`'s transitive `tower` satisfies `.layer()`.
- **`reqwest` 0.13 changes the client builder/TLS surface** → It is dev-only (transport tests); guarded by those tests. Keep `default-features = false` + `rustls-tls`.
- **Schema snapshots drift after the `rmcp` upgrade** → If snapshots change, review each diff manually; an unexpected snapshot change is a signal of a real behavior shift and must be understood, not blindly accepted.

## Migration Plan

1. Remove unused deps + feature wiring; fix the stale `tower` doc comment.
2. Build/test to confirm removals are clean.
3. Bump `rmcp` (and dev `rmcp`) to `1`, adapt call sites, build/test.
4. Bump `reqwest` dev-dep to `0.13`, adapt, test.
5. Raise the remaining direct-dependency version requirements to their latest releases.
6. `cargo update` to refresh the lockfile; full-suite + `--all-features` + clippy/fmt run.
7. `cargo audit`; confirm the `paste` advisory has cleared and the report is empty, resolving any remaining advisory before finishing.

Rollback: revert the `Cargo.toml`/`Cargo.lock`/source diff in one commit; no data or external state is touched.

## Open Questions

- None blocking. If the `rmcp` 1.x migration turns out to require non-trivial behavioral changes (not just signature adaptation), pause and split it into a dedicated change.
