mkdir -p ../e2e-diagnostics
set +e
cargo run --locked -- release-readiness-preflight \
  > ../e2e-diagnostics/release-preflight.json \
  2> ../e2e-diagnostics/release-preflight-stderr.log
preflight_exit_code=$?
set -e
printf '%s\n' "$preflight_exit_code" > ../e2e-diagnostics/release-preflight-exit-code.txt
cat ../e2e-diagnostics/release-preflight.json
cat ../e2e-diagnostics/release-preflight-stderr.log >&2
if [ "$preflight_exit_code" -ne 0 ]; then
  echo "Rust release readiness preflight failed with exit code $preflight_exit_code"
  exit "$preflight_exit_code"
fi
