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
  "github_sha": "${GITHUB_SHA}",
  "github_run_id": "${GITHUB_RUN_ID}",
  "github_run_attempt": "${GITHUB_RUN_ATTEMPT}"
}
EOF
cat e2e-diagnostics/runner-success.json

