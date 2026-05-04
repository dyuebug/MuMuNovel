#!/usr/bin/env python
"""Lightweight HTTP benchmark harness for MuMuNovel backend.

Constraints:
- No product code changes.
- Designed for reproducible evidence capture (P50/P95/P99).

This script:
1) Logs in via /api/auth/local/login (cookie-based auth)
2) Optionally creates a project + chapter for DB workload
3) Runs request loops for selected endpoints and records latency stats

Usage examples:
  python backend/tools/perf_http_bench.py --base-url http://127.0.0.1:8000 \
    --username admin --password secret --scenario light --requests 200 --concurrency 10

  python backend/tools/perf_http_bench.py --base-url http://127.0.0.1:8000 \
    --username admin --password secret --scenario db --requests 100 --concurrency 5

  python backend/tools/perf_http_bench.py --base-url http://127.0.0.1:8000 \
    --username admin --password secret --scenario task-trigger --requests 50 --concurrency 5
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import random
import string
import time
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

import httpx
import numpy as np


@dataclass(frozen=True)
class RequestResult:
    name: str
    method: str
    path: str
    status_code: int
    latency_ms: float
    error: Optional[str] = None


def _now_ms() -> float:
    return time.perf_counter() * 1000.0


def _percentiles(values_ms: List[float]) -> Dict[str, float]:
    if not values_ms:
        return {"count": 0}
    arr = np.asarray(values_ms, dtype=float)
    return {
        "count": int(arr.size),
        "min_ms": float(np.min(arr)),
        "p50_ms": float(np.percentile(arr, 50)),
        "p95_ms": float(np.percentile(arr, 95)),
        "p99_ms": float(np.percentile(arr, 99)),
        "max_ms": float(np.max(arr)),
        "mean_ms": float(np.mean(arr)),
    }


def _rand_suffix(n: int = 6) -> str:
    return "".join(random.choice(string.ascii_lowercase + string.digits) for _ in range(n))


async def _login(client: httpx.AsyncClient, username: str, password: str) -> None:
    r = await client.post(
        "/api/auth/local/login",
        json={"username": username, "password": password},
    )
    r.raise_for_status()


async def _create_project(client: httpx.AsyncClient, title_prefix: str = "perf") -> str:
    r = await client.post(
        "/api/projects",
        json={
            "title": f"{title_prefix}-{_rand_suffix()}",
            "description": "perf bench",
            "genre": "test",
            "theme": "perf",
        },
    )
    r.raise_for_status()
    data = r.json()
    return str(data.get("id") or data.get("project_id") or "")


async def _create_chapter(client: httpx.AsyncClient, project_id: str) -> str:
    r = await client.post(
        "/api/chapters",
        json={
            "project_id": project_id,
            "title": "perf-ch1",
            "chapter_number": 1,
            "content": "hello",
        },
    )
    r.raise_for_status()
    data = r.json()
    return str(data.get("id") or data.get("chapter_id") or "")


async def _request_once(
    client: httpx.AsyncClient,
    sem: asyncio.Semaphore,
    name: str,
    method: str,
    path: str,
    json_body: Optional[Dict[str, Any]] = None,
    timeout_s: Optional[float] = None,
) -> RequestResult:
    async with sem:
        start = _now_ms()
        try:
            r = await client.request(method, path, json=json_body, timeout=timeout_s)
            latency = _now_ms() - start
            return RequestResult(
                name=name,
                method=method,
                path=path,
                status_code=r.status_code,
                latency_ms=latency,
                error=None,
            )
        except Exception as e:
            latency = _now_ms() - start
            return RequestResult(
                name=name,
                method=method,
                path=path,
                status_code=0,
                latency_ms=latency,
                error=f"{type(e).__name__}: {e}",
            )


async def _run_case(
    client: httpx.AsyncClient,
    name: str,
    method: str,
    path: str,
    *,
    requests: int,
    concurrency: int,
    json_body: Optional[Dict[str, Any]] = None,
    timeout_s: Optional[float] = None,
) -> Dict[str, Any]:
    sem = asyncio.Semaphore(concurrency)
    tasks = [
        asyncio.create_task(
            _request_once(
                client,
                sem,
                name=name,
                method=method,
                path=path,
                json_body=json_body,
                timeout_s=timeout_s,
            )
        )
        for _ in range(requests)
    ]
    results = await asyncio.gather(*tasks)

    latencies = [r.latency_ms for r in results if r.error is None]
    errors = [r for r in results if r.error is not None]
    status_hist: Dict[str, int] = {}
    for r in results:
        status_hist[str(r.status_code)] = status_hist.get(str(r.status_code), 0) + 1

    return {
        "case": {
            "name": name,
            "method": method,
            "path": path,
            "requests": requests,
            "concurrency": concurrency,
        },
        "latency": _percentiles(latencies),
        "status_codes": status_hist,
        "errors": [{"error": e.error, "latency_ms": e.latency_ms} for e in errors[:20]],
        "error_count": len(errors),
    }


def _scenario_plan(scenario: str, project_id: Optional[str], chapter_id: Optional[str]) -> List[Tuple[str, str, str, Optional[Dict[str, Any]], Optional[float]]]:
    if scenario == "light":
        return [
            ("health", "GET", "/health", None, 5.0),
            ("livez", "GET", "/livez", None, 5.0),
        ]
    if scenario == "readyz":
        return [
            ("readyz", "GET", "/readyz", None, 5.0),
            ("db_sessions", "GET", "/health/db-sessions", None, 5.0),
        ]
    if scenario == "db":
        if not project_id:
            raise ValueError("db scenario requires project_id")
        return [
            ("projects_list", "GET", "/api/projects", None, 10.0),
            ("chapters_list", "GET", f"/api/chapters/project/{project_id}", None, 10.0),
        ]
    if scenario == "write":
        return [
            (
                "project_create",
                "POST",
                "/api/projects",
                {"title": f"perf-write-{_rand_suffix()}", "description": "bench"},
                15.0,
            ),
        ]
    if scenario == "task-trigger":
        if not project_id:
            raise ValueError("task-trigger scenario requires project_id")
        return [
            (
                "bg_task_outline_generate",
                "POST",
                "/api/background-tasks",
                {
                    "task_type": "outline_generate",
                    "project_id": project_id,
                    "payload": {
                        "prompt": "Generate a short outline for benchmarking.",
                        "enable_mcp": False,
                    },
                    "execution_mode": "auto",
                },
                30.0,
            ),
        ]
    if scenario == "chapter-generate-trigger":
        if not chapter_id:
            raise ValueError("chapter-generate-trigger scenario requires chapter_id")
        return [
            (
                "chapter_generate_background",
                "POST",
                f"/api/chapters/{chapter_id}/generate-background",
                {},
                30.0,
            )
        ]
    raise ValueError(f"unknown scenario: {scenario}")


async def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--username", default=os.getenv("MUMU_USERNAME"))
    parser.add_argument("--password", default=os.getenv("MUMU_PASSWORD"))
    parser.add_argument("--scenario", required=True, choices=[
        "light",
        "readyz",
        "db",
        "write",
        "task-trigger",
        "chapter-generate-trigger",
    ])
    parser.add_argument("--requests", type=int, default=200)
    parser.add_argument("--concurrency", type=int, default=10)
    parser.add_argument("--seed", type=int, default=1337)
    parser.add_argument("--out", default="")
    parser.add_argument("--create-fixtures", action="store_true")
    args = parser.parse_args()

    random.seed(args.seed)

    if not args.username or not args.password:
        raise SystemExit("Missing --username/--password or MUMU_USERNAME/MUMU_PASSWORD")

    async with httpx.AsyncClient(
        base_url=args.base_url,
        follow_redirects=True,
        trust_env=False,
    ) as client:
        await _login(client, args.username, args.password)

        project_id: Optional[str] = None
        chapter_id: Optional[str] = None
        if args.create_fixtures or args.scenario in {"db", "task-trigger", "chapter-generate-trigger"}:
            project_id = await _create_project(client)
        if args.create_fixtures or args.scenario in {"chapter-generate-trigger"}:
            if not project_id:
                raise RuntimeError("project_id missing")
            chapter_id = await _create_chapter(client, project_id)

        plan = _scenario_plan(args.scenario, project_id, chapter_id)
        cases: List[Dict[str, Any]] = []
        for (name, method, path, body, timeout_s) in plan:
            cases.append(
                await _run_case(
                    client,
                    name=name,
                    method=method,
                    path=path,
                    requests=args.requests,
                    concurrency=args.concurrency,
                    json_body=body,
                    timeout_s=timeout_s,
                )
            )

        report = {
            "meta": {
                "base_url": args.base_url,
                "scenario": args.scenario,
                "requests": args.requests,
                "concurrency": args.concurrency,
                "seed": args.seed,
                "project_id": project_id,
                "chapter_id": chapter_id,
                "generated_at_ms": int(time.time() * 1000),
            },
            "cases": cases,
        }

        payload = json.dumps(report, ensure_ascii=False, indent=2)
        if args.out:
            with open(args.out, "w", encoding="utf-8") as f:
                f.write(payload)
        else:
            print(payload)

    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
