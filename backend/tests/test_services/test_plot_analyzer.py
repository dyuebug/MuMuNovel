import json

import pytest

from app.services.json_helper import clean_json_response
from app.services.plot_analyzer import PlotAnalyzer
from app.services.prompt_service import PromptService

pytestmark = pytest.mark.asyncio


class FakeAIService:
    def __init__(self, responses):
        self._responses = list(responses)
        self.calls = []

    async def generate_text(self, **kwargs):
        self.calls.append(kwargs)
        index = min(len(self.calls) - 1, len(self._responses) - 1)
        return {"content": self._responses[index]}

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

    monkeypatch.setattr(PromptService, "format_prompt", staticmethod(lambda template, **kwargs: "ANALYZE_PROMPT"))

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
    assert ai_service.calls[0]["max_tokens"] == 4200
    assert retry_events and retry_events[0][0] == 1
