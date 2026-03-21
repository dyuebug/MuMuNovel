import pytest

import app.services.chapter_web_research_service as chapter_web_research_module
from app.services.chapter_web_research_service import (
    ChapterWebResearchService,
    WebResearchRuntimeConfig,
)


@pytest.mark.asyncio
async def test_should_use_direct_exa_search_when_base_url_provided(monkeypatch):
    captured = {}

    class FakeResponse:
        status_code = 200
        text = ""

        def raise_for_status(self):
            return None

        def json(self):
            return {
                "results": [
                    {
                        "title": "Relay Result",
                        "url": "https://example.com/source",
                        "text": "relay text",
                    }
                ]
            }

    class FakeAsyncClient:
        def __init__(self, *args, **kwargs):
            captured["timeout"] = kwargs.get("timeout")

        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def post(self, url, headers=None, json=None):
            captured["url"] = url
            captured["headers"] = headers
            captured["json"] = json
            return FakeResponse()

    monkeypatch.setattr(chapter_web_research_module.httpx, "AsyncClient", FakeAsyncClient)

    service = ChapterWebResearchService()
    payload = await service._run_exa_search(
        "relay test",
        WebResearchRuntimeConfig(
            exa_api_key="exa-test-key",
            exa_base_url="https://exa.chengtx.vip",
        ),
    )

    assert payload["mode"] == "direct_search_api"
    assert payload["request_url"] == "https://exa.chengtx.vip/search"
    assert payload["results"][0]["title"] == "Relay Result"
    assert captured["url"] == "https://exa.chengtx.vip/search"
    assert captured["headers"]["Authorization"] == "Bearer exa-test-key"
    assert captured["json"] == {"query": "relay test", "numResults": 3}


def test_should_build_runtime_config_from_saved_web_research_preferences():
    service = ChapterWebResearchService()

    runtime_config = service.build_runtime_config(
        preferences={
            "web_research": {
                "web_research_enabled": True,
                "web_research_exa_enabled": True,
                "web_research_exa_api_key": "exa-test-key",
                "web_research_exa_base_url": "https://exa.chengtx.vip",
                "web_research_grok_enabled": True,
                "web_research_grok_api_key": "grok-test-key",
                "web_research_grok_base_url": "https://grok.example.com/v1",
                "web_research_grok_model": "grok-4.1-fast",
            }
        }
    )

    assert runtime_config.enabled is True
    assert runtime_config.exa_enabled is True
    assert runtime_config.exa_api_key == "exa-test-key"
    assert runtime_config.exa_base_url == "https://exa.chengtx.vip"
    assert runtime_config.grok_api_key == "grok-test-key"
    assert runtime_config.grok_base_url == "https://grok.example.com/v1"


@pytest.mark.asyncio
async def test_should_use_openai_compatible_client_for_direct_grok_connection(monkeypatch):
    captured = {}

    class FakeOpenAIClient:
        def __init__(self, *, api_key, base_url, compat_profile, config=None, backup_urls=None):
            captured["api_key"] = api_key
            captured["base_url"] = base_url
            captured["compat_profile"] = compat_profile

        async def chat_completion(self, **kwargs):
            captured["chat_kwargs"] = kwargs
            return {"content": "OK from relay"}

    monkeypatch.setattr(chapter_web_research_module, "OpenAIClient", FakeOpenAIClient)

    service = ChapterWebResearchService()
    payload = await service._test_grok_direct_connection(
        WebResearchRuntimeConfig(
            grok_api_key="grok-test-key",
            grok_base_url="https://relay.example.com/v1",
            grok_model="grok-4.1-fast",
        )
    )

    assert payload["content"] == "OK from relay"
    assert payload["sources"] == []
    assert payload["mode"] == "direct_chat_test"
    assert captured["api_key"] == "grok-test-key"
    assert captured["base_url"] == "https://relay.example.com/v1"
    assert captured["compat_profile"] == "openai"
    assert captured["chat_kwargs"]["model"] == "grok-4.1-fast"


@pytest.mark.asyncio
async def test_should_append_v1_for_root_grok_base_url(monkeypatch):
    captured = {}

    class FakeOpenAIClient:
        def __init__(self, *, api_key, base_url, compat_profile, config=None, backup_urls=None):
            captured["base_url"] = base_url

        async def chat_completion(self, **kwargs):
            return {"content": "OK"}

    monkeypatch.setattr(chapter_web_research_module, "OpenAIClient", FakeOpenAIClient)

    service = ChapterWebResearchService()
    payload = await service._test_grok_direct_connection(
        WebResearchRuntimeConfig(
            grok_api_key="grok-test-key",
            grok_base_url="https://ai.zzhdsgsss.xyz",
            grok_model="grok-4.1-fast",
        )
    )

    assert payload["content"] == "OK"
    assert captured["base_url"] == "https://ai.zzhdsgsss.xyz/v1"


@pytest.mark.asyncio
async def test_should_fallback_to_stream_for_grok_connection_when_proxy_returns_sse(monkeypatch):
    class FakeOpenAIClient:
        def __init__(self, *, api_key, base_url, compat_profile, config=None, backup_urls=None):
            self.base_url = base_url

        async def chat_completion(self, **kwargs):
            raise RuntimeError("API 返回了非 JSON 内容，响应片段: data: {\"object\":\"chat.completion.chunk\"}")

        async def chat_completion_stream(self, **kwargs):
            yield {"content": "OK"}
            yield {"content": " from stream"}
            yield {"done": True}

    monkeypatch.setattr(chapter_web_research_module, "OpenAIClient", FakeOpenAIClient)

    service = ChapterWebResearchService()
    payload = await service._test_grok_direct_connection(
        WebResearchRuntimeConfig(
            grok_api_key="grok-test-key",
            grok_base_url="https://ai.zzhdsgsss.xyz",
            grok_model="grok-4.1-fast",
        )
    )

    assert payload["content"] == "OK from stream"


@pytest.mark.asyncio
async def test_should_fallback_to_stream_for_grok_search_when_proxy_returns_sse(monkeypatch):
    class FakeOpenAIClient:
        def __init__(self, *, api_key, base_url, compat_profile, config=None, backup_urls=None):
            self.base_url = base_url

        async def chat_completion(self, **kwargs):
            raise RuntimeError("API 返回了非 JSON 内容，响应片段: data: {\"object\":\"chat.completion.chunk\"}")

        async def chat_completion_stream(self, **kwargs):
            yield {"content": '{"content":"summary from stream","sources":[]}'}
            yield {"done": True}

    monkeypatch.setattr(chapter_web_research_module, "OpenAIClient", FakeOpenAIClient)

    service = ChapterWebResearchService()
    payload = await service._run_grok_direct_search(
        "fiction trends",
        WebResearchRuntimeConfig(
            grok_api_key="grok-test-key",
            grok_base_url="https://ai.zzhdsgsss.xyz",
            grok_model="grok-4.1-fast",
        )
    )

    assert payload["content"] == "summary from stream"
    assert payload["mode"] == "direct_chat_search"


@pytest.mark.asyncio
async def test_should_fallback_to_direct_grok_test_when_skill_script_missing(monkeypatch):
    service = ChapterWebResearchService()

    async def fake_run_grok_search(query, runtime_config):
        assert query
        assert runtime_config.grok_api_key == "grok-test-key"
        return {"error": "script_not_found", "detail": "脚本不存在"}

    async def fake_test_grok_direct_connection(runtime_config):
        assert runtime_config.grok_base_url == "https://relay.example.com/v1"
        assert runtime_config.grok_model == "grok-4.1-fast"
        return {"content": "relay ok", "sources": []}

    monkeypatch.setattr(service, "_run_grok_search", fake_run_grok_search)
    monkeypatch.setattr(service, "_test_grok_direct_connection", fake_test_grok_direct_connection)

    payload = await service.test_provider_connection(
        provider="grok",
        overrides={
            "enabled": True,
            "grok_enabled": True,
            "grok_api_key": "grok-test-key",
            "grok_base_url": "https://relay.example.com/v1",
            "grok_model": "grok-4.1-fast",
        },
    )

    assert payload["success"] is True
    assert payload["provider"] == "grok"
    assert payload["source_count"] == 0
    assert payload["response_preview"] == "relay ok"


@pytest.mark.asyncio
async def test_should_collect_assets_with_direct_fallback_when_skills_root_missing(monkeypatch):
    service = ChapterWebResearchService()

    monkeypatch.setattr(service, "skills_root", lambda: chapter_web_research_module.PROJECT_ROOT / ".missing-skills")

    async def fake_run_exa_search(query, runtime_config):
        assert query == "exa relay query"
        assert runtime_config.exa_base_url == "https://exa.chengtx.vip"
        return {
            "results": [
                {
                    "title": "Direct Exa",
                    "url": "https://example.com/direct-exa",
                    "text": "direct exa content",
                }
            ]
        }

    monkeypatch.setattr(service, "_run_exa_search", fake_run_exa_search)
    monkeypatch.setattr(service, "_write_archive", lambda **kwargs: "archive.json")

    payload = await service.collect_assets(
        user_id=None,
        db_session=None,
        exa_query="exa relay query",
        grok_query="",
        enable_web_research=True,
        archive_scope="test-project",
        archive_id="test-chapter",
        runtime_config=WebResearchRuntimeConfig(
            enabled=True,
            exa_enabled=True,
            exa_api_key="exa-test-key",
            exa_base_url="https://exa.chengtx.vip",
        ),
    )

    assert payload["enabled"] is True
    assert payload["archive_path"] == "archive.json"
    assert len(payload["assets"]) == 1
    assert payload["assets"][0]["title"] == "Direct Exa"
