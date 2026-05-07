# MuMuNovel Rust Backend — Multi-stage build
FROM rust:1.88-slim-bookworm AS builder

# Install build deps + mold linker (much lower memory than GNU ld)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev mold clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

# Dummy build to cache deps
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    CARGO_BUILD_JOBS=2 \
    CARGO_INCREMENTAL=false \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold -C codegen-units=256 -C debuginfo=0 -C opt-level=1" \
    cargo build --release 2>/dev/null; true
RUN rm -rf src

COPY src/ src/
# Ensure cargo detects changed source files after dummy build removal
RUN find src -type f -exec touch {} +
RUN CARGO_BUILD_JOBS=2 \
    CARGO_INCREMENTAL=false \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold -C codegen-units=256 -C debuginfo=0 -C opt-level=1" \
    cargo build --release && cp target/release/mumu-novel-backend /app/server

# Runtime
FROM rust:1.88-slim-bookworm AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/server /app/server

EXPOSE 8001

HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -f http://localhost:8001/health || exit 1

CMD ["/app/server"]
