#!/usr/bin/env bash
readyz_url="http://127.0.0.1:8003/readyz"
readyz_body="e2e-diagnostics/readyz.json"
readyz_status_file="e2e-diagnostics/readyz-http-status.txt"
for i in {1..60}; do
  http_status="$(curl -sS -o "$readyz_body" -w "%{http_code}" "$readyz_url" || true)"
  printf '%s\n' "$http_status" > "$readyz_status_file"
  if [ "$http_status" = "200" ]; then
    exit 0
  fi
  sleep 1
done
echo "Rust backend did not become ready in time"
echo "Last /readyz HTTP status: $(cat "$readyz_status_file" 2>/dev/null || echo unavailable)"
echo "Last /readyz response:"
cat "$readyz_body" || true
echo "Rust backend log:"
cat e2e-diagnostics/rust-backend.log || true
exit 1
