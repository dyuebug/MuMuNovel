"""Health probe route regression tests."""

import pytest
from fastapi.testclient import TestClient

from app import main as app_main


pytestmark = pytest.mark.asyncio


def test_should_register_health_probe_routes():
    route_paths = {route.path for route in app_main.app.routes}

    assert "/health" in route_paths
    assert "/livez" in route_paths
    assert "/readyz" in route_paths


def test_should_return_200_for_health():
    with TestClient(app_main.app) as client:
        response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_should_return_200_for_livez():
    with TestClient(app_main.app) as client:
        response = client.get("/livez")

    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_should_return_200_or_503_for_readyz():
    """readyz may be 503 if database is unavailable; verify response structure."""
    with TestClient(app_main.app) as client:
        response = client.get("/readyz")

    assert response.status_code in (200, 503)
    body = response.json()
    assert body["status"] in ("ready", "not_ready")
    assert "checks" in body
    assert "startup" in body["checks"]
    assert "database" in body["checks"]
