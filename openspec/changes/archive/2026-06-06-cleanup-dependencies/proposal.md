## Why

The `Cargo.toml` carries dependencies that are no longer referenced anywhere in the source or tests (dead weight on build time, audit surface, and supply-chain risk), and several dependencies — including the core MCP framework `rmcp` pinned to a pre-1.0 `0.9` release — are behind their latest published versions. Outdated crates expose the project to known security advisories (RustSec) that have already been fixed upstream. Removing the dead crates and upgrading every remaining dependency to its latest release keeps the graph lean, supported, and free of known vulnerabilities.

## What Changes

- Remove unused runtime dependencies and their feature wiring:
  - `tower` (declared + listed in the `transport-http` feature, but the bearer middleware uses `axum::middleware`, not `tower` directly).
  - `tower-http` (declared with the `trace` feature in `transport-http`, but `TraceLayer` is never used).
- Remove unused dev-dependencies: `predicates`, `rstest`, `assert_cmd` (zero references across `tests/`).
- Upgrade `rmcp` from `0.9` to the stable `1.x` line (both the runtime and dev-dependency entries), adapting any changed APIs. **BREAKING** at the dependency level (internal only; no change to the MCP tool contract).
- Upgrade the `reqwest` dev-dependency from `0.12` to `0.13`.
- Upgrade **every remaining direct dependency** to its latest published release by raising each version requirement in `Cargo.toml` to the current latest (e.g. `tokio`, `serde`/`serde_json`, `schemars`, `thiserror`, `anyhow`, `clap`, `tempfile`, `chrono`/`chrono-tz`, `ignore`, `base64`, `dashmap`, `camino`, `axum`, `tracing`/`tracing-subscriber`, `assert_fs`, `insta`). These stay within their current major, so the upgrade is a floor bump plus a lockfile refresh.
- Refresh `Cargo.lock` (`cargo update`) so direct and transitive dependencies resolve to their latest compatible versions, pulling in upstream security fixes.
- Add a security audit gate: run `cargo audit` against `Cargo.lock` and resolve every reported RustSec advisory (by bumping the responsible crate, or a `[patch]`/explicit transitive bump where a direct upgrade is insufficient).
- Confirm no behavioral change: existing tests and schema snapshots must continue to pass unchanged.

## Capabilities

### New Capabilities
<!-- None. This is dependency hygiene with no new behavior. -->

### Modified Capabilities
- `mcp-server`: No behavior change. The `Server binary lifecycle` and `Transport selection` requirements are restated to record that they continue to hold under the upgraded `rmcp` `1.x` line and after the removal of the unused direct `tower`/`tower-http` dependencies (the bearer middleware is provided by `axum`).

## Impact

- **Files**: `Cargo.toml` (dependency + feature edits), `Cargo.lock` (regenerated).
- **Source**: `src/transport/http.rs` doc comment references a `tower` middleware that is actually `axum` middleware — update wording. Any `rmcp` API call sites touched by the `0.9 → 1.x` upgrade across `src/` (notably `src/mcp.rs`, `src/transport/`) and the client harness in `tests/`.
- **Build/features**: `transport-http` feature definition loses `dep:tower` and `dep:tower-http`.
- **Risk**: The `rmcp` major upgrade is the only non-trivial change; it may alter server/transport construction APIs. All other moves are removals or in-range bumps. Regression is guarded by the existing stdio/http transport integration tests and schema snapshots.
