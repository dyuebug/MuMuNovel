import asyncio

import httpx
import json

import pytest

from tests.test_support.ai_gateway.ai_service import clean_json_response
from tests.test_support import plot_analyzer_test_support
from tests.test_support.plot_analyzer_test_support import PlotAnalyzer

pytestmark = pytest.mark.asyncio


class FakeAIService:
    def __init__(self, responses):
        self._responses = list(responses)
        self.calls = []

    async def generate_text(self, **kwargs):
        self.calls.append(kwargs)
        index = min(len(self.calls) - 1, len(self._responses) - 1)
        response = self._responses[index]
        if isinstance(response, Exception):
            raise response
        return {"content": response}

    def _clean_json_response(self, text: str) -> str:
        return clean_json_response(text)


async def test_should_retry_plot_analysis_with_stricter_json_mode(monkeypatch):
    invalid_json = '{"hooks": [{"type": "冲突", "content": "门外敲门", "strength"'
    valid_json = json.dumps(
        {
            "hooks": [
                {
                    "type": "冲突",
                    "content": "门外敲门",
                    "strength": 8,
                    "position": "开篇",
                    "keyword": "门外敲了两下玻璃",
                }
            ],
            "plot_points": [],
            "foreshadows": [],
            "scores": {"overall": 8.6},
        },
        ensure_ascii=False,
    )
    ai_service = FakeAIService([invalid_json, valid_json])
    analyzer = PlotAnalyzer(ai_service)

    retry_events = []

    async def on_retry(attempt, max_retries, wait_time, error_reason):
        retry_events.append((attempt, max_retries, wait_time, error_reason))

    monkeypatch.setattr(plot_analyzer_test_support, "format_prompt", lambda template, **kwargs: "ANALYZE_PROMPT")

    result = await analyzer.analyze_chapter(
        chapter_number=1,
        title="第一章",
        content="正文内容" * 400,
        word_count=3200,
        max_retries=2,
        on_retry=on_retry,
    )

    assert result is not None
    assert result["scores"]["overall"] == 8.6
    assert len(ai_service.calls) == 2
    assert ai_service.calls[0]["auto_mcp"] is False
    assert ai_service.calls[0]["handle_tool_calls"] is False
    assert ai_service.calls[0]["max_tokens"] == 3200
    assert ai_service.calls[0]["request_options"] == {"read_timeout": 45, "transport_max_retries": 1, "prefer_chat_completions": True}
    assert retry_events and retry_events[0][0] == 1


async def test_should_set_clear_timeout_message_when_plot_analysis_times_out(monkeypatch):
    ai_service = FakeAIService([asyncio.TimeoutError()])
    analyzer = PlotAnalyzer(ai_service)

    monkeypatch.setattr(plot_analyzer_test_support, "format_prompt", lambda template, **kwargs: "ANALYZE_PROMPT")

    result = await analyzer.analyze_chapter(
        chapter_number=1,
        title="第一章",
        content="测试正文" * 100,
        word_count=800,
        max_retries=1,
    )

    assert result is not None
    assert result["analysis_mode"] == "heuristic_fallback"
    assert result["hooks"]
    assert analyzer.last_error_message is not None
    assert "快速规则分析" in analyzer.last_error_message


async def test_should_set_clear_timeout_message_when_plot_analysis_hits_httpx_timeout(monkeypatch):
    ai_service = FakeAIService([httpx.ReadTimeout("upstream timeout")])
    analyzer = PlotAnalyzer(ai_service)

    monkeypatch.setattr(plot_analyzer_test_support, "format_prompt", lambda template, **kwargs: "ANALYZE_PROMPT")

    result = await analyzer.analyze_chapter(
        chapter_number=1,
        title="第一章",
        content="测试正文" * 100,
        word_count=800,
        max_retries=1,
    )

    assert result is not None
    assert result["analysis_mode"] == "heuristic_fallback"
    assert result["plot_points"]
    assert analyzer.last_error_message is not None
    assert "快速规则分析" in analyzer.last_error_message



async def test_should_build_heuristic_fallback_analysis_with_readable_summary(monkeypatch):
    ai_service = FakeAIService([asyncio.TimeoutError()])
    analyzer = PlotAnalyzer(ai_service)

    monkeypatch.setattr(plot_analyzer_test_support, "format_prompt", lambda template, **kwargs: "ANALYZE_PROMPT")

    result = await analyzer.analyze_chapter(
        chapter_number=3,
        title="钟楼下的灰潮",
        content="钟声骤响，城门封闭。林砚抱着账册冲向桥头，却被巡丁拦下。沈霁赶来布设界钉，灰潮沿着石桥蔓延。",
        word_count=1200,
        max_retries=1,
    )

    assert result is not None
    assert result["analysis_mode"] == "heuristic_fallback"
    assert result["summary"]
    assert result["scores"]["overall"] >= 4
    assert result["suggestions"]
