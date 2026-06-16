# -*- coding: utf-8 -*-

"""Smoke test for strangler gateway control-plane probes."""



from __future__ import annotations



import argparse
import contextlib
import copy
import http.server
import http.cookiejar
import json
import os
import re
import sys
import threading
import time
import uuid
import urllib.error
import urllib.request

from pathlib import Path

from typing import Any, Dict, List, Mapping, Sequence





DEFAULT_BASE_URL = 'http://127.0.0.1:8005'
DEFAULT_HTTP_TIMEOUT = 10.0
DEFAULT_PROFILE = 'deploy'
DEFAULT_LOGIN_PATH = '/api/auth/local/login'
MOCK_OPENAI_BASE_URL_PLACEHOLDER = 'mock_openai_base_url'
MOCK_OPENAI_MODEL_ID = 'smoke-model'
MOCK_OPENAI_STREAM_CHUNKS = ('烟测改写成功。', '第二段继续推进。')
MOCK_OPENAI_ORGANIZATION_STREAM_CHUNKS = (
    '{"name":"烟测组织","is_organization":true,"organization_type":"情报结社",',
    '"personality":"行事隐秘，擅长用互惠和情报交换维持影响力。",',
    '"background":"由边境商路上的旧情报网络演化而来，长期为各方势力提供灰色消息服务。",',
    '"appearance":"总部藏在旧港仓库区，以黑金徽记和无声守卫闻名。",',
    '"organization_purpose":"垄断边境情报与秘密交易通道。","power_level":67,',
    '"location":"北境旧港","motto":"消息即筹码","traits":["隐秘","渗透","交易"],',
    '"color":"黑金","organization_members":["烟测首领","烟测联络人"]}',
)
MOCK_OPENAI_INSPIRATION_OPTIONS_CHUNKS = (
    '{"prompt":"我先给你6个命名方向，挑一个最有爆点的：",',
    '"options":["雾钟封港","旧塔来信","边境夜雾","钟声未停","封锁前夜","失钟之城"]}',
)
MOCK_OPENAI_INSPIRATION_REFINE_CHUNKS = (
    '{"prompt":"选择更贴合反馈的新方向：",',
    '"options":["她刚想带母亲逃出旧港，钟声却先一步暴露了她的名字，整座边境城开始在雾里追她。",',
    '"边境封锁令落下那晚，她在旧塔脚下捡到失踪父亲的徽章，可真相每近一步，母亲就离危险更近一步。",',
    '"她以为查的是旧塔怪响，结果每一次钟声都在替某个人点名，而下一个被点到的偏偏是她唯一想保住的人。"]}',
)
MOCK_OPENAI_INSPIRATION_QUICK_CHUNKS = (
    '{"title":"雾钟封港",',
    '"description":"边境旧港突遭封锁那夜，废塔钟声一遍遍穿过浓雾，失踪多年的父亲名字竟出现在通缉广播里。她想带母亲逃离，却必须先查清钟声背后那张吃人的情报网，否则天亮前她们都会被送进黑名单。",',
    '"theme":"所谓真相，从来不是查到了就能说出口，而是你敢不敢为了保住最在乎的人，先把自己推上代价最高的位置。",',
    '"genre":["悬疑","都市","情报博弈"],',
    '"narrative_perspective":"第三人称"}',
)
PLACEHOLDER_PATTERN = re.compile(r'\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}')




class SmokeFailure(RuntimeError):

    """Raised when a smoke assertion fails."""





def repo_root() -> Path:

    return Path(__file__).resolve().parents[2]





def default_manifest_path() -> Path:

    return repo_root() / 'deploy' / 'strangler-gateway-probes.json'





def default_output_path() -> Path:

    return repo_root() / 'tmp' / 'smoke' / 'tmp_strangler_gateway_smoke_latest.json'


def default_env_file() -> Path:

    return repo_root() / '.env'





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
        '--env-file',
        type=Path,
        help='Optional .env file path for LOCAL_AUTH_USERNAME / LOCAL_AUTH_PASSWORD',
    )
    parser.add_argument(
        '--local-auth-username',
        help='Optional local auth username override for requires_login probes',
    )
    parser.add_argument(
        '--local-auth-password',
        help='Optional local auth password override for requires_login probes',
    )
    parser.add_argument(
        '--login-path',
        default=DEFAULT_LOGIN_PATH,
        help='Local auth login path used by requires_login probes',
    )

    parser.add_argument(
        '--validate-manifest-only',
        action='store_true',
        help='Validate manifest structure without issuing HTTP requests',
    )
    parser.add_argument(
        '--readiness-summary-only',
        action='store_true',
        help=(
            'Summarize route-group shrink-readiness signals from the manifest '
            'without profile filtering or HTTP requests'
        ),
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


def load_env_map(path: Path) -> Dict[str, str]:

    if not path.exists():

        raise SmokeFailure(f'.env file not found: {path}')

    values: Dict[str, str] = {}
    for raw_line in path.read_text(encoding='utf-8').splitlines():
        line = raw_line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = raw_line.split('=', 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def resolve_local_auth_credentials(
    *,
    username: str | None,
    password: str | None,
    env_file: Path | None,
) -> tuple[str, str, Path | None]:

    resolved_env_file = env_file.resolve() if env_file else default_env_file()
    env_map: Dict[str, str] = {}
    used_env_file: Path | None = None
    if resolved_env_file.exists():
        env_map = load_env_map(resolved_env_file)
        used_env_file = resolved_env_file

    resolved_username = (
        (username or '').strip()
        or str(os.getenv('LOCAL_AUTH_USERNAME') or '').strip()
        or str(env_map.get('LOCAL_AUTH_USERNAME') or '').strip()
    )
    resolved_password = (
        (password or '').strip()
        or str(os.getenv('LOCAL_AUTH_PASSWORD') or '').strip()
        or str(env_map.get('LOCAL_AUTH_PASSWORD') or '').strip()
    )

    if not resolved_username or not resolved_password:
        env_hint = (
            str(resolved_env_file)
            if used_env_file is not None
            else 'missing default .env; pass --env-file or CLI/env credentials'
        )
        raise SmokeFailure(
            'requires_login probes need LOCAL_AUTH_USERNAME / LOCAL_AUTH_PASSWORD; '
            f'env_hint={env_hint}'
        )

    return resolved_username, resolved_password, used_env_file





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


def require_extract_mapping(value: Any, *, label: str) -> Dict[str, str]:
    mapping = require_mapping(value, label=label)
    normalized: Dict[str, str] = {}
    for key, item in mapping.items():
        if not isinstance(key, str) or not key.strip():
            raise SmokeFailure(f'{label} keys must be non-empty strings')
        if not isinstance(item, str) or not item.strip():
            raise SmokeFailure(f'{label}.{key} must be a non-empty string JSON path')
        normalized[key.strip()] = item.strip()
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

        expected_statuses = probe.get('expected_statuses')
        if expected_statuses is not None:
            if (
                not isinstance(expected_statuses, list)
                or not expected_statuses
                or any(not isinstance(item, int) for item in expected_statuses)
            ):
                raise SmokeFailure(
                    f'manifest.probes[{index}].expected_statuses must be a non-empty integer array'
                )
            probe['expected_statuses'] = expected_statuses


        expected_json = probe.get('expected_json')
        if expected_json is not None and not isinstance(expected_json, dict):
            raise SmokeFailure(
                f'manifest.probes[{index}].expected_json must be an object when present'
            )

        expected_json_one_of = probe.get('expected_json_one_of')
        if expected_json_one_of is not None:
            if not isinstance(expected_json_one_of, list) or not expected_json_one_of:
                raise SmokeFailure(
                    f'manifest.probes[{index}].expected_json_one_of must be a non-empty array'
                )
            for option_index, option in enumerate(expected_json_one_of):
                if not isinstance(option, dict):
                    raise SmokeFailure(
                        f'manifest.probes[{index}].expected_json_one_of[{option_index}] must be an object'
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
        extract_json = probe.get('extract_json')
        if extract_json is not None:
            probe['extract_json'] = require_extract_mapping(
                extract_json,
                label=f'manifest.probes[{index}].extract_json',
            )
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

        requires_login = probe.get('requires_login')
        if requires_login is not None:
            if not isinstance(requires_login, bool):
                raise SmokeFailure(
                    f'manifest.probes[{index}].requires_login must be a boolean when present'
                )
            probe['requires_login'] = requires_login

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


class MockOpenAIServer:
    """Small OpenAI-compatible SSE server used by deterministic smoke probes."""

    def __init__(self) -> None:
        self._server = http.server.ThreadingHTTPServer(
            ('127.0.0.1', 0),
            _MockOpenAIRequestHandler,
        )
        self._thread = threading.Thread(
            target=self._server.serve_forever,
            name='strangler-smoke-mock-openai',
            daemon=True,
        )

    @property
    def base_url(self) -> str:
        host, port = self._server.server_address
        return f'http://{host}:{port}/v1'

    def __enter__(self) -> 'MockOpenAIServer':
        self._thread.start()
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        self.close()

    def close(self) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2.0)


class _MockOpenAIRequestHandler(http.server.BaseHTTPRequestHandler):
    server_version = 'StranglerSmokeMockOpenAI/1.0'

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
        return

    def do_GET(self) -> None:  # noqa: N802
        if self.path.rstrip('/') == '/v1/models':
            self._write_json({
                'object': 'list',
                'data': [
                    {
                        'id': MOCK_OPENAI_MODEL_ID,
                        'object': 'model',
                        'owned_by': 'strangler-smoke',
                    }
                ],
            })
            return
        self.send_error(404)

    def do_POST(self) -> None:  # noqa: N802
        if self.path.rstrip('/') not in ('/v1/chat/completions', '/chat/completions'):
            self.send_error(404)
            return

        content_length = int(self.headers.get('Content-Length') or '0')
        request_body = b''
        if content_length > 0:
            request_body = self.rfile.read(content_length)

        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
        self.send_header('Cache-Control', 'no-cache')
        self.end_headers()
        for chunk in self._pick_stream_chunks(request_body):
            payload = {
                'choices': [
                    {
                        'delta': {
                            'content': chunk,
                        }
                    }
                ]
            }
            self.wfile.write(f'data: {json.dumps(payload, ensure_ascii=False)}\n\n'.encode('utf-8'))
        self.wfile.write(b'data: [DONE]\n\n')

    def _pick_stream_chunks(self, request_body: bytes) -> tuple[str, ...]:
        if not request_body:
            return MOCK_OPENAI_STREAM_CHUNKS
        try:
            payload = json.loads(request_body.decode('utf-8'))
        except (UnicodeDecodeError, json.JSONDecodeError):
            return MOCK_OPENAI_STREAM_CHUNKS

        messages = payload.get('messages')
        if not isinstance(messages, list):
            return MOCK_OPENAI_STREAM_CHUNKS

        joined_content = '\n'.join(
            str(item.get('content', ''))
            for item in messages
            if isinstance(item, dict)
        )
        if 'SINGLE_ORGANIZATION_GENERATION' in joined_content:
            return MOCK_OPENAI_ORGANIZATION_STREAM_CHUNKS
        if 'INSPIRATION_TITLE_SYSTEM' in joined_content or 'INSPIRATION_TITLE_USER' in joined_content:
            return MOCK_OPENAI_INSPIRATION_OPTIONS_CHUNKS
        if 'INSPIRATION_THEME_SYSTEM' in joined_content or 'INSPIRATION_THEME_USER' in joined_content:
            return MOCK_OPENAI_INSPIRATION_REFINE_CHUNKS
        if (
            'INSPIRATION_QUICK_COMPLETE' in joined_content
            or '请在不偏离现有信息的前提下补全缺失字段，只返回JSON。' in joined_content
        ):
            return MOCK_OPENAI_INSPIRATION_QUICK_CHUNKS

        return MOCK_OPENAI_STREAM_CHUNKS

    def _write_json(self, payload: Mapping[str, Any]) -> None:
        encoded = json.dumps(payload, ensure_ascii=False).encode('utf-8')
        self.send_response(200)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


def start_mock_openai_server() -> MockOpenAIServer:
    return MockOpenAIServer()


def manifest_uses_placeholder(manifest: Mapping[str, Any], placeholder: str) -> bool:
    marker = f'{{{{{placeholder}}}}}'
    compact_marker = f'{{{{ {placeholder} }}}}'
    return any(
        _value_contains_placeholder(probe, marker, compact_marker)
        for probe in manifest.get('probes', [])
    )


def _value_contains_placeholder(value: Any, *markers: str) -> bool:
    if isinstance(value, str):
        return any(marker in value for marker in markers)
    if isinstance(value, list):
        return any(_value_contains_placeholder(item, *markers) for item in value)
    if isinstance(value, dict):
        return any(_value_contains_placeholder(item, *markers) for item in value.values())
    return False


def render_template_string(value: str, state: Mapping[str, Any], *, label: str) -> str:
    def replace(match: re.Match[str]) -> str:
        key = match.group(1).strip()
        if key not in state:
            raise SmokeFailure(f'{label} references unknown placeholder {key!r}')

        replacement = state[key]
        if replacement is None:
            return ''
        if isinstance(replacement, (dict, list)):
            return json.dumps(replacement, ensure_ascii=False)
        return str(replacement)

    return PLACEHOLDER_PATTERN.sub(replace, value)


def render_template_value(value: Any, state: Mapping[str, Any], *, label: str) -> Any:
    if isinstance(value, str):
        full_match = PLACEHOLDER_PATTERN.fullmatch(value)
        if full_match is not None:
            key = full_match.group(1).strip()
            if key not in state:
                raise SmokeFailure(f'{label} references unknown placeholder {key!r}')
            return copy.deepcopy(state[key])
        return render_template_string(value, state, label=label)

    if isinstance(value, list):
        return [
            render_template_value(item, state, label=f'{label}[{index}]')
            for index, item in enumerate(value)
        ]

    if isinstance(value, dict):
        rendered: Dict[str, Any] = {}
        for key, item in value.items():
            rendered[key] = render_template_value(item, state, label=f'{label}.{key}')
        return rendered

    return value


def resolve_probe_templates(probe: Mapping[str, Any], state: Mapping[str, Any]) -> Dict[str, Any]:
    resolved = dict(probe)
    probe_name = str(probe.get('name') or '<unknown>')

    for field in ('path', 'body', 'expected_text_startswith'):
        value = resolved.get(field)
        if value is not None:
            resolved[field] = render_template_string(
                str(value),
                state,
                label=f'{probe_name}.{field}',
            )

    for field in (
        'headers',
        'json_body',
        'multipart_form',
        'expected_json',
        'expected_json_one_of',
        'expected_header_contains',
        'expected_text_contains',
        'expected_content_type_contains',
    ):
        value = resolved.get(field)
        if value is not None:
            resolved[field] = render_template_value(
                value,
                state,
                label=f'{probe_name}.{field}',
            )

    return resolved


def extract_json_value(value: Any, json_path: str, *, label: str) -> Any:
    normalized = json_path.strip()
    if not normalized:
        raise SmokeFailure(f'{label} must not be empty')

    if normalized == '$':
        return copy.deepcopy(value)

    if normalized.startswith('$.'):
        normalized = normalized[2:]
    elif normalized.startswith('$'):
        normalized = normalized[1:]

    segments = [segment for segment in normalized.split('.') if segment]
    if not segments:
        return copy.deepcopy(value)

    current = value
    traversed: List[str] = []
    for segment in segments:
        traversed.append(segment)
        current_path = '.'.join(traversed)
        if isinstance(current, dict):
            if segment not in current:
                raise SmokeFailure(f'{label} missing object field {segment!r} at {current_path!r}')
            current = current[segment]
            continue

        if isinstance(current, list):
            try:
                index = int(segment)
            except ValueError as exc:
                raise SmokeFailure(
                    f'{label} expected numeric array index at {current_path!r}; got {segment!r}'
                ) from exc

            if index < 0 or index >= len(current):
                raise SmokeFailure(
                    f'{label} array index out of range at {current_path!r}; len={len(current)}'
                )
            current = current[index]
            continue

        raise SmokeFailure(
            f'{label} cannot descend into {type(current).__name__} at {current_path!r}'
        )

    return copy.deepcopy(current)


def collect_probe_extracts(
    probe: Mapping[str, Any],
    response: Mapping[str, Any],
) -> Dict[str, Any]:
    extract_json = probe.get('extract_json') or {}
    if not extract_json:
        return {}

    extracted: Dict[str, Any] = {}
    actual_body = response.get('body')
    for state_key, json_path in extract_json.items():
        extracted[state_key] = extract_json_value(
            actual_body,
            str(json_path),
            label=f'{probe["name"]}.extract_json.{state_key}',
        )
    return extracted




def collect_response_headers(headers: Any) -> Dict[str, str]:
    collected: Dict[str, List[str]] = {}
    for name, value in headers.items():
        key = str(name)
        collected.setdefault(key, []).append(str(value))
    return {key: '\n'.join(values) for key, values in collected.items()}


def build_opener(
    cookie_jar: http.cookiejar.CookieJar | None = None,
) -> urllib.request.OpenerDirector:
    if cookie_jar is None:
        return urllib.request.build_opener()
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookie_jar))


def collect_cookie_names(cookie_jar: http.cookiejar.CookieJar) -> List[str]:
    return sorted({cookie.name for cookie in cookie_jar})


def selected_probes_require_login(probes: Sequence[Mapping[str, Any]]) -> bool:
    return any(bool(probe.get('requires_login')) for probe in probes)


def selected_probes_require_token_cookie(probes: Sequence[Mapping[str, Any]]) -> bool:
    return any(
        bool(probe.get('requires_login')) and str(probe.get('owner')) == 'rust'
        for probe in probes
    )


def ensure_login_cookies(
    cookie_jar: http.cookiejar.CookieJar,
    *,
    require_token_cookie: bool,
) -> List[str]:
    cookie_names = collect_cookie_names(cookie_jar)
    required_names = {'user_id', 'session_expire_at'}
    if require_token_cookie:
        required_names.add('token')

    missing = sorted(required_names.difference(cookie_names))
    if missing:
        raise SmokeFailure(
            'login did not produce required cookies; '
            f'missing={missing!r} cookies={cookie_names!r}'
        )
    return cookie_names


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


def subset_matches_one_of(actual: Any, expected_options: Sequence[Any], *, label: str) -> None:
    failures: List[str] = []
    for index, expected in enumerate(expected_options):
        try:
            subset_matches(actual, expected)
            return
        except SmokeFailure as exc:
            failures.append(f'option[{index}]: {exc}')

    raise SmokeFailure(
        f'JSON one-of assertion failed for {label}: '
        f'none matched; failures={failures!r} body_preview={body_preview(actual)}'
    )





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
    opener: urllib.request.OpenerDirector | None = None,
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
    urlopen = opener.open if opener is not None else urllib.request.urlopen

    try:

        with urlopen(request, timeout=timeout) as response:

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


def bootstrap_local_login_session(
    *,
    base_url: str,
    timeout: float,
    username: str,
    password: str,
    login_path: str,
    require_token_cookie: bool,
) -> tuple[urllib.request.OpenerDirector, Dict[str, Any]]:

    cookie_jar = http.cookiejar.CookieJar()
    opener = build_opener(cookie_jar)
    response = request_probe(
        base_url=base_url,
        path=login_path,
        method='POST',
        timeout=timeout,
        json_body={'username': username, 'password': password},
        opener=opener,
    )

    body = response.get('body')
    if response['status_code'] != 200 or not isinstance(body, dict) or body.get('success') is not True:
        raise SmokeFailure(
            'local login bootstrap failed: '
            f'status={response["status_code"]} '
            f'body_preview={body_preview(body)}'
        )

    cookie_names = ensure_login_cookies(
        cookie_jar,
        require_token_cookie=require_token_cookie,
    )
    return opener, {
        'path': login_path,
        'status_code': response['status_code'],
        'elapsed_ms': response['elapsed_ms'],
        'message': body.get('message'),
        'cookie_names': cookie_names,
        'user_id': body.get('user', {}).get('user_id') if isinstance(body.get('user'), dict) else None,
    }





def ensure_probe_expectations(probe: Mapping[str, Any], response: Mapping[str, Any]) -> None:

    expected_status = probe['expected_status']
    expected_statuses = probe.get('expected_statuses') or [expected_status]

    actual_status = response['status_code']

    if actual_status not in expected_statuses:

        raise SmokeFailure(

            f'status mismatch for {probe["name"]}: '

            f'expected={expected_statuses} actual={actual_status} '

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

    expected_json_one_of = probe.get('expected_json_one_of') or []
    if expected_json_one_of:
        actual_body = response.get('body')
        if not isinstance(actual_body, dict):
            raise SmokeFailure(
                f'JSON one-of assertion failed for {probe["name"]}: body is not an object; '
                f'content_type={response.get("content_type")} body_preview={body_preview(actual_body)}'
            )
        subset_matches_one_of(actual_body, expected_json_one_of, label=str(probe["name"]))

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


def run_probes(
    *,
    manifest: Mapping[str, Any],
    base_url: str,
    timeout: float,
    opener: urllib.request.OpenerDirector | None = None,
    initial_state: Mapping[str, Any] | None = None,
) -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    state: Dict[str, Any] = dict(initial_state or {})

    for probe in manifest['probes']:

        result: Dict[str, Any] = {

            'name': probe['name'],

            'owner': probe['owner'],

            'method': probe['method'].upper(),

            'path': probe['path'],

            'expected_status': probe['expected_status'],
            'requires_login': bool(probe.get('requires_login')),

            'ok': False,

        }

        try:
            resolved_probe = resolve_probe_templates(probe, state)
            result['path'] = resolved_probe['path']
            if resolved_probe['path'] != probe['path']:
                result['template_path'] = probe['path']

            request_kwargs: Dict[str, Any] = {
                'base_url': base_url,
                'path': resolved_probe['path'],
                'method': resolved_probe['method'],
                'timeout': timeout,
                'headers': resolved_probe.get('headers'),
                'body': resolved_probe.get('body'),
                'json_body': resolved_probe.get('json_body'),
                'multipart_form': resolved_probe.get('multipart_form'),
            }
            if opener is not None and probe.get('requires_login'):
                request_kwargs['opener'] = opener

            response = request_probe(**request_kwargs)
            ensure_probe_expectations(resolved_probe, response)
            extracted = collect_probe_extracts(resolved_probe, response)
            if extracted:
                state.update(extracted)
            result.update({
                'ok': True,
                'status_code': response['status_code'],
                'elapsed_ms': response['elapsed_ms'],

                'content_type': response['content_type'],

                'body_preview': body_preview(response['body']),
                'extracted': extracted,

                'assertions': {
                    'expected_json': resolved_probe.get('expected_json'),
                    'expected_json_has_keys': resolved_probe.get('expected_json_has_keys'),
                    'expected_content_type_contains': resolved_probe.get('expected_content_type_contains'),
                    'expected_text_startswith': resolved_probe.get('expected_text_startswith'),
                    'expected_text_contains': resolved_probe.get('expected_text_contains'),
                },
            })
            print(

                f"[OK] owner={probe['owner']} path={resolved_probe['path']} "

                f"status={response['status_code']} elapsed_ms={response['elapsed_ms']}"

            )

        except Exception as exc:  # noqa: BLE001

            result['error'] = str(exc)

            print(

                f"[FAIL] owner={probe['owner']} path={result['path']} error={exc}",

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


def emit_summary_json(summary: Mapping[str, Any], *, stream: Any) -> None:

    payload = json.dumps(summary, ensure_ascii=False, indent=2)
    try:
        print(payload, file=stream)
        return
    except UnicodeEncodeError:
        buffer = getattr(stream, 'buffer', None)
        if buffer is None:
            print(payload.encode('utf-8', errors='replace').decode('utf-8'), file=stream)
            return
        buffer.write((payload + '\n').encode('utf-8', errors='replace'))
        buffer.flush()





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


def summarize_route_group_readiness(
    probes: Sequence[Mapping[str, Any]]
) -> Dict[str, Dict[str, Any]]:
    readiness: Dict[str, Dict[str, Any]] = {}

    for probe in probes:
        route_group = probe.get('route_group')
        if not isinstance(route_group, str) or not route_group.strip():
            continue

        normalized_route_group = route_group.strip()
        owner = str(probe['owner'])
        profiles = [str(item) for item in probe.get('profiles') or []]

        entry = readiness.setdefault(
            normalized_route_group,
            {
                'probe_count': 0,
                'owner_counts': {},
                'profile_counts': {},
                'probe_names_by_owner': {},
                'probe_names_by_profile': {},
                'dedicated_profiles': {
                    'owner': [],
                    'fallback': [],
                    'asymmetric': [],
                },
                'readiness_flags': {
                    'has_rust_owner': False,
                    'has_python_fallback': False,
                    'has_business_smoke': False,
                    'has_asymmetric_evidence': False,
                    'has_dedicated_owner_profile': False,
                    'has_dedicated_fallback_profile': False,
                    'has_dedicated_asymmetric_profile': False,
                },
            },
        )

        entry['probe_count'] += 1
        owner_counts = entry['owner_counts']
        owner_counts[owner] = owner_counts.get(owner, 0) + 1
        entry['probe_names_by_owner'].setdefault(owner, []).append(str(probe['name']))

        flags = entry['readiness_flags']
        if owner == 'rust':
            flags['has_rust_owner'] = True
        if owner == 'python-fallback':
            flags['has_python_fallback'] = True

        profile_counts = entry['profile_counts']
        probe_names_by_profile = entry['probe_names_by_profile']
        dedicated_profiles = entry['dedicated_profiles']
        for profile in profiles:
            profile_counts[profile] = profile_counts.get(profile, 0) + 1
            probe_names_by_profile.setdefault(profile, []).append(str(probe['name']))

            if profile == 'business':
                flags['has_business_smoke'] = True

            if profile.endswith('-owner') and profile not in dedicated_profiles['owner']:
                dedicated_profiles['owner'].append(profile)
            if profile.endswith('-fallback') and profile not in dedicated_profiles['fallback']:
                dedicated_profiles['fallback'].append(profile)
            if 'asymmetric' in profile and profile not in dedicated_profiles['asymmetric']:
                dedicated_profiles['asymmetric'].append(profile)

        flags['has_dedicated_owner_profile'] = bool(dedicated_profiles['owner'])
        flags['has_dedicated_fallback_profile'] = bool(dedicated_profiles['fallback'])
        flags['has_dedicated_asymmetric_profile'] = bool(dedicated_profiles['asymmetric'])
        flags['has_asymmetric_evidence'] = flags['has_dedicated_asymmetric_profile']

    return readiness


def build_readiness_summary(probes: Sequence[Mapping[str, Any]]) -> Dict[str, Any]:
    summary = {
        'probe_count': len(probes),
        'route_group_readiness': summarize_route_group_readiness(probes),
    }
    summary.update(summarize_probe_inventory(probes))
    return summary


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
                'requires_login': bool(probe.get('requires_login')),
                'extract_json': probe.get('extract_json'),
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
        validated_manifest = validate_manifest(
            load_json_file(args.manifest.resolve()),
            manifest_path=args.manifest.resolve(),
        )
        readiness_manifest = select_probes_by_route_group(
            validated_manifest,
            route_groups=args.route_groups,
        )
        readiness_manifest = select_probes_by_name(
            readiness_manifest,
            probe_names=args.probe_names,
        )
        summary['readiness_summary'] = build_readiness_summary(
            readiness_manifest['probes']
        )

        if args.readiness_summary_only:
            summary['mode'] = 'readiness-summary-only'
            summary['ok'] = True
            summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')
            write_summary(output_path, summary)
            print(json.dumps(summary, ensure_ascii=False, indent=2))
            return 0

        manifest = select_probes_by_profile(validated_manifest, profile=args.profile)
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

        initial_state: Dict[str, Any] = {}
        with contextlib.ExitStack() as stack:
            if manifest_uses_placeholder(manifest, MOCK_OPENAI_BASE_URL_PLACEHOLDER):
                mock_openai = stack.enter_context(start_mock_openai_server())
                initial_state[MOCK_OPENAI_BASE_URL_PLACEHOLDER] = mock_openai.base_url
                summary['mock_openai'] = {
                    'base_url': mock_openai.base_url,
                    'model': MOCK_OPENAI_MODEL_ID,
                }

            opener = None
            if selected_probes_require_login(manifest['probes']):
                local_auth_username, local_auth_password, used_env_file = resolve_local_auth_credentials(
                    username=args.local_auth_username,
                    password=args.local_auth_password,
                    env_file=args.env_file,
                )
                opener, login_summary = bootstrap_local_login_session(
                    base_url=args.base_url,
                    timeout=args.http_timeout,
                    username=local_auth_username,
                    password=local_auth_password,
                    login_path=args.login_path,
                    require_token_cookie=selected_probes_require_token_cookie(manifest['probes']),
                )
                summary['login'] = {
                    **login_summary,
                    'username': local_auth_username,
                    'env_file': str(used_env_file) if used_env_file is not None else None,
                }

            results = run_probes(
                manifest=manifest,
                base_url=args.base_url,
                timeout=args.http_timeout,
                opener=opener,
                initial_state=initial_state,
            )

        summary['mode'] = 'probe'

        summary['probes'] = results

        summary['ok'] = all(bool(item.get('ok')) for item in results)

        failed = [item['name'] for item in results if not item.get('ok')]

        summary['failed_probe_names'] = failed

        if failed:

            raise SmokeFailure(f'gateway smoke failed for probes={failed}')



        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')

        write_summary(output_path, summary)

        emit_summary_json(summary, stream=sys.stdout)

        return 0

    except Exception as exc:  # noqa: BLE001

        summary['error'] = str(exc)

        summary['finished_at'] = time.strftime('%Y-%m-%dT%H:%M:%S')

        write_summary(output_path, summary)

        emit_summary_json(summary, stream=sys.stderr)

        return 1





if __name__ == '__main__':

    raise SystemExit(main())
