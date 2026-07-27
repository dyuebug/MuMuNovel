#!/usr/bin/env bash
set -o pipefail
set +e
npm run e2e -- e2e/auth.spec.ts e2e/background-task-pages.spec.ts \
  2>&1 | tee ../e2e-diagnostics/playwright-smoke.log
playwright_exit_code=$?
set -e
printf '%s\n' "$playwright_exit_code" > ../e2e-diagnostics/playwright-smoke-exit-code.txt
exit "$playwright_exit_code"
