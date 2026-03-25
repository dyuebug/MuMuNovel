import pytest

from app import main as app_main


pytestmark = pytest.mark.asyncio


@pytest.fixture(autouse=True)
def reset_startup_status():
    app_main._reset_startup_status()
    yield
    app_main._reset_startup_status()


async def test_should_register_health_probe_routes():
    route_paths = {route.path for route in app_main.app.routes}

    assert "/health" in route_paths
    assert "/livez" in route_paths
    assert "/readyz" in route_paths


async def test_should_return_200_when_readyz_is_healthy(monkeypatch):
    async def fake_check_database_health(*args, **kwargs):
        return {
            "healthy": True,
            "checks": {
                "connection": {"status": "ok", "healthy": True},
            },
        }

    app_main._set_startup_ready(True)
    monkeypatch.setattr(app_main, "check_database_health", fake_check_database_health)

    response = await app_main.readiness_check()

    assert response.status_code == 200
    assert b"ready" in response.body


async def test_should_return_503_when_readyz_is_not_ready(monkeypatch):
    async def fake_check_database_health(*args, **kwargs):
        return {
            "healthy": False,
            "checks": {
                "error": {"status": "error", "healthy": False, "message": "db down"},
            },
        }

    app_main._set_startup_ready(True)
    monkeypatch.setattr(app_main, "check_database_health", fake_check_database_health)

    response = await app_main.readiness_check()

    assert response.status_code == 503
    assert b"not_ready" in response.body


async def test_should_return_503_when_warmup_is_not_ready_even_if_database_is_healthy(monkeypatch):
    async def fake_check_database_health(*args, **kwargs):
        return {
            "healthy": True,
            "checks": {
                "connection": {"status": "ok", "healthy": True},
            },
        }

    app_main._set_startup_ready(False)
    monkeypatch.setattr(app_main, "check_database_health", fake_check_database_health)

    response = await app_main.readiness_check()

    assert response.status_code == 503
    assert b"startup" in response.body
