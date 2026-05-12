# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock ./
COPY crates/weld-core/Cargo.toml crates/weld-core/
COPY crates/weld-server/Cargo.toml crates/weld-server/

# Create stub lib/main so cargo can resolve the dependency graph
RUN mkdir -p crates/weld-core/src crates/weld-server/src && \
    echo "pub fn stub() {}" > crates/weld-core/src/lib.rs && \
    echo "fn main() {}" > crates/weld-server/src/main.rs

RUN cargo build --release -p weld-server 2>/dev/null || true

# Now copy real source and sqlx offline cache
COPY crates/ crates/
COPY .sqlx/ .sqlx/

# Touch main.rs so cargo rebuilds it (stub was already compiled above)
RUN touch crates/weld-server/src/main.rs crates/weld-core/src/lib.rs

ENV SQLX_OFFLINE=true

RUN cargo build --release -p weld-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/weld-server /usr/local/bin/weld-server

# Copy migrations so the server can run them on startup if desired
COPY crates/weld-server/migrations /app/migrations

EXPOSE 3000

CMD ["weld-server"]
