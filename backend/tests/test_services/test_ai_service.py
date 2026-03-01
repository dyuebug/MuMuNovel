from unittest.mock import AsyncMock, Mock

import pytest

from app.services import ai_service

pytestmark = pytest.mark.asyncio


@pytest.fixture(autouse=True)
def fixture_should_mock_app_settings(mocker):
    # 统一模拟配置，避免测试依赖本地环境变量
    settings_values = {
        "default_ai_provider": "openai",
        "default_model": "gpt-test-default",
        "default_temperature": 0.35,
        "default_max_tokens": 1024,
        "openai_api_key": None,
        "openai_base_url": "https://default.openai.example/v1",
        "anthropic_api_key": None,
        "anthropic_base_url": "https://api.anthropic.example",
        "mcp_max_rounds": 3,
    }
    for key, value in settings_values.items():
        mocker.patch.object(ai_service.app_settings, key, value)
    return settings_values


@pytest.fixture
def fixture_should_mock_ai_dependencies(mocker):
    # 统一 mock 外部客户端与 provider
    return {
        "openai_client_cls": mocker.patch.object(ai_service, "OpenAIClient"),
        "anthropic_client_cls": mocker.patch.object(ai_service, "AnthropicClient"),
        "gemini_client_cls": mocker.patch.object(ai_service, "GeminiClient"),
        "openai_provider_cls": mocker.patch.object(ai_service, "OpenAIProvider"),
        "anthropic_provider_cls": mocker.patch.object(ai_service, "AnthropicProvider"),
        "gemini_provider_cls": mocker.patch.object(ai_service, "GeminiProvider"),
    }


def build_service(
    api_provider="openai",
    api_key="sk-test",
    api_base_url="https://provider.example/v1",
    user_id=None,
    db_session=None,
    enable_mcp=True,
    backup_urls=None,
    fallback_strategy="auto",
):
    return ai_service.AIService(
        api_provider=api_provider,
        api_key=api_key,
        api_base_url=api_base_url,
        default_model="model-test",
        default_temperature=0.7,
        default_max_tokens=666,
        default_system_prompt="系统提示",
        user_id=user_id,
        db_session=db_session,
        enable_mcp=enable_mcp,
        backup_urls=backup_urls,
        fallback_strategy=fallback_strategy,
    )


@pytest.fixture
def runtime_service(mocker):
    # 使用 __new__ 构造最小可用对象，聚焦方法行为
    service = ai_service.AIService.__new__(ai_service.AIService)
    service.api_provider = "openai"
    service.default_model = "gpt-runtime"
    service.default_temperature = 0.2
    service.default_max_tokens = 512
    service.default_system_prompt = "运行时系统提示"
    service.config = ai_service.default_config
    service.user_id = "user_001"
    service.db_session = object()
    service._enable_mcp = True
    service._cached_tools = None
    service._tools_loaded = False
    service._openai_provider = Mock()
    service._openai_provider.generate = AsyncMock(return_value={"content": "ok"})
    service._anthropic_provider = None
    service._gemini_provider = None
    return service


@pytest.mark.parametrize("provider_name", ["openai", "newapi", "azure", "custom"])
def test_should_initialize_openai_provider_when_provider_is_openai_compatible(
    provider_name,
    fixture_should_mock_ai_dependencies,
):
    backup_urls = [
        "https://backup-1.example/v1",
        "https://backup-2.example/v1",
    ]
    service = build_service(
        api_provider=provider_name,
        api_key="sk-provider",
        api_base_url="https://provider.example/v1",
        backup_urls=backup_urls,
        fallback_strategy="auto",
    )

    openai_client_cls = fixture_should_mock_ai_dependencies["openai_client_cls"]
    openai_provider_cls = fixture_should_mock_ai_dependencies["openai_provider_cls"]
    anthropic_client_cls = fixture_should_mock_ai_dependencies["anthropic_client_cls"]
    gemini_client_cls = fixture_should_mock_ai_dependencies["gemini_client_cls"]

    openai_client_cls.assert_called_once()
    args, kwargs = openai_client_cls.call_args
    assert args[0] == "sk-provider"
    assert args[1] == "https://provider.example/v1"
    assert args[2] == service.config
    assert kwargs["backup_urls"] == backup_urls
    assert kwargs["compat_profile"] == provider_name

    openai_provider_cls.assert_called_once_with(openai_client_cls.return_value)
    assert service._openai_provider is openai_provider_cls.return_value
    anthropic_client_cls.assert_not_called()
    gemini_client_cls.assert_not_called()


def test_should_skip_backup_urls_when_fallback_strategy_is_not_auto(
    fixture_should_mock_ai_dependencies,
):
    build_service(
        api_provider="openai",
        api_key="sk-provider",
        api_base_url="https://provider.example/v1",
        backup_urls=["https://backup.example/v1"],
        fallback_strategy="manual",
    )

    openai_client_cls = fixture_should_mock_ai_dependencies["openai_client_cls"]
    args, kwargs = openai_client_cls.call_args
    assert args[0] == "sk-provider"
    assert kwargs["backup_urls"] is None
    assert kwargs["compat_profile"] == "openai"


def test_should_initialize_anthropic_provider_when_provider_is_anthropic(
    fixture_should_mock_ai_dependencies,
):
    service = build_service(
        api_provider="anthropic",
        api_key="ak-provider",
        api_base_url="https://anthropic.example",
    )

    anthropic_client_cls = fixture_should_mock_ai_dependencies["anthropic_client_cls"]
    anthropic_provider_cls = fixture_should_mock_ai_dependencies["anthropic_provider_cls"]
    openai_client_cls = fixture_should_mock_ai_dependencies["openai_client_cls"]
    gemini_client_cls = fixture_should_mock_ai_dependencies["gemini_client_cls"]

    anthropic_client_cls.assert_called_once_with(
        "ak-provider",
        "https://anthropic.example",
        service.config,
    )
    anthropic_provider_cls.assert_called_once_with(anthropic_client_cls.return_value)
    assert service._anthropic_provider is anthropic_provider_cls.return_value
    openai_client_cls.assert_not_called()
    gemini_client_cls.assert_not_called()


def test_should_initialize_gemini_provider_when_provider_is_gemini(
    fixture_should_mock_ai_dependencies,
):
    service = build_service(
        api_provider="gemini",
        api_key="gm-provider",
        api_base_url="https://gemini.example",
    )

    gemini_client_cls = fixture_should_mock_ai_dependencies["gemini_client_cls"]
    gemini_provider_cls = fixture_should_mock_ai_dependencies["gemini_provider_cls"]
    openai_client_cls = fixture_should_mock_ai_dependencies["openai_client_cls"]
    anthropic_client_cls = fixture_should_mock_ai_dependencies["anthropic_client_cls"]

    gemini_client_cls.assert_called_once_with(
        "gm-provider",
        "https://gemini.example",
        service.config,
    )
    gemini_provider_cls.assert_called_once_with(gemini_client_cls.return_value)
    assert service._gemini_provider is gemini_provider_cls.return_value
    openai_client_cls.assert_not_called()
    anthropic_client_cls.assert_not_called()


@pytest.mark.parametrize(
    ("raw_provider", "expected"),
    [
        ("openai", "openai"),
        ("OPENAI", "openai"),
        ("newapi", "openai"),
        ("Azure", "openai"),
        ("custom", "openai"),
        ("ANTHROPIC", "anthropic"),
        ("Gemini", "gemini"),
        ("thirdParty", "thirdparty"),
    ],
)
def test_should_normalize_provider_name_when_provider_has_alias(
    runtime_service,
    raw_provider,
    expected,
):
    assert runtime_service._normalize_provider(raw_provider) == expected


@pytest.mark.parametrize("provider_name", ["openai", "newapi", "azure", "custom"])
def test_should_route_to_openai_provider_when_provider_is_openai_compatible(
    runtime_service,
    provider_name,
):
    target_provider = Mock()
    runtime_service._openai_provider = target_provider
    assert runtime_service._get_provider(provider_name) is target_provider


@pytest.mark.parametrize(
    ("provider_name", "provider_attr"),
    [
        ("anthropic", "_anthropic_provider"),
        ("gemini", "_gemini_provider"),
    ],
)
def test_should_route_to_specific_provider_when_provider_is_initialized(
    runtime_service,
    provider_name,
    provider_attr,
):
    target_provider = Mock()
    setattr(runtime_service, provider_attr, target_provider)
    assert runtime_service._get_provider(provider_name) is target_provider


def test_should_raise_error_when_provider_not_initialized(runtime_service):
    runtime_service._openai_provider = None
    runtime_service._anthropic_provider = None
    runtime_service._gemini_provider = None

    with pytest.raises(ValueError, match="Provider"):
        runtime_service._get_provider("openai")


def test_should_clear_cache_when_disable_mcp_from_enabled_state(runtime_service, mocker):
    runtime_service._enable_mcp = True
    clear_cache_mock = mocker.patch.object(runtime_service, "clear_mcp_cache")

    runtime_service.enable_mcp = False

    clear_cache_mock.assert_called_once_with()
    assert runtime_service.enable_mcp is False


def test_should_not_clear_cache_when_switch_mcp_to_enabled(runtime_service, mocker):
    runtime_service._enable_mcp = False
    clear_cache_mock = mocker.patch.object(runtime_service, "clear_mcp_cache")

    runtime_service.enable_mcp = True

    clear_cache_mock.assert_not_called()
    assert runtime_service.enable_mcp is True


def test_should_reset_cache_flags_when_clear_mcp_cache_with_tools(runtime_service):
    runtime_service._cached_tools = [{"name": "tool_a"}]
    runtime_service._tools_loaded = True

    runtime_service.clear_mcp_cache()

    assert runtime_service._cached_tools is None
    assert runtime_service._tools_loaded is False


def test_should_keep_cache_empty_when_clear_mcp_cache_without_tools(runtime_service):
    runtime_service._cached_tools = None
    runtime_service._tools_loaded = True

    runtime_service.clear_mcp_cache()

    assert runtime_service._cached_tools is None
    assert runtime_service._tools_loaded is False


async def test_should_skip_loading_tools_when_enable_mcp_is_false(runtime_service, mocker):
    runtime_service._enable_mcp = False
    runtime_service._cached_tools = [{"type": "function"}]
    runtime_service._tools_loaded = True
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=True)

    assert result is None
    assert runtime_service._cached_tools is None
    assert runtime_service._tools_loaded is False
    get_user_tools_mock.assert_not_awaited()


async def test_should_skip_loading_tools_when_auto_mcp_is_false(runtime_service, mocker):
    runtime_service._enable_mcp = True
    runtime_service._cached_tools = [{"type": "function"}]
    runtime_service._tools_loaded = True
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=False)

    assert result is None
    assert runtime_service._cached_tools is None
    assert runtime_service._tools_loaded is False
    get_user_tools_mock.assert_not_awaited()


async def test_should_skip_loading_tools_when_user_id_missing(runtime_service, mocker):
    runtime_service.user_id = None
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=True)

    assert result is None
    get_user_tools_mock.assert_not_awaited()


async def test_should_skip_loading_tools_when_db_session_missing(runtime_service, mocker):
    runtime_service.db_session = None
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=True)

    assert result is None
    get_user_tools_mock.assert_not_awaited()


async def test_should_return_cached_tools_when_tools_already_loaded(runtime_service, mocker):
    runtime_service._cached_tools = [{"type": "function", "function": {"name": "cached"}}]
    runtime_service._tools_loaded = True
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=True)

    assert result == runtime_service._cached_tools
    get_user_tools_mock.assert_not_awaited()


async def test_should_cache_tools_when_loading_tools_successful(runtime_service, mocker):
    loaded_tools = [{"type": "function", "function": {"name": "search"}}]
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
        return_value=loaded_tools,
    )

    result = await runtime_service._prepare_mcp_tools(
        auto_mcp=True,
        force_refresh=True,
    )

    assert result == loaded_tools
    assert runtime_service._cached_tools == loaded_tools
    assert runtime_service._tools_loaded is True
    get_user_tools_mock.assert_awaited_once_with(
        user_id=runtime_service.user_id,
        db_session=runtime_service.db_session,
        use_cache=True,
        force_refresh=True,
    )


async def test_should_return_none_when_loading_tools_failed(runtime_service, mocker):
    get_user_tools_mock = mocker.patch(
        "app.services.mcp_tools_loader.mcp_tools_loader.get_user_tools",
        new_callable=AsyncMock,
        side_effect=RuntimeError("load failed"),
    )

    result = await runtime_service._prepare_mcp_tools(auto_mcp=True)

    assert result is None
    assert runtime_service._tools_loaded is True
    assert runtime_service._cached_tools is None
    get_user_tools_mock.assert_awaited_once()


async def test_should_generate_text_normally_when_no_tool_calls(runtime_service, mocker):
    runtime_service._openai_provider.generate = AsyncMock(
        return_value={"content": "文本结果", "finish_reason": "stop"}
    )
    prepare_tools_mock = mocker.patch.object(
        runtime_service,
        "_prepare_mcp_tools",
        new=AsyncMock(),
    )

    result = await runtime_service.generate_text(
        prompt="生成摘要",
        auto_mcp=False,
    )

    assert result == {"content": "文本结果", "finish_reason": "stop"}
    prepare_tools_mock.assert_not_awaited()
    runtime_service._openai_provider.generate.assert_awaited_once_with(
        prompt="生成摘要",
        model="gpt-runtime",
        temperature=0.2,
        max_tokens=512,
        system_prompt="运行时系统提示",
        tools=None,
        tool_choice=None,
    )


async def test_should_prepare_mcp_tools_when_auto_mcp_enabled_and_tools_not_provided(
    runtime_service,
    mocker,
):
    mcp_tools = [{"type": "function", "function": {"name": "lookup"}}]
    runtime_service._openai_provider.generate = AsyncMock(
        return_value={"content": "带工具结果", "finish_reason": "stop"}
    )
    prepare_tools_mock = mocker.patch.object(
        runtime_service,
        "_prepare_mcp_tools",
        new=AsyncMock(return_value=mcp_tools),
    )

    result = await runtime_service.generate_text(
        prompt="查询数据",
        auto_mcp=True,
    )

    assert result["content"] == "带工具结果"
    prepare_tools_mock.assert_awaited_once_with(auto_mcp=True)
    runtime_service._openai_provider.generate.assert_awaited_once_with(
        prompt="查询数据",
        model="gpt-runtime",
        temperature=0.2,
        max_tokens=512,
        system_prompt="运行时系统提示",
        tools=mcp_tools,
        tool_choice=None,
    )


async def test_should_handle_tool_calls_when_response_contains_tool_calls(runtime_service, mocker):
    provider_response = {
        "content": "",
        "tool_calls": [{"id": "call_1", "function": {"name": "search"}}],
        "finish_reason": "tool_calls",
    }
    runtime_service._openai_provider.generate = AsyncMock(return_value=provider_response)
    handle_tool_calls_mock = mocker.patch.object(
        runtime_service,
        "_handle_tool_calls",
        new=AsyncMock(return_value={"content": "最终结果", "mcp_enhanced": True}),
    )

    result = await runtime_service.generate_text(
        prompt="请先调用工具",
        auto_mcp=False,
        handle_tool_calls=True,
    )

    assert result == {"content": "最终结果", "mcp_enhanced": True}
    handle_tool_calls_mock.assert_awaited_once()
    handle_kwargs = handle_tool_calls_mock.await_args.kwargs
    assert handle_kwargs["original_prompt"] == "请先调用工具"
    assert handle_kwargs["response"] == provider_response
    assert handle_kwargs["provider"] is None
    assert handle_kwargs["max_rounds"] == ai_service.app_settings.mcp_max_rounds


async def test_should_parse_json_successfully_when_first_attempt_valid(runtime_service, mocker):
    runtime_service.generate_text = AsyncMock(
        return_value={"content": '{"name":"mumu","ok":true}'}
    )
    parse_json_mock = mocker.patch.object(
        ai_service,
        "parse_json",
        return_value={"name": "mumu", "ok": True},
    )

    result = await runtime_service.call_with_json_retry(prompt="返回JSON对象")

    assert result == {"name": "mumu", "ok": True}
    runtime_service.generate_text.assert_awaited_once()
    parse_json_mock.assert_called_once_with('{"name":"mumu","ok":true}')
    call_kwargs = runtime_service.generate_text.await_args.kwargs
    assert call_kwargs["prompt"] == "返回JSON对象"
    assert call_kwargs["handle_tool_calls"] is True
    assert call_kwargs["auto_mcp"] is True


async def test_should_retry_with_hint_when_first_json_parse_failed(runtime_service, mocker):
    runtime_service.generate_text = AsyncMock(
        side_effect=[
            {"content": "第一次失败"},
            {"content": '{"retry":2,"ok":true}'},
        ]
    )
    mocker.patch.object(
        ai_service,
        "parse_json",
        side_effect=[ValueError("invalid"), {"retry": 2, "ok": True}],
    )

    result = await runtime_service.call_with_json_retry(
        prompt="请返回JSON",
        max_retries=2,
        auto_mcp=False,
    )

    assert result == {"retry": 2, "ok": True}
    assert runtime_service.generate_text.await_count == 2
    first_prompt = runtime_service.generate_text.await_args_list[0].kwargs["prompt"]
    second_prompt = runtime_service.generate_text.await_args_list[1].kwargs["prompt"]
    assert first_prompt == "请返回JSON"
    assert second_prompt != "请返回JSON"
    assert "第一次失败" in second_prompt


async def test_should_raise_error_when_json_parse_failed_after_max_retries(runtime_service, mocker):
    runtime_service.generate_text = AsyncMock(return_value={"content": "无效JSON"})
    mocker.patch.object(
        ai_service,
        "parse_json",
        side_effect=ValueError("invalid json"),
    )

    with pytest.raises(ValueError, match="JSON"):
        await runtime_service.call_with_json_retry(
            prompt="返回结构化结果",
            max_retries=2,
        )

    assert runtime_service.generate_text.await_count == 2


@pytest.mark.parametrize(
    ("expected_type", "parsed_data"),
    [
        ("object", [1, 2, 3]),
        ("array", {"a": 1}),
    ],
)
async def test_should_raise_error_when_parsed_json_type_not_expected(
    runtime_service,
    mocker,
    expected_type,
    parsed_data,
):
    runtime_service.generate_text = AsyncMock(return_value={"content": "{}"})
    mocker.patch.object(
        ai_service,
        "parse_json",
        return_value=parsed_data,
    )

    with pytest.raises(ValueError, match="JSON"):
        await runtime_service.call_with_json_retry(
            prompt="返回指定JSON类型",
            expected_type=expected_type,
            max_retries=1,
        )


def test_should_add_json_hint_when_build_retry_prompt():
    failed = "x" * 260
    hinted_prompt = ai_service.AIService._add_json_hint(
        prompt="原始提示",
        failed=failed,
        attempt=2,
    )

    assert "原始提示" in hinted_prompt
    assert "2" in hinted_prompt
    assert ("x" * 200) in hinted_prompt
    assert ("x" * 220) not in hinted_prompt


def test_should_delegate_clean_json_response_when_call_clean_method(mocker):
    clean_json_mock = mocker.patch.object(
        ai_service,
        "clean_json_response",
        return_value='{"cleaned": true}',
    )

    result = ai_service.AIService._clean_json_response("raw-text")

    assert result == '{"cleaned": true}'
    clean_json_mock.assert_called_once_with("raw-text")


def test_should_create_ai_service_when_call_create_user_ai_service(mocker):
    ai_service_cls = mocker.patch.object(ai_service, "AIService")

    result = ai_service.create_user_ai_service(
        api_provider="openai",
        api_key="sk-factory",
        api_base_url="https://factory.example/v1",
        model_name="gpt-factory",
        temperature=0.6,
        max_tokens=2048,
        system_prompt="工厂系统提示",
        backup_urls=["https://backup.factory/v1"],
        fallback_strategy="auto",
    )

    assert result is ai_service_cls.return_value
    ai_service_cls.assert_called_once_with(
        api_provider="openai",
        api_key="sk-factory",
        api_base_url="https://factory.example/v1",
        default_model="gpt-factory",
        default_temperature=0.6,
        default_max_tokens=2048,
        default_system_prompt="工厂系统提示",
        backup_urls=["https://backup.factory/v1"],
        fallback_strategy="auto",
    )


def test_should_create_ai_service_with_mcp_when_call_create_user_ai_service_with_mcp(mocker):
    ai_service_cls = mocker.patch.object(ai_service, "AIService")
    fake_db_session = object()

    result = ai_service.create_user_ai_service_with_mcp(
        api_provider="anthropic",
        api_key="ak-factory",
        api_base_url="https://factory.anthropic/v1",
        model_name="claude-factory",
        temperature=0.4,
        max_tokens=4096,
        user_id="user_factory_001",
        db_session=fake_db_session,
        system_prompt="MCP系统提示",
        enable_mcp=False,
        backup_urls=["https://backup.anthropic/v1"],
        fallback_strategy="manual",
    )

    assert result is ai_service_cls.return_value
    ai_service_cls.assert_called_once_with(
        api_provider="anthropic",
        api_key="ak-factory",
        api_base_url="https://factory.anthropic/v1",
        default_model="claude-factory",
        default_temperature=0.4,
        default_max_tokens=4096,
        default_system_prompt="MCP系统提示",
        user_id="user_factory_001",
        db_session=fake_db_session,
        enable_mcp=False,
        backup_urls=["https://backup.anthropic/v1"],
        fallback_strategy="manual",
    )
