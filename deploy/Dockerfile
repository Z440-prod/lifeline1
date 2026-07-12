# ─────────────────────────────────────────────────────────────────────────────
# Lifeline / Antigravity engine — production container
#
# One binary serves the API *and* the web app. Migrations are compiled into the
# binary (sqlx::migrate!) and run automatically on first boot against whatever
# Postgres you point DATABASE at, so there is no separate migration step.
#
# Build:  docker build -f deploy/Dockerfile -t lifeline .
# Run:    docker run -p 8443:8443 --env-file deploy/.env.production lifeline
# ─────────────────────────────────────────────────────────────────────────────

# ---- Stage 1: build ---------------------------------------------------------
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

# System deps for building (openssl/pkg-config are commonly needed by crates).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Cache dependencies first: copy manifests, build a stub, then the real source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release || true
RUN rm -rf src

# Now the real source + everything migrate!() and the build need.
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bin antigravity

# ---- Stage 2: runtime -------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 lifeline

# Runtime needs: the binary, the config dir, and the web app it serves.
COPY --from=builder /build/target/release/antigravity /usr/local/bin/antigravity
COPY config ./config
COPY web ./web

USER lifeline
EXPOSE 8443

# The platform (Fly/Render) terminates TLS at the edge; the app serves HTTP
# internally on 8443. Health check hits the built-in /health endpoint.
HEALTHCHECK --interval=30s --timeout=4s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8443/health || exit 1

CMD ["antigravity"]
