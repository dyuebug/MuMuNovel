import json

import httpx
import pytest
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import settings as settings_api
from app.models.settings import Settings

pytestmark = pytest.mark.asyncio


def build_settings_payload(**overrides):
    payload = {
        "api_provider": "openai",
        "api_key": "sk-new",
        "api_base_url": "https://api.openai.com/v1",
        "api_backup_urls": [
            "https://backup-1.example.com/v1",
            "https://backup-2.example.com/v1",
        ],
        "llm_model": "gpt-4.1-mini",
        "temperature": 0.4,
        "max_tokens": 1024,
        "web_research_enabled": True,
        "web_research_exa_enabled": True,
        "web_research_grok_enabled": True,
        "web_research_exa_api_key": "exa-test-key",
        "web_research_exa_base_url": "https://exa.chengtx.vip",
        "web_research_grok_api_key": "grok-test-key",
        "web_research_grok_base_url": "https://grok.example.com/v1",
        "web_research_grok_model": "grok-4.1-fast",
    }
    payload.update(overrides)
    return payload


def build_api_test_payload(**overrides):
    payload = {
        "api_key": "sk-test",
        "api_base_url": "https://api.openai.com/v1",
        "provider": "openai",
        "llm_model": "gpt-4.1-mini",
        "temperature": 0.3,
        "max_tokens": 256,
    }
    payload.update(overrides)
    return payload


def build_preset_payload(name: str = "preset-default", **config_overrides):
    config = {
        "api_provider": "openai",
        "api_key": "sk-preset",
        "api_base_url": "https://api.openai.com/v1",
        "llm_model": "gpt-4o-mini",
        "temperature": 0.5,
        "max_tokens": 2048,
    }
    config.update(config_overrides)
    return {
        "name": name,
        "description": f"{name} description",
        "config": config,
    }


async def fetch_settings(test_db: AsyncSession, user_id: str):
    result = await test_db.execute(
        select(Settings).where(Settings.user_id == user_id)
    )
    return result.scalar_one_or_none()


async def create_preset(async_client, name: str = "preset-default", **config_overrides):
    payload = build_preset_payload(name=name, **config_overrides)
    response = await async_client.post("/api/settings/presets", json=payload)
    assert response.status_code == 200
    return response.json()


async def test_should_return_401_when_unauthenticated_get_settings(unauth_async_client):
    response = await unauth_async_client.get("/api/settings")
    assert response.status_code == 401


async def test_should_return_200_when_authenticated_get_settings(async_client):
    response = await async_client.get("/api/settings")
    assert response.status_code == 200
    body = response.json()
    assert "user_id" in body
    assert "id" in body


async def test_should_auto_create_settings_on_first_get(
    async_client,
    test_db,
    mock_user,
    monkeypatch,
):
    expected_defaults = {
        "api_provider": "custom-provider",
        "api_key": "env-key",
        "api_base_url": "https://env.example.com/v1",
        "llm_model": "env-model",
        "temperature": 0.66,
        "max_tokens": 3210,
    }
    monkeypatch.setattr(settings_api, "read_env_defaults", lambda: expected_defaults)

    response = await async_client.get("/api/settings")
    assert response.status_code == 200
    body = response.json()
    assert body["api_provider"] == "custom-provider"
    assert body["api_key"] == "env-key"
    assert body["llm_model"] == "env-model"
    assert body["max_tokens"] == 3210

    saved = await fetch_settings(test_db, mock_user.user_id)
    assert saved is not None
    assert saved.api_provider == "custom-provider"
    assert saved.api_key == "env-key"


def test_read_env_defaults_should_skip_placeholder_openai_api_key(monkeypatch):
    monkeypatch.setattr(settings_api.app_settings, "openai_api_key", "your_openai_api_key_here")
    monkeypatch.setattr(settings_api.app_settings, "anthropic_api_key", "anthropic-live-key")
    monkeypatch.setattr(settings_api.app_settings, "gemini_api_key", None)

    defaults = settings_api.read_env_defaults()

    assert defaults["api_key"] == "anthropic-live-key"


async def test_should_return_existing_settings_when_already_saved(async_client, mock_settings):
    response = await async_client.get("/api/settings")
    assert response.status_code == 200
    body = response.json()
    assert body["id"] == mock_settings.id
    assert body["user_id"] == mock_settings.user_id
    assert body["api_provider"] == mock_settings.api_provider


async def test_should_create_settings_via_post_with_backup_urls_serialized(
    async_client,
    test_db,
    mock_user,
):
    payload = build_settings_payload()
    response = await async_client.post("/api/settings", json=payload)
    assert response.status_code == 200
    body = response.json()
    assert body["api_backup_urls"] == payload["api_backup_urls"]
    assert body["llm_model"] == payload["llm_model"]

    saved = await fetch_settings(test_db, mock_user.user_id)
    assert saved is not None
    assert saved.api_backup_urls == json.dumps(payload["api_backup_urls"], ensure_ascii=False)
    assert saved.llm_model == payload["llm_model"]


async def test_should_update_existing_settings_via_post_and_deactivate_changed_active_preset(
    async_client,
    test_db,
    mock_settings,
    mock_user,
):
    active_preset = {
        "id": "preset_active",
        "name": "active",
        "description": "active preset",
        "is_active": True,
        "created_at": "2026-01-01T00:00:00",
        "config": {
            "api_provider": mock_settings.api_provider,
            "api_key": mock_settings.api_key,
            "api_base_url": mock_settings.api_base_url,
            "llm_model": mock_settings.llm_model,
            "temperature": mock_settings.temperature,
            "max_tokens": mock_settings.max_tokens,
        },
    }
    mock_settings.preferences = json.dumps(
        {"api_presets": {"presets": [active_preset], "version": "1.0"}},
        ensure_ascii=False,
    )
    await test_db.commit()

    payload = build_settings_payload(llm_model="gpt-4.1", api_key="sk-updated")
    response = await async_client.post("/api/settings", json=payload)
    assert response.status_code == 200
    body = response.json()
    assert body["llm_model"] == "gpt-4.1"
    assert body["api_key"] == "sk-updated"

    saved = await fetch_settings(test_db, mock_user.user_id)
    prefs = json.loads(saved.preferences)
    assert prefs["api_presets"]["presets"][0]["is_active"] is False


async def test_should_store_web_research_settings_in_preferences(async_client, test_db, mock_user):
    payload = build_settings_payload()
    response = await async_client.post("/api/settings", json=payload)
    assert response.status_code == 200
    body = response.json()
    assert body["web_research_enabled"] is True
    assert body["web_research_exa_api_key"] == "exa-test-key"
    assert body["web_research_exa_base_url"] == "https://exa.chengtx.vip"
    assert body["web_research_grok_base_url"] == "https://grok.example.com/v1"

    saved = await fetch_settings(test_db, mock_user.user_id)
    prefs = json.loads(saved.preferences)
    web_research = prefs["web_research"]
    assert web_research["web_research_enabled"] is True
    assert web_research["web_research_exa_api_key"] == "exa-test-key"
    assert web_research["web_research_exa_base_url"] == "https://exa.chengtx.vip"
    assert web_research["web_research_grok_api_key"] == "grok-test-key"


async def test_should_update_settings_via_put(async_client, mock_settings):
    payload = {
        "llm_model": "gpt-4o-updated",
        "temperature": 0.25,
        "api_backup_urls": ["https://one.example.com/v1"],
    }
    response = await async_client.put("/api/settings", json=payload)
    assert response.status_code == 200
    body = response.json()
    assert body["llm_model"] == "gpt-4o-updated"
    assert body["temperature"] == 0.25
    assert body["api_backup_urls"] == ["https://one.example.com/v1"]


async def test_should_return_404_when_put_settings_without_existing(async_client):
    response = await async_client.put("/api/settings", json={"llm_model": "new-model"})
    assert response.status_code == 404


async def test_should_delete_settings_successfully(async_client, test_db, mock_settings, mock_user):
    response = await async_client.delete("/api/settings")
    assert response.status_code == 200
    body = response.json()
    assert body["user_id"] == mock_user.user_id

    saved = await fetch_settings(test_db, mock_user.user_id)
    assert saved is None


async def test_should_return_404_when_delete_settings_without_existing(async_client):
    response = await async_client.delete("/api/settings")
    assert response.status_code == 404


@pytest.mark.parametrize("provider", ["openai", "newapi", "custom", "sub2api"])
async def test_should_fetch_models_for_openai_compatible_providers(
    async_client,
    monkeypatch,
    provider,
):
    captured = {}

    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            captured["url"] = url
            captured["headers"] = headers
            return httpx.Response(
                status_code=200,
                json={"data": [{"id": "m1"}, {"id": "m2"}]},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "sk-test",
            "api_base_url": "https://provider.example.com/v1",
            "provider": provider,
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == provider
    assert body["count"] == 2
    assert captured["url"].endswith("/models")
    assert captured["headers"]["Authorization"] == "Bearer sk-test"


async def test_should_fallback_to_v1_models_when_models_endpoint_is_404(async_client, monkeypatch):
    captured_urls = []

    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            captured_urls.append(url)
            if url.endswith("/models") and not url.endswith("/v1/models"):
                return httpx.Response(
                    status_code=404,
                    json={"error": "not found"},
                    request=httpx.Request("GET", url),
                )
            return httpx.Response(
                status_code=200,
                json={"data": [{"id": "gpt-5.3-codex"}]},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "sk-test",
            "api_base_url": "https://ai.qaq.al",
            "provider": "sub2api",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "sub2api"
    assert body["count"] == 1
    assert body["models"][0]["value"] == "gpt-5.3-codex"
    assert captured_urls[0] == "https://ai.qaq.al/models"
    assert captured_urls[1] == "https://ai.qaq.al/v1/models"


async def test_should_handle_azure_models_empty_result_with_friendly_message(async_client, monkeypatch):
    captured = {}

    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            captured["url"] = url
            captured["headers"] = headers
            return httpx.Response(
                status_code=200,
                json={"data": []},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "azure-key",
            "api_base_url": "https://azure.example.com/openai",
            "provider": "azure",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "azure"
    assert body["count"] == 0
    assert "message" in body
    assert "Azure" in body["message"]
    assert captured["headers"]["api-key"] == "azure-key"
    assert "Authorization" not in captured["headers"]


async def test_should_handle_azure_404_with_friendly_message(async_client, monkeypatch):
    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            return httpx.Response(
                status_code=404,
                json={"error": "not found"},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "azure-key",
            "api_base_url": "https://azure.example.com/openai",
            "provider": "azure",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "azure"
    assert body["count"] == 0
    assert "Azure" in body["message"]


async def test_should_fetch_models_for_anthropic_provider(async_client, monkeypatch):
    captured = {}

    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            captured["url"] = url
            captured["headers"] = headers
            return httpx.Response(
                status_code=200,
                json={
                    "data": [
                        {
                            "id": "claude-3-5-sonnet",
                            "display_name": "Claude 3.5 Sonnet",
                        }
                    ]
                },
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "ak-anthropic",
            "api_base_url": "https://api.anthropic.com",
            "provider": "anthropic",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "anthropic"
    assert body["count"] == 1
    assert body["models"][0]["value"] == "claude-3-5-sonnet"
    assert body["models"][0]["description"] == "Claude 3.5 Sonnet"
    assert captured["headers"]["x-api-key"] == "ak-anthropic"


async def test_should_fetch_models_for_gemini_and_filter_generation_capability(async_client, monkeypatch):
    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            return httpx.Response(
                status_code=200,
                json={
                    "models": [
                        {
                            "name": "models/gemini-2.0-pro",
                            "displayName": "Gemini 2.0 Pro",
                            "supportedGenerationMethods": ["generateContent"],
                        },
                        {
                            "name": "models/embedding-001",
                            "displayName": "Embedding",
                            "supportedGenerationMethods": ["embedContent"],
                        },
                    ]
                },
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "gem-key",
            "api_base_url": "https://generativelanguage.googleapis.com/v1beta",
            "provider": "gemini",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "gemini"
    assert body["count"] == 1
    assert body["models"][0]["value"] == "gemini-2.0-pro"
    assert body["models"][0]["label"] == "Gemini 2.0 Pro"


async def test_should_return_400_when_fetch_models_network_error(async_client, monkeypatch):
    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            return None

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def get(self, url, headers=None):
            raise httpx.RequestError(
                "network down",
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "sk-test",
            "api_base_url": "https://provider.example.com/v1",
            "provider": "openai",
        },
    )
    assert response.status_code == 400
    body = response.json()
    assert "detail" in body


async def test_should_test_api_connection_successfully(async_client, monkeypatch):
    captured = {}

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            captured["call_kwargs"] = kwargs
            return "x" * 150

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["provider"] == "openai"
    assert body["model"] == "gpt-4.1-mini"
    assert isinstance(body["response_time_ms"], (int, float))
    assert len(body["response_preview"]) == 100
    assert captured["call_kwargs"]["auto_mcp"] is False


async def test_should_return_timeout_error_when_api_test_times_out(async_client, monkeypatch):
    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            raise TimeoutError("request timeout")

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is False
    assert body["error_type"] == "TimeoutError"
    assert "timeout" in body["error"]


@pytest.mark.parametrize("error_message", ["401 unauthorized", "404 not found", "429 rate limit"])
async def test_should_return_failure_for_common_api_errors(
    async_client,
    monkeypatch,
    error_message,
):
    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            raise RuntimeError(error_message)

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is False
    assert body["error_type"] == "RuntimeError"
    assert error_message in body["error"]


async def test_should_test_web_research_connection(async_client, monkeypatch):
    async def fake_test_provider_connection(**kwargs):
        assert kwargs["provider"] == "exa"
        assert kwargs["overrides"]["exa_base_url"] == "https://exa.chengtx.vip"
        return {
            "success": True,
            "provider": "exa",
            "message": "Exa 连接测试成功",
            "response_preview": "preview",
            "result_count": 1,
        }

    monkeypatch.setattr(
        settings_api.chapter_web_research_service,
        "test_provider_connection",
        fake_test_provider_connection,
    )

    response = await async_client.post(
        "/api/settings/test-web-research",
        json={
            "provider": "exa",
            "exa_api_key": "exa-test-key",
            "exa_base_url": "https://exa.chengtx.vip",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["provider"] == "exa"
    assert body["result_count"] == 1


async def test_should_detect_function_calling_support_when_tool_calls_present(async_client, monkeypatch):
    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [
                    {
                        "id": "call_001",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": '{"city":"北京"}'},
                    }
                ],
                "content": "",
            }

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["supported"] is True
    assert body["details"]["has_tool_calls"] is True
    assert body["details"]["tool_call_count"] == 1


async def test_should_mark_function_calling_unsupported_when_plain_text(async_client, monkeypatch):
    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            return "plain text response"

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["supported"] is False
    assert body["details"]["response_type"] == "text"


async def test_should_return_timeout_for_function_calling_check(async_client, monkeypatch):
    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            raise TimeoutError("call timeout")

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is False
    assert body["supported"] is None
    assert body["error_type"] == "TimeoutError"


async def test_should_return_empty_presets_list_when_no_presets(async_client):
    response = await async_client.get("/api/settings/presets")
    assert response.status_code == 200
    body = response.json()
    assert body["presets"] == []
    assert body["total"] == 0
    assert body["active_preset_id"] is None


async def test_should_create_preset_and_list_it(async_client):
    created = await create_preset(async_client, name="primary-preset")
    assert created["name"] == "primary-preset"
    assert created["is_active"] is False
    assert created["id"].startswith("preset_")

    response = await async_client.get("/api/settings/presets")
    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 1
    assert body["presets"][0]["id"] == created["id"]
    assert body["active_preset_id"] is None


async def test_should_update_existing_preset(async_client):
    created = await create_preset(async_client, name="to-update")
    preset_id = created["id"]

    update_payload = {
        "name": "updated-name",
        "description": "updated description",
        "config": {
            "api_provider": "anthropic",
            "api_key": "ak-updated",
            "api_base_url": "https://api.anthropic.com",
            "llm_model": "claude-3-5-sonnet",
            "temperature": 0.2,
            "max_tokens": 4096,
        },
    }
    response = await async_client.put(f"/api/settings/presets/{preset_id}", json=update_payload)
    assert response.status_code == 200
    body = response.json()
    assert body["id"] == preset_id
    assert body["name"] == "updated-name"
    assert body["config"]["api_provider"] == "anthropic"
    assert body["config"]["llm_model"] == "claude-3-5-sonnet"


async def test_should_return_404_when_update_missing_preset(async_client):
    response = await async_client.put(
        "/api/settings/presets/missing-preset",
        json={"name": "whatever"},
    )
    assert response.status_code == 404


async def test_should_delete_preset_successfully(async_client):
    created = await create_preset(async_client, name="to-delete")
    preset_id = created["id"]

    response = await async_client.delete(f"/api/settings/presets/{preset_id}")
    assert response.status_code == 200
    body = response.json()
    assert body["preset_id"] == preset_id

    list_response = await async_client.get("/api/settings/presets")
    assert list_response.status_code == 200
    assert list_response.json()["total"] == 0


async def test_should_not_delete_active_preset(async_client):
    created = await create_preset(async_client, name="active-preset")
    preset_id = created["id"]

    activate_response = await async_client.post(f"/api/settings/presets/{preset_id}/activate")
    assert activate_response.status_code == 200

    delete_response = await async_client.delete(f"/api/settings/presets/{preset_id}")
    assert delete_response.status_code == 400


async def test_should_activate_preset_and_apply_config_to_main_settings(async_client):
    seed_settings_payload = build_settings_payload(
        api_provider="openai",
        api_key="sk-before",
        api_base_url="https://api.openai.com/v1",
        llm_model="before-model",
        temperature=0.9,
        max_tokens=512,
    )
