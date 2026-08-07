mkdir -p ../e2e-diagnostics
set +e
cargo run --locked -- migration-executor \
  > ../e2e-diagnostics/migration-executor.json \
  2> ../e2e-diagnostics/migration-executor-stderr.log
migration_exit_code=$?
set -e
printf '%s\n' "$migration_exit_code" > ../e2e-diagnostics/migration-executor-exit-code.txt
cat ../e2e-diagnostics/migration-executor.json
cat ../e2e-diagnostics/migration-executor-stderr.log >&2
if [ "$migration_exit_code" -ne 0 ]; then
  echo "Rust migration executor failed with exit code $migration_exit_code"
  exit "$migration_exit_code"
fi