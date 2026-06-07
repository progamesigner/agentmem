## Why

`agentmem` ships as platform binaries (the tag-triggered `release-binaries` job), but there is no container image — the most common way to run an HTTP MCP server in CI, Kubernetes, or Compose. We want a published image that is as small as the workload allows and that carries complete, build-derived OCI metadata so consumers can trace any pulled image back to its exact source, revision, and version.

## What Changes

- Add a multi-stage `Dockerfile` that cross-compiles a fully static musl binary and ships it on `scratch` (no shell, no CA bundle, no `/tmp` — verified safe: atomic writes use `NamedTempFile::new_in(parent)` inside the vault, and `chrono-tz` bakes the IANA database into the binary). The image runs as a numeric nonroot user and defaults to the HTTP transport.
- Add a `.dockerignore` to keep the build context minimal.
- Add `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` build targets; the image is published as a multi-arch manifest (`linux/amd64` + `linux/arm64`).
- Add a `publish-image` job to `.github/workflows/ci.yml`, gated on tag pushes (reusing `if: startsWith(github.ref, 'refs/tags/')` and `needs: check`), that logs in to and pushes to the **GitHub Container Registry** (`ghcr.io/progamesigner/agentmem`). It uses the latest official actions: `actions/checkout@v6`, `docker/setup-buildx-action@v4`, `docker/login-action@v4`, `docker/metadata-action@v6`, `docker/build-push-action@v7`.
- Publish three tags per release: `:{version}`, `:latest`, and `:sha-<gitsha>`.
- Stamp all ten requested OCI labels dynamically — `created`, `revision`, `version`, `title`, `description`, `source`, `url` from `docker/metadata-action`; `authors`, `documentation`, `vendor` from custom labels sourced via `cargo metadata`.
- Fill the missing `authors`, `documentation`, and `homepage` fields in `Cargo.toml` so the `authors`/`documentation`/`vendor` labels derive from a single source of truth rather than literals embedded in the workflow.
- Document running the container (including the `scratch`-can't-`HEALTHCHECK` caveat: orchestrators probe `GET /health` instead) in the README.

## Capabilities

### New Capabilities
- `container-image`: Defines the published container image — base, static-binary build, supported architectures, registry and tag scheme, the required OCI image labels and their dynamic sources, and runtime expectations (nonroot, transport default, health probing).

### Modified Capabilities
<!-- None. This change adds distribution packaging; it does not alter runtime behavior specified by configuration, mcp-server, memory-tools, vault-storage, or context-http-api. -->

## Impact

- **New files**: `Dockerfile`, `.dockerignore`, `openspec/specs/container-image/spec.md`.
- **Modified files**: `.github/workflows/ci.yml` (new `publish-image` job), `Cargo.toml` (add `authors`, `documentation`, `homepage`), `README.md` (container usage section).
- **CI/permissions**: the new job needs `permissions: { contents: read, packages: write }` and authenticates to GHCR with `${{ github.actor }}` / `${{ secrets.GITHUB_TOKEN }}`. No new repository secrets required.
- **Toolchain**: adds two musl Rust targets to the build (cross-compiled in the builder stage to avoid QEMU compilation of arm64).
- **No runtime/behavior change**: the existing `release-binaries` job and all gnu binary artifacts are unaffected.
