# MuMuNovel Rust Backend — Multi-stage build
FROM rust:1.86-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY src/ src/
RUN cargo build --release && cp target/release/mumu-novel-backend /app/server

# Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/server /app/server

EXPOSE 8001

HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -f http://localhost:8001/health || exit 1

CMD ["/app/server"]
