# Start Rust backend
mkdir -p ../e2e-diagnostics
cargo build --locked
nohup ./target/debug/mumu-novel-backend \
  > ../e2e-diagnostics/rust-backend.log 2>&1 &
backend_pid=$!
printf '%s\n' "$backend_pid" > /tmp/rust-backend.pid
printf '%s\n' "$backend_pid" > ../e2e-diagnostics/rust-backend.pid


# Stop Rust backend and record lifecycle
lifecycle_file="e2e-diagnostics/rust-backend-lifecycle.json"
if [ ! -f /tmp/rust-backend.pid ]; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "cleanup_status": "not_started",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  exit 0
fi

backend_pid="$(cat /tmp/rust-backend.pid)"
if ! kill -0 "$backend_pid" 2>/dev/null; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "cleanup_status": "already_exited",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  cat e2e-diagnostics/rust-backend.log || true
  exit 1
fi

kill -TERM "$backend_pid"
for _ in {1..10}; do
  if ! kill -0 "$backend_pid" 2>/dev/null; then
    cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "cleanup_status": "terminated",
  "termination_signal": "TERM"
}
EOF
    cat "$lifecycle_file"
    cat e2e-diagnostics/rust-backend.log || true
    exit 0
  fi
  sleep 1
done

kill -KILL "$backend_pid" || true
cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "cleanup_status": "forced_kill",
  "termination_signal": "KILL"
}
EOF
cat "$lifecycle_file"
cat e2e-diagnostics/rust-backend.log || true
exit 1


# Record successful Rust E2E evidence
cat > e2e-diagnostics/runner-success.json <<EOF
{
  "schema_version": 1,
  "evidence_status": "passed",
  "runtime_owner": "backend-rs",
  "database": "postgresql",
  "migration_executor": "passed",
  "release_readiness_preflight": "passed",
  "readyz": "passed",
  "releasez": "passed",
  "playwright_smoke": "passed",
  "backend_lifecycle": "passed",
  "github_sha": "${GITHUB_SHA}",
  "github_run_id": "${GITHUB_RUN_ID}",
  "github_run_attempt": "${GITHUB_RUN_ATTEMPT}"
}
EOF
cat e2e-diagnostics/runner-success.json

