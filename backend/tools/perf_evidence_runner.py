#!/usr/bin/env python
"""Run a fixed performance measurement checklist and emit evidence artifacts.

This script runs multiple scenarios via perf_http_bench and writes JSON reports
into an output directory.

It is intentionally simple and local-first.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from pathlib import Path


def _run(cmd: list[str]) -> str:
    proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"Command failed ({proc.returncode}): {' '.join(cmd)}\n{proc.stdout}")
    return proc.stdout


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--username", default=os.getenv("MUMU_USERNAME", ""))
    parser.add_argument("--password", default=os.getenv("MUMU_PASSWORD", ""))
    parser.add_argument("--out-dir", default="backend/tools/perf_artifacts")
    parser.add_argument("--requests", type=int, default=200)
    parser.add_argument("--concurrency", type=int, default=10)
    parser.add_argument("--seed", type=int, default=1337)
    args = parser.parse_args()

    if not args.username or not args.password:
        raise SystemExit("Missing --username/--password or env MUMU_USERNAME/MUMU_PASSWORD")

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    scenarios = [
        "light",
        "readyz",
        "db",
        "task-trigger",
    ]

    meta = {
        "base_url": args.base_url,
        "requests": args.requests,
        "concurrency": args.concurrency,
        "seed": args.seed,
        "scenarios": scenarios,
        "started_at_ms": int(time.time() * 1000),
    }
    (out_dir / "meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding="utf-8")

    for scenario in scenarios:
        out_file = out_dir / f"{scenario}.json"
        cmd = [
            "python",
            "backend/tools/perf_http_bench.py",
            "--base-url",
            args.base_url,
            "--username",
            args.username,
            "--password",
            args.password,
            "--scenario",
            scenario,
            "--requests",
            str(args.requests),
            "--concurrency",
            str(args.concurrency),
            "--seed",
            str(args.seed),
            "--out",
            str(out_file),
        ]
        _run(cmd)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
