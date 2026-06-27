import pytest

from tests.test_support.chapter_web_research_test_support import (
    GrokSearchAdapter,
    GrokSearchAdapterError,
)


def test_should_normalize_grok_api_base_url_with_default_v1_path():
    assert (
        GrokSearchAdapter.normalize_api_base_url("https://relay.example.com")
        == "https://relay.example.com/v1"
    )
    assert (
        GrokSearchAdapter.normalize_api_base_url(" https://relay.example.com/custom/ ")
        == "https://relay.example.com/custom"
    )


def test_should_build_chat_completions_url_from_normalized_base_url():
    assert (
        GrokSearchAdapter.build_chat_completions_url("https://relay.example.com")
        == "https://relay.example.com/v1/chat/completions"
    )

    with pytest.raises(GrokSearchAdapterError, match="Base URL"):
        GrokSearchAdapter.build_chat_completions_url("")


@pytest.mark.asyncio
async def test_should_parse_streaming_response_from_sse_chunks():
    class FakeResponse:
        async def aiter_lines(self):
            yield 'data: {"choices":[{"delta":{"content":"hello "}}]}'
            yield 'data: {"choices":[{"delta":{"content":"world"}}]}'
            yield "data: [DONE]"

    parsed = await GrokSearchAdapter._parse_streaming_response(FakeResponse())

    assert parsed == "hello world"


@pytest.mark.asyncio
async def test_should_parse_streaming_response_from_merged_json_body():
    class FakeResponse:
        async def aiter_lines(self):
            yield '{"choices":[{"message":{"content":"merged answer"}}]}'

    parsed = await GrokSearchAdapter._parse_streaming_response(FakeResponse())

    assert parsed == "merged answer"


def test_should_normalize_sources_from_mixed_items():
    normalized = GrokSearchAdapter._normalize_sources(
        [
            {
                "title": "Source A",
                "url": "https://example.com/a",
                "summary": "summary text",
            },
            {
                "url": "https://example.com/b",
                "description": "description text",
            },
            "ignored",
            {},
        ]
    )

    assert normalized == [
        {
            "title": "Source A",
            "url": "https://example.com/a",
            "snippet": "summary text",
        },
        {
            "title": "https://example.com/b",
            "url": "https://example.com/b",
            "snippet": "description text",
        },
    ]
