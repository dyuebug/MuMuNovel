#!/usr/bin/env bash
lifecycle_file="e2e-diagnostics/rust-backend-lifecycle.json"
if [ ! -f /tmp/rust-backend.pid ]; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "identity_status": "not_available",
  "cleanup_status": "not_started",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  exit 0
fi

backend_pid="$(cat /tmp/rust-backend.pid)"
case "$backend_pid" in
  ''|*[!0-9]*)
    cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid_raw": "${backend_pid}",
  "identity_status": "invalid_pid",
  "cleanup_status": "signal_refused",
  "termination_signal": null
}
EOF
    cat "$lifecycle_file"
    exit 1
    ;;
esac

if [ ! -f e2e-diagnostics/rust-backend-binary-path.txt ] || [ ! -f e2e-diagnostics/rust-backend-binary-sha256.txt ]; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "identity_status": "expected_identity_missing",
  "cleanup_status": "signal_refused",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  exit 1
fi

expected_binary_path="$(cat e2e-diagnostics/rust-backend-binary-path.txt)"
expected_binary_sha256="$(cat e2e-diagnostics/rust-backend-binary-sha256.txt)"
if ! kill -0 "$backend_pid" 2>/dev/null; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "expected_binary_path": "${expected_binary_path}",
  "expected_binary_sha256": "${expected_binary_sha256}",
  "identity_status": "process_unavailable",
  "cleanup_status": "already_exited",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  cat e2e-diagnostics/rust-backend.log || true
  exit 1
fi

if [ ! -e "/proc/$backend_pid/exe" ]; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "expected_binary_path": "${expected_binary_path}",
  "expected_binary_sha256": "${expected_binary_sha256}",
  "identity_status": "process_executable_unavailable",
  "cleanup_status": "signal_refused",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  exit 1
fi

observed_binary_path="$(readlink -f "/proc/$backend_pid/exe")"
observed_binary_sha256="$(sha256sum "/proc/$backend_pid/exe" | awk '{print $1}')"
if [ "$observed_binary_path" != "$expected_binary_path" ] || [ "$observed_binary_sha256" != "$expected_binary_sha256" ]; then
  cat > "$lifecycle_file" <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_target": "./target/debug/mumu-novel-backend",
  "process_pid": ${backend_pid},
  "expected_binary_path": "${expected_binary_path}",
  "observed_binary_path": "${observed_binary_path}",
  "expected_binary_sha256": "${expected_binary_sha256}",
  "observed_binary_sha256": "${observed_binary_sha256}",
  "identity_status": "mismatch",
  "cleanup_status": "signal_refused",
  "termination_signal": null
}
EOF
  cat "$lifecycle_file"
  echo "Refusing to signal PID $backend_pid because process identity does not match"
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
  "expected_binary_path": "${expected_binary_path}",
  "observed_binary_path": "${observed_binary_path}",
  "expected_binary_sha256": "${expected_binary_sha256}",
  "observed_binary_sha256": "${observed_binary_sha256}",
  "identity_status": "verified",
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
  "expected_binary_path": "${expected_binary_path}",
  "observed_binary_path": "${observed_binary_path}",
  "expected_binary_sha256": "${expected_binary_sha256}",
  "observed_binary_sha256": "${observed_binary_sha256}",
  "identity_status": "verified",
  "cleanup_status": "forced_kill",
  "termination_signal": "KILL"
}
EOF
cat "$lifecycle_file"
cat e2e-diagnostics/rust-backend.log || true
exit 1
