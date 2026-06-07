# syntax=docker/dockerfile:1

# ---- builder ----------------------------------------------------------------
# Rust + zig + cargo-zigbuild in one image. Pinned to the *build* platform so
# the arm64 artifact is cross-compiled on the native runner rather than built
# under slow QEMU emulation. cargo-zigbuild links musl targets via zig.
FROM --platform=$BUILDPLATFORM messense/cargo-zigbuild:0.20.0 AS builder

# Docker sets TARGETARCH to the architecture currently being assembled.
ARG TARGETARCH

WORKDIR /src

# Resolve the musl target triple for this architecture and stash it so the
# build step and the runtime COPY agree on the output path.
RUN case "$TARGETARCH" in \
        amd64) echo "x86_64-unknown-linux-musl"  > /tmp/triple ;; \
        arm64) echo "aarch64-unknown-linux-musl" > /tmp/triple ;; \
        *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac

# Add the musl target to the toolchain selected by rust-toolchain.toml.
COPY rust-toolchain.toml ./
RUN rustup target add "$(cat /tmp/triple)"

# Compile the static binary. The allowlisted .dockerignore keeps the context to
# Cargo.*, rust-toolchain.toml, and src/. Copy the result out of the cache mount
# (and pre-create the vault mountpoint) in the same layer so both persist.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo zigbuild --release --target "$(cat /tmp/triple)" \
 && cp "target/$(cat /tmp/triple)/release/agentmem" /agentmem \
 && mkdir -p /vault

# ---- runtime ----------------------------------------------------------------
# Empty base: the static binary needs no libc, CA bundle, shell, or /tmp.
FROM scratch AS runtime

COPY --from=builder /agentmem /agentmem
# Vault mountpoint owned by the nonroot uid so atomic writes (NamedTempFile in
# the vault dir) and unmounted / anonymous-volume runs succeed.
COPY --from=builder --chown=65532:65532 /vault /vault

# Run unprivileged. The static binary resolves no usernames, so a bare numeric
# uid:gid works without /etc/passwd.
USER 65532:65532

# AGENTMEM_ROOT_DIR is required and canonicalised at startup — point it at the
# volume. Bind HTTP to all interfaces; the built-in default (127.0.0.1) is
# unreachable from outside the container.
ENV AGENTMEM_ROOT_DIR=/vault \
    AGENTMEM_TRANSPORT=http \
    AGENTMEM_HTTP_BIND=0.0.0.0:8000

VOLUME ["/vault"]
EXPOSE 8000

ENTRYPOINT ["/agentmem"]
