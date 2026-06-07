## 1. Project metadata (source of truth for labels)

- [x] 1.1 Add `authors`, `documentation`, and `homepage` to `[package]` in `Cargo.toml`
- [x] 1.2 Run `cargo metadata --format-version 1` locally and confirm the new fields are readable for label extraction

## 2. Dockerfile and build context

- [x] 2.1 Add a deny-all `.dockerignore` (`*`) that re-includes only `!Cargo.toml`, `!Cargo.lock`, `!rust-toolchain.toml`, `!src`
- [x] 2.1a Verify the effective build context contains only those allowlisted paths (e.g. inspect the context sent to the daemon / a throwaway `COPY . /ctx` listing)
- [x] 2.2 Write the builder stage: `FROM --platform=$BUILDPLATFORM <rust-toolchain image>`, install a pinned zig + `cargo-zigbuild`
- [x] 2.3 Map `TARGETARCH` → musl triple (`amd64`→`x86_64-unknown-linux-musl`, `arm64`→`aarch64-unknown-linux-musl`) and `cargo zigbuild --release` that target
- [x] 2.4 Write the runtime stage: `FROM scratch`, `COPY` the static binary to `/agentmem`, `USER 65532:65532`, `EXPOSE` the HTTP port, `ENTRYPOINT ["/agentmem"]` with default args selecting the HTTP transport

## 3. Local verification

- [x] 3.1 Build the image for the host arch and confirm it runs: start the container with a mounted vault and curl `GET /health` → `ok`
- [x] 3.2 Verify the binary is statically linked and the image contains no shell (e.g. `docker run --rm <img> --print-config` works; inspecting the layer shows only the binary)
- [x] 3.3 Confirm a write through the MCP/HTTP path succeeds (atomic temp write lands in the mounted vault, not `/tmp`)

## 4. CI publish job (GHCR)

- [x] 4.1 Add a `publish-image` job to `.github/workflows/ci.yml` with `needs: check`, `if: startsWith(github.ref, 'refs/tags/')`, and `permissions: { contents: read, packages: write }`
- [x] 4.2 Steps: `actions/checkout@v6` → `docker/setup-buildx-action@v4` → `docker/login-action@v4` (registry `ghcr.io`, `github.actor` / `secrets.GITHUB_TOKEN`)
- [x] 4.3 Add a shell step that reads `authors` and `documentation`/`homepage` via `cargo metadata` and exports them to `$GITHUB_ENV`
- [x] 4.4 Add `docker/metadata-action@v6` (id `meta`): images `ghcr.io/progamesigner/agentmem`; tags `type=semver,pattern={{version}}`, `type=raw,value=latest`, `type=sha,prefix=sha-`; custom `labels:` for `org.opencontainers.image.authors`, `.documentation`, `.vendor`
- [x] 4.5 Add `docker/build-push-action@v7`: `platforms: linux/amd64,linux/arm64`, `push: true`, `tags`/`labels` from `${{ steps.meta.outputs.* }}`
- [x] 4.6 Add a post-build smoke step (or `load`+run on amd64) that curls `/health` before the push is considered trusted

## 5. Label verification

- [x] 5.1 After a (test) tag build, inspect the manifest and confirm both `linux/amd64` and `linux/arm64` are present
- [x] 5.2 Inspect image config and confirm all ten OCI labels (`created`, `revision`, `version`, `title`, `description`, `source`, `url`, `authors`, `documentation`, `vendor`) are present and non-empty
- [x] 5.3 Confirm the three tags (`:{version}`, `:latest`, `:sha-<gitsha>`) reference the same digest

## 6. Documentation

- [x] 6.1 Add a "Running the container" section to `README.md`: `docker run` example with a mounted vault, the HTTP port, and setting `AGENTMEM_HTTP_BEARER` for non-loopback binds
- [x] 6.2 Document the `scratch`-can't-`HEALTHCHECK` caveat: orchestrators probe `GET /health` (with a k8s liveness / Compose example)
- [x] 6.3 Document the GHCR image reference and tag scheme

## 7. Final checks

- [x] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` (per repo CI convention)
- [x] 7.2 Validate the change with `openspec validate add-container-image-release`
