#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
cleanup="$root/cleanup.sh"
summary="$root/lifecycle-harness-summary.txt"
: > "$summary"

prepare_case() {
  case_dir="$root/$1"
  mkdir -p "$case_dir/e2e-diagnostics"
  : > "$case_dir/e2e-diagnostics/rust-backend.log"
  cd "$case_dir"
}

run_terminated() {
  prepare_case terminated
  /usr/bin/sleep 30 &
  pid=$!
  expected_path="$(readlink -f "/proc/$pid/exe")"
  expected_sha="$(sha256sum "/proc/$pid/exe" | awk '{print $1}')"
  printf '%s\n' "$pid" > /tmp/rust-backend.pid
  printf '%s\n' "$expected_path" > e2e-diagnostics/rust-backend-binary-path.txt
  printf '%s\n' "$expected_sha" > e2e-diagnostics/rust-backend-binary-sha256.txt
  set +e
  bash "$cleanup" > cleanup.log 2>&1
  code=$?
  set -e
  printf 'terminated_exit=%s\n' "$code" >> "$summary"
  test "$code" -eq 0
  ! kill -0 "$pid" 2>/dev/null
  grep -q '"identity_status": "verified"' e2e-diagnostics/rust-backend-lifecycle.json
  grep -q '"cleanup_status": "terminated"' e2e-diagnostics/rust-backend-lifecycle.json
}

run_mismatch() {
  prepare_case identity-mismatch
  /usr/bin/sleep 30 &
  pid=$!
  expected_path="$(readlink -f /usr/bin/false)"
  expected_sha="$(sha256sum "$expected_path" | awk '{print $1}')"
  printf '%s\n' "$pid" > /tmp/rust-backend.pid
  printf '%s\n' "$expected_path" > e2e-diagnostics/rust-backend-binary-path.txt
  printf '%s\n' "$expected_sha" > e2e-diagnostics/rust-backend-binary-sha256.txt
  set +e
  bash "$cleanup" > cleanup.log 2>&1
  code=$?
  set -e
  printf 'identity_mismatch_exit=%s\n' "$code" >> "$summary"
  test "$code" -eq 1
  kill -0 "$pid" 2>/dev/null
  grep -q '"identity_status": "mismatch"' e2e-diagnostics/rust-backend-lifecycle.json
  grep -q '"cleanup_status": "signal_refused"' e2e-diagnostics/rust-backend-lifecycle.json
  kill -TERM "$pid"
  wait "$pid" 2>/dev/null || true
}

run_invalid_pid() {
  prepare_case invalid-pid
  printf '%s\n' 'not-a-pid' > /tmp/rust-backend.pid
  set +e
  bash "$cleanup" > cleanup.log 2>&1
  code=$?
  set -e
  printf 'invalid_pid_exit=%s\n' "$code" >> "$summary"
  test "$code" -eq 1
  grep -q '"identity_status": "invalid_pid"' e2e-diagnostics/rust-backend-lifecycle.json
  grep -q '"cleanup_status": "signal_refused"' e2e-diagnostics/rust-backend-lifecycle.json
}

run_already_exited() {
  prepare_case already-exited
  /usr/bin/true &
  pid=$!
  wait "$pid"
  printf '%s\n' "$pid" > /tmp/rust-backend.pid
  printf '%s\n' '/usr/bin/true' > e2e-diagnostics/rust-backend-binary-path.txt
  printf '%s\n' "$(sha256sum /usr/bin/true | awk '{print $1}')" > e2e-diagnostics/rust-backend-binary-sha256.txt
  set +e
  bash "$cleanup" > cleanup.log 2>&1
  code=$?
  set -e
  printf 'already_exited_exit=%s\n' "$code" >> "$summary"
  test "$code" -eq 1
  grep -q '"identity_status": "process_unavailable"' e2e-diagnostics/rust-backend-lifecycle.json
  grep -q '"cleanup_status": "already_exited"' e2e-diagnostics/rust-backend-lifecycle.json
}

run_terminated
run_mismatch
run_invalid_pid
run_already_exited
printf 'LIFECYCLE_HARNESS=PASS\n' >> "$summary"
cat "$summary"
