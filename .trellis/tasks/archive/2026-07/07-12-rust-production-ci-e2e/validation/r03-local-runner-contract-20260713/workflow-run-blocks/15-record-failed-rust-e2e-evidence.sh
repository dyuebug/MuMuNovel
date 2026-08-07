#!/usr/bin/env bash
cat > e2e-diagnostics/runner-failure.json <<EOF
{
  "schema_version": 1,
  "evidence_status": "failed",
  "runtime_owner": "backend-rs",
  "diagnostics_directory": "e2e-diagnostics",
  "github_sha": "${GITHUB_SHA}",
  "github_run_id": "${GITHUB_RUN_ID}",
  "github_run_attempt": "${GITHUB_RUN_ATTEMPT}"
}
EOF
cat e2e-diagnostics/runner-failure.json
