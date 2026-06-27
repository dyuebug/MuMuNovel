from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest
from email.message import Message


MODULE_PATH = Path(__file__).resolve().parents[2] / 'tools' / 'run_strangler_gateway_smoke.py'


def load_gateway_smoke_module():
    spec = importlib.util.spec_from_file_location('backend_tools_run_strangler_gateway_smoke', MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError('unable to load run_strangler_gateway_smoke.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_validate_manifest_accepts_expected_shape():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'expected_json': {'status': 'ok'},
                'headers': {'X-Probe': 'health'},
                'profiles': ['deploy', 'route-groups'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['manifest_version'] == 1
    assert len(validated['probes']) == 1
    assert validated['probes'][0]['owner'] == 'rust'
    assert validated['probes'][0]['headers'] == {'X-Probe': 'health'}
    assert validated['probes'][0]['profiles'] == ['deploy', 'route-groups']


def test_validate_manifest_accepts_requires_login_flag():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-get-business-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 200,
                'expected_json_has_keys': ['user_id', 'has_api_key', 'api_provider', 'llm_model'],
                'requires_login': True,
                'profiles': ['business'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['requires_login'] is True


def test_validate_manifest_accepts_extract_json_mapping():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-presets-create-business-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/presets',
                'json_body': {
                    'name': 'Smoke Preset',
                },
                'extract_json': {
                    'preset_id': '$.id',
                    'preset_name': '$.name',
                },
                'expected_status': 200,
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['extract_json'] == {
        'preset_id': '$.id',
        'preset_name': '$.name',
    }


def test_validate_manifest_rejects_duplicate_probe_names():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'duplicate',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
            },
            {
                'name': 'duplicate',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
            },
        ],
    }

    with pytest.raises(smoke.SmokeFailure, match='duplicate probe name'):
        smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))


def test_subset_matches_requires_nested_json_contract():
    smoke = load_gateway_smoke_module()

    smoke.subset_matches(
        {
            'status': 'ready',
            'checks': {
                'startup': {'ready': True, 'duration_ms': 12},
                'database': {'healthy': True, 'latency_ms': 4},
            },
            'extra': 'ignored',
        },
        {
            'status': 'ready',
            'checks': {
                'startup': {'ready': True},
                'database': {'healthy': True},
            },
        },
    )

    with pytest.raises(smoke.SmokeFailure, match=r'\$\.checks\.database\.healthy'):
        smoke.subset_matches(
            {'checks': {'database': {'healthy': False}}},
            {'checks': {'database': {'healthy': True}}},
        )


def test_run_probes_records_owner_path_and_error(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'expected_json': {'status': 'ok'},
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'expected_content_type_contains': ['text/html'],
            },
        ],
    }

    responses = {
        '/health': {
            'status_code': 200,
            'elapsed_ms': 4.2,
            'content_type': 'application/json; charset=utf-8',
            'body': {'status': 'ok', 'owner': 'rust'},
        },
        '/': {
            'status_code': 503,
            'elapsed_ms': 3.1,
            'content_type': 'text/html; charset=utf-8',
            'body': '<html>degraded</html>',
        },
    }

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
    ):
        return responses[path]

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert [item['owner'] for item in results] == ['rust', 'rust']
    assert results[0]['ok'] is True
    assert results[0]['status_code'] == 200
    assert results[1]['ok'] is False
    assert results[1]['path'] == '/'
    assert 'status mismatch' in results[1]['error']


def test_run_probes_only_passes_opener_for_requires_login_probe(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-business-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 200,
                'expected_json_has_keys': ['user_id'],
                'requires_login': True,
            },
            {
                'name': 'health-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'expected_json': {'status': 'ok'},
            },
        ],
    }
    captured = []
    opener = object()

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
        opener=None,
    ):
        captured.append({'path': path, 'opener': opener})
        if path == '/api/settings':
            return {
                'status_code': 200,
                'elapsed_ms': 3.2,
                'content_type': 'application/json; charset=utf-8',
                'headers': {},
                'body': {'user_id': 'local_test'},
            }
        return {
            'status_code': 200,
            'elapsed_ms': 2.1,
            'content_type': 'application/json; charset=utf-8',
            'headers': {},
            'body': {'status': 'ok'},
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(
        manifest=manifest,
        base_url='http://localhost:8005',
        timeout=10.0,
        opener=opener,
    )

    assert [item['ok'] for item in results] == [True, True]
    assert captured == [
        {'path': '/api/settings', 'opener': opener},
        {'path': '/health', 'opener': None},
    ]


def test_mock_openai_returns_inspiration_json_stream_chunks():
    smoke = load_gateway_smoke_module()
    handler = smoke._MockOpenAIRequestHandler.__new__(smoke._MockOpenAIRequestHandler)

    title_request = {
        'messages': [
            {
                'role': 'system',
                'content': '<prompt_template_key value="INSPIRATION_TITLE_SYSTEM" />',
            },
            {
                'role': 'user',
                'content': '我的想法是：边境旧港谜案',
            },
        ]
    }
    refine_request = {
        'messages': [
            {
                'role': 'system',
                'content': '<prompt_template_key value="INSPIRATION_THEME_SYSTEM" />',
            },
            {
                'role': 'user',
                'content': '请根据反馈重写主题',
            },
        ]
    }
    quick_request = {
        'messages': [
            {
                'role': 'system',
                'content': '<prompt_template_key value="INSPIRATION_QUICK_COMPLETE" />',
            },
            {
                'role': 'user',
                'content': '请在不偏离现有信息的前提下补全缺失字段，只返回JSON。',
            },
        ]
    }

    title_chunks = handler._pick_stream_chunks(
        json.dumps(title_request, ensure_ascii=False).encode('utf-8')
    )
    refine_chunks = handler._pick_stream_chunks(
        json.dumps(refine_request, ensure_ascii=False).encode('utf-8')
    )
    quick_chunks = handler._pick_stream_chunks(
        json.dumps(quick_request, ensure_ascii=False).encode('utf-8')
    )

    assert ''.join(title_chunks).startswith('{"prompt":"我先给你6个命名方向')
    assert '雾钟封港' in ''.join(title_chunks)
    assert ''.join(refine_chunks).startswith('{"prompt":"选择更贴合反馈的新方向')
    assert '唯一想保住的人' in ''.join(refine_chunks)
    assert ''.join(quick_chunks).startswith('{"title":"雾钟封港"')
    assert '"narrative_perspective":"第三人称"' in ''.join(quick_chunks)


def test_run_probes_uses_initial_state_for_template_resolution(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'chapter-regeneration-configure-mock-openai-business-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings',
                'json_body': {
                    'api_base_url': '{{mock_openai_base_url}}',
                    'llm_model': 'smoke-model',
                },
                'expected_status': 200,
                'expected_json': {
                    'api_base_url': '{{mock_openai_base_url}}',
                    'llm_model': 'smoke-model',
                },
            }
        ],
    }
    captured = {}

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
    ):
        captured['json_body'] = json_body
        return {
            'status_code': 200,
            'elapsed_ms': 2.4,
            'content_type': 'application/json; charset=utf-8',
            'headers': {},
            'body': {
                'api_base_url': json_body['api_base_url'],
                'llm_model': json_body['llm_model'],
            },
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(
        manifest=manifest,
        base_url='http://localhost:8005',
        timeout=10.0,
        initial_state={'mock_openai_base_url': 'http://127.0.0.1:3210/v1'},
    )

    assert results[0]['ok'] is True
    assert captured['json_body']['api_base_url'] == 'http://127.0.0.1:3210/v1'


def test_mock_openai_server_supports_models_and_streaming_chat_completions():
    smoke = load_gateway_smoke_module()

    with smoke.start_mock_openai_server() as server:
        models = smoke.request_probe(
            base_url=server.base_url,
            path='/models',
            method='GET',
            timeout=5.0,
        )
        completion = smoke.request_probe(
            base_url=server.base_url,
            path='/chat/completions',
            method='POST',
            timeout=5.0,
            json_body={
                'model': 'smoke-model',
                'messages': [{'role': 'user', 'content': 'rewrite'}],
                'stream': True,
            },
        )

    assert models['status_code'] == 200
    assert models['body']['data'][0]['id'] == 'smoke-model'
    assert completion['status_code'] == 200
    assert 'text/event-stream' in completion['content_type']
    assert '烟测改写成功。' in completion['body']
    assert 'data: [DONE]' in completion['body']


def test_manifest_uses_placeholder_detects_nested_mock_openai_reference():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'chapter-regeneration-configure-mock-openai-business-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings',
                'json_body': {
                    'api_base_url': '{{mock_openai_base_url}}',
                },
                'expected_status': 200,
            }
        ],
    }

    assert smoke.manifest_uses_placeholder(manifest, 'mock_openai_base_url') is True
    assert smoke.manifest_uses_placeholder(manifest, 'missing_placeholder') is False


def test_run_probes_passes_headers_and_json_body(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'wizard-outline-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/outline',
                'headers': {'X-Smoke-Probe': 'route-groups'},
                'json_body': {'projectId': 'test-project-id'},
                'expected_status': 401,
                'expected_json': {'detail': '未登录，请先登录'},
            }
        ],
    }
    captured = {}

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
    ):
        captured['base_url'] = base_url
        captured['path'] = path
        captured['method'] = method
        captured['timeout'] = timeout
        captured['headers'] = headers
        captured['body'] = body
        captured['json_body'] = json_body
        captured['multipart_form'] = multipart_form
        return {
            'status_code': 401,
            'elapsed_ms': 6.1,
            'content_type': 'application/json; charset=utf-8',
            'body': {'detail': '未登录，请先登录'},
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert results[0]['ok'] is True
    assert captured['headers'] == {'X-Smoke-Probe': 'route-groups'}
    assert captured['body'] is None
    assert captured['json_body'] == {'projectId': 'test-project-id'}
    assert captured['multipart_form'] is None


def test_run_probes_supports_extract_json_and_placeholder_templates(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-presets-create-business-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/presets',
                'json_body': {
                    'name': 'Smoke Preset',
                    'description': 'created by smoke',
                },
                'extract_json': {
                    'preset_id': '$.id',
                    'preset_name': '$.name',
                },
                'expected_status': 200,
                'expected_json': {
                    'id': 'preset_123',
                    'name': 'Smoke Preset',
                },
            },
            {
                'name': 'settings-presets-activate-business-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/presets/{{preset_id}}/activate',
                'headers': {
                    'X-Smoke-Preset': '{{preset_name}}',
                },
                'expected_status': 200,
                'expected_json': {
                    'preset_id': '{{preset_id}}',
                    'preset_name': '{{preset_name}}',
                },
            },
        ],
    }
    captured = []

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
    ):
        captured.append(
            {
                'path': path,
                'headers': headers,
                'json_body': json_body,
            }
        )
        if path == '/api/settings/presets':
            return {
                'status_code': 200,
                'elapsed_ms': 5.4,
                'content_type': 'application/json; charset=utf-8',
                'headers': {},
                'body': {'id': 'preset_123', 'name': 'Smoke Preset'},
            }

        return {
            'status_code': 200,
            'elapsed_ms': 3.6,
            'content_type': 'application/json; charset=utf-8',
            'headers': {},
            'body': {'preset_id': 'preset_123', 'preset_name': 'Smoke Preset'},
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert [item['ok'] for item in results] == [True, True]
    assert results[0]['extracted'] == {
        'preset_id': 'preset_123',
        'preset_name': 'Smoke Preset',
    }
    assert captured == [
        {
            'path': '/api/settings/presets',
            'headers': None,
            'json_body': {
                'name': 'Smoke Preset',
                'description': 'created by smoke',
            },
        },
        {
            'path': '/api/settings/presets/preset_123/activate',
            'headers': {'X-Smoke-Preset': 'Smoke Preset'},
            'json_body': None,
        },
    ]


def test_run_probes_rejects_unknown_placeholder_reference(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-presets-delete-business-rust',
                'owner': 'rust',
                'method': 'DELETE',
                'path': '/api/settings/presets/{{missing_preset_id}}',
                'expected_status': 200,
            }
        ],
    }

    def fake_request_probe(**kwargs):  # pragma: no cover - should never run
        raise AssertionError('request_probe should not be called when placeholder resolution fails')

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert results[0]['ok'] is False
    assert "unknown placeholder 'missing_preset_id'" in results[0]['error']


def test_bootstrap_local_login_session_requires_expected_cookies(monkeypatch):
    smoke = load_gateway_smoke_module()

    class FakeCookie:
        def __init__(self, name: str):
            self.name = name

    class FakeCookieJar:
        def __iter__(self):
            return iter([
                FakeCookie('token'),
                FakeCookie('user_id'),
                FakeCookie('session_expire_at'),
            ])

    fake_opener = object()

    monkeypatch.setattr(smoke.http.cookiejar, 'CookieJar', lambda: FakeCookieJar())
    monkeypatch.setattr(smoke, 'build_opener', lambda cookie_jar: fake_opener)

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
        opener=None,
    ):
        assert opener is fake_opener
        assert json_body == {'username': 'admin', 'password': 'secret'}
        return {
            'status_code': 200,
            'elapsed_ms': 6.4,
            'content_type': 'application/json; charset=utf-8',
            'headers': {},
            'body': {
                'success': True,
                'message': '登录成功',
                'user': {'user_id': 'local_test_user'},
            },
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    opener, summary = smoke.bootstrap_local_login_session(
        base_url='http://localhost:8005',
        timeout=10.0,
        username='admin',
        password='secret',
        login_path='/api/auth/local/login',
        require_token_cookie=True,
    )

    assert opener is fake_opener
    assert summary == {
        'path': '/api/auth/local/login',
        'status_code': 200,
        'elapsed_ms': 6.4,
        'message': '登录成功',
        'cookie_names': ['session_expire_at', 'token', 'user_id'],
        'user_id': 'local_test_user',
    }


def test_ensure_login_cookies_allows_python_style_cookie_set_without_token():
    smoke = load_gateway_smoke_module()

    class FakeCookie:
        def __init__(self, name: str):
            self.name = name

    cookie_names = smoke.ensure_login_cookies(
        [
            FakeCookie('user_id'),
            FakeCookie('session_expire_at'),
        ],
        require_token_cookie=False,
    )

    assert cookie_names == ['session_expire_at', 'user_id']


def test_run_probes_passes_multipart_form(monkeypatch):
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'book-import-create-task-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/book-import/tasks',
                'multipart_form': {
                    'fields': {
                        'import_mode': 'append',
                        'create_new_project': 'true',
                    },
                    'files': {
                        'file': {
                            'filename': 'test-import.txt',
                            'content': '测试内容',
                            'content_type': 'text/plain; charset=utf-8',
                        }
                    },
                },
                'expected_status': 401,
                'expected_json': {'detail': '未登录，请先登录'},
            }
        ],
    }
    captured = {}

    def fake_request_probe(
        *,
        base_url: str,
        path: str,
        method: str,
        timeout: float,
        headers=None,
        body=None,
        json_body=None,
        multipart_form=None,
    ):
        captured['headers'] = headers
        captured['body'] = body
        captured['json_body'] = json_body
        captured['multipart_form'] = multipart_form
        return {
            'status_code': 401,
            'elapsed_ms': 4.7,
            'content_type': 'application/json; charset=utf-8',
            'body': {'detail': '未登录，请先登录'},
        }

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert results[0]['ok'] is True
    assert captured['body'] is None
    assert captured['json_body'] is None
    assert captured['multipart_form'] == manifest['probes'][0]['multipart_form']


def test_summarize_probe_inventory_groups_owner_and_route_group():
    smoke = load_gateway_smoke_module()
    probes = [
        {
            'name': 'settings-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/settings',
            'expected_status': 401,
            'route_group': 'settings',
        },
        {
            'name': 'chapters-analysis-auth-guard-python-fallback',
            'owner': 'python-fallback',
            'method': 'GET',
            'path': '/api/chapters/test-chapter-id/analysis',
            'expected_status': 401,
            'route_group': 'chapters',
        },
        {
            'name': 'chapters-project-list-auth-guard-python-fallback',
            'owner': 'python-fallback',
            'method': 'GET',
            'path': '/api/chapters/project/test-project-id',
            'expected_status': 401,
            'route_group': 'chapters',
        },
        {
            'name': 'rust-health',
            'owner': 'rust',
            'method': 'GET',
            'path': '/health',
            'expected_status': 200,
        },
    ]

    summary = smoke.summarize_probe_inventory(probes)

    assert summary['owner_counts'] == {'rust': 2, 'python-fallback': 2}
    assert summary['route_group_counts'] == {'settings': 1, 'chapters': 2}
    assert summary['route_group_probe_names'] == {
        'settings': ['settings-auth-guard-rust'],
        'chapters': [
            'chapters-analysis-auth-guard-python-fallback',
            'chapters-project-list-auth-guard-python-fallback',
        ],
    }


def test_manifest_summary_includes_inventory_rollups():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['phase5-p0'],
            },
            {
                'name': 'chapters-project-list-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/chapters/project/test-project-id',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['phase5-p0-fallback'],
            },
        ],
    }

    summary = smoke.manifest_summary(
        manifest,
        manifest_path=Path('deploy/strangler-gateway-probes.json'),
    )

    assert summary['probe_count'] == 2
    assert summary['owner_counts'] == {'rust': 1, 'python-fallback': 1}
    assert summary['route_group_counts'] == {'settings': 1, 'chapters': 1}
    assert summary['route_group_probe_names'] == {
        'settings': ['settings-auth-guard-rust'],
        'chapters': ['chapters-project-list-auth-guard-python-fallback'],
    }


def test_summarize_route_group_readiness_rolls_up_owner_profile_and_business_flags():
    smoke = load_gateway_smoke_module()
    probes = [
        {
            'name': 'settings-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/settings',
            'expected_status': 401,
            'route_group': 'settings',
            'profiles': ['route-groups', 'phase5-p0', 'phase5-settings-owner'],
        },
        {
            'name': 'settings-presets-get-business-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/settings/presets',
            'expected_status': 200,
            'route_group': 'settings',
            'profiles': [
                'route-groups',
                'business',
                'phase5-p1',
                'phase5-settings-business',
                'phase5-settings-business-owner',
            ],
            'requires_login': True,
        },
        {
            'name': 'settings-test-business-rust',
            'owner': 'rust',
            'method': 'POST',
            'path': '/api/settings/test',
            'expected_status': 200,
            'route_group': 'settings',
            'profiles': [
                'route-groups',
                'business',
                'phase5-p1',
                'phase5-settings-business',
                'phase5-settings-business-owner',
            ],
            'requires_login': True,
        },
        {
            'name': 'projects-validate-import-public-rust',
            'owner': 'rust',
            'method': 'POST',
            'path': '/api/projects/validate-import',
            'expected_status': 200,
            'route_group': 'projects',
            'profiles': ['route-groups', 'phase5-p0', 'phase5-projects-owner', 'business'],
        },
        {
            'name': 'projects-list-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/projects',
            'expected_status': 401,
            'route_group': 'projects',
            'profiles': ['route-groups', 'phase5-p0', 'phase5-projects-owner'],
        },
        {
            'name': 'memories-list-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/memories/projects/test-project-id/memories',
            'expected_status': 401,
            'route_group': 'memories',
            'profiles': ['route-groups', 'phase5-p0'],
        },
        {
            'name': 'memories-search-auth-guard-rust',
            'owner': 'rust',
            'method': 'POST',
            'path': '/api/memories/projects/test-project-id/search?query=test',
            'expected_status': 401,
            'route_group': 'memories',
            'profiles': ['route-groups', 'phase5-p0'],
        },
    ]

    summary = smoke.summarize_route_group_readiness(probes)

    assert summary['settings']['probe_count'] == 3
    assert summary['settings']['owner_counts'] == {'rust': 3}
    assert summary['settings']['dedicated_profiles'] == {
        'owner': ['phase5-settings-owner', 'phase5-settings-business-owner'],
        'fallback': [],
        'asymmetric': [],
    }
    assert summary['settings']['readiness_flags'] == {
        'has_rust_owner': True,
        'has_python_fallback': False,
        'has_business_smoke': True,
        'has_asymmetric_evidence': False,
        'has_dedicated_owner_profile': True,
        'has_dedicated_fallback_profile': False,
        'has_dedicated_asymmetric_profile': False,
    }

    assert summary['projects']['probe_count'] == 2
    assert summary['projects']['owner_counts'] == {'rust': 2}
    assert summary['projects']['readiness_flags']['has_business_smoke'] is True
    assert summary['projects']['readiness_flags']['has_dedicated_owner_profile'] is True
    assert summary['projects']['readiness_flags']['has_dedicated_fallback_profile'] is False
    assert summary['projects']['readiness_flags']['has_dedicated_asymmetric_profile'] is False

    assert summary['memories']['probe_count'] == 2
    assert summary['memories']['owner_counts'] == {'rust': 2}
    assert summary['memories']['readiness_flags']['has_business_smoke'] is False
    assert summary['memories']['readiness_flags']['has_dedicated_owner_profile'] is False
    assert summary['memories']['readiness_flags']['has_dedicated_fallback_profile'] is False
    assert summary['memories']['readiness_flags']['has_dedicated_asymmetric_profile'] is False


def test_build_readiness_summary_includes_inventory_rollups():
    smoke = load_gateway_smoke_module()
    probes = [
        {
            'name': 'projects-validate-import-public-rust',
            'owner': 'rust',
            'method': 'POST',
            'path': '/api/projects/validate-import',
            'expected_status': 200,
            'route_group': 'projects',
            'profiles': ['phase5-projects-owner', 'business'],
        },
        {
            'name': 'projects-list-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/projects',
            'expected_status': 401,
            'route_group': 'projects',
            'profiles': ['phase5-projects-owner'],
        },
        {
            'name': 'memories-list-auth-guard-rust',
            'owner': 'rust',
            'method': 'GET',
            'path': '/api/memories/projects/test-project-id/memories',
            'expected_status': 401,
            'route_group': 'memories',
            'profiles': ['route-groups', 'phase5-p0'],
        },
        {
            'name': 'memories-search-auth-guard-rust',
            'owner': 'rust',
            'method': 'POST',
            'path': '/api/memories/projects/test-project-id/search?query=test',
            'expected_status': 401,
            'route_group': 'memories',
            'profiles': ['route-groups', 'phase5-p0'],
        },
    ]

    summary = smoke.build_readiness_summary(probes)

    assert summary['probe_count'] == 4
    assert summary['owner_counts'] == {'rust': 4}
    assert summary['route_group_counts'] == {'projects': 2, 'memories': 2}
    assert summary['route_group_readiness']['projects']['readiness_flags'] == {
        'has_rust_owner': True,
        'has_python_fallback': False,
        'has_business_smoke': True,
        'has_asymmetric_evidence': False,
        'has_dedicated_owner_profile': True,
        'has_dedicated_fallback_profile': False,
        'has_dedicated_asymmetric_profile': False,
    }
    assert summary['route_group_readiness']['memories']['readiness_flags'] == {
        'has_rust_owner': True,
        'has_python_fallback': False,
        'has_business_smoke': False,
        'has_asymmetric_evidence': False,
        'has_dedicated_owner_profile': False,
        'has_dedicated_fallback_profile': False,
        'has_dedicated_asymmetric_profile': False,
    }


def test_deploy_manifest_tracks_single_generation_active_readiness_owner():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    single_generation = summary['route_group_readiness']['chapter_single_generation']
    assert single_generation['probe_count'] == 6
    assert single_generation['owner_counts'] == {'rust': 6}
    assert single_generation['profile_counts']['phase5-single-generation-owner'] == 6
    assert single_generation['dedicated_profiles']['owner'] == [
        'phase5-single-generation-owner'
    ]
    assert single_generation['readiness_flags']['has_rust_owner'] is True
    assert single_generation['readiness_flags']['has_business_smoke'] is True
    assert single_generation['readiness_flags']['has_dedicated_owner_profile'] is True

    probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-single-generation-active-gateway-smoke-rust'
    )
    assert probe['expected_json']['probe_count'] == 2
    expected_probes = probe['expected_json']['probes']
    assert [item['name'] for item in expected_probes] == [
        'chapter-single-generation-active-gateway-rust-owner',
        'chapter-single-generation-active-gateway-fallback-freeze-candidate',
    ]
    assert [item['execution_path'] for item in expected_probes] == [
        'rust_candidate_executor',
        'rust_candidate_executor',
    ]
    expected_probe = expected_probes[0]
    assert (
        expected_probe['readiness_evidence']['owner_scope']
        == 'active_route_gateway_stream_background_runtime_terminal'
    )
    covered_rust_owners = expected_probe['readiness_evidence']['covered_rust_owners']
    assert covered_rust_owners[:2] == [
        'chapter_generation_routes',
        'chapter_candidate_route_gateway_service',
    ]
    assert 'chapter_single_generation_prepare_service' in covered_rust_owners
    assert 'chapter_single_generation_stream_workflow_service' in covered_rust_owners
    assert 'chapter_single_generation_runtime_state_service' in covered_rust_owners
    assert 'chapter_batch_generation_task_payload_base_service' in covered_rust_owners
    assert (
        'chapter_candidate_executor_production_adapter_service::quality_adapter_owner'
        in covered_rust_owners
    )
    assert 'chapter_candidate_record_service' in covered_rust_owners
    assert 'chapter_single_generation_runtime_restore_workflow_service' in covered_rust_owners
    assert 'chapter_single_generation_write_workflow_service' not in covered_rust_owners
    assert 'chapter_generation_task_semantics_service' not in covered_rust_owners
    assert 'chapter_candidate_provider_stream_service' not in covered_rust_owners
    assert 'chapter_candidate_quality_adapter_service' not in covered_rust_owners
    assert 'chapter_single_generation_stream_success_response_service' not in covered_rust_owners
    assert 'chapter_single_generation_background_response_service' not in covered_rust_owners
    assert 'chapter_single_generation_terminal_state_service' not in covered_rust_owners
    assert expected_probe['readiness_evidence']['python_source_map'] == []
    assert (
        expected_probe['readiness_evidence']['python_source_map_policy']['status']
        == 'source_map_only'
    )
    assert (
        expected_probe['readiness_evidence']['python_source_map_policy']
        ['python_bootstrap_status']
        == 'bootstrap_registration_retired_test_only_route_wiring_loader_remains'
    )
    assert (
        expected_probe['readiness_evidence']['active_gateway_cutover']
        ['python_bootstrap_registration']
        == 'bootstrap_registration_deleted_no_route_wiring_loader_remains'
    )
    assert (
        expected_probe['readiness_evidence']['python_source_map_policy']
        ['final_frozen_boundary']
        == 'single_generation_active_route_source_map_surface_empty_prepare_query_source_maps_deleted'
    )
    assert (
        expected_probe['readiness_evidence']['python_source_map_policy']
        ['shared_prepare_query_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['rollback_policy']['manifest_owner_baseline']
        == 'rust = 131, python-fallback = 0'
    )
    assert (
        expected_probe['readiness_evidence']['rollback_policy']
        ['python_source_map_action']
        == 'single_generation_active_route_source_maps_deleted_prerequisite_logic_rehomed_to_shared_chapter_query_source_map'
    )
    assert (
        expected_probe['readiness_evidence']['background_response']['message']
        == '单章后台生成任务已创建'
    )
    freeze_probe = expected_probes[1]
    assert (
        freeze_probe['readiness_evidence']['fallback_shrink_readiness']
        ['active_route_smoke_consumes_freeze_candidate']
        is True
    )
    assert (
        freeze_probe['readiness_evidence']['fallback_shrink_readiness']
        ['fallback_freeze_config_validated']
        is True
    )
    assert (
        freeze_probe['readiness_evidence']['fallback_shrink_readiness']
        ['python_fallback_removal_ready']
        is True
    )
    assert freeze_probe['readiness_evidence']['shared_candidate_runtime_owner_contract'][
        'python_source_map'
    ] == []
    assert freeze_probe['readiness_evidence']['shared_candidate_runtime_owner_contract'][
        'rollback_boundary'
    ]['single_generation_entry_source_maps'] == []
    assert (
        freeze_probe['readiness_evidence']['python_source_map_policy']
        ['final_frozen_boundary']
        == 'single_generation_active_route_source_map_surface_empty_prepare_query_source_maps_deleted'
    )
    assert (
        freeze_probe['readiness_evidence']['python_source_map_policy']
        ['shared_prepare_query_source_maps']
        == []
    )
    assert (
        probe['expected_json']['probes'][1]['readiness_evidence']['next_cutover_gate']
        == 'single-generation route entry source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/candidate and prepare/orchestration source-map packages'
    )

    batch_generation = summary['route_group_readiness']['chapter_batch_generation']
    expected_batch_probe_count = sum(
        1
        for item in manifest['probes']
        if item.get('route_group') == 'chapter_batch_generation'
        and item.get('owner') == 'rust'
    )
    expected_batch_owner_profile_count = sum(
        1
        for item in manifest['probes']
        if item.get('route_group') == 'chapter_batch_generation'
        and item.get('owner') == 'rust'
        and 'phase5-batch-generation-owner' in item.get('profiles', [])
    )
    assert batch_generation['probe_count'] == expected_batch_probe_count
    assert batch_generation['owner_counts'] == {'rust': expected_batch_probe_count}
    assert (
        batch_generation['profile_counts']['phase5-batch-generation-owner']
        == expected_batch_owner_profile_count
    )
    assert batch_generation['dedicated_profiles']['owner'] == [
        'phase5-batch-generation-owner'
    ]
    assert batch_generation['readiness_flags']['has_rust_owner'] is True
    assert batch_generation['readiness_flags']['has_business_smoke'] is True
    assert batch_generation['readiness_flags']['has_dedicated_owner_profile'] is True

    batch_probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-batch-generation-active-gateway-smoke-rust'
    )
    assert batch_probe['expected_json']['probe_count'] == 2
    expected_batch_probes = batch_probe['expected_json']['probes']
    assert [item['name'] for item in expected_batch_probes] == [
        'chapter-batch-generation-active-gateway-rust-owner',
        'chapter-batch-generation-active-gateway-fallback-freeze-candidate',
    ]
    expected_batch_probe = expected_batch_probes[0]
    covered_batch_rust_owners = expected_batch_probe['readiness_evidence'][
        'covered_rust_owners'
    ]
    assert covered_batch_rust_owners == [
        'chapter_batch_generation',
        'chapter_batch_generation_write_workflow_service',
        'chapter_batch_generation_runtime_state_service',
        'chapter_batch_generation_read_context_service',
        'chapter_batch_generation_resume_task_command_service',
        'chapter_batch_generation_task_payload_base_service',
        'chapter_candidate_route_gateway_service',
        'chapter_generation_runtime_service',
        'chapter_candidate_executor_production_adapter_service::quality_adapter_owner',
        'chapter_candidate_executor_production_adapter_service::quality_adapter_owner',
        'chapter_candidate_record_service',
    ]
    for retired_batch_owner in {
        'chapter_generation_task_semantics_service',
        'chapter_candidate_event_service',
        'chapter_candidate_provider_stream_service',
        'chapter_candidate_quality_adapter_service',
    }:
        assert retired_batch_owner not in covered_batch_rust_owners
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['python_bootstrap_status']
        == 'bootstrap_registration_deleted_no_route_wiring_loader_remains'
    )
    assert (
        expected_batch_probe['readiness_evidence']['active_gateway_cutover']
        ['python_bootstrap_registration']
        == 'bootstrap_registration_deleted_no_route_wiring_loader_remains'
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['full_module_freeze_ready']
        is True
    )
    assert expected_batch_probe['readiness_evidence']['python_source_map'] == []
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['freeze_scope']
        == 'batch_generation_route_package_source_map_surface'
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['read_context_source_maps']
        == []
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['shared_candidate_runtime_source_maps']
        == []
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['shared_projection_source_maps']
        == []
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        ['delete_candidate_boundary']
        == 'batch_generation_route_package_delete_completed_after_logged_in_db_smoke_and_test_seam_migration'
    )
    assert (
        expected_batch_probe['readiness_evidence']['python_source_map_policy']
        .get('frozen_module_files', [])
        == []
    )
    batch_freeze_probe = expected_batch_probes[1]
    assert (
        batch_freeze_probe['readiness_evidence']['fallback_shrink_readiness']
        ['python_fallback_removal_ready']
        is True
    )

    retired_fallback_probes = {
        'chapters-batch-active-tasks-auth-guard-python-fallback',
        'chapters-batch-stream-auth-guard-python-fallback',
        'chapters-batch-resume-auth-guard-python-fallback',
        'chapters-generate-background-auth-guard-python-fallback',
        'chapters-generate-stream-auth-guard-python-fallback',
        'chapters-regeneration-tasks-auth-guard-python-fallback',
        'chapters-analysis-auth-guard-python-fallback',
        'chapters-batch-analysis-status-auth-guard-python-fallback',
        'chapters-project-list-auth-guard-python-fallback',
        'chapters-batch-status-task-not-found-python-fallback',
        'chapters-batch-cancel-task-not-found-python-fallback',
        'users-current-auth-guard-python-fallback',
        'users-list-auth-guard-python-fallback',
        'users-set-admin-auth-guard-python-fallback',
        'users-reset-password-auth-guard-python-fallback',
        'book-import-create-task-auth-guard-python-fallback',
        'book-import-task-status-auth-guard-python-fallback',
        'book-import-preview-auth-guard-python-fallback',
        'book-import-cancel-auth-guard-python-fallback',
        'book-import-apply-auth-guard-python-fallback',
        'book-import-retry-stream-auth-guard-python-fallback',
        'book-import-apply-stream-auth-guard-python-fallback',
        'outlines-project-list-auth-guard-python-fallback',
        'outlines-list-auth-guard-python-fallback',
        'outlines-generate-stream-auth-guard-python-fallback',
        'outlines-batch-expand-stream-auth-guard-python-fallback',
        'outlines-create-chapters-from-plans-auth-guard-python-fallback',
        'characters-project-list-auth-guard-python-fallback',
        'characters-list-auth-guard-python-fallback',
        'characters-generate-stream-auth-guard-python-fallback',
        'characters-export-auth-guard-python-fallback',
        'characters-import-auth-guard-python-fallback',
        'auth-logout-public-python-fallback',
        'auth-user-auth-guard-python-fallback',
        'auth-password-status-auth-guard-python-fallback',
        'auth-password-set-auth-guard-python-fallback',
        'auth-password-initialize-auth-guard-python-fallback',
        'auth-refresh-auth-guard-python-fallback',
        'auth-callback-missing-code-python-fallback',
        'auth-local-login-invalid-credentials-python-fallback',
        'auth-bind-login-invalid-credentials-python-fallback',
        'characters-validate-import-auth-guard-python-fallback',
        'python-fallback-root',
    }
    manifest_probe_names = {item['name'] for item in manifest['probes']}
    route_group_counts = {
        item['route_group']: sum(
            1
            for probe in manifest['probes']
            if probe.get('route_group') == item['route_group']
            and probe.get('owner') == 'rust'
        )
        for item in manifest['probes']
        if item.get('route_group')
    }
    expected_rust_probe_count = sum(
        1 for item in manifest['probes'] if item.get('owner') == 'rust'
    )
    assert retired_fallback_probes.isdisjoint(manifest_probe_names)
    assert summary['owner_counts'].get('python-fallback', 0) == 0
    assert summary['owner_counts']['rust'] == expected_rust_probe_count
    users = summary['route_group_readiness']['users']
    assert users['owner_counts'] == {'rust': route_group_counts['users']}
    assert users['readiness_flags']['has_rust_owner'] is True
    assert users['readiness_flags']['has_python_fallback'] is False
    assert users['dedicated_profiles']['fallback'] == []
    admin = summary['route_group_readiness']['admin']
    assert admin['owner_counts'] == {'rust': route_group_counts['admin']}
    assert admin['readiness_flags']['has_rust_owner'] is True
    assert admin['readiness_flags']['has_python_fallback'] is False
    assert admin['readiness_flags']['has_business_smoke'] is True
    assert admin['dedicated_profiles']['owner'] == ['phase5-admin-business-owner']
    assert admin['dedicated_profiles']['fallback'] == []
    assert 'admin-users-list-business-rust' in admin['probe_names_by_profile']['business']
    assert 'admin-users-delete-business-rust' in admin['probe_names_by_profile']['business']
    auth = summary['route_group_readiness']['auth']
    assert auth['owner_counts'] == {'rust': route_group_counts['auth']}
    assert auth['readiness_flags']['has_rust_owner'] is True
    assert auth['readiness_flags']['has_python_fallback'] is False
    assert auth['dedicated_profiles']['fallback'] == []
    book_import = summary['route_group_readiness']['book_import']
    assert book_import['owner_counts'] == {'rust': route_group_counts['book_import']}
    assert book_import['readiness_flags']['has_rust_owner'] is True
    assert book_import['readiness_flags']['has_python_fallback'] is False
    assert book_import['dedicated_profiles']['fallback'] == []
    outlines = summary['route_group_readiness']['outlines']
    assert outlines['owner_counts'] == {'rust': route_group_counts['outlines']}
    assert outlines['readiness_flags']['has_rust_owner'] is True
    assert outlines['readiness_flags']['has_python_fallback'] is False
    assert outlines['dedicated_profiles']['fallback'] == []
    characters = summary['route_group_readiness']['characters']
    assert characters['owner_counts'] == {'rust': route_group_counts['characters']}
    assert characters['readiness_flags']['has_rust_owner'] is True
    assert characters['readiness_flags']['has_python_fallback'] is False
    assert characters['readiness_flags']['has_asymmetric_evidence'] is True
    assert characters['dedicated_profiles']['fallback'] == []
    assert characters['dedicated_profiles']['asymmetric'] == ['phase5-p1-asymmetric']
    ai_test = summary['route_group_readiness']['ai_test']
    assert ai_test['owner_counts'] == {'rust': route_group_counts['ai_test']}
    assert ai_test['readiness_flags']['has_rust_owner'] is True
    assert ai_test['readiness_flags']['has_python_fallback'] is False
    assert ai_test['readiness_flags']['has_asymmetric_evidence'] is True
    assert ai_test['dedicated_profiles']['owner'] == ['phase5-ai-test-owner']
    assert ai_test['dedicated_profiles']['fallback'] == []


def test_deploy_manifest_promotes_admin_logged_in_business_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    admin = summary['route_group_readiness']['admin']
    assert admin['readiness_flags']['has_rust_owner'] is True
    assert admin['readiness_flags']['has_python_fallback'] is False
    assert admin['readiness_flags']['has_business_smoke'] is True
    assert admin['dedicated_profiles']['owner'] == ['phase5-admin-business-owner']
    assert admin['dedicated_profiles']['fallback'] == []
    assert admin['owner_counts'] == {'rust': 11}

    profile_probes = [
        item
        for item in manifest['probes']
        if 'phase5-admin-business-owner' in item.get('profiles', [])
    ]
    profile_probe_names = [item['name'] for item in profile_probes]
    assert profile_probe_names == [
        'admin-users-fixture-register-target-rust',
        'admin-users-list-business-rust',
        'admin-users-update-business-rust',
        'admin-users-toggle-status-business-rust',
        'admin-users-reset-password-business-rust',
        'admin-users-delete-business-rust',
    ]

    fixture = profile_probes[0]
    assert fixture.get('route_group') is None
    assert fixture['extract_json'] == {
        'admin_business_target_user_id': '$.user.user_id'
    }

    business_route_probes = profile_probes[1:]
    assert all(item['route_group'] == 'admin' for item in business_route_probes)
    assert all(item['owner'] == 'rust' for item in business_route_probes)
    assert all(item['requires_login'] is True for item in business_route_probes)
    assert all('business' in item['profiles'] for item in business_route_probes)
    assert all('phase5-p1' in item['profiles'] for item in business_route_probes)
    assert [
        item['path']
        for item in business_route_probes
        if item['name'] != 'admin-users-list-business-rust'
    ] == [
        '/api/admin/users/{{admin_business_target_user_id}}',
        '/api/admin/users/{{admin_business_target_user_id}}/toggle-status',
        '/api/admin/users/{{admin_business_target_user_id}}/reset-password',
        '/api/admin/users/{{admin_business_target_user_id}}',
    ]


def test_deploy_manifest_promotes_chapters_candidate_gateway_business_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    chapters = summary['route_group_readiness']['chapters']
    assert chapters['readiness_flags']['has_rust_owner'] is True
    assert chapters['readiness_flags']['has_python_fallback'] is False
    assert chapters['readiness_flags']['has_business_smoke'] is True
    assert chapters['readiness_flags']['has_asymmetric_evidence'] is True
    assert chapters['readiness_flags']['has_dedicated_owner_profile'] is True
    assert chapters['owner_counts'] == {'rust': 18}
    assert chapters['dedicated_profiles']['fallback'] == []
    assert (
        'phase5-chapters-candidate-gateway-owner'
        in chapters['dedicated_profiles']['owner']
    )
    assert (
        'chapter-candidate-route-gateway-smoke-rust'
        in chapters['probe_names_by_profile']['business']
    )

    probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-candidate-route-gateway-smoke-rust'
    )
    assert probe['profiles'] == [
        'deploy',
        'route-groups',
        'business',
        'phase5-chapters-candidate-gateway-owner',
        'phase5-p1',
    ]
    assert probe['expected_json']['rollback_boundary'] == 'python_candidate_executor_fallback'
    assert probe['expected_json']['probe_count'] == 3
    expected_probes = probe['expected_json']['probes']
    assert [item['owner'] for item in expected_probes] == [
        'rust',
        'rust',
        'python-fallback',
    ]
    assert [item['execution_path'] for item in expected_probes] == [
        'rust_candidate_executor',
        'rust_candidate_executor',
        'python_fallback',
    ]
    assert (
        expected_probes[1]['name']
        == 'chapter-candidate-route-gateway-fallback-freeze-candidate'
    )
    assert (
        expected_probes[1]['readiness_evidence']['fallback_shrink_readiness']
        ['fallback_freeze_config_validated']
        is True
    )
    assert (
        expected_probes[1]['readiness_evidence']['fallback_shrink_readiness']
        ['python_fallback_removal_ready']
        is True
    )
    covered_rust_owners = expected_probes[0]['readiness_evidence'][
        'covered_rust_owners'
    ]
    assert covered_rust_owners == [
        'chapter_candidate_route_gateway_service',
        'chapter_candidate_executor_production_adapter_service',
        'chapter_candidate_executor_default_dependency_service',
        'chapter_candidate_executor_production_adapter_service::quality_adapter_owner',
        'chapter_candidate_executor_service',
        'chapter_candidate_generation_service',
        'chapter_candidate_record_service',
        'chapter_candidate_word_budget_repair_service',
        'chapter_candidate_targeted_final_repair_service',
        'chapter_candidate_finalize_service',
        'chapter_candidate_rerank_service',
        'chapter_candidate_runtime_state_service',
        'chapter_candidate_output_service',
    ]
    for retired_owner in {
        'chapter_candidate_provider_stream_service',
        'chapter_candidate_quality_adapter_service',
    }:
        assert retired_owner not in covered_rust_owners


def test_deploy_manifest_promotes_chapter_analysis_logged_in_route_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    analysis = summary['route_group_readiness']['chapter_analysis']
    assert analysis['owner_counts'] == {'rust': 8}
    assert analysis['readiness_flags']['has_rust_owner'] is True
    assert analysis['readiness_flags']['has_python_fallback'] is False
    assert analysis['readiness_flags']['has_business_smoke'] is True
    assert analysis['readiness_flags']['has_dedicated_owner_profile'] is True
    assert analysis['dedicated_profiles']['owner'] == [
        'phase5-chapter-analysis-owner'
    ]

    business_probe_names = analysis['probe_names_by_profile']['business']
    assert business_probe_names == [
        'chapter-analysis-view-logged-in-not-found-rust',
        'chapter-analysis-quality-metrics-logged-in-not-found-rust',
        'chapter-analysis-status-logged-in-not-found-rust',
        'chapter-analysis-trigger-logged-in-not-found-rust',
        'chapter-analysis-view-business-rust',
        'chapter-analysis-quality-metrics-business-rust',
        'chapter-analysis-status-business-rust',
        'chapter-analysis-batch-status-business-rust',
    ]
    assert all(
        item['requires_login'] is True
        for item in manifest['probes']
        if item['name'] in business_probe_names
    )
    assert {
        item['path']: item['expected_json']
        for item in manifest['probes']
        if item['name'] in business_probe_names[:4]
    } == {
        '/api/chapters/test-chapter-id/analysis': {
            'detail': 'Chapter not found or access denied'
        },
        '/api/chapters/test-chapter-id/quality-metrics': {
            'detail': 'Chapter not found or access denied'
        },
        '/api/chapters/test-chapter-id/analysis/status': {
            'detail': 'Chapter not found or access denied'
        },
        '/api/chapters/test-chapter-id/analyze': {
            'detail': 'Chapter not found or access denied'
        },
    }
    success_probes = [
        item for item in manifest['probes'] if item['name'] in business_probe_names[4:]
    ]
    assert [item['expected_status'] for item in success_probes] == [200, 200, 200, 200]
    assert success_probes[0]['expected_json']['analysis'] == {
        'plot_stage': 'opening',
        'conflict_level': 7,
        'hooks_count': 1,
        'plot_points_count': 1,
        'analysis_report': '分析烟测报告',
    }
    assert success_probes[1]['expected_json']['latest_metrics'] == {
        'quality_gate': {'decision': 'pass'},
        'score': 91,
    }
    assert success_probes[2]['expected_json'] == {
        'has_task': True,
        'chapter_id': '{{chapter_analysis_business_chapter_id}}',
        'status': 'completed',
        'progress': 100,
    }
    assert success_probes[3]['json_body'] == {
        'chapter_ids': ['{{chapter_analysis_business_chapter_id}}']
    }
    assert success_probes[3]['expected_json'] == {
        'project_id': '{{chapter_analysis_business_project_id}}',
        'total': 1,
    }
    assert success_probes[3]['expected_text_contains'] == [
        '{{chapter_analysis_business_chapter_id}}'
    ]
    profile_probe_names = [
        item['name']
        for item in manifest['probes']
        if 'phase5-chapter-analysis-owner' in item.get('profiles', [])
    ]
    assert profile_probe_names == [
        'chapter-analysis-view-logged-in-not-found-rust',
        'chapter-analysis-quality-metrics-logged-in-not-found-rust',
        'chapter-analysis-status-logged-in-not-found-rust',
        'chapter-analysis-trigger-logged-in-not-found-rust',
        'chapter-analysis-fixture-import-project-business-rust',
        'chapter-analysis-fixture-list-chapter-business-rust',
        'chapter-analysis-view-business-rust',
        'chapter-analysis-quality-metrics-business-rust',
        'chapter-analysis-status-business-rust',
        'chapter-analysis-batch-status-business-rust',
    ]
    fixture_probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-analysis-fixture-import-project-business-rust'
    )
    assert fixture_probe['extract_json'] == {
        'chapter_analysis_business_project_id': '$.project_id'
    }
    assert fixture_probe['expected_json']['statistics'] == {
        'chapters': 1,
        'story_memories': 1,
        'generation_history': 1,
        'plot_analysis': 1,
    }


def test_deploy_manifest_promotes_chapter_crud_logged_in_route_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    crud = summary['route_group_readiness']['chapter_crud']
    assert crud['owner_counts'] == {'rust': 13}
    assert crud['readiness_flags']['has_rust_owner'] is True
    assert crud['readiness_flags']['has_python_fallback'] is False
    assert crud['readiness_flags']['has_business_smoke'] is True
    assert crud['readiness_flags']['has_dedicated_owner_profile'] is True
    assert crud['dedicated_profiles']['owner'] == ['phase5-chapter-crud-owner']

    business_probe_names = crud['probe_names_by_profile']['business']
    assert business_probe_names == [
        'chapter-crud-list-logged-in-project-not-found-rust',
        'chapter-crud-project-list-logged-in-project-not-found-rust',
        'chapter-crud-detail-logged-in-not-found-rust',
        'chapter-crud-navigation-logged-in-not-found-rust',
        'chapter-crud-annotations-logged-in-not-found-rust',
        'chapter-crud-can-generate-logged-in-not-found-rust',
        'chapter-crud-quality-trend-logged-in-project-not-found-rust',
        'chapter-crud-project-list-business-rust',
        'chapter-crud-detail-business-rust',
        'chapter-crud-navigation-business-rust',
        'chapter-crud-annotations-business-rust',
        'chapter-crud-can-generate-business-rust',
        'chapter-crud-quality-trend-business-rust',
    ]
    owner_profile_names = [
        item['name']
        for item in manifest['probes']
        if 'phase5-chapter-crud-owner' in item.get('profiles', [])
    ]
    assert owner_profile_names == [
        'chapter-crud-list-logged-in-project-not-found-rust',
        'chapter-crud-project-list-logged-in-project-not-found-rust',
        'chapter-crud-detail-logged-in-not-found-rust',
        'chapter-crud-navigation-logged-in-not-found-rust',
        'chapter-crud-annotations-logged-in-not-found-rust',
        'chapter-crud-can-generate-logged-in-not-found-rust',
        'chapter-crud-quality-trend-logged-in-project-not-found-rust',
        'chapter-crud-fixture-import-project-business-rust',
        'chapter-crud-fixture-list-chapters-business-rust',
        'chapter-crud-project-list-business-rust',
        'chapter-crud-detail-business-rust',
        'chapter-crud-navigation-business-rust',
        'chapter-crud-annotations-business-rust',
        'chapter-crud-can-generate-business-rust',
        'chapter-crud-quality-trend-business-rust',
    ]
    probes_by_name = {
        item['name']: item
        for item in manifest['probes']
        if item['name'] in owner_profile_names
    }
    assert all(item['requires_login'] is True for item in probes_by_name.values())
    assert probes_by_name[
        'chapter-crud-list-logged-in-project-not-found-rust'
    ]['expected_json'] == {
        'success': False,
        'message': 'Project not found or access denied',
    }
    assert probes_by_name[
        'chapter-crud-project-list-logged-in-project-not-found-rust'
    ]['expected_json'] == {'detail': 'Project not found'}
    assert probes_by_name[
        'chapter-crud-detail-logged-in-not-found-rust'
    ]['expected_json'] == {
        'success': False,
        'message': 'Chapter not found or access denied',
    }
    assert probes_by_name[
        'chapter-crud-quality-trend-logged-in-project-not-found-rust'
    ]['expected_json'] == {
        'detail': 'Project not found or access denied'
    }
    assert probes_by_name[
        'chapter-crud-fixture-import-project-business-rust'
    ]['extract_json'] == {
        'chapter_crud_business_project_id': '$.project_id'
    }
    assert probes_by_name[
        'chapter-crud-fixture-import-project-business-rust'
    ]['expected_json']['statistics'] == {
        'chapters': 3,
        'story_memories': 1,
        'generation_history': 1,
    }
    assert probes_by_name[
        'chapter-crud-fixture-list-chapters-business-rust'
    ]['extract_json'] == {
        'chapter_crud_business_first_chapter_id': '$.items.0.id',
        'chapter_crud_business_chapter_id': '$.items.1.id',
        'chapter_crud_business_third_chapter_id': '$.items.2.id',
    }
    assert probes_by_name[
        'chapter-crud-project-list-business-rust'
    ]['expected_json']['items'][0] == {
        'title': 'CRUD烟测第一章',
        'chapter_number': 1,
    }
    assert probes_by_name['chapter-crud-detail-business-rust']['expected_json'] == {
        'success': True,
        'id': '{{chapter_crud_business_chapter_id}}',
        'project_id': '{{chapter_crud_business_project_id}}',
        'chapter_number': 2,
        'title': 'CRUD烟测第二章',
    }
    assert probes_by_name[
        'chapter-crud-navigation-business-rust'
    ]['expected_json']['current'] == {
        'id': '{{chapter_crud_business_chapter_id}}',
        'chapter_number': 2,
        'title': 'CRUD烟测第二章',
    }
    assert probes_by_name[
        'chapter-crud-annotations-business-rust'
    ]['expected_json']['summary'] == {
        'total_annotations': 1,
        'hooks': 1,
    }
    assert probes_by_name[
        'chapter-crud-can-generate-business-rust'
    ]['expected_json_has_keys'] == [
        'can_generate',
        'reason',
        'previous_chapters',
        'chapter_number',
    ]
    assert probes_by_name[
        'chapter-crud-quality-trend-business-rust'
    ]['expected_json']['items'][0] == {
        'chapter_id': '{{chapter_crud_business_chapter_id}}',
        'chapter_number': 2,
        'title': 'CRUD烟测第二章',
    }
    assert probes_by_name[
        'chapter-crud-quality-trend-business-rust'
    ]['expected_json_has_keys'] == ['quality_metrics_summary']


def test_deploy_manifest_promotes_chapter_draft_business_route_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    draft = summary['route_group_readiness']['chapter_draft']
    assert draft['owner_counts'] == {'rust': 8}
    assert draft['readiness_flags']['has_rust_owner'] is True
    assert draft['readiness_flags']['has_python_fallback'] is False
    assert draft['readiness_flags']['has_business_smoke'] is True
    assert draft['readiness_flags']['has_dedicated_owner_profile'] is True
    assert draft['dedicated_profiles']['owner'] == ['phase5-chapter-draft-owner']

    business_probe_names = draft['probe_names_by_profile']['business']
    assert business_probe_names == [
        'chapter-draft-auto-revision-load-logged-in-not-found-rust',
        'chapter-draft-auto-revision-apply-logged-in-not-found-rust',
        'chapter-draft-candidate-load-logged-in-not-found-rust',
        'chapter-draft-candidate-apply-logged-in-not-found-rust',
        'chapter-draft-auto-revision-load-business-rust',
        'chapter-draft-auto-revision-apply-business-rust',
        'chapter-draft-candidate-load-business-rust',
        'chapter-draft-candidate-apply-business-rust',
    ]
    probes = [
        item for item in manifest['probes'] if item['name'] in business_probe_names
    ]
    assert all(item['requires_login'] is True for item in probes)
    not_found_probes = probes[:4]
    success_probes = probes[4:]
    assert all(item['expected_status'] == 404 for item in not_found_probes)
    assert all(
        item['expected_json'] == {'detail': 'Chapter not found or access denied'}
        for item in not_found_probes
    )
    assert [item['expected_status'] for item in success_probes] == [200, 200, 200, 200]
    assert success_probes[0]['extract_json'] == {
        'chapter_draft_auto_revision_history_id': '$.auto_revision_draft.history_id'
    }
    assert success_probes[0]['expected_json']['auto_revision_draft'] == {
        'revised_text_preview': '自动修订烟测成功。第二段保留悬疑推进。',
        'revised_text': '自动修订烟测成功。第二段保留悬疑推进。',
        'has_full_text': True,
    }
    assert success_probes[1]['json_body'] == {
        'history_id': '{{chapter_draft_auto_revision_history_id}}',
        'allow_stale': True,
    }
    assert success_probes[2]['extract_json'] == {
        'chapter_draft_candidate_attempt_id': '$.candidate_draft.attempt_id'
    }
    assert success_probes[2]['expected_json']['candidate_draft'] == {
        'source': 'chapter',
        'attempt_state': 'manual_review',
        'quality_gate_action': 'manual_review',
        'content': '烟测改写成功。第二段继续推进。',
    }
    assert success_probes[3]['json_body'] == {
        'attempt_id': '{{chapter_draft_candidate_attempt_id}}',
        'allow_stale': True,
    }
    profile_probe_names = [
        item['name']
        for item in manifest['probes']
        if 'phase5-chapter-draft-owner' in item.get('profiles', [])
    ]
    assert profile_probe_names == [
        'chapter-draft-auto-revision-load-logged-in-not-found-rust',
        'chapter-draft-auto-revision-apply-logged-in-not-found-rust',
        'chapter-draft-candidate-load-logged-in-not-found-rust',
        'chapter-draft-candidate-apply-logged-in-not-found-rust',
        'chapter-draft-fixture-import-project-business-rust',
        'chapter-draft-fixture-list-chapter-business-rust',
        'chapter-draft-auto-revision-load-business-rust',
        'chapter-draft-auto-revision-apply-business-rust',
        'chapter-draft-configure-mock-openai-business-rust',
        'chapter-draft-generate-candidate-draft-business-rust',
        'chapter-draft-candidate-load-business-rust',
        'chapter-draft-candidate-apply-business-rust',
        'chapter-draft-cleanup-project-business-rust',
    ]
    generate_probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-draft-generate-candidate-draft-business-rust'
    )
    assert generate_probe.get('route_group') is None
    assert '"quality_gate_action":"manual_review"' in generate_probe['expected_text_contains']


def test_deploy_manifest_promotes_chapter_regeneration_stream_workflow_readiness():
    smoke = load_gateway_smoke_module()
    manifest_path = MODULE_PATH.parents[2] / 'deploy' / 'strangler-gateway-probes.json'
    manifest = smoke.validate_manifest(
        json.loads(manifest_path.read_text(encoding='utf-8')),
        manifest_path=manifest_path,
    )
    summary = smoke.build_readiness_summary(manifest['probes'])

    regeneration = summary['route_group_readiness']['chapter_regeneration']
    assert regeneration['owner_counts'] == {'rust': 13}
    assert regeneration['readiness_flags']['has_rust_owner'] is True
    assert regeneration['readiness_flags']['has_python_fallback'] is False
    assert regeneration['readiness_flags']['has_business_smoke'] is True
    assert regeneration['readiness_flags']['has_dedicated_owner_profile'] is True
    assert regeneration['dedicated_profiles']['owner'] == [
        'phase5-chapter-regeneration-owner'
    ]
    assert (
        'chapter-regeneration-stream-workflow-smoke-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-full-stream-logged-in-not-found-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-partial-stream-logged-in-not-found-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-apply-partial-logged-in-not-found-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-tasks-logged-in-not-found-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-fixture-import-project-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-fixture-list-chapter-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-configure-mock-openai-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-full-stream-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-partial-stream-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-apply-partial-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-tasks-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    assert (
        'chapter-regeneration-fixture-delete-project-business-rust'
        in regeneration['probe_names_by_profile']['business']
    )
    tasks_probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-regeneration-tasks-business-rust'
    )
    assert tasks_probe['expected_json'] == {
        'chapter_id': '{{chapter_regeneration_business_chapter_id}}',
        'total': 1,
        'tasks': [
            {
                'status': 'completed',
            }
        ],
    }
    assert tasks_probe['expected_json_has_keys'] == [
        'chapter_id',
        'total',
        'tasks',
    ]

    probe = next(
        item
        for item in manifest['probes']
        if item['name'] == 'chapter-regeneration-stream-workflow-smoke-rust'
    )
    assert probe['profiles'] == [
        'deploy',
        'route-groups',
        'business',
        'phase5-p1',
        'phase5-chapter-regeneration-owner',
    ]
    assert probe['expected_json']['rollback_boundary'] == (
        'chapter_regeneration_python_source_map'
    )
    expected_probe = probe['expected_json']['probes'][0]
    assert expected_probe['result']['full_stream_owner_consumed'] is True
    assert expected_probe['result']['partial_stream_owner_consumed'] is True
    assert expected_probe['readiness_evidence']['python_source_map'] == []
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['full_module_freeze_ready']
        is True
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']['freeze_scope']
        == 'chapter_regeneration_route_package_source_map_surface'
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['stream_orchestration_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['prepare_owner_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['shared_prepare_dependency_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['shared_context_compaction_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']
        ['query_owner_source_maps']
        == []
    )
    assert (
        expected_probe['readiness_evidence']['source_map_policy']['freeze_reason']
        == 'Rust regeneration route group has dedicated owner-profile business probes for full stream, partial stream, apply partial, task list, and cleanup; the production chapter_regeneration route shell is now physically deleted, and the surviving Python follow-up work sits outside this direct route/workflow source-map package.'
    )
    assert (
        expected_probe['readiness_evidence']['next_cutover_gate']
        == 'chapter-regeneration route/workflow source-map package is physically closed out; any surviving Python work is outside this direct regeneration package'
    )


def test_ensure_probe_expectations_supports_text_assertions():
    smoke = load_gateway_smoke_module()
    probe = {
        'name': 'wizard-stream-sse',
        'owner': 'rust',
        'method': 'POST',
        'path': '/api/wizard-stream/outline',
        'expected_status': 200,
        'expected_content_type_contains': ['text/event-stream'],
        'expected_text_startswith': 'event: progress',
        'expected_text_contains': ['data:', 'processing'],
    }
    response = {
        'status_code': 200,
        'elapsed_ms': 5.0,
        'content_type': 'text/event-stream; charset=utf-8',
        'body': 'event: progress\ndata: {"status":"processing"}\n\n',
    }

    smoke.ensure_probe_expectations(probe, response)

    with pytest.raises(smoke.SmokeFailure, match='text assertion failed'):
        smoke.ensure_probe_expectations(
            probe,
            {
                **response,
                'body': 'event: progress\ndata: {"status":"queued"}\n\n',
            },
        )


def test_ensure_probe_expectations_supports_json_key_assertions():
    smoke = load_gateway_smoke_module()
    probe = {
        'name': 'changelog-public-rust',
        'owner': 'rust',
        'method': 'GET',
        'path': '/api/changelog',
        'expected_status': 200,
        'expected_json_has_keys': ['commits', 'cached', 'cache_time'],
    }
    response = {
        'status_code': 200,
        'elapsed_ms': 5.0,
        'content_type': 'application/json; charset=utf-8',
        'body': {'commits': [], 'cached': True, 'cache_time': '2026-05-19T11:21:22.521060285+00:00'},
    }

    smoke.ensure_probe_expectations(probe, response)

    with pytest.raises(smoke.SmokeFailure, match='JSON key assertion failed'):
        smoke.ensure_probe_expectations(
            probe,
            {
                **response,
                'body': {'commits': [], 'cached': True},
            },
        )


def test_ensure_probe_expectations_supports_json_one_of_and_statuses():
    smoke = load_gateway_smoke_module()
    probe = {
        'name': 'batch-status-runtime-rust',
        'owner': 'rust',
        'method': 'GET',
        'path': '/api/chapters/batch-generate/task-1/status',
        'expected_status': 200,
        'expected_statuses': [200, 400],
        'expected_json_one_of': [
            {'status': 'pending', 'batch_id': 'task-1'},
            {'status': 'running', 'batch_id': 'task-1'},
            {'detail': 'Cannot cancel task in status failed'},
        ],
    }

    smoke.ensure_probe_expectations(
        probe,
        {
            'status_code': 200,
            'elapsed_ms': 5.0,
            'content_type': 'application/json; charset=utf-8',
            'body': {'status': 'running', 'batch_id': 'task-1'},
        },
    )
    smoke.ensure_probe_expectations(
        probe,
        {
            'status_code': 400,
            'elapsed_ms': 5.0,
            'content_type': 'application/json; charset=utf-8',
            'body': {'detail': 'Cannot cancel task in status failed'},
        },
    )

    with pytest.raises(smoke.SmokeFailure, match='JSON one-of assertion failed'):
        smoke.ensure_probe_expectations(
            probe,
            {
                'status_code': 200,
                'elapsed_ms': 5.0,
                'content_type': 'application/json; charset=utf-8',
                'body': {'status': 'completed', 'batch_id': 'task-1'},
            },
        )


def test_validate_manifest_accepts_header_assertion():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'auth-logout-public-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/auth/logout',
                'expected_status': 200,
                'expected_header_contains': {'Set-Cookie': 'token='},
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['expected_header_contains'] == {'Set-Cookie': 'token='}


def test_collect_response_headers_preserves_repeated_set_cookie_values():
    smoke = load_gateway_smoke_module()
    headers = Message()
    headers.add_header('Set-Cookie', 'token=; Path=/; Max-Age=0')
    headers.add_header('Set-Cookie', 'user_id=; Path=/; Max-Age=0')
    headers.add_header('Content-Type', 'application/json; charset=utf-8')

    collected = smoke.collect_response_headers(headers)

    assert collected['Set-Cookie'] == 'token=; Path=/; Max-Age=0\nuser_id=; Path=/; Max-Age=0'
    assert collected['Content-Type'] == 'application/json; charset=utf-8'


def test_ensure_probe_expectations_supports_header_assertions():
    smoke = load_gateway_smoke_module()
    probe = {
        'name': 'auth-logout-public-rust',
        'owner': 'rust',
        'method': 'POST',
        'path': '/api/auth/logout',
        'expected_status': 200,
        'expected_header_contains': {'Set-Cookie': 'token='},
    }
    response = {
        'status_code': 200,
        'elapsed_ms': 4.0,
        'content_type': 'application/json; charset=utf-8',
        'headers': {'Set-Cookie': 'token=; Path=/; Max-Age=0'},
        'body': {'success': True, 'message': '已登出'},
    }

    smoke.ensure_probe_expectations(probe, response)

    with pytest.raises(smoke.SmokeFailure, match='header assertion failed'):
        smoke.ensure_probe_expectations(
            probe,
            {
                **response,
                'headers': {'Set-Cookie': 'session_expire_at=; Path=/; Max-Age=0'},
            },
        )

    multi_header_response = {
        **response,
        'headers': {
            'Set-Cookie': 'session_expire_at=; Path=/; Max-Age=0\ntoken=; Path=/; Max-Age=0'
        },
    }
    smoke.ensure_probe_expectations(probe, multi_header_response)


def test_validate_manifest_rejects_conflicting_request_body_fields():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'invalid-body',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/analysis/status/batch',
                'body': '{}',
                'json_body': {'chapter_ids': []},
                'expected_status': 401,
            }
        ],
    }

    with pytest.raises(smoke.SmokeFailure, match='must not define more than one of body, json_body, multipart_form'):
        smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))


def test_validate_manifest_accepts_json_key_assertion():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'changelog-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/changelog',
                'expected_status': 200,
                'expected_json_has_keys': ['commits', 'cached', 'cache_time'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['expected_json_has_keys'] == ['commits', 'cached', 'cache_time']


def test_validate_manifest_accepts_multipart_form():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'book-import-create-task-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/book-import/tasks',
                'multipart_form': {
                    'fields': {
                        'import_mode': 'append',
                        'create_new_project': 'true',
                    },
                    'files': {
                        'file': {
                            'filename': 'test-import.txt',
                            'content': '测试内容',
                            'content_type': 'text/plain; charset=utf-8',
                        }
                    },
                },
                'expected_status': 401,
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['multipart_form']['fields'] == {
        'import_mode': 'append',
        'create_new_project': 'true',
    }
    assert validated['probes'][0]['multipart_form']['files']['file'] == {
        'filename': 'test-import.txt',
        'content': '测试内容',
        'content_type': 'text/plain; charset=utf-8',
    }


def test_encode_multipart_form_data_builds_expected_sections():
    smoke = load_gateway_smoke_module()
    payload, boundary = smoke.encode_multipart_form_data(
        {
            'fields': {'import_mode': 'append'},
            'files': {
                'file': {
                    'filename': 'test-import.txt',
                    'content': 'hello world',
                    'content_type': 'text/plain; charset=utf-8',
                }
            },
        }
    )

    text = payload.decode('utf-8')
    assert boundary in text
    assert 'Content-Disposition: form-data; name="import_mode"' in text
    assert 'Content-Disposition: form-data; name="file"; filename="test-import.txt"' in text
    assert 'Content-Type: text/plain; charset=utf-8' in text
    assert 'hello world' in text


def test_select_probes_by_profile_supports_business_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'profiles': ['deploy'],
            },
            {
                'name': 'auth-config-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/config',
                'expected_status': 200,
                'profiles': ['route-groups', 'business'],
            },
            {
                'name': 'auth-logout-public-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/auth/logout',
                'expected_status': 200,
                'profiles': ['route-groups', 'business', 'phase5-p1'],
            },
            {
                'name': 'auth-linuxdo-url-misconfig-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/linuxdo/url',
                'expected_status': 400,
                'profiles': ['route-groups', 'business', 'phase5-p1'],
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy', 'business'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    business = smoke.select_probes_by_profile(validated, profile='business')

    assert [probe['name'] for probe in business['probes']] == [
        'auth-config-public-rust',
        'auth-logout-public-rust',
        'auth-linuxdo-url-misconfig-rust',
        'rust-spa-root',
    ]


def test_select_probes_by_profile_supports_phase5_p0_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'projects-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/projects',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'settings-models-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings/models?provider=openai&api_key=test-key&api_base_url=http://127.0.0.1:9/v1',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'settings-fetch-models-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/fetch-models',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'settings-test-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/test',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'settings-check-function-calling-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/settings/check-function-calling',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'wizard-stream-outline-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/outline',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'wizard-stream-cleanup-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/cleanup/test-project-id',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'wizard-stream-career-system-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/career-system',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'wizard-stream-characters-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/characters',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-batch-stream-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/test-batch-id/stream',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-batch-resume-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/batch-generate/test-batch-id/resume',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-generate-background-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/test-chapter-id/generate-background',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-generate-stream-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/test-chapter-id/generate-stream',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'auth-config-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/config',
                'expected_status': 200,
                'profiles': ['route-groups', 'business'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    phase5_p0 = smoke.select_probes_by_profile(validated, profile='phase5-p0')

    assert [probe['name'] for probe in phase5_p0['probes']] == [
        'settings-auth-guard-rust',
        'projects-list-auth-guard-rust',
        'settings-models-auth-guard-rust',
        'settings-fetch-models-auth-guard-rust',
        'settings-test-auth-guard-rust',
        'settings-check-function-calling-auth-guard-rust',
        'wizard-stream-outline-auth-guard-rust',
        'wizard-stream-cleanup-auth-guard-rust',
        'wizard-stream-career-system-auth-guard-rust',
        'wizard-stream-characters-auth-guard-rust',
        'chapters-batch-stream-auth-guard-rust',
        'chapters-batch-resume-auth-guard-rust',
        'chapters-generate-background-auth-guard-rust',
        'chapters-generate-stream-auth-guard-rust',
    ]


def test_select_probes_by_profile_supports_phase5_p0_fallback_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy', 'business', 'phase5-p0-fallback'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    phase5_p0_fallback = smoke.select_probes_by_profile(validated, profile='phase5-p0-fallback')

    assert [probe['name'] for probe in phase5_p0_fallback['probes']] == [
        'rust-spa-root',
    ]


def test_select_probes_by_profile_supports_phase5_p0_asymmetric_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'chapters-batch-status-auth-guard-rust-asymmetric',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/test-batch-id/status',
                'route_group': 'chapters',
                'expected_status': 401,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'chapters-batch-status-task-not-found-rust-asymmetric',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/test-batch-id/status',
                'route_group': 'chapters',
                'expected_status': 404,
                'requires_login': True,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'chapters-batch-cancel-auth-guard-rust-asymmetric',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/batch-generate/test-batch-id/cancel',
                'route_group': 'chapters',
                'expected_status': 401,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'chapters-batch-cancel-task-not-found-rust-asymmetric',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/chapters/batch-generate/test-batch-id/cancel',
                'route_group': 'chapters',
                'expected_status': 404,
                'requires_login': True,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    phase5_p0_asymmetric = smoke.select_probes_by_profile(validated, profile='phase5-p0-asymmetric')

    assert [probe['name'] for probe in phase5_p0_asymmetric['probes']] == [
        'chapters-batch-status-auth-guard-rust-asymmetric',
        'chapters-batch-status-task-not-found-rust-asymmetric',
        'chapters-batch-cancel-auth-guard-rust-asymmetric',
        'chapters-batch-cancel-task-not-found-rust-asymmetric',
    ]


def test_select_probes_by_profile_supports_phase5_p1_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'auth-config-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/config',
                'expected_status': 200,
                'route_group': 'auth',
                'profiles': ['route-groups', 'business', 'phase5-p1'],
            },
            {
                'name': 'auth-logout-public-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/auth/logout',
                'expected_status': 200,
                'route_group': 'auth',
                'profiles': ['route-groups', 'business', 'phase5-p1'],
            },
            {
                'name': 'auth-linuxdo-url-misconfig-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/linuxdo/url',
                'expected_status': 400,
                'route_group': 'auth',
                'profiles': ['route-groups', 'business', 'phase5-p1'],
            },
            {
                'name': 'users-current-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/users/current',
                'expected_status': 401,
                'route_group': 'users',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'auth-user-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/user',
                'expected_status': 401,
                'route_group': 'auth',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'users-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/users',
                'expected_status': 401,
                'route_group': 'users',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'auth-password-status-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/password/status',
                'expected_status': 401,
                'route_group': 'auth',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'background-tasks-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/background-tasks',
                'expected_status': 401,
                'route_group': 'background_tasks',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'background-tasks-create-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/background-tasks',
                'expected_status': 401,
                'route_group': 'background_tasks',
                'profiles': ['route-groups', 'phase5-p1'],
            },
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['route-groups', 'phase5-p0'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    phase5_p1 = smoke.select_probes_by_profile(validated, profile='phase5-p1')

    assert [probe['name'] for probe in phase5_p1['probes']] == [
        'auth-config-public-rust',
        'auth-logout-public-rust',
        'auth-linuxdo-url-misconfig-rust',
        'users-current-auth-guard-rust',
        'auth-user-auth-guard-rust',
        'users-list-auth-guard-rust',
        'auth-password-status-auth-guard-rust',
        'background-tasks-list-auth-guard-rust',
        'background-tasks-create-auth-guard-rust',
    ]


def test_validate_manifest_accepts_health_db_sessions_probe():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health-db-sessions',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health/db-sessions',
                'expected_status': 200,
                'expected_json': {
                    'status': 'ok',
                    'session_stats': {'active': 0, 'idle': 0, 'total': 0},
                    'warning': None,
                },
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['path'] == '/health/db-sessions'


def test_validate_manifest_accepts_livez_probe():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-livez',
                'owner': 'rust',
                'method': 'GET',
                'path': '/livez',
                'expected_status': 200,
                'expected_json': {'status': 'ok'},
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['path'] == '/livez'


def test_select_probes_by_profile_filters_manifest():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'profiles': ['deploy'],
            },
            {
                'name': 'settings-auth-guard',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'profiles': ['route-groups'],
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    deploy_only = smoke.select_probes_by_profile(validated, profile='deploy')
    route_groups = smoke.select_probes_by_profile(validated, profile='route-groups')

    assert [probe['name'] for probe in deploy_only['probes']] == ['rust-health', 'rust-spa-root']
    assert [probe['name'] for probe in route_groups['probes']] == ['settings-auth-guard', 'rust-spa-root']


def test_select_probes_by_profile_supports_business_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'profiles': ['deploy'],
            },
            {
                'name': 'auth-config-public-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/auth/config',
                'expected_status': 200,
                'profiles': ['route-groups', 'business'],
            },
            {
                'name': 'rust-spa-root',
                'owner': 'rust',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy', 'business'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    business = smoke.select_probes_by_profile(validated, profile='business')

    assert [probe['name'] for probe in business['probes']] == ['auth-config-public-rust', 'rust-spa-root']


def test_select_probes_by_profile_rejects_empty_match():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'profiles': ['deploy'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    with pytest.raises(smoke.SmokeFailure, match='no probes matched profile'):
        smoke.select_probes_by_profile(validated, profile='route-groups')


def test_select_probes_by_name_filters_manifest():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'projects-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/projects',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'wizard-stream-outline-auth-guard-rust',
                'owner': 'rust',
                'method': 'POST',
                'path': '/api/wizard-stream/outline',
                'expected_status': 401,
                'profiles': ['route-groups', 'phase5-p0'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    selected = smoke.select_probes_by_name(
        validated,
        probe_names=['wizard-stream-outline-auth-guard-rust', 'settings-auth-guard-rust'],
    )

    assert [probe['name'] for probe in selected['probes']] == [
        'settings-auth-guard-rust',
        'wizard-stream-outline-auth-guard-rust',
    ]


def test_select_probes_by_name_rejects_missing_probe():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'profiles': ['deploy'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    with pytest.raises(smoke.SmokeFailure, match='no probes matched names'):
        smoke.select_probes_by_name(validated, probe_names=['missing-probe'])


def test_validate_manifest_accepts_route_group():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['route-groups', 'phase5-p0'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['probes'][0]['route_group'] == 'settings'


def test_select_probes_by_route_group_filters_manifest():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters?project_id=test-project-id',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-project-list-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters/project/test-project-id',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['route-groups', 'phase5-p0'],
            },
            {
                'name': 'chapters-analysis-auth-guard-rust',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/chapters/test-chapter-id/analysis',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['route-groups', 'phase5-p0'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    selected = smoke.select_probes_by_route_group(validated, route_groups=['chapters'])

    assert [probe['name'] for probe in selected['probes']] == [
        'chapters-list-auth-guard-rust',
        'chapters-project-list-auth-guard-rust',
        'chapters-analysis-auth-guard-rust',
    ]


def test_select_probes_by_route_group_rejects_missing_group():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'rust-health',
                'owner': 'rust',
                'method': 'GET',
                'path': '/health',
                'expected_status': 200,
                'route_group': 'health',
                'profiles': ['deploy'],
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    with pytest.raises(smoke.SmokeFailure, match='no probes matched route_groups'):
        smoke.select_probes_by_route_group(validated, route_groups=['settings'])
