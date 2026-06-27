import asyncio
import json

import httpx
import pytest
from unittest.mock import AsyncMock

from tests.test_support.ai_gateway.ai_config import AIClientConfig
from tests.test_support.ai_gateway.ai_clients import base_client as base_client_module
from tests.test_support.ai_gateway.ai_clients.openai_client import OpenAIClient


class FakeStreamResponse:
    def __init__(self, lines):
        self._lines = lines

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        return False

    def raise_for_status(self):
        return None

    async def aiter_lines(self):
        for line in self._lines:
            yield line


class FakeInvalidJsonResponse:
    status_code = 200
    text = "<html>bad gateway</html>"

    def raise_for_status(self):
        return None

    def json(self):
        raise json.JSONDecodeError("Expecting value", "", 0)


class FakeDelayedStreamResponse:
    def __init__(self, lines, first_line_delay):
        self._lines = lines
        self._first_line_delay = first_line_delay

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        return False

    def raise_for_status(self):
        return None

    async def aiter_lines(self):
        for index, line in enumerate(self._lines):
            if index == 0 and self._first_line_delay > 0:
                await asyncio.sleep(self._first_line_delay)
            yield line


class FakeSSEJsonResponse:
    status_code = 200
    text = '\n'.join([
        'data: {"choices":[{"delta":{"content":"Hello "},"finish_reason":null}]}',
        'data: {"choices":[{"delta":{"content":"world"},"finish_reason":"stop"}]}',
        'data: [DONE]',
    ])

    def raise_for_status(self):
        return None

    def json(self):
        raise json.JSONDecodeError("Expecting value", "", 0)

class FakeTransportStreamContext:
    def __init__(self, response=None, enter_error=None):
        self._response = response
        self._enter_error = enter_error

    async def __aenter__(self):
        if self._enter_error is not None:
            raise self._enter_error
        return self._response

    async def __aexit__(self, exc_type, exc, tb):
        return False





def test_should_key_shared_semaphore_by_max_concurrency():
    base_client_module._semaphore_pool.clear()

    low = base_client_module._get_semaphore(1)
    high = base_client_module._get_semaphore(30)
    low_again = base_client_module._get_semaphore(1)

    assert low is low_again
    assert low is not high


@pytest.mark.asyncio
async def test_should_call_responses_endpoint_when_profile_is_sub2api():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.qaq.al",
        compat_profile="sub2api",
    )

    client._request_with_retry = AsyncMock(
        return_value={
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "pong"}],
                }
            ],
        }
    )

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
    )

    client._request_with_retry.assert_awaited_once()
    args = client._request_with_retry.await_args.args
    assert args[0] == "POST"
    assert args[1] == "/responses"
    assert args[2]["model"] == "gpt-5.3-codex"
    assert args[2]["input"][0]["role"] == "user"
    assert args[2]["input"][0]["content"] == [{"type": "input_text", "text": "ping"}]
    assert args[2]["max_output_tokens"] == 128

    assert result["content"] == "pong"
    assert result["finish_reason"] == "stop"
    assert result["tool_calls"] is None


@pytest.mark.asyncio
async def test_should_parse_function_call_from_responses_payload():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.qaq.al",
        compat_profile="sub2api",
    )

    client._request_with_retry = AsyncMock(
        return_value={
            "status": "completed",
            "output": [
                {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "get_weather",
                    "arguments": '{"city":"Beijing"}',
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "done"}],
                },
            ],
        }
    )

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        tools=[
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {
                        "type": "object",
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "properties": {"city": {"type": "string"}},
                    },
                },
            }
        ],
    )

    args = client._request_with_retry.await_args.args
    sent_payload = args[2]
    assert sent_payload["input"][0]["content"] == [{"type": "input_text", "text": "weather"}]
    assert sent_payload["tools"][0]["name"] == "get_weather"
    assert sent_payload["tools"][0]["parameters"].get("$schema") is None

    assert result["finish_reason"] == "tool_calls"
    assert result["tool_calls"] is not None
    assert result["tool_calls"][0]["id"] == "call_1"
    assert result["tool_calls"][0]["function"]["name"] == "get_weather"
    assert json.loads(result["tool_calls"][0]["function"]["arguments"])["city"] == "Beijing"


@pytest.mark.asyncio
async def test_should_fallback_to_chat_completions_when_responses_returns_bad_gateway_for_tools():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz/v1",
        compat_profile="sub2api",
    )

    responses_error = httpx.HTTPStatusError(
        "Server error '502 Bad Gateway' for url 'https://free.9e.nz/responses'",
        request=httpx.Request("POST", "https://free.9e.nz/responses"),
        response=httpx.Response(502, request=httpx.Request("POST", "https://free.9e.nz/responses")),
    )
    client._request_with_retry = AsyncMock(
        side_effect=[
            responses_error,
            {
                "choices": [
                    {
                        "message": {
                            "content": "",
                            "tool_calls": [
                                {
                                    "id": "call_chat_1",
                                    "type": "function",
                                    "function": {
                                        "name": "get_weather",
                                        "arguments": '{"city":"Beijing"}',
                                    },
                                }
                            ],
                        },
                        "finish_reason": "tool_calls",
                    }
                ]
            },
        ]
    )

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        tools=[
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                    },
                },
            }
        ],
        tool_choice="auto",
    )

    assert client._request_with_retry.await_count == 2
    first_call = client._request_with_retry.await_args_list[0].args
    second_call = client._request_with_retry.await_args_list[1].args

    assert first_call[1] == "/responses"
    assert "input" in first_call[2]
    assert second_call[1] == "/chat/completions"
    assert "messages" in second_call[2]
    assert second_call[2]["tool_choice"] == "auto"

    assert result["finish_reason"] == "tool_calls"
    assert result["tool_calls"] is not None
    assert result["tool_calls"][0]["id"] == "call_chat_1"
    diagnostics = client.get_transport_diagnostics()
    assert diagnostics["summary"]["api_mode_fallback_used"] is True
    assert diagnostics["summary"]["api_mode_fallback_count"] == 1
    assert diagnostics["summary"]["request_completed_latency_ms"] is not None
    assert diagnostics["summary"]["final_latency_ms"] is not None
    assert diagnostics["summary"]["api_modes_tried"] == ["responses", "chat_completions"]


@pytest.mark.asyncio
async def test_should_retry_chat_completions_with_v1_suffix_when_base_url_without_v1_returns_html():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    responses_error = httpx.HTTPStatusError(
        "Server error '502 Bad Gateway' for url 'https://free.9e.nz/responses'",
        request=httpx.Request("POST", "https://free.9e.nz/responses"),
        response=httpx.Response(502, request=httpx.Request("POST", "https://free.9e.nz/responses")),
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        base_url = kwargs.get("base_url_override") or client.base_url
        call_log.append((base_url, endpoint))
        if len(call_log) == 1:
            raise responses_error
        return {
            "choices": [
                {
                    "message": {"content": "plain text fallback", "tool_calls": None},
                    "finish_reason": "stop",
                }
            ]
        }

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        tools=[
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                    },
                },
            }
        ],
        tool_choice="auto",
    )

    assert call_log == [
        ("https://free.9e.nz", "/responses"),
        ("https://free.9e.nz/v1", "/chat/completions"),
    ]
    assert client.base_url == "https://free.9e.nz"
    assert result["finish_reason"] == "stop"
    assert result["tool_calls"] is None
    assert result["content"] == "plain text fallback"
    diagnostics = client.get_transport_diagnostics()
    assert diagnostics["summary"]["normalized_base_url_used"] is True


@pytest.mark.asyncio
async def test_should_keep_chat_completions_endpoint_for_openai_profile():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://api.openai.com/v1",
        compat_profile="openai",
    )

    client._request_with_retry = AsyncMock(
        return_value={
            "choices": [
                {
                    "message": {"content": "hello"},
                    "finish_reason": "stop",
                }
            ]
        }
    )

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "hi"}],
        model="gpt-4o-mini",
        temperature=0.2,
        max_tokens=128,
    )

    args = client._request_with_retry.await_args.args
    assert args[1] == "/chat/completions"
    assert "messages" in args[2]
    assert result["content"] == "hello"


@pytest.mark.asyncio
async def test_should_stream_responses_delta_and_tool_calls_for_sub2api():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.qaq.al",
        compat_profile="sub2api",
    )

    stream_lines = [
        'data: {"type":"response.output_text.delta","delta":"Hello "}',
        'data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"get_weather","arguments":"{\\"city\\":\\"Beijing\\"}"}}',
        'data: {"type":"response.completed","response":{"status":"completed"}}',
    ]

    client._request_with_retry = AsyncMock(return_value=FakeStreamResponse(stream_lines))

    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "hi"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
    ):
        chunks.append(chunk)

    args = client._request_with_retry.await_args.args
    assert args[1] == "/responses"
    assert any(chunk.get("content") == "Hello " for chunk in chunks)
    assert any(chunk.get("tool_calls") for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)


@pytest.mark.asyncio
async def test_should_fallback_stream_to_v1_chat_completions_when_responses_stream_fails():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    responses_error = httpx.HTTPStatusError(
        "Server error '502 Bad Gateway' for url 'https://free.9e.nz/responses'",
        request=httpx.Request("POST", "https://free.9e.nz/responses"),
        response=httpx.Response(502, request=httpx.Request("POST", "https://free.9e.nz/responses")),
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        base_url = kwargs.get("base_url_override") or client.base_url
        call_log.append((base_url, endpoint, stream))
        if len(call_log) == 1:
            raise responses_error
        return FakeStreamResponse([
            'data: {"choices":[{"delta":{"content":"Hello "},"finish_reason":null}]}',
            'data: [DONE]',
        ])

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "hi"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
    ):
        chunks.append(chunk)

    assert call_log == [
        ("https://free.9e.nz", "/responses", True),
        ("https://free.9e.nz/v1", "/chat/completions", True),
    ]
    assert client.base_url == "https://free.9e.nz"
    assert any(chunk.get("content") == "Hello " for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)


@pytest.mark.asyncio
async def test_should_raise_readable_error_when_response_is_not_json():
    config = AIClientConfig()
    config.retry.max_retries = 1
    config.rate_limit.request_delay = 0

    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.zzhdsgsss.xyz",
        compat_profile="openai",
        config=config,
    )
    client.http_client.request = AsyncMock(return_value=FakeInvalidJsonResponse())

    with pytest.raises(RuntimeError) as exc_info:
        await client.chat_completion(
            messages=[{"role": "user", "content": "ping"}],
            model="grok-4.1-fast",
            temperature=0.0,
            max_tokens=32,
        )

    assert "non-JSON content" in str(exc_info.value)
    assert "/v1" in str(exc_info.value)


@pytest.mark.asyncio
async def test_should_parse_sse_text_body_when_proxy_returns_data_lines():
    config = AIClientConfig()
    config.retry.max_retries = 1
    config.rate_limit.request_delay = 0

    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.zzhdsgsss.xyz/v1",
        compat_profile="openai",
        config=config,
    )
    client.http_client.request = AsyncMock(return_value=FakeSSEJsonResponse())

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="grok-4.1-fast",
        temperature=0.0,
        max_tokens=32,
    )

    assert result["content"] == "Hello world"
    assert result["finish_reason"] == "stop"


@pytest.mark.asyncio
async def test_should_forward_request_options_when_chat_completion_uses_transport_override():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.qaq.al",
        compat_profile="sub2api",
    )

    client._request_with_retry = AsyncMock(
        return_value={
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "pong"}],
                }
            ],
        }
    )

    request_options = {"read_timeout": 45, "transport_max_retries": 1}
    await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        request_options=request_options,
    )

    assert client._request_with_retry.await_args.kwargs["request_options"] == request_options


@pytest.mark.asyncio
async def test_should_prefer_chat_completions_with_v1_suffix_for_sub2api_text_request():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, kwargs.get("request_options")))
        return {
            "choices": [
                {
                    "message": {"content": "plain text fallback", "tool_calls": None},
                    "finish_reason": "stop",
                }
            ]
        }

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    request_options = {"prefer_chat_completions": True}
    result = await client.chat_completion(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        request_options=request_options,
    )

    assert call_log == [
        ("https://free.9e.nz/v1", "/chat/completions", request_options),
    ]
    assert client.base_url == "https://free.9e.nz"
    assert result["finish_reason"] == "stop"
    assert result["content"] == "plain text fallback"


@pytest.mark.asyncio
async def test_should_prefer_chat_completions_with_v1_suffix_for_sub2api_stream_request():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, stream, kwargs.get("request_options")))
        return FakeStreamResponse([
            'data: {"choices":[{"delta":{"content":"stream fallback"}}]}',
            'data: [DONE]',
        ])

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    request_options = {"prefer_chat_completions": True}
    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        request_options=request_options,
    ):
        chunks.append(chunk)

    assert call_log == [
        ("https://free.9e.nz/v1", "/chat/completions", True, request_options),
    ]
    assert client.base_url == "https://free.9e.nz"
    assert any(chunk.get("content") == "stream fallback" for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)



@pytest.mark.asyncio
async def test_should_prefer_normalized_v1_candidate_for_openai_text_request_when_requested():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://gateway.example.com",
        compat_profile="openai",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, kwargs.get("request_options")))
        return {
            "choices": [
                {
                    "message": {"content": "normalized candidate", "tool_calls": None},
                    "finish_reason": "stop",
                }
            ]
        }

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    request_options = {"prefer_normalized_v1_candidate": True}
    result = await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="gpt-4.1",
        temperature=0.2,
        max_tokens=64,
        request_options=request_options,
    )

    assert call_log == [
        ("https://gateway.example.com/v1", "/chat/completions", request_options),
    ]
    assert result["content"] == "normalized candidate"
    assert result["finish_reason"] == "stop"


@pytest.mark.asyncio
async def test_should_fallback_from_normalized_v1_candidate_to_root_for_openai_text_request():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://gateway.example.com",
        compat_profile="openai",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, kwargs.get("request_options")))
        current_base_url = kwargs.get("base_url_override") or client.base_url
        if current_base_url.endswith('/v1'):
            raise httpx.HTTPStatusError(
                "Client error '404 Not Found' for url 'https://gateway.example.com/v1/chat/completions'",
                request=httpx.Request("POST", "https://gateway.example.com/v1/chat/completions"),
                response=httpx.Response(404, request=httpx.Request("POST", "https://gateway.example.com/v1/chat/completions")),
            )
        return {
            "choices": [
                {
                    "message": {"content": "root candidate", "tool_calls": None},
                    "finish_reason": "stop",
                }
            ]
        }

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    request_options = {"prefer_normalized_v1_candidate": True}
    result = await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="gpt-4.1",
        temperature=0.2,
        max_tokens=64,
        request_options=request_options,
    )

    assert call_log == [
        ("https://gateway.example.com/v1", "/chat/completions", request_options),
        ("https://gateway.example.com", "/chat/completions", request_options),
    ]
    assert result["content"] == "root candidate"


@pytest.mark.asyncio
async def test_should_retry_v1_when_root_chat_completions_returns_html_for_text_request():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://gateway.example.com",
        compat_profile="openai",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        current_base_url = kwargs.get("base_url_override") or client.base_url
        call_log.append((current_base_url, endpoint, kwargs.get("request_options")))
        if current_base_url == "https://gateway.example.com":
            raise RuntimeError(
                "API returned non-JSON content. The Base URL may be incorrect "
                "(for example, missing /v1). HTTP 200, response preview: <!doctype html>"
            )
        return {
            "choices": [
                {
                    "message": {"content": "v1 candidate", "tool_calls": None},
                    "finish_reason": "stop",
                }
            ]
        }

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "ping"}],
        model="deepseek-v4-pro",
        temperature=0.2,
        max_tokens=64,
    )

    assert call_log == [
        ("https://gateway.example.com", "/chat/completions", None),
        ("https://gateway.example.com/v1", "/chat/completions", None),
    ]
    assert result["content"] == "v1 candidate"
    assert result["finish_reason"] == "stop"


@pytest.mark.asyncio
async def test_should_prefer_normalized_v1_candidate_for_openai_stream_request_when_requested():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://gateway.example.com",
        compat_profile="openai",
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, stream, kwargs.get("request_options")))
        return FakeStreamResponse([
            'data: {"choices":[{"delta":{"content":"normalized stream"}}]}',
            'data: [DONE]',
        ])

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    request_options = {"prefer_normalized_v1_candidate": True}
    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-4.1",
        temperature=0.2,
        max_tokens=64,
        request_options=request_options,
    ):
        chunks.append(chunk)

    assert call_log == [
        ("https://gateway.example.com/v1", "/chat/completions", True, request_options),
    ]
    assert any(chunk.get("content") == "normalized stream" for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)


@pytest.mark.asyncio
async def test_should_not_fallback_to_bare_root_for_sub2api_text_request_after_v1_server_error():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    request_options = {"prefer_chat_completions": True}
    server_error = httpx.HTTPStatusError(
        "Server error '502 Bad Gateway' for url 'https://free.9e.nz/v1/chat/completions'",
        request=httpx.Request("POST", "https://free.9e.nz/v1/chat/completions"),
        response=httpx.Response(502, request=httpx.Request("POST", "https://free.9e.nz/v1/chat/completions")),
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        call_log.append(((kwargs.get("base_url_override") or client.base_url), endpoint, kwargs.get("request_options")))
        raise server_error

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    with pytest.raises(httpx.HTTPStatusError):
        await client.chat_completion(
            messages=[{"role": "user", "content": "weather"}],
            model="gpt-5.3-codex",
            temperature=0.2,
            max_tokens=128,
            request_options=request_options,
        )

    assert call_log == [
        ("https://free.9e.nz/v1", "/chat/completions", request_options),
    ]
    assert client.base_url == "https://free.9e.nz"


@pytest.mark.asyncio
async def test_should_convert_system_and_user_messages_to_responses_input_blocks():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://ai.qaq.al",
        compat_profile="sub2api",
    )

    payload = client._build_payload(
        messages=[
            {"role": "system", "content": "system rule"},
            {"role": "user", "content": "hello"},
        ],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        use_responses_api=True,
    )

    assert payload["input"] == [
        {"role": "system", "content": [{"type": "input_text", "text": "system rule"}]},
        {"role": "user", "content": [{"type": "input_text", "text": "hello"}]},
    ]


@pytest.mark.asyncio
async def test_should_fallback_to_chat_completions_when_responses_times_out_for_tools():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz/v1",
        compat_profile="sub2api",
    )

    responses_error = httpx.ReadTimeout(
        "Read timed out",
        request=httpx.Request("POST", "https://free.9e.nz/responses"),
    )
    client._request_with_retry = AsyncMock(
        side_effect=[
            responses_error,
            {
                "choices": [
                    {
                        "message": {
                            "content": "",
                            "tool_calls": [
                                {
                                    "id": "call_chat_timeout_1",
                                    "type": "function",
                                    "function": {
                                        "name": "get_weather",
                                        "arguments": '{"city":"Beijing"}',
                                    },
                                }
                            ],
                        },
                        "finish_reason": "tool_calls",
                    }
                ]
            },
        ]
    )

    result = await client.chat_completion(
        messages=[{"role": "user", "content": "weather"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
        tools=[
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                    },
                },
            }
        ],
        tool_choice="auto",
    )

    assert client._request_with_retry.await_count == 2
    first_call = client._request_with_retry.await_args_list[0].args
    second_call = client._request_with_retry.await_args_list[1].args
    assert first_call[1] == "/responses"
    assert second_call[1] == "/chat/completions"
    assert result["finish_reason"] == "tool_calls"
    assert result["tool_calls"][0]["id"] == "call_chat_timeout_1"


@pytest.mark.asyncio
async def test_should_fallback_stream_to_v1_chat_completions_when_responses_stream_times_out():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    responses_error = httpx.ReadTimeout(
        "Read timed out",
        request=httpx.Request("POST", "https://free.9e.nz/responses"),
    )

    call_log = []

    async def fake_request(method, endpoint, payload, stream=False, **kwargs):
        base_url = kwargs.get("base_url_override") or client.base_url
        call_log.append((base_url, endpoint, stream))
        if len(call_log) == 1:
            raise responses_error
        return FakeStreamResponse([
            'data: {"choices":[{"delta":{"content":"Hello timeout "},"finish_reason":null}]}',
            'data: [DONE]',
        ])

    client._request_with_retry = AsyncMock(side_effect=fake_request)

    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "hi"}],
        model="gpt-5.3-codex",
        temperature=0.2,
        max_tokens=128,
    ):
        chunks.append(chunk)

    assert call_log == [
        ("https://free.9e.nz", "/responses", True),
        ("https://free.9e.nz/v1", "/chat/completions", True),
    ]
    assert client.base_url == "https://free.9e.nz"
    assert any(chunk.get("content") == "Hello timeout " for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)

    diagnostics = client.get_transport_diagnostics()
    summary = diagnostics["summary"]
    assert summary["api_mode_fallback_count"] == 1
    assert summary["candidate_fallback_count"] == 1
    assert summary["fallback_count"] == 2
    assert summary["first_chunk_latency_ms"] is not None
    assert summary["final_latency_ms"] is not None
    assert all(attempt.get("duration_ms") is not None for attempt in diagnostics["attempts"])


@pytest.mark.asyncio
async def test_should_timeout_when_chat_completions_stream_waits_too_long_for_first_data_line():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://api.example.com/v1",
        compat_profile="openai",
    )

    client._request_with_retry = AsyncMock(
        return_value=FakeDelayedStreamResponse(
            [
                'data: {"choices":[{"delta":{"content":"late body"},"finish_reason":null}]}',
                'data: [DONE]',
            ],
            first_line_delay=0.05,
        )
    )

    with pytest.raises(httpx.ReadTimeout, match='first SSE data line'):
        async for _ in client.chat_completion_stream(
            messages=[{"role": "user", "content": "hi"}],
            model="gpt-4o-mini",
            temperature=0.2,
            max_tokens=64,
            request_options={"first_chunk_timeout": 0.01},
        ):
            pass


@pytest.mark.asyncio
async def test_should_timeout_when_responses_stream_waits_too_long_for_first_data_line():
    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://free.9e.nz",
        compat_profile="sub2api",
    )

    client._request_with_retry = AsyncMock(
        return_value=FakeDelayedStreamResponse(
            [
                'data: {"type":"response.output_text.delta","delta":"late body"}',
                'data: {"type":"response.completed","response":{"status":"completed"}}',
            ],
            first_line_delay=0.05,
        )
    )

    with pytest.raises(httpx.ReadTimeout, match='first SSE data line'):
        async for _ in client.chat_completion_stream(
            messages=[{"role": "user", "content": "hi"}],
            model="gpt-5.3-codex",
            temperature=0.2,
            max_tokens=128,
            request_options={"first_chunk_timeout": 0.01},
        ):
            pass

    assert [call.args[1] for call in client._request_with_retry.await_args_list] == [
        "/responses",
        "/chat/completions",
    ]


@pytest.mark.asyncio
async def test_should_failover_stream_request_to_backup_url_before_first_chunk():
    config = AIClientConfig()
    config.retry.max_retries = 1
    config.rate_limit.request_delay = 0

    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://primary.example.com/v1",
        backup_urls=["https://backup.example.com/v1"],
        compat_profile="openai",
        config=config,
    )

    stream_calls = []

    def fake_stream(method, url, **kwargs):
        stream_calls.append((method, url))
        if len(stream_calls) == 1:
            return FakeTransportStreamContext(
                enter_error=httpx.ConnectError(
                    "connect failed",
                    request=httpx.Request(method, url),
                )
            )
        return FakeTransportStreamContext(
            response=FakeStreamResponse([
                'data: {"choices":[{"delta":{"content":"backup body"},"finish_reason":null}]}',
                'data: [DONE]',
            ])
        )

    client.http_client.stream = fake_stream

    chunks = []
    async for chunk in client.chat_completion_stream(
        messages=[{"role": "user", "content": "hi"}],
        model="gpt-4o-mini",
        temperature=0.2,
        max_tokens=64,
    ):
        chunks.append(chunk)

    assert stream_calls == [
        ("POST", "https://primary.example.com/v1/chat/completions"),
        ("POST", "https://backup.example.com/v1/chat/completions"),
    ]
    assert any(chunk.get("content") == "backup body" for chunk in chunks)
    assert any(chunk.get("done") is True for chunk in chunks)

    diagnostics = client.get_transport_diagnostics()
    summary = diagnostics["summary"]
    assert summary["failover_count"] == 1
    assert summary["backup_endpoint_used"] is True
    assert summary["first_chunk_latency_ms"] is not None
    assert summary["final_latency_ms"] is not None
    assert all(attempt.get("duration_ms") is not None for attempt in diagnostics["attempts"])

def test_should_disable_env_proxy_for_loopback_base_url():
    base_client_module._http_client_pool.clear()

    loopback_client = OpenAIClient(
        api_key="sk-test",
        base_url="http://127.0.0.1:8317/v1",
        compat_profile="sub2api",
    )
    external_client = OpenAIClient(
        api_key="sk-test-external",
        base_url="https://api.openai.com/v1",
        compat_profile="openai",
    )

    assert loopback_client.http_client._trust_env is False
    assert external_client.http_client._trust_env is True


def test_should_append_docker_host_candidate_for_loopback_v1_base_url(monkeypatch):
    monkeypatch.setattr(OpenAIClient, "_is_running_in_docker", staticmethod(lambda: True))

    client = OpenAIClient(
        api_key="sk-test",
        base_url="http://127.0.0.1:8317/v1",
        compat_profile="sub2api",
    )

    assert client._build_chat_completions_base_url_candidates() == [
        "http://127.0.0.1:8317/v1",
        "http://host.docker.internal:8317/v1",
    ]


def test_should_append_docker_host_candidate_for_localhost_base_url(monkeypatch):
    monkeypatch.setattr(OpenAIClient, "_is_running_in_docker", staticmethod(lambda: True))

    client = OpenAIClient(
        api_key="sk-test",
        base_url="http://localhost:8317",
        compat_profile="openai",
    )

    assert client._build_chat_completions_base_url_candidates() == [
        "http://localhost:8317",
        "http://host.docker.internal:8317",
        "http://localhost:8317/v1",
        "http://host.docker.internal:8317/v1",
    ]


def test_should_append_http_fallback_candidates_for_local_https_gateway(monkeypatch):
    monkeypatch.setattr(OpenAIClient, "_is_running_in_docker", staticmethod(lambda: True))

    client = OpenAIClient(
        api_key="sk-test",
        base_url="https://127.0.0.1:8317/v1",
        compat_profile="sub2api",
    )

    assert client._build_chat_completions_base_url_candidates() == [
        "https://127.0.0.1:8317/v1",
        "https://host.docker.internal:8317/v1",
        "http://127.0.0.1:8317/v1",
        "http://host.docker.internal:8317/v1",
    ]
