from __future__ import annotations

import importlib.util
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
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
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

    assert [item['owner'] for item in results] == ['rust', 'python-fallback']
    assert results[0]['ok'] is True
    assert results[0]['status_code'] == 200
    assert results[1]['ok'] is False
    assert results[1]['path'] == '/'
    assert 'status mismatch' in results[1]['error']


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
            'name': 'chapters-batch-active-tasks-auth-guard-python-fallback',
            'owner': 'python-fallback',
            'method': 'GET',
            'path': '/api/chapters/batch-generate/active-tasks',
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
            'chapters-batch-active-tasks-auth-guard-python-fallback',
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
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
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
        'python-fallback-root',
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
                'name': 'settings-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/settings',
                'expected_status': 401,
                'route_group': 'settings',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'chapters-batch-active-tasks-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/active-tasks',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['phase5-p0-fallback'],
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
            {
                'name': 'wizard-stream-career-system-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'POST',
                'path': '/api/wizard-stream/career-system',
                'expected_status': 401,
                'route_group': 'wizard-stream',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'wizard-stream-characters-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'POST',
                'path': '/api/wizard-stream/characters',
                'expected_status': 401,
                'route_group': 'wizard-stream',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'chapters-batch-stream-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/test-batch-id/stream',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'chapters-batch-resume-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'POST',
                'path': '/api/chapters/batch-generate/test-batch-id/resume',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'chapters-generate-background-auth-guard-python-fallback',
                'owner': 'python-fallback',
                'method': 'POST',
                'path': '/api/chapters/test-chapter-id/generate-background',
                'expected_status': 401,
                'route_group': 'chapters',
                'profiles': ['phase5-p0-fallback'],
            },
            {
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy', 'business'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    phase5_p0_fallback = smoke.select_probes_by_profile(validated, profile='phase5-p0-fallback')

    assert [probe['name'] for probe in phase5_p0_fallback['probes']] == [
        'settings-auth-guard-python-fallback',
        'chapters-batch-active-tasks-auth-guard-python-fallback',
        'chapters-project-list-auth-guard-python-fallback',
        'wizard-stream-career-system-auth-guard-python-fallback',
        'wizard-stream-characters-auth-guard-python-fallback',
        'chapters-batch-stream-auth-guard-python-fallback',
        'chapters-batch-resume-auth-guard-python-fallback',
        'chapters-generate-background-auth-guard-python-fallback',
    ]


def test_select_probes_by_profile_supports_phase5_p0_asymmetric_profile():
    smoke = load_gateway_smoke_module()
    manifest = {
        'manifest_version': 1,
        'probes': [
            {
                'name': 'settings-models-auth-guard-rust-asymmetric',
                'owner': 'rust',
                'method': 'GET',
                'path': '/api/settings/models?provider=openai',
                'route_group': 'settings',
                'expected_status': 401,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'settings-models-public-network-error-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/settings/models?provider=openai',
                'route_group': 'settings',
                'expected_status': 400,
                'profiles': ['phase5-p0-asymmetric'],
            },
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
                'name': 'chapters-batch-status-task-not-found-python-fallback',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/api/chapters/batch-generate/test-batch-id/status',
                'route_group': 'chapters',
                'expected_status': 404,
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
                'name': 'chapters-batch-cancel-task-not-found-python-fallback',
                'owner': 'python-fallback',
                'method': 'POST',
                'path': '/api/chapters/batch-generate/test-batch-id/cancel',
                'route_group': 'chapters',
                'expected_status': 404,
                'profiles': ['phase5-p0-asymmetric'],
            },
            {
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
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
        'settings-models-auth-guard-rust-asymmetric',
        'settings-models-public-network-error-python-fallback',
        'chapters-batch-status-auth-guard-rust-asymmetric',
        'chapters-batch-status-task-not-found-python-fallback',
        'chapters-batch-cancel-auth-guard-rust-asymmetric',
        'chapters-batch-cancel-task-not-found-python-fallback',
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
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    deploy_only = smoke.select_probes_by_profile(validated, profile='deploy')
    route_groups = smoke.select_probes_by_profile(validated, profile='route-groups')

    assert [probe['name'] for probe in deploy_only['probes']] == ['rust-health', 'python-fallback-root']
    assert [probe['name'] for probe in route_groups['probes']] == ['settings-auth-guard', 'python-fallback-root']


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
                'name': 'python-fallback-root',
                'owner': 'python-fallback',
                'method': 'GET',
                'path': '/',
                'expected_status': 200,
                'profiles': ['deploy', 'business'],
            },
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))
    business = smoke.select_probes_by_profile(validated, profile='business')

    assert [probe['name'] for probe in business['probes']] == ['auth-config-public-rust', 'python-fallback-root']


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
