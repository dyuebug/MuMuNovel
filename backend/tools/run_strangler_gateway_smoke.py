# -*- coding: utf-8 -*-
"""Smoke test for strangler gateway control-plane probes."""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Mapping, Sequence


DEFAULT_BASE_URL = 'http://127.0.0.1:8005'
DEFAULT_HTTP_TIMEOUT = 10.0


class SmokeFailure(RuntimeError):
    """Raised when a smoke assertion fails."""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_manifest_path() -> Path:
    return repo_root() / 'deploy' / 'strangler-gateway-probes.json'


def default_output_path() -> Path:
    return repo_root() / 'tmp' / 'smoke' / 'tmp_strangler_gateway_smoke_latest.json'


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description='Validate gateway probes for the strangler migration control plane'
    )
    parser.add_argument('--base-url', default=DEFAULT_BASE_URL, help='Gateway base URL')
    parser.add_argument(
        '--manifest',
        type=Path,
        default=default_manifest_path(),
        help='Probe manifest JSON path',
    )
    parser.add_argument(
        '--http-timeout',
        type=float,
        default=DEFAULT_HTTP_TIMEOUT,
        help='Single HTTP request timeout in seconds',
    )
    parser.add_argument(
        '--output',
        type=Path,
        default=default_output_path(),
        help='JSON output path (defaults to tmp/smoke/)',
    )
    parser.add_argument(
        '--validate-manifest-only',
        action='store_true',
        help='Validate manifest structure without issuing HTTP requests',
    )
    return parser


def load_json_file(path: Path) -> Any:
    if not path.exists():
        raise SmokeFailure(f'JSON file not found: {path}')
    try:
        return json.loads(path.read_text(encoding='utf-8'))
    except json.JSONDecodeError as exc:
        raise SmokeFailure(f'Invalid JSON file: {path}\nerror={exc}') from exc


def require_mapping(value: Any, *, label: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise SmokeFailure(f'{label} must be an object; got {type(value).__name__}')
    return value


def require_sequence(value: Any, *, label: str) -> Sequence[Any]:
    if not isinstance(value, list):
        raise SmokeFailure(f'{label} must be an array; got {type(value).__name__}')
    return value


def validate_manifest(raw_manifest: Any, *, manifest_path: Path) -> Dict[str, Any]:
    manifest = dict(require_mapping(raw_manifest, label='manifest'))
    version = manifest.get('manifest_version')
    if not isinstance(version, int) or version < 1:
        raise SmokeFailure(f'manifest_version must be an integer >= 1: {manifest_path}')

    probes_raw = require_sequence(manifest.get('probes'), label='manifest.probes')
    probes: List[Dict[str, Any]] = []
    names_seen = set()

    for index, probe_raw in enumerate(probes_raw):
        probe = dict(require_mapping(probe_raw, label=f'manifest.probes[{index}]'))
        name = probe.get('name')
        owner = probe.get('owner')
        method = probe.get('method')
        path = probe.get('path')
        expected_status = probe.get('expected_status')

        if not isinstance(name, str) or not name.strip():
            raise SmokeFailure(f'manifest.probes[{index}].name must be a non-empty string')
        if name in names_seen:
            raise SmokeFailure(f'duplicate probe name: {name}')
        names_seen.add(name)

        if not isinstance(owner, str) or not owner.strip():
            raise SmokeFailure(f'manifest.probes[{index}].owner must be a non-empty string')
        if not isinstance(method, str) or method.upper() not in {'GET', 'POST', 'PUT', 'PATCH', 'DELETE'}:
            raise SmokeFailure(f'manifest.probes[{index}].method must be a supported HTTP method')
        if not isinstance(path, str) or not path.startswith('/'):
            raise SmokeFailure(f'manifest.probes[{index}].path must start with /')
        if not isinstance(expected_status, int):
            raise SmokeFailure(f'manifest.probes[{index}].expected_status must be an integer')

        expected_json = probe.get('expected_json')
        if expected_json is not None and not isinstance(expected_json, dict):
            raise SmokeFailure(f'manifest.probes[{index}].expected_json must be an object when present')

        expected_content_types = probe.get('expected_content_type_contains')
        if expected_content_types is not None:
            require_sequence(expected_content_types, label=f'manifest.probes[{index}].expected_content_type_contains')
            if not expected_content_types:
                raise SmokeFailure(
                    f'manifest.probes[{index}].expected_content_type_contains must not be empty'
                )
            for item in expected_content_types:
                if not isinstance(item, str) or not item.strip():
                    raise SmokeFailure(
                        f'manifest.probes[{index}].expected_content_type_contains items must be non-empty strings'
                    )

        probes.append(probe)

    manifest['probes'] = probes
    return manifest


def decode_body(raw: bytes, content_type: str) -> Any:
    text = raw.decode('utf-8', errors='replace')
    if 'application/json' in content_type.lower():
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text
    return text


def body_preview(body: Any, *, limit: int = 400) -> str:
    if isinstance(body, (dict, list)):
        text = json.dumps(body, ensure_ascii=False)
    else:
        text = str(body)
    return text[:limit]


def subset_matches(actual: Any, expected: Any, *, path: str = '$') -> None:
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            raise SmokeFailure(f'{path} expected object but got {type(actual).__name__}')
        for key, expected_value in expected.items():
            if key not in actual:
                raise SmokeFailure(f'{path}.{key} missing from response JSON')
            subset_matches(actual[key], expected_value, path=f'{path}.{key}')
        return

    if isinstance(expected, list):
        if not isinstance(actual, list):
            raise SmokeFailure(f'{path} expected array but got {type(actual).__name__}')
        if len(actual) < len(expected):
            raise SmokeFailure(f'{path} expected at least {len(expected)} items but got {len(actual)}')
        for index, expected_value in enumerate(expected):
            subset_matches(actual[index], expected_value, path=f'{path}[{index}]')
        return

    if actual != expected:
        raise SmokeFailure(f'{path} expected {expected!r} but got {actual!r}')


def request_probe(*, base_url: str, path: str, method: str, timeout: float) -> Dict[str, Any]:
    url = f"{base_url.rstrip('/')}{path}"
    request = urllib.request.Request(
        url,
        method=method.upper(),
        headers={
            'Accept': 'application/json, text/html, text/plain, */*',
            'User-Agent': 'codex-strangler-gateway-smoke/1.0',
        },
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
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


def ensure_probe_expectations(probe: Mapping[str, Any], response: Mapping[str, Any]) -> None:
    expected_status = probe['expected_status']
    actual_status = response['status_code']
    if actual_status != expected_status:
        raise SmokeFailure(
            f'status mismatch for {probe["name"]}: '
            f'expected={expected_status} actual={actual_status} '
            f'body_preview={body_preview(response.get("body"))}'
        )

    expected_json = probe.get('expected_json')
    if expected_json is not None:
        actual_body = response.get('body')
        if not isinstance(actual_body, dict):
            raise SmokeFailure(
                f'JSON assertion failed for {probe["name"]}: body is not an object; '
                f'content_type={response.get("content_type")} body_preview={body_preview(actual_body)}'
            )
        subset_matches(actual_body, expected_json)

    expected_content_types = probe.get('expected_content_type_contains') or []
    if expected_content_types:
        content_type = str(response.get('content_type') or '').lower()
        matches = [item for item in expected_content_types if item.lower() in content_type]
        if not matches:
            raise SmokeFailure(
                f'content type mismatch for {probe["name"]}: '
                f'expected one of {expected_content_types} actual={response.get("content_type")!r}'
            )


def run_probes(*, manifest: Mapping[str, Any], base_url: str, timeout: float) -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    for probe in manifest['probes']:
        result: Dict[str, Any] = {
            'name': probe['name'],
            'owner': probe['owner'],
            'method': probe['method'].upper(),
            'path': probe['path'],
            'expected_status': probe['expected_status'],
            'ok': False,
        }
        try:
            response = request_probe(
                base_url=base_url,
                path=probe['path'],
                method=probe['method'],
                timeout=timeout,
            )
            ensure_probe_expectations(probe, response)
            result.update({
                'ok': True,
                'status_code': response['status_code'],
                'elapsed_ms': response['elapsed_ms'],
                'content_type': response['content_type'],
                'body_preview': body_preview(response['body']),
                'assertions': {
                    'expected_json': probe.get('expected_json'),
                    'expected_content_type_contains': probe.get('expected_content_type_contains'),
                },
            })
            print(
                f"[OK] owner={probe['owner']} path={probe['path']} "
                f"status={response['status_code']} elapsed_ms={response['elapsed_ms']}"
            )
        except Exception as exc:  # noqa: BLE001
            result['error'] = str(exc)
            print(
                f"[FAIL] owner={probe['owner']} path={probe['path']} error={exc}",
                file=sys.stderr,
            )
        results.append(result)
    return results


def write_summary(path: Path, summary: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')


def manifest_summary(manifest: Mapping[str, Any], *, manifest_path: Path) -> Dict[str, Any]:
    return {
        'manifest_path': str(manifest_path),
        'manifest_version': manifest['manifest_version'],
        'probe_count': len(manifest['probes']),
        'probes': [
            {
                'name': probe['name'],
                'owner': probe['owner'],
                'method': probe['method'].upper(),
                'path': probe['path'],
                'expected_status': probe['expected_status'],
            }
            for probe in manifest['probes']
        ],
    }


def main() -> int:
    args = build_parser().parse_args()
    started_at = time.strftime('%Y-%m-%dT%H:%M:%S')
    output_path = args.output.resolve()

    summary: Dict[str, Any] = {
        'ok': False,
        'base_url': args.base_url,
        'started_at': started_at,
        'output_path': str(output_path),
    }

    try:
        manifest = validate_manifest(load_json_file(args.manifest.resolve()), manifest_path=args.manifest.resolve())
        summary.update(manifest_summary(manifest, manifest_path=args.manifest.resolve()))

        if args.validate_manifest_only:
            summary['mode'] = 'validate-manifest-only'
            summary['ok'] = True
            summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
            write_summary(output_path, summary)
            print(json.dumps(summary, ensure_ascii=False, indent=2))
            return 0

        results = run_probes(manifest=manifest, base_url=args.base_url, timeout=args.http_timeout)
        summary['mode'] = 'probe'
        summary['probes'] = results
        summary['ok'] = all(bool(item.get('ok')) for item in results)
        failed = [item['name'] for item in results if not item.get('ok')]
        summary['failed_probe_names'] = failed
        if failed:
            raise SmokeFailure(f'gateway smoke failed for probes={failed}')

        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
        write_summary(output_path, summary)
        print(json.dumps(summary, ensure_ascii=False, indent=2))
        return 0
    except Exception as exc:  # noqa: BLE001
        summary['error'] = str(exc)
        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
        write_summary(output_path, summary)
        print(json.dumps(summary, ensure_ascii=False, indent=2), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
