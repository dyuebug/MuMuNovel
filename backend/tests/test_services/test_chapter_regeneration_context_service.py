from types import SimpleNamespace
from unittest.mock import AsyncMock

import pytest

from app.models.chapter import Chapter
from app.services import chapter_regeneration_context_service as regeneration_context_service


class _ScalarResult:
    def __init__(self, value):
        self._value = value

    def scalar_one_or_none(self):
        return self._value


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
