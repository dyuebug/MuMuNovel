# -*- coding: utf-8 -*-
"""Smoke test for chapter batch terminal status semantics."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import uuid
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

DEFAULT_BASE_URL = 'http://127.0.0.1:8003'
DEFAULT_PG_CONTAINER = 'mumunovel-postgres-new'
DEFAULT_PG_DB = 'mumuai_novel'
DEFAULT_PG_USER = 'mumuai'
DEFAULT_PG_PASSWORD = '123456'


@dataclass
class ScenarioResult:
    name: str
    task_id: str
    status: str
    terminal_reason: Optional[str]
    terminal_label: Optional[str]
    review_required: bool
    can_resume: bool
    expected_terminal_label: str
    ok: bool
    response: Dict[str, Any]


class SmokeFailure(RuntimeError):
    """Raised when smoke validation fails."""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description='Validate batch terminal status semantics against a live app')
    parser.add_argument('--base-url', default=DEFAULT_BASE_URL, help='Application base URL')
    parser.add_argument('--ready-path', default='/readyz', help='Readiness probe path')
    parser.add_argument('--pg-container', default=DEFAULT_PG_CONTAINER, help='Postgres container name')
    parser.add_argument('--pg-db', default=DEFAULT_PG_DB, help='Postgres database name')
    parser.add_argument('--pg-user', default=DEFAULT_PG_USER, help='Postgres user name')
    parser.add_argument('--pg-password', default=DEFAULT_PG_PASSWORD, help='Postgres password')
    parser.add_argument('--ready-timeout', type=float, default=30.0, help='Total seconds to wait for readyz')
    parser.add_argument('--http-timeout', type=float, default=10.0, help='Single HTTP request timeout in seconds')
    parser.add_argument('--output', type=Path, help='Optional JSON output path')
    return parser


def http_get_json(url: str, *, timeout: float, cookie_user_id: Optional[str] = None) -> Dict[str, Any]:
    headers = {'Accept': 'application/json'}
    if cookie_user_id:
        headers['Cookie'] = f'user_id={cookie_user_id}'
    request = Request(url, headers=headers, method='GET')
    try:
        with urlopen(request, timeout=timeout) as response:
            body = response.read().decode('utf-8')
            return json.loads(body)
    except HTTPError as exc:
        body = exc.read().decode('utf-8', errors='replace')
        raise SmokeFailure(
            f"HTTP {exc.code} request failed: {url}\n"
            f"response: {body}"
        ) from exc
    except URLError as exc:
        raise SmokeFailure(
            f"Request failed: {url}\n"
            f"error: {exc}"
        ) from exc
    except json.JSONDecodeError as exc:
        raise SmokeFailure(
            f"Response is not valid JSON: {url}\n"
            f"error: {exc}"
        ) from exc


def wait_until_ready(base_url: str, ready_path: str, timeout_seconds: float, http_timeout: float) -> Dict[str, Any]:
    deadline = time.time() + timeout_seconds
    ready_url = f"{base_url.rstrip('/')}{ready_path}"
    last_error: Optional[Exception] = None
    while time.time() < deadline:
        try:
            payload = http_get_json(ready_url, timeout=http_timeout)
            if payload.get('status') == 'ready':
                return payload
            last_error = SmokeFailure(f'ready endpoint returned non-ready payload: {payload}')
        except Exception as exc:  # noqa: BLE001
            last_error = exc
        time.sleep(2)
    raise SmokeFailure(f'ready probe timed out: {ready_url}; last_error={last_error}')


def run_psql(sql: str, *, container: str, db: str, user: str, password: str) -> str:
    command = [
        'docker',
        'exec',
        '-e',
        f'PGPASSWORD={password}',
        container,
        'psql',
        '-U',
        user,
        '-d',
        db,
        '-v',
        'ON_ERROR_STOP=1',
        '-At',
        '-c',
        sql,
    ]
    completed = subprocess.run(
        command,
        text=True,
        capture_output=True,
        encoding='utf-8',
        errors='replace',
        check=False,
    )
    if completed.returncode != 0:
        raise SmokeFailure(
            'psql command failed\n'
            f'command={command}\n'
            f'stdout={completed.stdout}\n'
            f'stderr={completed.stderr}'
        )
    return completed.stdout.strip()


def sql_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def sql_json(value: Any) -> str:
    return sql_quote(json.dumps(value, ensure_ascii=False)) + '::json'


def build_failed_chapter(*, chapter_id: str, chapter_number: int, title: str, label: Optional[str]) -> Dict[str, Any]:
    payload: Dict[str, Any] = {
        'chapter_id': chapter_id,
        'chapter_number': chapter_number,
        'title': title,
        'error': 'quality gate blocked; manual review required',
        'retry_count': 2,
        'phase': 'quality_blocked',
        'quality_gate_status': 'blocked',
        'quality_gate_decision': 'manual_review',
        'quality_gate_failed_metrics': ['Conflict chain'],
    }
    if label is not None:
        payload['quality_gate_label'] = label
    return payload


def insert_smoke_records(*, container: str, db: str, user: str, password: str, smoke_user_id: str, scenarios: List[Dict[str, Any]]) -> None:
    user_sql = f"""
INSERT INTO users (user_id, username, display_name, avatar_url, trust_level, is_admin, linuxdo_id)
VALUES ({sql_quote(smoke_user_id)}, {sql_quote(smoke_user_id)}, {sql_quote('Smoke User')}, NULL, 1, FALSE, {sql_quote(smoke_user_id)})
ON CONFLICT (user_id) DO UPDATE SET
  username = EXCLUDED.username,
  display_name = EXCLUDED.display_name,
  avatar_url = EXCLUDED.avatar_url,
  trust_level = EXCLUDED.trust_level,
  is_admin = EXCLUDED.is_admin,
  linuxdo_id = EXCLUDED.linuxdo_id;
"""
    run_psql(user_sql, container=container, db=db, user=user, password=password)

    values_sql: List[str] = []
    for scenario in scenarios:
        values_sql.append(
            '('
            + ', '.join([
                sql_quote(scenario['task_id']),
                sql_quote(scenario['project_id']),
                sql_quote(smoke_user_id),
                '2',
                '1',
                sql_json([scenario['chapter_id']]),
                '3000',
                'FALSE',
                sql_quote('failed'),
                '1',
                '0',
                sql_json([scenario['failed_chapter']]),
                'NULL',
                '2',
                '0',
                '2',
                'NOW()',
                'NOW()',
                sql_quote('chapter 2 needs manual review'),
            ])
            + ')'
        )

    task_sql = (
        """
INSERT INTO batch_generation_tasks (
  id,
  project_id,
  user_id,
  start_chapter_number,
  chapter_count,
  chapter_ids,
  target_word_count,
  enable_analysis,
  status,
  total_chapters,
  completed_chapters,
  failed_chapters,
  current_chapter_id,
  current_chapter_number,
  current_retry_count,
  max_retries,
  started_at,
  completed_at,
  error_message
)
VALUES
"""
        + ',\n'.join(values_sql)
        + '\nON CONFLICT (id) DO NOTHING;'
    )
    run_psql(task_sql, container=container, db=db, user=user, password=password)


def cleanup_smoke_records(*, container: str, db: str, user: str, password: str, smoke_user_id: str, task_ids: List[str]) -> None:
    sql = (
        'DELETE FROM batch_generation_tasks WHERE id IN '
        f"({', '.join(sql_quote(task_id) for task_id in task_ids)});\n"
        f"DELETE FROM users WHERE user_id = {sql_quote(smoke_user_id)};"
    )
    run_psql(sql, container=container, db=db, user=user, password=password)


def verify_scenario(*, base_url: str, scenario_name: str, task_id: str, smoke_user_id: str, expected_terminal_label: str, http_timeout: float) -> ScenarioResult:
    payload = http_get_json(
        f"{base_url.rstrip('/')}/api/chapters/batch-generate/{task_id}/status",
        timeout=http_timeout,
        cookie_user_id=smoke_user_id,
    )
    ok = (
        payload.get('status') == 'failed'
        and payload.get('terminal_reason') == 'manual_review'
        and payload.get('terminal_label') == expected_terminal_label
        and payload.get('review_required') is True
        and payload.get('can_resume') is True
    )
    if not ok:
        raise SmokeFailure(
            'Status payload does not match expectation\n'
            f'task_id={task_id}\n'
            f'expected_terminal_label={expected_terminal_label}\n'
            f'payload={json.dumps(payload, ensure_ascii=False, indent=2)}'
        )
    return ScenarioResult(
        name=scenario_name,
        task_id=task_id,
        status=str(payload.get('status')),
        terminal_reason=payload.get('terminal_reason'),
        terminal_label=payload.get('terminal_label'),
        review_required=bool(payload.get('review_required')),
        can_resume=bool(payload.get('can_resume')),
        expected_terminal_label=expected_terminal_label,
        ok=True,
        response=payload,
    )


def main() -> int:
    args = build_parser().parse_args()
    ready_payload = wait_until_ready(args.base_url, args.ready_path, args.ready_timeout, args.http_timeout)

    smoke_user_id = f'smoke_user_{uuid.uuid4().hex[:12]}'
    scenarios = [
        {
            'name': 'with_label',
            'task_id': str(uuid.uuid4()),
            'project_id': str(uuid.uuid4()),
            'chapter_id': str(uuid.uuid4()),
            'failed_chapter': build_failed_chapter(
                chapter_id=str(uuid.uuid4()),
                chapter_number=2,
                title='chapter-2',
                label='manual review',
            ),
            'expected_terminal_label': 'manual review',
        },
        {
            'name': 'without_label',
            'task_id': str(uuid.uuid4()),
            'project_id': str(uuid.uuid4()),
            'chapter_id': str(uuid.uuid4()),
            'failed_chapter': build_failed_chapter(
                chapter_id=str(uuid.uuid4()),
                chapter_number=2,
                title='chapter-2',
                label=None,
            ),
            'expected_terminal_label': '\u9700\u4eba\u5de5\u590d\u6838',
        },
    ]
    task_ids = [scenario['task_id'] for scenario in scenarios]

    try:
        insert_smoke_records(
            container=args.pg_container,
            db=args.pg_db,
            user=args.pg_user,
            password=args.pg_password,
            smoke_user_id=smoke_user_id,
            scenarios=scenarios,
        )
        results = [
            verify_scenario(
                base_url=args.base_url,
                scenario_name=scenario['name'],
                task_id=scenario['task_id'],
                smoke_user_id=smoke_user_id,
                expected_terminal_label=scenario['expected_terminal_label'],
                http_timeout=args.http_timeout,
            )
            for scenario in scenarios
        ]
    finally:
        try:
            cleanup_smoke_records(
                container=args.pg_container,
                db=args.pg_db,
                user=args.pg_user,
                password=args.pg_password,
                smoke_user_id=smoke_user_id,
                task_ids=task_ids,
            )
        except Exception as cleanup_error:  # noqa: BLE001
            print(f'WARN: failed to cleanup smoke data: {cleanup_error}', file=sys.stderr)

    summary = {
        'base_url': args.base_url,
        'ready': ready_payload,
        'scenarios': [asdict(item) for item in results],
    }
    summary_json = json.dumps(summary, ensure_ascii=False, indent=2)
    print(summary_json)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(summary_json + '\n', encoding='utf-8')
        print(f'\nSummary written to: {args.output}')

    return 0


if __name__ == '__main__':
    raise SystemExit(main())
