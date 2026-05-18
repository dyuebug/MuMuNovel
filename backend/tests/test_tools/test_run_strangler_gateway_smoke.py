from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


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
            }
        ],
    }

    validated = smoke.validate_manifest(manifest, manifest_path=Path('deploy/strangler-gateway-probes.json'))

    assert validated['manifest_version'] == 1
    assert len(validated['probes']) == 1
    assert validated['probes'][0]['owner'] == 'rust'


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

    def fake_request_probe(*, base_url: str, path: str, method: str, timeout: float):
        return responses[path]

    monkeypatch.setattr(smoke, 'request_probe', fake_request_probe)

    results = smoke.run_probes(manifest=manifest, base_url='http://localhost:8005', timeout=10.0)

    assert [item['owner'] for item in results] == ['rust', 'python-fallback']
    assert results[0]['ok'] is True
    assert results[0]['status_code'] == 200
    assert results[1]['ok'] is False
    assert results[1]['path'] == '/'
    assert 'status mismatch' in results[1]['error']
