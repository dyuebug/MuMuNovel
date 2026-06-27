from types import SimpleNamespace
from unittest.mock import AsyncMock

import pytest

import tests.test_support.database_test_support as app_database
from migrator_app.models.chapter import Chapter
from tests.test_support import (
    chapter_regeneration_route_test_adapter as regeneration_context_service,
)


class _ScalarResult:
    def __init__(self, value):
        self._value = value

    def scalar_one_or_none(self):
        return self._value


class _ScalarsResult:
    def __init__(self, items):
        self._items = items

    def all(self):
        return list(self._items)


class _ExecuteResult:
    def __init__(self, *, scalar=None, scalars=None):
        self._scalar = scalar
        self._scalars = scalars or []

    def scalar_one_or_none(self):
        return self._scalar

    def scalars(self):
        return _ScalarsResult(self._scalars)


@pytest.mark.asyncio
async def test_should_raise_when_regeneration_source_chapter_content_is_empty():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="empty-chapter",
        content="",
    )

    with pytest.raises(ValueError) as exc_info:
        await regeneration_context_service.prepare_chapter_regeneration_stream_context(
            AsyncMock(),
            chapter=chapter,
            regenerate_request=SimpleNamespace(modification_source="custom"),
            user_id="user-1",
        )
    assert "当前章节缺少可重写的原始内容" in str(exc_info.value)


@pytest.mark.asyncio
async def test_should_raise_when_analysis_required_but_missing_for_regeneration():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="need-analysis",
        content="legacy content",
    )
    db_session = AsyncMock()
    db_session.execute = AsyncMock(return_value=_ScalarResult(None))

    with pytest.raises(LookupError) as exc_info:
        await regeneration_context_service.prepare_chapter_regeneration_stream_context(
            db_session,
            chapter=chapter,
            regenerate_request=SimpleNamespace(modification_source="analysis_suggestions"),
            user_id="user-1",
        )
    assert "未找到对应的章节分析" in str(exc_info.value)


@pytest.mark.asyncio
async def test_should_prepare_regeneration_stream_context_with_analysis_and_preparation(monkeypatch):
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="ready",
        content="legacy content",
    )
    analysis = SimpleNamespace(id="analysis-1")
    db_session = AsyncMock()
    db_session.execute = AsyncMock(return_value=_ScalarResult(analysis))
    preparation = regeneration_context_service.ChapterRegenerationPreparation(
        effective_regenerate_request=SimpleNamespace(target_word_count=500),
        style_content="style",
        style_id=7,
        project_context={"project_title": "Project"},
        story_runtime_contract={"contract": True},
    )

    async def fake_prepare_context(*args, **kwargs):
        return preparation

    monkeypatch.setattr(
        regeneration_context_service,
        "prepare_chapter_regeneration_context",
        fake_prepare_context,
    )

    stream_context = await regeneration_context_service.prepare_chapter_regeneration_stream_context(
        db_session,
        chapter=chapter,
        regenerate_request=SimpleNamespace(modification_source="mixed"),
        user_id="user-1",
    )

    assert stream_context.chapter is chapter
    assert stream_context.analysis is analysis
    assert stream_context.user_id == "user-1"
    assert stream_context.effective_regenerate_request is preparation.effective_regenerate_request
    assert stream_context.project_context == {"project_title": "Project"}
    assert stream_context.style_content == "style"
    assert stream_context.style_id == 7
    assert stream_context.story_runtime_contract == {"contract": True}

@pytest.mark.asyncio
async def test_should_forward_web_research_fields_into_regeneration_context(monkeypatch):
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="ready",
        content="legacy content",
        summary="outline summary",
    )
    project = SimpleNamespace(
        id="project-1",
        outline_mode="one-to-many",
        title="Project",
        genre="suspense",
        theme="truth and cost",
        narrative_perspective="third_person",
        world_time_period="modern",
        world_location="harbor city",
        world_atmosphere="tense",
    )
    outline = SimpleNamespace(content="outline content")
    db_session = AsyncMock()
    db_session.execute = AsyncMock(side_effect=[
        _ExecuteResult(scalar=project),
        _ExecuteResult(scalars=[SimpleNamespace(name="Lin")]),
    ])

    collect_assets_mock = AsyncMock(return_value={
        "assets": [
            {
                "title": "harbor rules",
                "source": "https://example.com/harbor",
                "summary": "Guild rules shape who can speak and trade.",
            }
        ]
    })
    build_story_packet_mock = AsyncMock(return_value=SimpleNamespace(source="packet"))

    monkeypatch.setattr(regeneration_context_service, "_load_regeneration_outline", AsyncMock(return_value=outline))
    monkeypatch.setattr(regeneration_context_service, "build_characters_info_with_careers", AsyncMock(return_value="Lin / investigator"))
    monkeypatch.setattr(
        regeneration_context_service,
        "resolve_chapter_quality_profile",
        AsyncMock(return_value={"style_content": "style", "resolved_style_id": 7}),
    )
    monkeypatch.setattr(
        regeneration_context_service,
        "resolve_generation_story_repair_state_for_chapter",
        AsyncMock(return_value={"payload": None, "active_story_repair_payload": None}),
    )
    monkeypatch.setattr(
        regeneration_context_service,
        "story_repair_payload_to_prompt_kwargs",
        lambda payload: {},
    )
    monkeypatch.setattr(
        regeneration_context_service,
        "build_story_generation_packet_with_project_continuity",
        build_story_packet_mock,
    )
    monkeypatch.setattr(
        regeneration_context_service.chapter_web_research_service,
        "collect_for_chapter",
        collect_assets_mock,
    )
    monkeypatch.setattr(
        regeneration_context_service,
        "_build_outline_structure_runtime_sources",
        lambda outline_obj: {"anchor": outline_obj.content},
    )
    monkeypatch.setattr(
        regeneration_context_service,
        "build_chapter_generation_runtime_bundle",
        lambda **kwargs: SimpleNamespace(
            prompt_quality_kwargs={"quality_preset": "plot_drive"},
            story_runtime_contract={"contract": True},
        ),
    )

    request = regeneration_context_service.ChapterRegenerateRequest(
        enable_web_research=True,
        web_research_query="harbor guild rules",
        story_creation_brief="Keep pressure visible in every turn.",
        target_word_count=3200,
    )

    preparation = await regeneration_context_service.prepare_chapter_regeneration_context(
        db_session,
        chapter=chapter,
        regenerate_request=request,
        user_id="user-1",
    )

    collect_kwargs = collect_assets_mock.await_args.kwargs
    assert collect_kwargs["enable_web_research"] is True
    assert collect_kwargs["web_research_query"] == "harbor guild rules"
    assert collect_kwargs["story_creation_brief"] == "Keep pressure visible in every turn."

    source_request = build_story_packet_mock.await_args.kwargs["source"]
    assert source_request.enable_web_research is True
    assert source_request.web_research_query == "harbor guild rules"
    assert preparation.effective_regenerate_request.enable_web_research is True
    assert preparation.project_context["external_assets"][0]["title"] == "harbor rules"
    assert preparation.project_context["reference_assets"] == preparation.project_context["external_assets"]

