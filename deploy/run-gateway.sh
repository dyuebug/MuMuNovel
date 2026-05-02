#!/usr/bin/env bash
# MuMuNovel Strangler Fig — local dev launcher
# Starts Python:8000 + Rust:8001 + nginx:80
# Requires: nginx, python, cargo (Rust)

set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NGINX_CONF="$ROOT/deploy/nginx/mumunovel.conf"
NGINX_INCLUDE="$ROOT/deploy/nginx/conf.d"

echo "=== MuMuNovel Strangler Fig Gateway ==="

# 1. Start Rust backend
echo "[1/3] Rust backend → :8001"
cd "$ROOT/backend-rs"
cargo run --release &
RUST_PID=$!
echo "  PID: $RUST_PID"

# 2. Start Python backend
echo "[2/3] Python backend → :8000"
cd "$ROOT/backend"
source .venv/bin/activate 2>/dev/null || true
python -m uvicorn app.main:app --host 127.0.0.1 --port 8000 &
PYTHON_PID=$!
echo "  PID: $PYTHON_PID"

# 3. Start nginx
echo "[3/3] nginx → :80"
nginx -c "$NGINX_CONF" -t
nginx -c "$NGINX_CONF"
echo "  nginx started"

echo ""
echo "Gateway ready: http://localhost"
echo "  Rust   → http://localhost:8001 (77 CRUD endpoints)"
echo "  Python → http://localhost:8000 (127 endpoints, AI orchestration)"
echo ""
echo "Press Ctrl+C to stop all services"

cleanup() {
    echo "Shutting down..."
    nginx -s quit 2>/dev/null || true
    kill $PYTHON_PID 2>/dev/null || true
    kill $RUST_PID 2>/dev/null || true
    wait
    echo "Done."
}
trap cleanup EXIT INT TERM

wait
