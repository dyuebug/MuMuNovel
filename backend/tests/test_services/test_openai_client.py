import json

import pytest
from unittest.mock import AsyncMock

from app.services.ai_clients.openai_client import OpenAIClient


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
    assert sent_payload["tools"][0]["function"]["parameters"].get("$schema") is None

    assert result["finish_reason"] == "tool_calls"
    assert result["tool_calls"] is not None
    assert result["tool_calls"][0]["id"] == "call_1"
    assert result["tool_calls"][0]["function"]["name"] == "get_weather"
    assert json.loads(result["tool_calls"][0]["function"]["arguments"])["city"] == "Beijing"


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
