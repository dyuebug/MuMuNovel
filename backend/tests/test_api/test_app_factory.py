import pytest
from fastapi.testclient import TestClient

from app.bootstrap.app_factory import create_app
from app.main import app as main_app
from app.middleware.auth_middleware import user_manager


pytestmark = pytest.mark.asyncio


async def test_should_create_app_and_register_health_routes():
    app = create_app()
    route_paths = {route.path for route in app.routes}

    assert "/health" in route_paths
    assert "/livez" in route_paths
    assert "/readyz" in route_paths


def test_main_app_should_serve_json_health_routes_before_spa_fallback():
    client = TestClient(main_app)

    assert client.get('/health').json() == {'status': 'ok'}
    assert client.get('/livez').json() == {'status': 'ok'}
    assert client.get('/health/db-sessions').json()['status'] == 'ok'


def test_main_app_should_skip_auth_lookup_for_health_routes(monkeypatch):
    async def fake_get_user(user_id: str):
        raise AssertionError('health routes should not query user manager')

    monkeypatch.setattr(user_manager, 'get_user', fake_get_user)
    client = TestClient(main_app)

    assert client.get('/health', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/livez', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/health/db-sessions', cookies={'user_id': 'user-1'}).json()['status'] == 'ok'
