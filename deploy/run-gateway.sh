#!/usr/bin/env bash
# MuMuNovel Strangler Fig — local dev launcher
# Starts Rust:8001 + nginx:80
# Requires: nginx, cargo (Rust)

set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NGINX_CONF="$ROOT/deploy/nginx/mumunovel.conf"
NGINX_INCLUDE="$ROOT/deploy/nginx/conf.d"

echo "=== MuMuNovel Strangler Fig Gateway ==="

# 1. Start Rust backend
echo "[1/2] Rust backend → :8001"
cd "$ROOT/backend-rs"
cargo run --release &
RUST_PID=$!
echo "  PID: $RUST_PID"

# 2. Start nginx
echo "[2/2] nginx → :80"
nginx -c "$NGINX_CONF" -t
nginx -c "$NGINX_CONF"
echo "  nginx started"

echo ""
echo "Gateway ready: http://localhost"
echo "  Rust   → http://localhost:8001"
echo "  Python → db-migrator only; no local runtime backend"
echo ""
echo "Press Ctrl+C to stop all services"

cleanup() {
    echo "Shutting down..."
    nginx -s quit 2>/dev/null || true
    kill $RUST_PID 2>/dev/null || true
    wait
    echo "Done."
}
trap cleanup EXIT INT TERM

wait
