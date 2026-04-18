import json

import httpx
import pytest
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import settings as settings_api
from app.models.settings import Settings

pytestmark = pytest.mark.asyncio


@pytest.fixture(autouse=True)
def clear_probe_cache_fixture():
    settings_api.clear_probe_result_cache()
    yield
    settings_api.clear_probe_result_cache()


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
        "web_research_grok_search_enabled": True,
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
    assert body["web_research_grok_search_enabled"] is True

    saved = await fetch_settings(test_db, mock_user.user_id)
    prefs = json.loads(saved.preferences)
    web_research = prefs["web_research"]
    assert web_research["web_research_enabled"] is True
    assert web_research["web_research_exa_api_key"] == "exa-test-key"
    assert web_research["web_research_exa_base_url"] == "https://exa.chengtx.vip"
    assert web_research["web_research_grok_api_key"] == "grok-test-key"
    assert web_research["web_research_grok_search_enabled"] is True


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
    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "https://api.openai.com/v1"
    assert body["details"]["endpoint_diagnostics"]["backup_endpoints"] == []


async def test_should_prefer_chat_completions_for_sub2api_api_connection_probe(async_client, monkeypatch):
    captured = {}

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            captured["call_kwargs"] = kwargs
            return {"content": "ok", "tool_calls": None, "finish_reason": "stop"}

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(
            provider="sub2api",
            api_base_url="https://free.9e.nz",
            llm_model="gpt-5.4",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert captured["call_kwargs"]["request_options"]["prefer_chat_completions"] is True
    assert captured["call_kwargs"]["request_options"]["transport_max_retries"] == 1


async def test_should_prefer_chat_completions_for_sub2api_function_calling_probe(async_client, monkeypatch):
    captured = {}

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            captured["call_kwargs"] = kwargs
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [
                    {
                        "id": "call_001",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": '{"city":"??"}'},
                    }
                ],
                "content": "",
            }

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(
            provider="sub2api",
            api_base_url="https://free.9e.nz",
            llm_model="gpt-5.4",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["supported"] is True
    assert captured["call_kwargs"]["request_options"]["prefer_chat_completions"] is True
    assert captured["call_kwargs"]["request_options"]["transport_max_retries"] == 1
    assert captured["call_kwargs"]["max_tokens"] == 64
    assert captured["call_kwargs"]["tool_choice"] == "required"



async def test_should_enable_normalized_v1_candidate_for_custom_api_connection_probe(async_client, monkeypatch):
    captured = {}

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            captured["call_kwargs"] = kwargs
            return {"content": "TEST_OK", "tool_calls": None, "finish_reason": "stop"}

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(
            provider="custom",
            api_base_url="https://gateway.example.com",
            llm_model="gpt-4.1",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    request_options = captured["call_kwargs"]["request_options"]
    assert request_options["prefer_chat_completions"] is True
    assert request_options["prefer_normalized_v1_candidate"] is True
    assert request_options["transport_max_retries"] == 1
    assert request_options["read_timeout"] == 10.0


async def test_should_enable_normalized_v1_candidate_for_custom_function_calling_probe(async_client, monkeypatch):
    captured = {}

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            captured["call_kwargs"] = kwargs
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [
                    {
                        "id": "call_001",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": '{"city":"Beijing"}'},
                    }
                ],
                "content": "",
            }

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(
            provider="custom",
            api_base_url="https://gateway.example.com",
            llm_model="gpt-4.1",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["supported"] is True
    request_options = captured["call_kwargs"]["request_options"]
    assert request_options["prefer_chat_completions"] is True
    assert request_options["prefer_normalized_v1_candidate"] is True
    assert request_options["transport_max_retries"] == 1
    assert request_options["read_timeout"] == 10.0
    assert captured["call_kwargs"]["max_tokens"] == 64
    assert captured["call_kwargs"]["tool_choice"] == "required"

async def test_should_pass_backup_urls_and_fallback_strategy_to_api_connection_probe(async_client, monkeypatch):
    captured = {}
    backup_urls = [
        "https://backup-1.example.com/v1",
        "https://backup-2.example.com/v1",
    ]

    transport_diagnostics = {
        "events": [{"type": "api_mode_selected", "api_mode": "chat_completions"}],
        "attempts": [{"result": "success", "api_mode": "chat_completions"}],
        "summary": {"total_attempts": 1},
    }

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            return {"content": "ok", "tool_calls": None, "finish_reason": "stop"}

        def get_transport_diagnostics(self, provider=None):
            return transport_diagnostics

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(
            api_backup_urls=backup_urls,
            fallback_strategy="manual",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert captured["init_kwargs"]["backup_urls"] == backup_urls
    assert captured["init_kwargs"]["fallback_strategy"] == "manual"
    assert body["details"]["endpoint_diagnostics"]["backup_endpoints"] == backup_urls
    assert body["details"]["endpoint_diagnostics"]["fallback_strategy"] == "manual"
    assert body["details"]["transport_diagnostics"] == transport_diagnostics
    assert body["details"]["transport_diagnostics"] == transport_diagnostics


async def test_should_pass_backup_urls_and_fallback_strategy_to_function_calling_probe(async_client, monkeypatch):
    captured = {}
    backup_urls = ["https://backup-1.example.com/v1"]

    transport_diagnostics = {
        "events": [{"type": "api_mode_selected", "api_mode": "responses"}],
        "attempts": [{"result": "success", "api_mode": "responses"}],
        "summary": {"total_attempts": 1},
    }

    class FakeAIService:
        def __init__(self, **kwargs):
            captured["init_kwargs"] = kwargs

        async def generate_text(self, **kwargs):
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [
                    {
                        "id": "call_001",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": '{"city":"??"}'},
                    }
                ],
                "content": "",
            }

        def get_transport_diagnostics(self, provider=None):
            return transport_diagnostics

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(
            api_backup_urls=backup_urls,
            fallback_strategy="manual",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["supported"] is True
    assert captured["init_kwargs"]["backup_urls"] == backup_urls
    assert captured["init_kwargs"]["fallback_strategy"] == "manual"
    assert body["details"]["endpoint_diagnostics"]["backup_endpoints"] == backup_urls
    assert body["details"]["endpoint_diagnostics"]["fallback_strategy"] == "manual"

    assert body["details"]["transport_diagnostics"] == transport_diagnostics

async def test_should_return_timeout_error_when_api_test_times_out(async_client, monkeypatch):
    transport_diagnostics = {
        "events": [{"type": "api_mode_selected", "api_mode": "chat_completions"}],
        "attempts": [{"result": "network_error", "api_mode": "chat_completions"}],
        "summary": {"total_attempts": 1},
    }

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            raise TimeoutError("request timeout")

        def get_transport_diagnostics(self, provider=None):
            return transport_diagnostics

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
    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "https://api.openai.com/v1"
    assert body["details"]["endpoint_diagnostics"]["backup_endpoints"] == []

async def test_should_return_gateway_error_guidance_for_api_probe(async_client, monkeypatch):
    transport_diagnostics = {
        "events": [{"type": "chat_completions_candidate_failed", "api_mode": "chat_completions"}],
        "attempts": [{"result": "http_error", "api_mode": "chat_completions", "status_code": 502}],
        "summary": {"total_attempts": 1},
    }
    request = httpx.Request("POST", "http://127.0.0.1:8317/v1/chat/completions")
    response = httpx.Response(502, request=request)
    gateway_error = httpx.HTTPStatusError(
        "Server error '502 Bad Gateway' for url 'http://127.0.0.1:8317/v1/chat/completions'",
        request=request,
        response=response,
    )

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            raise gateway_error

        def get_transport_diagnostics(self, provider=None):
            return transport_diagnostics

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    response = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(
            api_base_url="http://127.0.0.1:8317/v1",
            provider="openai_responses",
        ),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is False
    assert body["error_type"] == "HTTPStatusError"
    assert body["details"]["http_status_code"] == 502
    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "http://127.0.0.1:8317/v1"
    assert body["details"]["endpoint_diagnostics"]["auto_failover_enabled"] is False
    assert body["details"]["transport_diagnostics"] == transport_diagnostics
    assert any("local gateway or proxy" in item.lower() for item in body["suggestions"])
    assert any("backup endpoint" in item.lower() for item in body["suggestions"])


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
    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "https://api.openai.com/v1"
    assert body["details"]["endpoint_diagnostics"]["fallback_strategy"] == "auto"


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
            "search_status": "success_with_sources",
            "status_note": None,
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
    assert body["search_status"] == "success_with_sources"


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
    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "https://api.openai.com/v1"


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

    assert body["details"]["endpoint_diagnostics"]["primary_endpoint"] == "https://api.openai.com/v1"
    assert body["details"]["endpoint_diagnostics"]["backup_endpoints"] == []
    assert body["details"]["endpoint_diagnostics"]["fallback_strategy"] == "auto"

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



async def test_should_cache_api_connection_probe_results(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            return {"content": "测试成功", "tool_calls": None, "finish_reason": "stop"}

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post("/api/settings/test", json=build_api_test_payload())
    second = await async_client.post("/api/settings/test", json=build_api_test_payload())

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is True
    assert calls["count"] == 1


async def test_should_normalize_api_connection_probe_cache_key_by_probe_max_tokens(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            return {"content": "ok", "tool_calls": None, "finish_reason": "stop"}

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(max_tokens=128),
    )
    second = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(max_tokens=4096),
    )

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is True
    assert calls["count"] == 1
    assert first.json()["details"]["probe_max_tokens"] == 64
    assert second.json()["details"]["probe_max_tokens"] == 64


async def test_should_cache_function_calling_probe_results(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [{"id": "call_001", "type": "function", "function": {"name": "get_weather", "arguments": '{\"city\":\"北京\"}'}}],
                "content": "",
            }

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post("/api/settings/check-function-calling", json=build_api_test_payload())
    second = await async_client.post("/api/settings/check-function-calling", json=build_api_test_payload())

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is True
    assert calls["count"] == 1

async def test_should_not_reuse_api_connection_probe_cache_when_backup_urls_change(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            return {"content": "ok", "tool_calls": None, "finish_reason": "stop"}

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(api_backup_urls=["https://backup-a.example.com/v1"]),
    )
    second = await async_client.post(
        "/api/settings/test",
        json=build_api_test_payload(api_backup_urls=["https://backup-b.example.com/v1"]),
    )

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is False
    assert calls["count"] == 2


async def test_should_not_reuse_function_calling_probe_cache_when_fallback_strategy_changes(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            return {
                "finish_reason": "tool_calls",
                "tool_calls": [{"id": "call_001", "type": "function", "function": {"name": "get_weather", "arguments": '{\"city\":\"??\"}'}}],
                "content": "",
            }

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(
            api_backup_urls=["https://backup-a.example.com/v1"],
            fallback_strategy="auto",
        ),
    )
    second = await async_client.post(
        "/api/settings/check-function-calling",
        json=build_api_test_payload(
            api_backup_urls=["https://backup-a.example.com/v1"],
            fallback_strategy="manual",
        ),
    )

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is False
    assert calls["count"] == 2


async def test_should_not_cache_api_connection_probe_timeout(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            raise TimeoutError("request timeout")

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post("/api/settings/test", json=build_api_test_payload())
    second = await async_client.post("/api/settings/test", json=build_api_test_payload())

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is False
    assert calls["count"] == 2


async def test_should_not_cache_function_calling_probe_timeout(async_client, monkeypatch):
    calls = {"count": 0}

    class FakeAIService:
        def __init__(self, **kwargs):
            return None

        async def generate_text(self, **kwargs):
            calls["count"] += 1
            raise TimeoutError("call timeout")

    monkeypatch.setattr(settings_api, "AIService", FakeAIService)

    first = await async_client.post("/api/settings/check-function-calling", json=build_api_test_payload())
    second = await async_client.post("/api/settings/check-function-calling", json=build_api_test_payload())

    assert first.status_code == 200
    assert second.status_code == 200
    assert first.json()["cached"] is False
    assert second.json()["cached"] is False
    assert calls["count"] == 2



async def test_should_fallback_to_docker_host_models_when_loopback_is_unreachable(async_client, monkeypatch):
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
            if url.startswith("http://127.0.0.1:8317"):
                raise httpx.ConnectError("connection refused", request=httpx.Request("GET", url))
            return httpx.Response(
                status_code=200,
                json={"data": [{"id": "gpt-5.3-codex"}]},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api, "_is_running_in_docker_environment", lambda: True)
    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "sk-test",
            "api_base_url": "http://127.0.0.1:8317/v1",
            "provider": "sub2api",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "sub2api"
    assert body["count"] == 1
    assert body["models"][0]["value"] == "gpt-5.3-codex"
    assert captured_urls[0] == "http://127.0.0.1:8317/v1/models"
    assert captured_urls[1] == "http://host.docker.internal:8317/v1/models"



async def test_should_fallback_from_local_https_models_to_http(async_client, monkeypatch):
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
            if url.startswith("https://"):
                raise httpx.ConnectError("ssl record layer failure", request=httpx.Request("GET", url))
            return httpx.Response(
                status_code=200,
                json={"data": [{"id": "gpt-5.3-codex"}]},
                request=httpx.Request("GET", url),
            )

    monkeypatch.setattr(settings_api, "_is_running_in_docker_environment", lambda: True)
    monkeypatch.setattr(settings_api.httpx, "AsyncClient", FakeAsyncClient)

    response = await async_client.get(
        "/api/settings/models",
        params={
            "api_key": "sk-test",
            "api_base_url": "https://127.0.0.1:8317/v1",
            "provider": "sub2api",
        },
    )
    assert response.status_code == 200
    body = response.json()
    assert body["provider"] == "sub2api"
    assert body["count"] == 1
    assert body["models"][0]["value"] == "gpt-5.3-codex"
    assert captured_urls == [
        "https://127.0.0.1:8317/v1/models",
        "https://host.docker.internal:8317/v1/models",
        "http://127.0.0.1:8317/v1/models",
    ]
