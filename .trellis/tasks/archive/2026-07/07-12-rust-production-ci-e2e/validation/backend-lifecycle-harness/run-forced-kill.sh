#!/usr/bin/env bash
set +e
cd "$(dirname "$0")"
rm -rf e2e-diagnostics
mkdir -p e2e-diagnostics
: > e2e-diagnostics/rust-backend.log
bash -c 'trap "" TERM; while :; do sleep 1; done' &
pid=$!
printf '%s
' "$pid" > /tmp/rust-backend.pid
bash cleanup.sh
code=$?
exit "$code"
