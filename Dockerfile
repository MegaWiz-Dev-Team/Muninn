# ── Stage 1: Build ──
FROM rust:1.83-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY src/ src/
RUN cargo build --release

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/muninn /app/muninn

ENV PORT=8500
ENV DATABASE_PATH=/app/data/muninn.db
ENV RUST_LOG=muninn=info,tower_http=info

EXPOSE 8500

VOLUME ["/app/data"]

CMD ["/app/muninn"]
