# -*- coding: utf-8 -*-

"""Smoke test for strangler gateway control-plane probes."""



from __future__ import annotations



import argparse

import json

import sys

import time

import uuid

import urllib.error

import urllib.request

from pathlib import Path

from typing import Any, Dict, List, Mapping, Sequence





DEFAULT_BASE_URL = 'http://127.0.0.1:8005'
DEFAULT_HTTP_TIMEOUT = 10.0
DEFAULT_PROFILE = 'deploy'




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
    parser.add_argument(
        '--profile',
        default=DEFAULT_PROFILE,
        help='Probe profile to execute (defaults to deploy)',
    )
    parser.add_argument(
        '--probe-name',
        action='append',
        dest='probe_names',
        help='Exact probe name to execute; repeat to run multiple named probes',
    )
    parser.add_argument(
        '--route-group',
        action='append',
        dest='route_groups',
        help='Route-group label to execute; repeat to run multiple route groups',
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


def require_string_list(value: Any, *, label: str) -> List[str]:
    items = require_sequence(value, label=label)
    if not items:
        raise SmokeFailure(f'{label} must not be empty')

    normalized: List[str] = []
    for item in items:
        if not isinstance(item, str) or not item.strip():
            raise SmokeFailure(f'{label} items must be non-empty strings')
        normalized.append(item)
    return normalized


def require_string_mapping(value: Any, *, label: str) -> Dict[str, str]:
    mapping = require_mapping(value, label=label)
    normalized: Dict[str, str] = {}
    for key, item in mapping.items():
        if not isinstance(key, str) or not key.strip():
            raise SmokeFailure(f'{label} keys must be non-empty strings')
        if not isinstance(item, str) or not item.strip():
            raise SmokeFailure(f'{label}.{key} must be a non-empty string')
        normalized[key] = item
    return normalized


def require_multipart_form(value: Any, *, label: str) -> Dict[str, Any]:
    multipart = dict(require_mapping(value, label=label))
    fields = multipart.get('fields')
    files = multipart.get('files')

    normalized: Dict[str, Any] = {}
    if fields is not None:
        normalized['fields'] = require_string_mapping(fields, label=f'{label}.fields')

    if files is not None:
        files_mapping = require_mapping(files, label=f'{label}.files')
        normalized_files: Dict[str, Dict[str, str]] = {}
        for field_name, file_spec_raw in files_mapping.items():
            if not isinstance(field_name, str) or not field_name.strip():
                raise SmokeFailure(f'{label}.files keys must be non-empty strings')
            file_spec = require_mapping(file_spec_raw, label=f'{label}.files.{field_name}')

            filename = file_spec.get('filename', field_name)
            content = file_spec.get('content')
            content_type = file_spec.get('content_type', 'application/octet-stream')

            if not isinstance(filename, str) or not filename.strip():
                raise SmokeFailure(f'{label}.files.{field_name}.filename must be a non-empty string')
            if not isinstance(content, str):
                raise SmokeFailure(f'{label}.files.{field_name}.content must be a string')
            if not isinstance(content_type, str) or not content_type.strip():
                raise SmokeFailure(
                    f'{label}.files.{field_name}.content_type must be a non-empty string'
                )

            normalized_files[field_name] = {
                'filename': filename,
                'content': content,
                'content_type': content_type,
            }

        normalized['files'] = normalized_files

    if not normalized.get('fields') and not normalized.get('files'):
        raise SmokeFailure(f'{label} must define at least one multipart field or file')

    return normalized


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
            raise SmokeFailure(
                f'manifest.probes[{index}].expected_json must be an object when present'
            )

        expected_json_has_keys = probe.get('expected_json_has_keys')
        if expected_json_has_keys is not None:
            probe['expected_json_has_keys'] = require_string_list(
                expected_json_has_keys,
                label=f'manifest.probes[{index}].expected_json_has_keys',
            )

        headers = probe.get('headers')
        if headers is not None:
            probe['headers'] = require_string_mapping(headers, label=f'manifest.probes[{index}].headers')

        request_body = probe.get('body')
        json_body = probe.get('json_body')
        multipart_form = probe.get('multipart_form')
        payload_field_count = sum(
            item is not None for item in (request_body, json_body, multipart_form)
        )
        if payload_field_count > 1:
            raise SmokeFailure(
                f'manifest.probes[{index}] must not define more than one of body, json_body, multipart_form'
            )
        if request_body is not None and not isinstance(request_body, str):
            raise SmokeFailure(f'manifest.probes[{index}].body must be a string when present')
        if json_body is not None:
            try:
                json.dumps(json_body, ensure_ascii=False)
            except TypeError as exc:
                raise SmokeFailure(
                    f'manifest.probes[{index}].json_body must be JSON-serializable'
                ) from exc
        if multipart_form is not None:
            probe['multipart_form'] = require_multipart_form(
                multipart_form,
                label=f'manifest.probes[{index}].multipart_form',
            )

        expected_content_types = probe.get('expected_content_type_contains')
        if expected_content_types is not None:
            probe['expected_content_type_contains'] = require_string_list(
                expected_content_types,
                label=f'manifest.probes[{index}].expected_content_type_contains',
            )

        expected_header_contains = probe.get('expected_header_contains')
        if expected_header_contains is not None:
            probe['expected_header_contains'] = require_string_mapping(
                expected_header_contains,
                label=f'manifest.probes[{index}].expected_header_contains',
            )

        expected_text_contains = probe.get('expected_text_contains')
        if expected_text_contains is not None:
            probe['expected_text_contains'] = require_string_list(
                expected_text_contains,
                label=f'manifest.probes[{index}].expected_text_contains',
            )

        expected_text_startswith = probe.get('expected_text_startswith')
        if expected_text_startswith is not None:
            if not isinstance(expected_text_startswith, str) or not expected_text_startswith.strip():
                raise SmokeFailure(
                    f'manifest.probes[{index}].expected_text_startswith must be a non-empty string'
                )

        route_group = probe.get('route_group')
        if route_group is not None:
            if not isinstance(route_group, str) or not route_group.strip():
                raise SmokeFailure(
                    f'manifest.probes[{index}].route_group must be a non-empty string when present'
                )
            probe['route_group'] = route_group.strip()

        profiles = probe.get('profiles')
        if profiles is not None:
            normalized_profiles = require_string_list(
                profiles,
                label=f'manifest.probes[{index}].profiles',
            )
            seen_profiles = set()
            deduped_profiles = []
            for item in normalized_profiles:
                normalized = item.strip()
                if normalized not in seen_profiles:
                    seen_profiles.add(normalized)
                    deduped_profiles.append(normalized)
            probe['profiles'] = deduped_profiles

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


def body_text(body: Any) -> str:
    if isinstance(body, (dict, list)):
        return json.dumps(body, ensure_ascii=False)
    if body is None:
        return ''
    return str(body)


def body_preview(body: Any, *, limit: int = 400) -> str:
    text = body_text(body)
    return text[:limit]




def collect_response_headers(headers: Any) -> Dict[str, str]:
    collected: Dict[str, List[str]] = {}
    for name, value in headers.items():
        key = str(name)
        collected.setdefault(key, []).append(str(value))
    return {key: '\n'.join(values) for key, values in collected.items()}


def encode_multipart_form_data(multipart_form: Mapping[str, Any]) -> tuple[bytes, str]:
    boundary = f'----codex-strangler-{uuid.uuid4().hex}'
    chunks: List[bytes] = []

    for field_name, value in (multipart_form.get('fields') or {}).items():
        chunks.extend(
            [
                f'--{boundary}\r\n'.encode('utf-8'),
                f'Content-Disposition: form-data; name="{field_name}"\r\n\r\n'.encode('utf-8'),
                str(value).encode('utf-8'),
                b'\r\n',
            ]
        )

    for field_name, file_spec in (multipart_form.get('files') or {}).items():
        chunks.extend(
            [
                f'--{boundary}\r\n'.encode('utf-8'),
                (
                    f'Content-Disposition: form-data; name="{field_name}"; '
                    f'filename="{file_spec["filename"]}"\r\n'
                ).encode('utf-8'),
                f'Content-Type: {file_spec["content_type"]}\r\n\r\n'.encode('utf-8'),
                str(file_spec['content']).encode('utf-8'),
                b'\r\n',
            ]
        )

    chunks.append(f'--{boundary}--\r\n'.encode('utf-8'))
    return b''.join(chunks), boundary


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





def request_probe(
    *,
    base_url: str,
    path: str,
    method: str,
    timeout: float,
    headers: Mapping[str, str] | None = None,
    body: str | None = None,
    json_body: Any | None = None,
    multipart_form: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    url = f"{base_url.rstrip('/')}{path}"
    request_headers = {
        'Accept': 'application/json, text/html, text/plain, */*',
        'User-Agent': 'codex-strangler-gateway-smoke/1.0',
    }
    if headers:
        request_headers.update(headers)

    request_data: bytes | None = None
    if json_body is not None:
        request_data = json.dumps(json_body, ensure_ascii=False).encode('utf-8')
        request_headers.setdefault('Content-Type', 'application/json; charset=utf-8')
    elif multipart_form is not None:
        request_data, boundary = encode_multipart_form_data(multipart_form)
        request_headers.setdefault('Content-Type', f'multipart/form-data; boundary={boundary}')
    elif body is not None:
        request_data = body.encode('utf-8')
        request_headers.setdefault('Content-Type', 'text/plain; charset=utf-8')

    request = urllib.request.Request(
        url,
        data=request_data,
        method=method.upper(),
        headers=request_headers,
    )
    started = time.perf_counter()

    try:

        with urllib.request.urlopen(request, timeout=timeout) as response:

            raw = response.read()

            status_code = response.getcode()

            content_type = response.headers.get('Content-Type', '')
            response_headers = collect_response_headers(response.headers)
    except urllib.error.HTTPError as exc:

        raw = exc.read()

        status_code = exc.code

        content_type = exc.headers.get('Content-Type', '')
        response_headers = collect_response_headers(exc.headers)
    except urllib.error.URLError as exc:

        raise SmokeFailure(f'request failed: {url}\nerror={exc}') from exc



    elapsed_ms = round((time.perf_counter() - started) * 1000, 2)

    body = decode_body(raw, content_type)

    return {

        'url': url,

        'status_code': status_code,

        'elapsed_ms': elapsed_ms,

        'content_type': content_type,

        'headers': response_headers,
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

    expected_json_has_keys = probe.get('expected_json_has_keys') or []
    if expected_json_has_keys:
        actual_body = response.get('body')
        if not isinstance(actual_body, dict):
            raise SmokeFailure(
                f'JSON key assertion failed for {probe["name"]}: body is not an object; '
                f'content_type={response.get("content_type")} body_preview={body_preview(actual_body)}'
            )
        missing = [item for item in expected_json_has_keys if item not in actual_body]
        if missing:
            raise SmokeFailure(
                f'JSON key assertion failed for {probe["name"]}: '
                f'missing={missing!r} body_preview={body_preview(actual_body)}'
            )

    expected_content_types = probe.get('expected_content_type_contains') or []
    if expected_content_types:
        content_type = str(response.get('content_type') or '').lower()
        matches = [item for item in expected_content_types if item.lower() in content_type]
        if not matches:
            raise SmokeFailure(

                f'content type mismatch for {probe["name"]}: '

                f'expected one of {expected_content_types} actual={response.get("content_type")!r}'
            )

    expected_header_contains = probe.get('expected_header_contains') or {}
    if expected_header_contains:
        actual_headers = response.get('headers')
        if not isinstance(actual_headers, dict):
            raise SmokeFailure(
                f'header assertion failed for {probe["name"]}: headers not available'
            )
        for header_name, expected_fragment in expected_header_contains.items():
            actual_value = None
            for actual_name, value in actual_headers.items():
                if str(actual_name).lower() == str(header_name).lower():
                    actual_value = str(value)
                    break
            if actual_value is None:
                raise SmokeFailure(
                    f'header assertion failed for {probe["name"]}: missing header {header_name!r}'
                )
            if expected_fragment not in actual_value:
                raise SmokeFailure(
                    f'header assertion failed for {probe["name"]}: '
                    f'header={header_name!r} expected_fragment={expected_fragment!r} '
                    f'actual_value={actual_value!r}'
                )

    expected_text_startswith = probe.get('expected_text_startswith')
    if expected_text_startswith is not None:
        actual_text = body_text(response.get('body'))
        if not actual_text.startswith(expected_text_startswith):
            raise SmokeFailure(
                f'text prefix mismatch for {probe["name"]}: '
                f'expected_prefix={expected_text_startswith!r} '
                f'body_preview={body_preview(response.get("body"))}'
            )

    expected_text_contains = probe.get('expected_text_contains') or []
    if expected_text_contains:
        actual_text = body_text(response.get('body'))
        missing = [item for item in expected_text_contains if item not in actual_text]
        if missing:
            raise SmokeFailure(
                f'text assertion failed for {probe["name"]}: '
                f'missing={missing!r} body_preview={body_preview(response.get("body"))}'
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
                headers=probe.get('headers'),
                body=probe.get('body'),
                json_body=probe.get('json_body'),
                multipart_form=probe.get('multipart_form'),
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
                    'expected_json_has_keys': probe.get('expected_json_has_keys'),
                    'expected_content_type_contains': probe.get('expected_content_type_contains'),
                    'expected_text_startswith': probe.get('expected_text_startswith'),
                    'expected_text_contains': probe.get('expected_text_contains'),
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


def select_probes_by_profile(manifest: Mapping[str, Any], *, profile: str) -> Dict[str, Any]:
    selected = []
    for probe in manifest['probes']:
        profiles = probe.get('profiles')
        if profiles is None or profile in profiles:
            selected.append(probe)

    if not selected:
        raise SmokeFailure(f'no probes matched profile={profile!r}')

    filtered_manifest = dict(manifest)
    filtered_manifest['probes'] = selected
    return filtered_manifest


def select_probes_by_name(
    manifest: Mapping[str, Any], *, probe_names: Sequence[str] | None
) -> Dict[str, Any]:
    if not probe_names:
        return dict(manifest)

    requested: List[str] = []
    seen_requested = set()
    for item in probe_names:
        normalized = str(item).strip()
        if normalized and normalized not in seen_requested:
            seen_requested.add(normalized)
            requested.append(normalized)

    if not requested:
        raise SmokeFailure('probe_names must contain at least one non-empty probe name')

    selected = [probe for probe in manifest['probes'] if probe['name'] in seen_requested]
    found_names = {probe['name'] for probe in selected}
    missing = [name for name in requested if name not in found_names]
    if missing:
        raise SmokeFailure(f'no probes matched names={missing!r}')

    filtered_manifest = dict(manifest)
    filtered_manifest['probes'] = selected
    return filtered_manifest


def select_probes_by_route_group(
    manifest: Mapping[str, Any], *, route_groups: Sequence[str] | None
) -> Dict[str, Any]:
    if not route_groups:
        return dict(manifest)

    requested: List[str] = []
    seen_requested = set()
    for item in route_groups:
        normalized = str(item).strip()
        if normalized and normalized not in seen_requested:
            seen_requested.add(normalized)
            requested.append(normalized)

    if not requested:
        raise SmokeFailure('route_groups must contain at least one non-empty route-group name')

    selected = [
        probe
        for probe in manifest['probes']
        if probe.get('route_group') in seen_requested
    ]
    found_groups = {probe.get('route_group') for probe in selected}
    missing = [name for name in requested if name not in found_groups]
    if missing:
        raise SmokeFailure(f'no probes matched route_groups={missing!r}')

    filtered_manifest = dict(manifest)
    filtered_manifest['probes'] = selected
    return filtered_manifest




def write_summary(path: Path, summary: Mapping[str, Any]) -> None:

    path.parent.mkdir(parents=True, exist_ok=True)

    path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')





def summarize_probe_inventory(probes: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:

    owner_counts: Dict[str, int] = {}
    route_group_counts: Dict[str, int] = {}
    route_group_probe_names: Dict[str, List[str]] = {}

    for probe in probes:
        owner = str(probe['owner'])
        owner_counts[owner] = owner_counts.get(owner, 0) + 1

        route_group = probe.get('route_group')
        if isinstance(route_group, str) and route_group.strip():
            normalized_route_group = route_group.strip()
            route_group_counts[normalized_route_group] = (
                route_group_counts.get(normalized_route_group, 0) + 1
            )
            route_group_probe_names.setdefault(normalized_route_group, []).append(
                str(probe['name'])
            )

    return {
        'owner_counts': owner_counts,
        'route_group_counts': route_group_counts,
        'route_group_probe_names': route_group_probe_names,
    }


def manifest_summary(manifest: Mapping[str, Any], *, manifest_path: Path) -> Dict[str, Any]:
    summary = {
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
                'route_group': probe.get('route_group'),
                'profiles': probe.get('profiles'),
            }
            for probe in manifest['probes']
        ],
    }
    summary.update(summarize_probe_inventory(manifest['probes']))
    return summary




def main() -> int:

    args = build_parser().parse_args()

    started_at = time.strftime('%Y-%m-%dT%H:%M:%S')

    output_path = args.output.resolve()



    summary: Dict[str, Any] = {
        'ok': False,
        'base_url': args.base_url,
        'profile': args.profile,
        'probe_names': args.probe_names,
        'route_groups': args.route_groups,
        'started_at': started_at,
        'output_path': str(output_path),
    }


    try:

        manifest = validate_manifest(load_json_file(args.manifest.resolve()), manifest_path=args.manifest.resolve())
        manifest = select_probes_by_profile(manifest, profile=args.profile)
        manifest = select_probes_by_route_group(manifest, route_groups=args.route_groups)
        manifest = select_probes_by_name(manifest, probe_names=args.probe_names)
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
