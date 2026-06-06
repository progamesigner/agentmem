## 1. Remove unused dependencies

- [x] 1.1 In `Cargo.toml`, remove the `tower` and `tower-http` dependency entries and drop `dep:tower` and `dep:tower-http` from the `transport-http` feature array.
- [x] 1.2 In `Cargo.toml` `[dev-dependencies]`, remove `predicates`, `rstest`, and `assert_cmd`.
- [x] 1.3 In `src/transport/http.rs`, fix the module doc comment that references a `tower` middleware to correctly say `axum` middleware.
- [x] 1.4 Build the default feature set (`cargo build`) and `cargo build --features transport-http` to confirm `axum`'s transitive `tower` still satisfies the `.layer()` call.
- [x] 1.5 Run `cargo test --all-features` to confirm the dev-dep removals leave the test suite compiling and green.

## 2. Upgrade rmcp to the stable 1.x line

- [x] 2.1 In `Cargo.toml`, bump the runtime `rmcp` requirement from `0.9` to `1`, keeping the `server`, `transport-io`, and `transport-streamable-http-server` features.
- [x] 2.2 In `Cargo.toml` `[dev-dependencies]`, bump the `rmcp` requirement to `1`, keeping the `client`, `transport-child-process`, and `transport-streamable-http-client-reqwest` features.
- [x] 2.3 Run `cargo build --all-features` and adapt any changed `rmcp` API call sites in `src/mcp.rs` and `src/transport/` until it compiles.
- [x] 2.4 Adapt the `rmcp` client harness call sites in `tests/` (stdio + http transport tests) to the 1.x API until `cargo test --all-features` compiles.
- [x] 2.5 Run `cargo test --all-features`; review any `insta` schema-snapshot diffs manually and only accept changes that are understood and intended. If the migration requires behavioral (not signature-only) changes, pause and reassess scope.

## 3. Upgrade reqwest dev-dependency

- [x] 3.1 In `Cargo.toml` `[dev-dependencies]`, bump `reqwest` from `0.12` to `0.13`, keeping `default-features = false` and the `rustls-tls` feature.
- [x] 3.2 Adapt any `reqwest` client/builder usage in the HTTP transport tests to the 0.13 API and run `cargo test --all-features`.

## 4. Upgrade remaining direct dependencies to latest

- [x] 4.1 In `Cargo.toml`, raise each remaining direct dependency's version requirement to its latest published release (current targets: `tokio` 1.52, `tracing` 0.1.44, `tracing-subscriber` 0.3.23, `serde`/`serde_json` 1.0.228/1.0.150, `schemars` 1.2, `thiserror` 2.0.18, `anyhow` 1.0.102, `clap` 4.6, `tempfile` 3.27, `chrono` 0.4.45, `chrono-tz` 0.10.4, `ignore` 0.4.26, `base64` 0.22.1, `dashmap` 6.2, `camino` 1.2, `axum` 0.8.9, `assert_fs` 1.1.4, `insta` 1.47). Trim to minor precision where sensible; re-check crates.io at implementation time in case newer releases have shipped.
- [x] 4.2 Run `cargo build --all-features` and `cargo test --all-features`; adapt to any minor API changes surfaced by the bumps.

## 5. Refresh lockfile, audit, and final verification

- [x] 5.1 Run `cargo update` to refresh `Cargo.lock` so direct and transitive dependencies resolve to their latest compatible versions.
- [x] 5.2 Run `cargo audit`. Confirm the `paste` advisory (RUSTSEC-2024-0436), previously pulled in only by `rmcp 0.9.1`, has cleared after the `rmcp` upgrade. Resolve any remaining advisory by bumping the responsible crate, `cargo update -p <crate> --precise <ver>`, or a `[patch]` entry — the audit must end clean.
- [x] 5.3 Run the full gate: `cargo build --all-features`, `cargo test --all-features`, `cargo clippy --all-features -- -D warnings`, and `cargo fmt --check`.
- [x] 5.4 Verify the trimmed graph: confirm `tower`, `tower-http`, `predicates`, `rstest`, and `assert_cmd` no longer appear as direct dependencies (`cargo tree -e no-dev --depth 1` and a `Cargo.toml` review).
