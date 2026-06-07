## Context

`agentmem` is a single-binary Rust HTTP MCP server. CI already builds gnu release binaries on tag pushes (`release-binaries`) but produces no container image. The runtime workload is favorable for an extremely small image:

- It is a pure server — it serves HTTP (axum) and reads/writes a filesystem vault. No outbound TLS, so no CA bundle is needed.
- Atomic writes use `tempfile::NamedTempFile::new_in(parent)` (`src/storage.rs:159`), i.e. temp files land **inside the vault directory**, not a global `/tmp`. The image needs no writable `/tmp`.
- `chrono-tz` compiles the IANA timezone database **into the binary**, so no `/usr/share/zoneinfo` is required at runtime.
- There is already a `GET /health` liveness route (`src/transport/http.rs`).

These facts make `FROM scratch` viable, not just `distroless`. The remaining engineering is: produce a static musl binary for two architectures, ship it on `scratch`, and stamp ten dynamic OCI labels during a GHCR publish step.

## Goals / Non-Goals

**Goals:**
- Smallest practical image: static musl binary on `scratch`.
- Multi-arch manifest: `linux/amd64` + `linux/arm64`, built without slow QEMU-emulated compilation.
- Ten OCI labels populated dynamically from build context and `Cargo.toml`.
- Publish to GHCR on tag pushes using the latest official actions.
- A manual `docker build` still produces a runnable image (labels are applied by CI, not required for the build to succeed).

**Non-Goals:**
- Replacing or removing the existing gnu `release-binaries` artifacts.
- An embedded `HEALTHCHECK` (impossible on `scratch`); liveness is the orchestrator's job via `GET /health`.
- Publishing to Docker Hub or any registry other than GHCR.
- A non-musl / glibc image variant.

## Decisions

### D1: Base image — `scratch`

Chosen over `distroless/static:nonroot` and `alpine`. The workload needs no CA bundle, no shell, and no `/tmp`, so `scratch` is the smallest correct choice. Cost: we must supply a numeric nonroot user ourselves (no `/etc/passwd` lookup needed — a bare `USER 65532:65532` works for a static binary that performs no username resolution). Alternative `distroless/static:nonroot` was ~2 MB larger for conveniences this workload does not use; `alpine` adds a busybox shell we do not need.

### D2: Cross-compile with `cargo-zigbuild` in a `BUILDPLATFORM`-pinned builder stage

The builder stage is pinned to the native build platform (`FROM --platform=$BUILDPLATFORM`) and uses `cargo-zigbuild` to cross-compile to the musl triple selected from the Docker `TARGETPLATFORM`/`TARGETARCH` build arg. This compiles arm64 **on the amd64 runner** (no QEMU), keeping CI fast, while still letting `docker/build-push-action` drive the multi-platform manifest.

Mapping:
```
TARGETARCH=amd64 → x86_64-unknown-linux-musl
TARGETARCH=arm64 → aarch64-unknown-linux-musl
```

Alternatives considered:
- **QEMU-emulated buildx** (let `cargo build` run under emulation for arm64): simplest Dockerfile but arm64 compilation is ~10× slower — rejected.
- **`cross`**: works, but spawns per-target docker containers and is heavier to wire inside a buildx build than zigbuild — rejected as the primary path (noted as a fallback).
- **Compile in CI matrix, `COPY` prebuilt binaries**: fast, but splits the build across job + Dockerfile and complicates the multi-arch manifest assembly — rejected in favor of a single self-contained Dockerfile.

### D3: Labels via `docker/metadata-action@v6` + custom labels from `cargo metadata`

`metadata-action` auto-emits `created`, `revision`, `version`, `title`, `description`, `source`, `url` (and `licenses`) from the Git/GitHub context. The three it does not emit — `authors`, `documentation`, `vendor` — are added via its `labels:` input, sourced from a preceding shell step that reads `Cargo.toml` with `cargo metadata --format-version 1` (for `authors`, `documentation`/`homepage`) and a constant for `vendor` (the org, `progamesigner`). `build-push-action` consumes `${{ steps.meta.outputs.labels }}`. This keeps all ten labels dynamic and avoids duplicating project facts in YAML.

### D4: `Cargo.toml` is the source of truth for `authors`/`documentation`

`Cargo.toml` currently lacks `authors`, `documentation`, and `homepage`. We add them so the corresponding labels derive from one place. This also benefits crate publication metadata generally.

### D5: Tag scheme — `:{version}` + `:latest` + `:sha-<gitsha>`

Configured through `metadata-action`'s `tags:` input (`type=semver,pattern={{version}}`, `type=raw,value=latest`, `type=sha,prefix=sha-`). `:latest` is a moving convenience tag, `:{version}` is the human-facing release, `:sha-<gitsha>` is immutable for precise pinning.

### D6: Publish job reuses existing CI gating

A new `publish-image` job mirrors `release-binaries`: `needs: check` and `if: startsWith(github.ref, 'refs/tags/')`. It declares `permissions: { contents: read, packages: write }` and authenticates with `${{ github.actor }}` / `${{ secrets.GITHUB_TOKEN }}` — no new repository secrets.

### D7: Default entrypoint serves HTTP

`ENTRYPOINT ["/agentmem"]` with default args selecting the HTTP transport bound to a documented port/interface. `EXPOSE` documents the port. Operators override args/`--user`/volume mounts as needed.

### D8: Deny-all `.dockerignore` with an explicit allowlist

The `.dockerignore` ignores everything and re-includes only the compile inputs, rather than enumerating things to exclude:

```
*
!Cargo.toml
!Cargo.lock
!rust-toolchain.toml
!src
```

Rationale: an allowlist is fail-closed — new top-level files (scratch dirs, future tooling, secrets, the large `target/`) are excluded by default instead of silently leaking into the build context and busting the layer cache. Investigation confirmed the build needs nothing else: there is no `build.rs` and no `include_str!`/`include_bytes!` of non-`.rs` files, and `Cargo.toml`'s `readme` field does not block `cargo build`. `!src` re-includes the whole source tree (Docker un-ignores a directory recursively); the contents are "mostly `.rs`" as expected. Alternative — a blocklist enumerating `target/`, `.git/`, etc. — was rejected as fail-open and higher-maintenance.

This pairs with the multi-stage build (D1/D2): the builder stage compiles from the allowlisted context, and the `scratch` runtime stage `COPY`s only the resulting binary, so no source or toolchain reaches the final image.

## Risks / Trade-offs

- **No in-image `HEALTHCHECK`** → Documented: orchestrators (k8s liveness, Compose with a probe sidecar) hit `GET /health`. The route already exists.
- **musl static build pulls in differences from the gnu builds** (allocator, edge-case syscalls) → Mitigation: a smoke test in CI that runs the built image and curls `/health` before the push is trusted; the existing test suite still runs under `check` on gnu.
- **`scratch` has no debugging surface** (no shell to `exec` into) → Accepted trade-off for size; debugging uses logs (stderr tracing) and local non-scratch runs.
- **`cargo-zigbuild` / zig toolchain availability in the builder image** → Pin a known builder image/tag that bundles the Rust toolchain and install a pinned zig + `cargo-zigbuild`; `cross` remains a documented fallback if zigbuild breaks.
- **Default unauthenticated HTTP bind** → The server already warns when `AGENTMEM_HTTP_BEARER` is unset and the bind is non-loopback; README must call out setting the bearer for any non-loopback container deployment.
- **`{{version}}` vs `Cargo.toml` version drift** → The release tag drives the `version` label/tag; document that the Git tag should match the `Cargo.toml` version at release time.
