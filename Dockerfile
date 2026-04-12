# ── Multi-stage Dockerfile with cargo-chef for dependency caching ──
# Produces a fully static musl binary running on scratch.
#
# Build:  docker build -t demo-cli .
#         docker build --build-arg BIN_NAME=demo-cli --build-arg FEATURES=some_feature -t demo-cli .
# Run:    docker run --rm demo-cli

ARG RUST_VERSION=1.77
ARG BIN_NAME=demo-cli

# ── Stage 1: chef — base image with cargo-chef ────────────────────
FROM rust:${RUST_VERSION}-alpine AS chef
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static && \
    cargo install cargo-chef --locked
WORKDIR /app

# ── Stage 2: planner — compute dependency recipe ──────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: builder — cook deps then build ───────────────────────
FROM chef AS builder

ARG BIN_NAME=demo-cli
ARG FEATURES=
ARG PROFILE=release

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook \
    --recipe-path recipe.json \
    --profile ${PROFILE} \
    ${FEATURES:+--features ${FEATURES}}

COPY . .
RUN cargo build \
    --profile ${PROFILE} \
    --bin ${BIN_NAME} \
    ${FEATURES:+--features ${FEATURES}} && \
    # Move binary to a predictable location regardless of profile name
    cp target/$([ "${PROFILE}" = "dev" ] && echo "debug" || echo "${PROFILE}")/${BIN_NAME} /app/binary

# ── Stage 4: runtime — minimal scratch image ─────────────────────
FROM scratch

ARG BIN_NAME=demo-cli

LABEL org.opencontainers.image.title="${BIN_NAME}" \
      org.opencontainers.image.source="https://github.com/OWNER/REPO" \
      org.opencontainers.image.description="Rust comprehensive template"

COPY --from=builder /app/binary /usr/local/bin/app
# Run as non-root (numeric UID required for scratch)
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/app"]
