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
