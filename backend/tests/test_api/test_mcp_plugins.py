from types import SimpleNamespace

import pytest
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient

from app.api import mcp_plugins as mcp_plugins_api

pytestmark = pytest.mark.asyncio


@pytest.fixture
async def mcp_plugins_client(monkeypatch):
    app = FastAPI()
    app.include_router(mcp_plugins_api.router, prefix="/api")

    async def override_require_login():
        return SimpleNamespace(user_id="user-1", is_admin=True)

    app.dependency_overrides[mcp_plugins_api.require_login] = override_require_login
    monkeypatch.setattr(mcp_plugins_api.mcp_client, "get_metrics", lambda tool_name=None: {"total_calls": 3, "tool_name": tool_name})
    monkeypatch.setattr(mcp_plugins_api.mcp_client, "get_cache_stats", lambda: {"total_entries": 1})
    monkeypatch.setattr(mcp_plugins_api.mcp_client, "get_session_stats", lambda: {"total_sessions": 2})

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


async def test_should_route_metrics_and_stats_before_dynamic_plugin_id(mcp_plugins_client: AsyncClient):
    metrics = await mcp_plugins_client.get("/api/mcp/plugins/metrics")
    assert metrics.status_code == 200
    assert metrics.json()["metrics"]["total_calls"] == 3

    cache_stats = await mcp_plugins_client.get("/api/mcp/plugins/cache/stats")
    assert cache_stats.status_code == 200
    assert cache_stats.json()["cache_stats"]["total_entries"] == 1

    session_stats = await mcp_plugins_client.get("/api/mcp/plugins/sessions/stats")
    assert session_stats.status_code == 200
    assert session_stats.json()["session_stats"]["total_sessions"] == 2
