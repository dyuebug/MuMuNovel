from types import SimpleNamespace

import pytest

from tests.test_support import plot_expansion_test_support as plot_expansion


pytestmark = pytest.mark.asyncio


async def test_should_resolve_plot_expansion_template_from_module_level_owner_surface(
    monkeypatch,
):
    captured = {}

    async def fake_get_template(template_key, user_id, db):
        captured["args"] = (template_key, user_id, db)
        return "FAKE_TEMPLATE"

    monkeypatch.setattr(plot_expansion, "get_template", fake_get_template)

    result = await plot_expansion._get_plot_expansion_template(
        "OUTLINE_EXPAND_SINGLE",
        "user-1",
        object(),
    )

    assert result == "FAKE_TEMPLATE"
    assert captured["args"][0] == "OUTLINE_EXPAND_SINGLE"
    assert captured["args"][1] == "user-1"


async def test_should_consume_module_level_prompt_owner_surface_in_single_batch_generation(
    monkeypatch,
):
    captured = {}

    class _FakeExecuteResult:
        class _FakeScalars:
            @staticmethod
            def all():
                return []

        @staticmethod
        def scalars():
            return _FakeExecuteResult._FakeScalars()

    class _FakeDb:
        async def execute(self, *_args, **_kwargs):
            return _FakeExecuteResult()

    class _FakeAIService:
        async def generate_text_stream(self, **kwargs):
            captured["ai_prompt"] = kwargs["prompt"]
            yield '{"chapters":[]}'

    async def fake_get_template(template_key, user_id, db):
        captured["template_args"] = (template_key, user_id, db)
        return "RAW_TEMPLATE"

    def fake_format_prompt(template, **kwargs):
        captured["format_template"] = template
        captured["format_kwargs"] = kwargs
        return "FORMATTED_PROMPT"

    monkeypatch.setattr(plot_expansion, "get_template", fake_get_template)
    monkeypatch.setattr(plot_expansion, "format_prompt", fake_format_prompt)

    service = plot_expansion.PlotExpansionService(_FakeAIService())
    async def fake_get_outline_context(outline, project_id, db):
        return "CONTEXT_INFO"

    monkeypatch.setattr(
        service,
        "_get_outline_context",
        fake_get_outline_context,
    )
    monkeypatch.setattr(
        service,
        "_parse_expansion_response",
        lambda ai_content, outline_id: [{"outline_id": outline_id, "raw": ai_content}],
    )

    result = await service._generate_chapters_single_batch(
        outline=SimpleNamespace(id="outline-1", order_index=1, title="Outline", content="Outline content"),
        project=SimpleNamespace(
            id="project-1",
            user_id="user-1",
            title="Project",
            genre="玄幻",
            theme="热血",
            narrative_perspective="第三人称",
            world_time_period="古代",
            world_location="山城",
            world_atmosphere="压抑",
        ),
        db=_FakeDb(),
        target_chapter_count=3,
        expansion_strategy="balanced",
        enable_scene_analysis=True,
        provider=None,
        model=None,
    )

    assert result[0]["outline_id"] == "outline-1"
    assert captured["template_args"][0] == "OUTLINE_EXPAND_SINGLE"
    assert captured["format_template"] == "RAW_TEMPLATE"
    assert captured["format_kwargs"]["outline_title"] == "Outline"
    assert captured["ai_prompt"] == "FORMATTED_PROMPT"
