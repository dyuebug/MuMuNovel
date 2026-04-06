# -*- coding: utf-8 -*-
"""Smoke test for live settings probes via real login and saved settings."""

from __future__ import annotations

import argparse
import http.cookiejar
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional


DEFAULT_BASE_URL = 'http://127.0.0.1:8003'
DEFAULT_READY_PATH = '/readyz'
DEFAULT_READY_TIMEOUT = 45.0
DEFAULT_HTTP_TIMEOUT = 15.0
DEFAULT_PROBE_TIMEOUT = 30.0


class SmokeFailure(RuntimeError):
    """Raised when a smoke assertion fails."""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description='Validate /api/settings probes against the live application using saved settings'
    )
    parser.add_argument('--base-url', default=DEFAULT_BASE_URL, help='Application base URL')
    parser.add_argument('--ready-path', default=DEFAULT_READY_PATH, help='Readiness endpoint path')
    parser.add_argument('--env-file', type=Path, help='Optional .env file path override')
    parser.add_argument('--ready-timeout', type=float, default=DEFAULT_READY_TIMEOUT, help='Total seconds to wait for ready state')
    parser.add_argument('--http-timeout', type=float, default=DEFAULT_HTTP_TIMEOUT, help='Timeout for login/settings requests in seconds')
    parser.add_argument('--probe-timeout', type=float, default=DEFAULT_PROBE_TIMEOUT, help='Timeout for probe requests in seconds')
    parser.add_argument('--output', type=Path, help='Optional JSON output path')
    return parser


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def resolve_env_file(cli_env_file: Optional[Path]) -> Path:
    if cli_env_file:
        return cli_env_file.resolve()
    return repo_root() / '.env'


def load_env_map(env_file: Path) -> Dict[str, str]:
    if not env_file.exists():
        raise SmokeFailure(f'.env file not found: {env_file}')

    values: Dict[str, str] = {}
    for raw_line in env_file.read_text(encoding='utf-8').splitlines():
        line = raw_line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = raw_line.split('=', 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def get_secret(name: str, env_map: Dict[str, str]) -> str:
    value = os.getenv(name) or env_map.get(name)
    if not value:
        raise SmokeFailure(f'missing required config: {name}')
    return value


def mask_secret(value: str) -> str:
    if not value:
        return ''
    if len(value) <= 8:
        return '*' * len(value)
    return f'{value[:4]}***{value[-4:]}'


def build_opener(cookie_jar: http.cookiejar.CookieJar) -> urllib.request.OpenerDirector:
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookie_jar))
    opener.addheaders = [
        ('User-Agent', 'codex-settings-probe-smoke/1.0'),
        ('Accept', 'application/json, text/event-stream, text/plain, */*'),
    ]
    return opener


def decode_body(raw: bytes, content_type: str) -> Any:
    text = raw.decode('utf-8', errors='replace')
    if 'application/json' in content_type:
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text
    return text


def request(
    opener: urllib.request.OpenerDirector,
    *,
    base_url: str,
    method: str,
    path: str,
    payload: Optional[Dict[str, Any]] = None,
    timeout: float,
) -> Dict[str, Any]:
    url = f"{base_url.rstrip('/')}{path}"
    headers: Dict[str, str] = {}
    data = None
    if payload is not None:
        data = json.dumps(payload, ensure_ascii=False).encode('utf-8')
        headers['Content-Type'] = 'application/json'

    req = urllib.request.Request(url, data=data, headers=headers, method=method.upper())
    started = time.perf_counter()
    try:
        with opener.open(req, timeout=timeout) as response:
            raw = response.read()
            status_code = response.getcode()
            content_type = response.headers.get('Content-Type', '')
    except urllib.error.HTTPError as exc:
        raw = exc.read()
        status_code = exc.code
        content_type = exc.headers.get('Content-Type', '')
    except urllib.error.URLError as exc:
        raise SmokeFailure(f'request failed: {url}\nerror={exc}') from exc

    elapsed_ms = round((time.perf_counter() - started) * 1000, 2)
    body = decode_body(raw, content_type)
    return {
        'url': url,
        'status_code': status_code,
        'elapsed_ms': elapsed_ms,
        'content_type': content_type,
        'body': body,
    }


def wait_until_ready(
    opener: urllib.request.OpenerDirector,
    *,
    base_url: str,
    ready_path: str,
    ready_timeout: float,
    http_timeout: float,
) -> Dict[str, Any]:
    deadline = time.time() + ready_timeout
    last_payload: Optional[Dict[str, Any]] = None
    last_error: Optional[str] = None
    while time.time() < deadline:
        try:
            response = request(
                opener,
                base_url=base_url,
                method='GET',
                path=ready_path,
                timeout=http_timeout,
            )
            last_payload = response
            body = response.get('body')
            if response['status_code'] == 200 and isinstance(body, dict) and body.get('status') == 'ready':
                return response
            last_error = f'non-ready response: {body}'
        except SmokeFailure as exc:
            last_error = str(exc)
        time.sleep(2)

    raise SmokeFailure(
        f'ready probe timed out: {base_url.rstrip("/")}{ready_path}; '
        f'last_error={last_error}; last_payload={last_payload}'
    )


def assert_json_response(response: Dict[str, Any], *, label: str) -> Dict[str, Any]:
    body = response.get('body')
    if not isinstance(body, dict):
        raise SmokeFailure(
            f'{label} did not return JSON\n'
            f'status={response.get("status_code")}\n'
            f'content_type={response.get("content_type")}\n'
            f'body_preview={str(body)[:400]}'
        )
    return body


def ensure_login_cookies(cookie_jar: http.cookiejar.CookieJar) -> None:
    cookie_names = {cookie.name for cookie in cookie_jar}
    if 'user_id' not in cookie_names:
        raise SmokeFailure(f'login did not produce required cookie; cookies={sorted(cookie_names)}')


def build_probe_payload(settings_body: Dict[str, Any]) -> Dict[str, Any]:
    provider = settings_body.get('api_provider') or settings_body.get('provider_type') or 'openai'
    payload: Dict[str, Any] = {
        'api_key': settings_body.get('api_key') or '',
        'api_base_url': settings_body.get('api_base_url') or '',
        'provider': provider,
        'llm_model': settings_body.get('llm_model') or '',
        'temperature': settings_body.get('temperature'),
        'max_tokens': 1024,
    }
    backup_urls = settings_body.get('api_backup_urls')
    if isinstance(backup_urls, list):
        payload['api_backup_urls'] = backup_urls
    fallback_strategy = settings_body.get('fallback_strategy')
    if isinstance(fallback_strategy, str) and fallback_strategy.strip():
        payload['fallback_strategy'] = fallback_strategy.strip()

    required_fields = ['api_key', 'api_base_url', 'provider', 'llm_model']
    missing = [field for field in required_fields if not payload.get(field)]
    if missing:
        raise SmokeFailure(f'saved settings are incomplete; missing fields={missing}')
    return payload


def summarize_probe_response(response_body: Dict[str, Any]) -> Dict[str, Any]:
    details = response_body.get('details') if isinstance(response_body.get('details'), dict) else {}
    diagnostics = details.get('transport_diagnostics') if isinstance(details, dict) else {}
    return {
        'success': response_body.get('success'),
        'supported': response_body.get('supported'),
        'message': response_body.get('message'),
        'response_time_ms': response_body.get('response_time_ms'),
        'error_type': response_body.get('error_type'),
        'error': response_body.get('error'),
        'transport_summary': diagnostics.get('summary') if isinstance(diagnostics, dict) else None,
        'suggestions': response_body.get('suggestions'),
    }


def main() -> int:
    args = build_parser().parse_args()
    env_file = resolve_env_file(args.env_file)
    env_map = load_env_map(env_file)
    username = get_secret('LOCAL_AUTH_USERNAME', env_map)
    password = get_secret('LOCAL_AUTH_PASSWORD', env_map)

    cookie_jar = http.cookiejar.CookieJar()
    opener = build_opener(cookie_jar)
    summary: Dict[str, Any] = {
        'base_url': args.base_url,
        'env_file': str(env_file),
        'started_at': time.strftime('%Y-%m-%dT%H:%M:%S'),
    }

    try:
        ready_response = wait_until_ready(
            opener,
            base_url=args.base_url,
            ready_path=args.ready_path,
            ready_timeout=args.ready_timeout,
            http_timeout=args.http_timeout,
        )
        summary['readyz'] = {
            'status_code': ready_response['status_code'],
            'elapsed_ms': ready_response['elapsed_ms'],
            'body': ready_response['body'],
        }

        login_response = request(
            opener,
            base_url=args.base_url,
            method='POST',
            path='/api/auth/local/login',
            payload={'username': username, 'password': password},
            timeout=args.http_timeout,
        )
        login_body = assert_json_response(login_response, label='local login')
        if login_response['status_code'] != 200 or login_body.get('success') is not True:
            raise SmokeFailure(f'login failed: {login_response}')
        ensure_login_cookies(cookie_jar)
        summary['login'] = {
            'status_code': login_response['status_code'],
            'elapsed_ms': login_response['elapsed_ms'],
            'username': username,
            'password_hint': '*' * len(password),
            'message': login_body.get('message'),
            'cookie_names': sorted({cookie.name for cookie in cookie_jar}),
        }

        settings_response = request(
            opener,
            base_url=args.base_url,
            method='GET',
            path='/api/settings',
            timeout=args.http_timeout,
        )
        settings_body = assert_json_response(settings_response, label='get settings')
        probe_payload = build_probe_payload(settings_body)
        summary['settings_snapshot'] = {
            'provider': probe_payload['provider'],
            'llm_model': probe_payload['llm_model'],
            'api_base_url': probe_payload['api_base_url'],
            'api_key_hint': mask_secret(probe_payload['api_key']),
            'fallback_strategy': probe_payload.get('fallback_strategy'),
            'backup_url_count': len(probe_payload.get('api_backup_urls') or []),
        }

        settings_test_response = request(
            opener,
            base_url=args.base_url,
            method='POST',
            path='/api/settings/test',
            payload=probe_payload,
            timeout=args.probe_timeout,
        )
        settings_test_body = assert_json_response(settings_test_response, label='settings test')
        summary['settings_test'] = summarize_probe_response(settings_test_body)

        function_probe_response = request(
            opener,
            base_url=args.base_url,
            method='POST',
            path='/api/settings/check-function-calling',
            payload=probe_payload,
            timeout=args.probe_timeout,
        )
        function_probe_body = assert_json_response(function_probe_response, label='function calling probe')
        summary['function_calling_probe'] = summarize_probe_response(function_probe_body)

        if settings_test_body.get('success') is not True:
            raise SmokeFailure(f'/api/settings/test failed: {json.dumps(summary["settings_test"], ensure_ascii=False)}')
        if function_probe_body.get('success') is not True:
            raise SmokeFailure(
                f'/api/settings/check-function-calling failed: '
                f'{json.dumps(summary["function_calling_probe"], ensure_ascii=False)}'
            )
        if function_probe_body.get('supported') is not True:
            raise SmokeFailure(
                'function calling probe completed but model was not confirmed as supported: '
                f'{json.dumps(summary["function_calling_probe"], ensure_ascii=False)}'
            )

        summary['ok'] = True
        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
        output_text = json.dumps(summary, ensure_ascii=False, indent=2)
        print(output_text)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_text + '\n', encoding='utf-8', newline='\n')
        return 0
    except Exception as exc:  # noqa: BLE001
        summary['ok'] = False
        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
        summary['error'] = str(exc)
        output_text = json.dumps(summary, ensure_ascii=False, indent=2)
        print(output_text, file=sys.stderr)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_text + '\n', encoding='utf-8', newline='\n')
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
