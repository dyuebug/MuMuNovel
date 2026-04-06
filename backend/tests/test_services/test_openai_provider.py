import httpx
from unittest.mock import AsyncMock

import pytest

from app.services.ai_providers.openai_provider import OpenAIProvider

pytestmark = pytest.mark.asyncio


async def _failing_stream(*args, **kwargs):
    raise httpx.HTTPStatusError(
        "Server error '502 Bad Gateway'",
        request=httpx.Request("POST", "https://free.9e.nz/v1/chat/completions"),
        response=httpx.Response(502, request=httpx.Request("POST", "https://free.9e.nz/v1/chat/completions")),
    )
    if False:
        yield {}


async def test_should_fallback_to_non_stream_completion_when_stream_fails_before_content():
    client = type("ClientStub", (), {})()
    client.chat_completion_stream = _failing_stream
    client.chat_completion = AsyncMock(
        return_value={
            "content": "fallback body",
            "tool_calls": None,
            "finish_reason": "stop",
        }
    )

    provider = OpenAIProvider(client)

    chunks = []
    async for chunk in provider.generate_stream(
        prompt="hello",
        model="gpt-test",
        temperature=0.2,
        max_tokens=64,
    ):
        chunks.append(chunk)

    assert chunks == ["fallback body"]
    client.chat_completion.assert_awaited_once()


async def _timeout_stream(*args, **kwargs):
    raise httpx.ReadTimeout(
        "Read timed out",
        request=httpx.Request("POST", "https://free.9e.nz/v1/chat/completions"),
    )
    if False:
        yield {}


async def test_should_fallback_to_non_stream_completion_when_stream_times_out_before_content():
    client = type("ClientStub", (), {})()
    client.chat_completion_stream = _timeout_stream
    client.chat_completion = AsyncMock(
        return_value={
            "content": "timeout fallback body",
            "tool_calls": None,
            "finish_reason": "stop",
        }
    )

    provider = OpenAIProvider(client)

    chunks = []
    async for chunk in provider.generate_stream(
        prompt="hello",
        model="gpt-test",
        temperature=0.2,
        max_tokens=64,
    ):
        chunks.append(chunk)

    assert chunks == ["timeout fallback body"]
    client.chat_completion.assert_awaited_once()

async def _empty_done_stream(*args, **kwargs):
    yield {"done": True}


async def test_should_fallback_to_non_stream_completion_when_stream_finishes_without_content():
    client = type("ClientStub", (), {})()
    client.chat_completion_stream = _empty_done_stream
    client.chat_completion = AsyncMock(
        return_value={
            "content": "done fallback body",
            "tool_calls": None,
            "finish_reason": "stop",
        }
    )

    provider = OpenAIProvider(client)

    chunks = []
    async for chunk in provider.generate_stream(
        prompt="hello",
        model="gpt-test",
        temperature=0.2,
        max_tokens=64,
    ):
        chunks.append(chunk)

    assert chunks == ["done fallback body"]
    client.chat_completion.assert_awaited_once()


async def test_should_raise_original_error_when_non_stream_fallback_disabled():
    client = type("ClientStub", (), {})()
    client.chat_completion_stream = _failing_stream
    client.chat_completion = AsyncMock()

    provider = OpenAIProvider(client)

    with pytest.raises(httpx.HTTPStatusError):
        async for _chunk in provider.generate_stream(
            prompt="hello",
            model="gpt-test",
            temperature=0.2,
            max_tokens=64,
            request_options={"allow_non_stream_fallback": False},
        ):
            pass

    client.chat_completion.assert_not_awaited()


async def test_should_raise_runtime_error_when_empty_stream_and_non_stream_fallback_disabled():
    client = type("ClientStub", (), {})()
    client.chat_completion_stream = _empty_done_stream
    client.chat_completion = AsyncMock()

    provider = OpenAIProvider(client)

    with pytest.raises(RuntimeError, match="non-stream fallback is disabled"):
        async for _chunk in provider.generate_stream(
            prompt="hello",
            model="gpt-test",
            temperature=0.2,
            max_tokens=64,
            request_options={"allow_non_stream_fallback": False},
        ):
            pass

    client.chat_completion.assert_not_awaited()

