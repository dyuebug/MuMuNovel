releasez_url="http://127.0.0.1:8003/releasez"
releasez_body="e2e-diagnostics/releasez.json"
releasez_status_file="e2e-diagnostics/releasez-http-status.txt"
http_status="$(curl -sS -o "$releasez_body" -w "%{http_code}" "$releasez_url" || true)"
printf '%s\n' "$http_status" > "$releasez_status_file"
if [ "$http_status" = "200" ]; then
  echo "Rust release readiness contract verified"
  exit 0
fi
echo "Rust release readiness contract failed"
echo "Last /releasez HTTP status: $(cat "$releasez_status_file" 2>/dev/null || echo unavailable)"
echo "Last /releasez response:"
cat "$releasez_body" || true
echo "Last /readyz response:"
cat e2e-diagnostics/readyz.json || true
exit 1
