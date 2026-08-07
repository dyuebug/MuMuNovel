#!/usr/bin/env bash
mkdir -p ../e2e-diagnostics
cargo build --locked
binary_path="$(realpath ./target/debug/mumu-novel-backend)"
binary_sha256="$(sha256sum "$binary_path" | awk '{print $1}')"
printf '%s\n' "$binary_path" > ../e2e-diagnostics/rust-backend-binary-path.txt
printf '%s\n' "$binary_sha256" > ../e2e-diagnostics/rust-backend-binary-sha256.txt

nohup "$binary_path" \
  > ../e2e-diagnostics/rust-backend.log 2>&1 &
backend_pid=$!
printf '%s\n' "$backend_pid" > /tmp/rust-backend.pid
printf '%s\n' "$backend_pid" > ../e2e-diagnostics/rust-backend.pid

for _ in {1..10}; do
  if [ -e "/proc/$backend_pid/exe" ]; then
    break
  fi
  if ! kill -0 "$backend_pid" 2>/dev/null; then
    break
  fi
  sleep 0.1
done

if [ ! -e "/proc/$backend_pid/exe" ]; then
  cat > ../e2e-diagnostics/rust-backend-identity.json <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_pid": ${backend_pid},
  "expected_binary_path": "${binary_path}",
  "expected_binary_sha256": "${binary_sha256}",
  "identity_status": "process_executable_unavailable"
}
EOF
  cat ../e2e-diagnostics/rust-backend-identity.json
  cat ../e2e-diagnostics/rust-backend.log || true
  exit 1
fi

observed_binary_path="$(readlink -f "/proc/$backend_pid/exe")"
observed_binary_sha256="$(sha256sum "/proc/$backend_pid/exe" | awk '{print $1}')"
identity_status="verified"
if [ "$observed_binary_path" != "$binary_path" ] || [ "$observed_binary_sha256" != "$binary_sha256" ]; then
  identity_status="mismatch"
fi
cat > ../e2e-diagnostics/rust-backend-identity.json <<EOF
{
  "schema_version": 1,
  "runtime_owner": "backend-rs",
  "process_pid": ${backend_pid},
  "expected_binary_path": "${binary_path}",
  "observed_binary_path": "${observed_binary_path}",
  "expected_binary_sha256": "${binary_sha256}",
  "observed_binary_sha256": "${observed_binary_sha256}",
  "identity_status": "${identity_status}"
}
EOF
cat ../e2e-diagnostics/rust-backend-identity.json
if [ "$identity_status" != "verified" ]; then
  echo "Rust backend process identity did not match the built binary"
  exit 1
fi
