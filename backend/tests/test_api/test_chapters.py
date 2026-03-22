import json
from types import SimpleNamespace
from typing import Any
from datetime import datetime, timedelta

import pytest
import pytest_asyncio
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from app.api import chapters as chapters_api
from app.database import Base, get_db as app_get_db
from app.models.analysis_task import AnalysisTask
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.outline import Outline
from app.models.project import Project
from app.models.regeneration_task import RegenerationTask

pytestmark = pytest.mark.asyncio


class FakeAIService:
    def __init__(self):
        self.chunks = ["流式片段A", "流式片段B"]
        self.calls: list[dict[str, Any]] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(kwargs)
        for chunk in self.chunks:
            yield chunk


@pytest.fixture
def fake_ai_service():
    return FakeAIService()


@pytest.fixture(autouse=True)
def mock_side_effect_services(monkeypatch):
    async def fake_delete_chapter_memories(*args, **kwargs):
        return None

    async def fake_delete_chapter_foreshadows(*args, **kwargs):
        return {"deleted_count": 0}

    async def fake_auto_plant_pending_foreshadows(*args, **kwargs):
        return {"planted_count": 0}

    async def fake_analyze_chapter_background(*args, **kwargs):
        return True

    async def fake_execute_batch_generation(*args, **kwargs):
        return None

    monkeypatch.setattr(
        chapters_api.memory_service,
        "delete_chapter_memories",
        fake_delete_chapter_memories,
    )
    monkeypatch.setattr(
        chapters_api.foreshadow_service,
        "delete_chapter_foreshadows",
        fake_delete_chapter_foreshadows,
    )
    monkeypatch.setattr(
        chapters_api.foreshadow_service,
        "auto_plant_pending_foreshadows",
        fake_auto_plant_pending_foreshadows,
    )
    monkeypatch.setattr(
        chapters_api,
        "analyze_chapter_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )


@pytest_asyncio.fixture
async def chapters_session_factory():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    try:
        yield async_sessionmaker(engine, expire_on_commit=False)
    finally:
        await engine.dispose()


@pytest_asyncio.fixture
async def chapters_client(chapters_session_factory, fake_ai_service, mock_user, monkeypatch):
    app = FastAPI()
    app.include_router(chapters_api.router, prefix="/api")

    async def override_get_db(_request=None):
        async with chapters_session_factory() as session:
            yield session

    async def override_get_user_ai_service():
        return fake_ai_service

    @app.middleware("http")
    async def inject_user_state(request, call_next):
        header_user_id = request.headers.get("x-test-user-id", mock_user.user_id)
        if header_user_id == "__none__":
            request.state.user_id = None
            request.state.user = None
        else:
            request.state.user_id = header_user_id
            request.state.user = (
                mock_user
                if header_user_id == mock_user.user_id
                else SimpleNamespace(user_id=header_user_id)
            )
        return await call_next(request)

    app.dependency_overrides[app_get_db] = override_get_db
    app.dependency_overrides[chapters_api.get_user_ai_service] = override_get_user_ai_service

    monkeypatch.setattr(chapters_api, "get_db", override_get_db)

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


def parse_sse_data(stream_text: str) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line in stream_text.splitlines():
        if line.startswith("data: "):
            events.append(json.loads(line.removeprefix("data: ")))
    return events


async def create_project(chapters_session_factory, user_id: str, **overrides) -> Project:
    async with chapters_session_factory() as session:
        project = Project(
            user_id=user_id,
            title=overrides.get("title", "测试项目"),
            genre=overrides.get("genre", "奇幻"),
            theme=overrides.get("theme", "成长"),
            outline_mode=overrides.get("outline_mode", "one-to-many"),
            current_words=overrides.get("current_words", 0),
            narrative_perspective=overrides.get("narrative_perspective", "third_person"),
            default_creative_mode=overrides.get("default_creative_mode"),
            default_story_focus=overrides.get("default_story_focus"),
            default_plot_stage=overrides.get("default_plot_stage"),
            default_story_creation_brief=overrides.get("default_story_creation_brief"),
        )
        session.add(project)
        await session.commit()
        await session.refresh(project)
        return project


async def create_outline(
    chapters_session_factory,
    project_id: str,
    order_index: int = 1,
    title: str = "大纲",
    content: str = "章节大纲",
) -> Outline:
    async with chapters_session_factory() as session:
        outline = Outline(
            project_id=project_id,
            title=title,
            content=content,
            order_index=order_index,
        )
        session.add(outline)
        await session.commit()
        await session.refresh(outline)
        return outline


async def create_chapter(
    chapters_session_factory,
    project_id: str,
    chapter_number: int,
    title: str,
    content: str | None = None,
    outline_id: str | None = None,
    status: str = "draft",
    expansion_plan: str | None = None,
) -> Chapter:
    async with chapters_session_factory() as session:
        chapter = Chapter(
            project_id=project_id,
            chapter_number=chapter_number,
            title=title,
            content=content,
            word_count=len(content) if content else 0,
            status=status,
            outline_id=outline_id,
            expansion_plan=expansion_plan,
        )
        session.add(chapter)
        await session.commit()
        await session.refresh(chapter)
        return chapter


async def test_should_handle_chapter_crud_and_project_word_count(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        title="第一章总纲",
    )

    create_response = await chapters_client.post(
        "/api/chapters",
        json={
            "project_id": project.id,
            "chapter_number": 1,
            "title": "第一章",
            "content": "abc",
            "outline_id": outline.id,
        },
    )
    assert create_response.status_code == 200
    created = create_response.json()
    chapter_id = created["id"]
    assert created["word_count"] == 3
    assert created["status"] == "draft"

    list_response = await chapters_client.get(f"/api/chapters/project/{project.id}")
    assert list_response.status_code == 200
    list_body = list_response.json()
    assert list_body["total"] == 1
    assert list_body["items"][0]["outline_title"] == "第一章总纲"

    detail_response = await chapters_client.get(f"/api/chapters/{chapter_id}")
    assert detail_response.status_code == 200
    assert detail_response.json()["title"] == "第一章"

    update_response = await chapters_client.put(
        f"/api/chapters/{chapter_id}",
        json={"title": "第一章（修订）", "content": "abcdef"},
    )
    assert update_response.status_code == 200
    updated = update_response.json()
    assert updated["title"] == "第一章（修订）"
    assert updated["word_count"] == 6

    async with chapters_session_factory() as session:
        words_result = await session.execute(
            select(Project.current_words).where(Project.id == project.id)
        )
        assert words_result.scalar_one() == 6

    delete_response = await chapters_client.delete(f"/api/chapters/{chapter_id}")
    assert delete_response.status_code == 200

    async with chapters_session_factory() as session:
        deleted_chapter = await session.get(Chapter, chapter_id)
        assert deleted_chapter is None

        words_result = await session.execute(
            select(Project.current_words).where(Project.id == project.id)
        )
        assert words_result.scalar_one() == 0


async def test_should_return_401_when_create_chapter_without_user_id(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    response = await chapters_client.post(
        "/api/chapters",
        headers={"x-test-user-id": "__none__"},
        json={
            "project_id": project.id,
            "chapter_number": 1,
            "title": "未登录创建",
            "content": "abc",
        },
    )
    assert response.status_code == 401


async def test_should_return_404_when_accessing_project_owned_by_other_user(
    chapters_client,
    chapters_session_factory,
):
    foreign_project = await create_project(
        chapters_session_factory,
        user_id="another-user",
        title="他人项目",
    )

    response = await chapters_client.get(f"/api/chapters/project/{foreign_project.id}")
    assert response.status_code == 404


async def test_should_return_404_when_chapter_not_found(chapters_client):
    response = await chapters_client.get("/api/chapters/not-exists")
    assert response.status_code == 404


@pytest.mark.parametrize(
    ("outline_mode", "expected_builder"),
    [("one-to-many", "many"), ("one-to-one", "one")],
)
async def test_should_build_context_with_expected_builder_during_generate_stream(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
    monkeypatch,
    outline_mode,
    expected_builder,
):
    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        outline_mode=outline_mode,
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待生成章节",
    )

    calls = {"many": 0, "one": 0}

    class FakeContext:
        chapter_outline = "模拟大纲"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = "角色A"
        chapter_careers = "职业A"
        foreshadow_reminders = ""
        relevant_memories = ""
        recent_chapters_context = ""
        context_stats = {}

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            calls["many"] += 1
            return FakeContext()

    class FakeOneToOneBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            calls["one"] += 1
            return FakeContext()

    async def fake_get_template(*args, **kwargs):
        return "模板"

    def fake_format_prompt(template, **kwargs):
        return "mock-generate-prompt"

    monkeypatch.setattr(chapters_api, "OneToManyContextBuilder", FakeOneToManyBuilder)
    monkeypatch.setattr(chapters_api, "OneToOneContextBuilder", FakeOneToOneBuilder)
    monkeypatch.setattr(chapters_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(chapters_api.PromptService, "format_prompt", fake_format_prompt)

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["段落甲", "段落乙"]

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-stream",
        json={"target_word_count": 500},
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "chunk" for event in events)
    assert any(event.get("type") == "result" for event in events)

    assert calls[expected_builder] == 1
    unexpected_builder = "one" if expected_builder == "many" else "many"
    assert calls[unexpected_builder] == 0

    assert fake_ai_service.calls
    last_call = fake_ai_service.calls[-1]
    assert last_call["prompt"] == "mock-generate-prompt"
    assert last_call["max_tokens"] == 2000

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.status == "completed"
        assert saved_chapter.content == "段落甲段落乙"


async def test_should_stream_partial_regenerate_with_mock_ai_response(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待局部重写章节",
        content="ABCDEFG",
        status="completed",
    )

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["重", "写"]

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/partial-regenerate-stream",
        json={
            "selected_text": "BCD",
            "start_position": 1,
            "end_position": 4,
            "user_instructions": "增强表现力",
            "context_chars": 120,
            "length_mode": "similar",
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    assert result_event["data"]["new_text"] == "重写"
    assert result_event["data"]["original_word_count"] == 3

    assert fake_ai_service.calls
    assert fake_ai_service.calls[-1]["max_tokens"] == 500



async def test_should_sanitize_partial_regenerate_text_before_apply(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="\u5c40\u90e8\u6539\u5199\u6e05\u6d17\u6d4b\u8bd5",
        content="ABCDEFG",
        status="completed",
    )

    new_text = (
        "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
        "\u4e0b\u4e00\u79d2\uff0c\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002"
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/apply-partial-regenerate",
        json={
            "new_text": new_text,
            "start_position": 1,
            "end_position": 4,
        },
    )
    assert response.status_code == 200

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.content == (
            "A"
            "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
            "\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002"
            "EFG"
        )
async def test_should_return_400_when_partial_regenerate_position_invalid(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="非法位置测试章节",
        content="ABCDEFG",
        status="completed",
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/partial-regenerate-stream",
        json={
            "selected_text": "BCD",
            "start_position": 4,
            "end_position": 2,
            "user_instructions": "增强表现力",
            "context_chars": 120,
            "length_mode": "similar",
        },
    )
    assert response.status_code == 400


async def test_should_create_batch_generation_task_and_query_status(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="前置章节已完成",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content=None,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="第三章",
        content=None,
    )

    create_response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 2,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
        },
    )
    assert create_response.status_code == 200
    body = create_response.json()
    batch_id = body["batch_id"]
    assert len(body["chapters_to_generate"]) == 2

    status_response = await chapters_client.get(
        f"/api/chapters/batch-generate/{batch_id}/status"
    )
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["batch_id"] == batch_id
    assert status_body["status"] == "pending"
    assert status_body["total"] == 2
    assert status_body["completed"] == 0
    assert status_body["stage_code"] == "6.writing"
    assert status_body["execution_mode"] == "interactive"
    assert status_body["checkpoint"]["current_chapter_number"] is None
    assert status_body["latest_quality_metrics"] is None
    assert status_body["quality_metrics_summary"] is None

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, batch_id)
        assert task is not None
        assert task.chapter_count == 2


async def test_should_forward_creative_mode_to_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="前置章节已完成",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content=None,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="第三章",
        content=None,
    )

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 2,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
            "creative_mode": "payoff",
            "story_focus": "foreshadow_payoff",
            "plot_stage": "ending",
        },
    )

    assert response.status_code == 200
    assert captured["creative_mode"] == "payoff"
    assert captured["story_focus"] == "foreshadow_payoff"
    assert captured["plot_stage"] == "ending"


async def test_should_fallback_to_project_generation_defaults_for_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )

    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief="默认要求：保持连载感和推进效率。",
    )

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="前置章节已完成",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content=None,
    )

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 1,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
        },
    )

    assert response.status_code == 200
    assert captured["creative_mode"] == "hook"
    assert captured["story_focus"] == "advance_plot"
    assert captured["plot_stage"] == "development"
    assert captured["story_creation_brief"] == "默认要求：保持连载感和推进效率。"


async def test_should_expose_runtime_workflow_phase_in_batch_status(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="running",
            total_chapters=1,
            completed_chapters=0,
            current_chapter_id="chapter-1",
            current_chapter_number=1,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)

    async with chapters_api.task_workflow_lock:
        chapters_api.task_workflow_state_cache.pop(task.id, None)

    await chapters_api.publish_task_stream_event(
        task.id,
        {
            "type": "analysis_started",
            "chapter_id": "chapter-1",
            "chapter_number": 1,
            "message": "analysis started",
            "progress": 85,
            "phase": "parsing",
        },
    )

    response = await chapters_client.get(f"/api/chapters/batch-generate/{task.id}/status")
    assert response.status_code == 200
    body = response.json()
    assert body["stage_code"] == "6.writing.parsing"
    assert body["checkpoint"]["progress_phase"] == "parsing"
    assert body["checkpoint"]["last_event"] == "analysis_started"
    assert body["checkpoint"]["current_chapter_number"] == 1


async def test_should_resume_failed_batch_task_from_current_chapter(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )
    chapter_3 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=2,
            chapter_ids=[chapter_2.id, chapter_3.id],
            status="failed",
            total_chapters=2,
            completed_chapters=0,
            current_chapter_id=chapter_2.id,
            current_chapter_number=2,
            current_retry_count=1,
            max_retries=2,
            error_message="mock failed",
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 200
    body = response.json()
    assert body["resumed_from_batch_id"] == source_task_id
    assert body["status"] == "pending"
    assert body["stage_code"] == "6.writing.loading"
    resumed_task_id = body["batch_id"]
    assert resumed_task_id != source_task_id

    async with chapters_session_factory() as session:
        resumed_task = await session.get(BatchGenerationTask, resumed_task_id)
        assert resumed_task is not None
        assert resumed_task.status == "pending"
        assert resumed_task.start_chapter_number == 2
        assert resumed_task.chapter_ids == [chapter_2.id, chapter_3.id]
        assert resumed_task.total_chapters == 2
        assert resumed_task.completed_chapters == 0

    async with chapters_api.task_workflow_lock:
        runtime = dict(chapters_api.task_workflow_state_cache.get(resumed_task_id) or {})
    assert runtime.get("phase") == "loading"
    assert runtime.get("resume_from_batch_id") == source_task_id


async def test_should_resume_cancelled_task_from_completed_checkpoint_when_current_missing(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready-1",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content="ready-2",
        status="completed",
    )
    chapter_3 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="chapter-3",
        content=None,
    )
    chapter_4 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=4,
        title="chapter-4",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=3,
            chapter_ids=[chapter_2.id, chapter_3.id, chapter_4.id],
            status="cancelled",
            total_chapters=3,
            completed_chapters=1,
            current_chapter_id="missing-chapter-id",
            current_chapter_number=3,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 200
    body = response.json()
    resumed_task_id = body["batch_id"]
    assert body["resumed_from_batch_id"] == source_task_id
    assert body["task_type"] == "chapters_batch_generate"
    assert body["checkpoint"]["current_chapter_number"] == 3

    async with chapters_session_factory() as session:
        resumed_task = await session.get(BatchGenerationTask, resumed_task_id)
        assert resumed_task is not None
        assert resumed_task.chapter_ids == [chapter_3.id, chapter_4.id]
        assert resumed_task.start_chapter_number == 3
        assert resumed_task.total_chapters == 2


async def test_should_reject_resume_when_batch_task_not_terminal(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-1",
        content="ready",
        status="completed",
    )
    chapter_2 = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="chapter-2",
        content=None,
    )

    async with chapters_session_factory() as session:
        source_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=1,
            chapter_ids=[chapter_2.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(source_task)
        await session.commit()
        await session.refresh(source_task)
        source_task_id = source_task.id

    response = await chapters_client.post(f"/api/chapters/batch-generate/{source_task_id}/resume")
    assert response.status_code == 400
    assert "Only failed or cancelled tasks can be resumed" in response.json()["detail"]


async def test_should_return_latest_chapter_quality_metrics(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="评分章节",
        content="正文内容",
        status="completed",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="旧记录",
                generated_content="纯文本旧数据",
                model="default",
                created_at=now - timedelta(minutes=5),
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="新记录",
                generated_content=json.dumps(
                    {
                        "log_type": "chapter_generation_quality_v1",
                        "quality_metrics": {
                            "overall_score": 78.5,
                            "conflict_chain_hit_rate": 70.0,
                            "rule_grounding_hit_rate": 82.0,
                            "outline_alignment_rate": 75.0,
                            "dialogue_naturalness_rate": 68.0,
                        },
                    },
                    ensure_ascii=False,
                ),
                model="default",
                created_at=now - timedelta(minutes=1),
            )
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/{chapter.id}/quality-metrics")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["has_metrics"] is True
    assert body["latest_metrics"]["overall_score"] == 78.5
    assert body["latest_metrics"]["conflict_chain_hit_rate"] == 70.0
    assert body["latest_metrics"]["rule_grounding_hit_rate"] == 82.0
    assert body["generated_at"] is not None


def test_should_build_runtime_prompt_with_serial_style_guard():
    project = SimpleNamespace(
        world_time_period="近未来",
        world_location="临海三环",
        world_atmosphere="潮湿压迫",
        world_rules="门影会在镜面附近折返",
    )

    runtime_prompt = chapters_api._build_chapter_runtime_system_prompt(
        project=project,
        style_content="写作风格建议：低AI连载感",
        chapter_outline="【关键事件】\n- 主角带队撤离\n- 出现镜面门",
        previous_summary="上一章队伍已进入高风险走廊",
        style_name="低AI连载感",
        style_preset_id="low_ai_serial",
    )

    assert "连载感优先" in runtime_prompt
    assert "情绪要有层次" in runtime_prompt
    assert "慎用“像……/仿佛/像……一样”" in runtime_prompt
    assert "少用“下一秒/那一瞬/忽然/不是……而是……”" in runtime_prompt
    assert "台词长度控制：单句以6-18字为主" not in runtime_prompt


def test_should_resolve_generation_temperature_by_style_profile():
    serial_profile = chapters_api._detect_style_profile(
        style_name="低AI连载感",
        style_preset_id="low_ai_serial",
        style_content="",
    )
    life_profile = chapters_api._detect_style_profile(
        style_name="低AI生活化",
        style_preset_id="low_ai_life",
        style_content="",
    )
    default_profile = chapters_api._detect_style_profile(
        style_name="默认风格",
        style_preset_id="",
        style_content="",
    )

    assert chapters_api._resolve_generation_temperature(serial_profile) == pytest.approx(0.82)
    assert chapters_api._resolve_generation_temperature(life_profile) == pytest.approx(0.78)
    assert chapters_api._resolve_generation_temperature(default_profile) == pytest.approx(0.72)


def test_should_append_serial_guard_when_apply_style_to_prompt():
    merged_prompt = chapters_api.WritingStyleManager.apply_style_to_prompt(
        base_prompt="基础提示词",
        style_content="写作风格建议：低AI连载感，强调现场感",
    )

    assert "连载强化要点" in merged_prompt
    assert "人物情绪要有层次" in merged_prompt
    assert "比喻要克制" in merged_prompt
    assert "慎用高频定式句法" in merged_prompt


def test_should_detect_workflow_meta_line_in_generated_content():
    text = "\n".join([
        "以下是章节正文：",
        "步骤1：先输出冲突",
        "他推门而入，雨水沿着衣角滴到地砖上。",
    ])

    assert chapters_api._contains_chapter_workflow_meta_text(text) is True


def test_should_not_misclassify_story_text_with_plan_ab():
    text = "他把方案A塞进口袋，转头对同伴说先按旧路撤。"

    assert chapters_api._contains_chapter_workflow_meta_text(text) is False


def test_should_sanitize_generated_narrative_text_keep_story_lines():
    raw_text = "\n".join(
        [
            "以下是章节正文：",
            "执行1.1：先描述冲突",
            "门外的风越刮越急，他还是把灯点亮了。",
            "调用Agent补全设定",
            "她没有回答，只把手里的信纸折成更小的一块。",
        ]
    )

    cleaned, removed_count = chapters_api._sanitize_generated_narrative_text(raw_text)

    assert removed_count == 3
    assert "执行1.1" not in cleaned
    assert "调用Agent" not in cleaned
    assert "门外的风越刮越急" in cleaned
    assert "她没有回答" in cleaned


def test_should_lightly_polish_high_frequency_template_phrases():
    first_next = "下一秒，门外有人敲了两下玻璃。"
    second_next = "下一秒，收银台下的灯灭了。"
    first_moment = "那一瞬，他听见冰柜里咯地一声。"
    second_moment = "那一瞬，她已经把刀收回袖口。"
    first_simile = "雨丝像细针一样扎在玻璃上。"
    second_simile = "风声像砂纸一样刮过卷帘门。"
    third_simile = "裂纹像旧瓷一样往手背上爬。"
    vague_simile = "地上的水痕像有什么东西拖过去。"

    raw_text = "\n".join(
        [
            first_next,
            second_next,
            first_moment,
            second_moment,
            first_simile,
            second_simile,
            third_simile,
            vague_simile,
        ]
    )

    cleaned, removed_count = chapters_api._sanitize_generated_narrative_text(raw_text)

    assert removed_count == 0
    assert first_next in cleaned
    assert second_next not in cleaned
    assert "收银台下的灯灭了。" in cleaned
    assert first_moment in cleaned
    assert second_moment not in cleaned
    assert "她已经把刀收回袖口。" in cleaned
    assert first_simile in cleaned
    assert second_simile in cleaned
    assert "裂纹像旧瓷那样往手背上爬。" in cleaned
    assert "像有什么东西" not in cleaned
    assert "像有东西拖过去。" in cleaned


def test_should_mark_ai_identity_disclaimer_as_meta_text():
    text = "作为AI助手，我将先给出执行计划再输出正文。"

    assert chapters_api._contains_chapter_workflow_meta_text(text) is True


async def test_should_create_single_chapter_background_generation_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台生成大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待后台生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200},
    )
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["status"] == "pending"
    assert body["task_id"]

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, body["task_id"])
        assert task is not None
        assert task.chapter_count == 1
        assert task.chapter_ids == [chapter.id]
        assert task.target_word_count == 1200
        assert task.enable_analysis is True


async def test_should_allow_disabling_analysis_for_single_chapter_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章关闭分析大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待关闭分析章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200, "enable_analysis": False},
    )
    assert response.status_code == 200
    body = response.json()

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, body["task_id"])
        assert task is not None
        assert task.enable_analysis is False


async def test_should_forward_creative_mode_to_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台创作模式大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待创作模式后台生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
            "creative_mode": "suspense",
            "story_focus": "reveal_mystery",
            "plot_stage": "climax",
            "story_creation_brief": "本轮先把正面对撞和章尾牵引写实",
            "story_repair_summary": "优先补强冲突抬压",
            "story_repair_targets": ["写实受阻", "升级代价"],
            "story_preserve_strengths": ["保留对白辨识度"],
        },
    )

    assert response.status_code == 200
    assert captured["creative_mode"] == "suspense"
    assert captured["story_focus"] == "reveal_mystery"
    assert captured["plot_stage"] == "climax"
    assert captured["story_creation_brief"] == "本轮先把正面对撞和章尾牵引写实"
    assert captured["story_repair_summary"] == "优先补强冲突抬压"
    assert captured["story_repair_targets"] == ["写实受阻", "升级代价"]
    assert captured["story_preserve_strengths"] == ["保留对白辨识度"]


async def test_should_fallback_to_project_generation_defaults_for_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )

    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="suspense",
        default_story_focus="reveal_mystery",
        default_plot_stage="climax",
        default_story_creation_brief="默认要求：重点写实对撞与悬念收束。",
    )
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台默认值大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
        },
    )

    assert response.status_code == 200
    assert captured["creative_mode"] == "suspense"
    assert captured["story_focus"] == "reveal_mystery"
    assert captured["plot_stage"] == "climax"
    assert captured["story_creation_brief"] == "默认要求：重点写实对撞与悬念收束。"


async def test_should_reuse_active_background_task_for_same_chapter(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="复用任务大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待复用任务章节",
        content=None,
        outline_id=outline.id,
    )

    first = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 900},
    )
    assert first.status_code == 200
    first_task_id = first.json()["task_id"]

    second = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 900},
    )
    assert second.status_code == 200
    second_body = second.json()
    assert second_body["task_id"] == first_task_id
    assert "已有后台生成任务" in second_body["message"]


async def test_should_list_active_batch_generation_tasks_for_current_user(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    other_project = await create_project(chapters_session_factory, user_id="other-user", title="其他项目")

    async with chapters_session_factory() as session:
        user_single_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="pending",
            total_chapters=1,
            completed_chapters=0,
        )
        user_batch_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=2,
            chapter_count=3,
            chapter_ids=["chapter-2", "chapter-3", "chapter-4"],
            status="running",
            total_chapters=3,
            completed_chapters=1,
        )
        user_completed_task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=5,
            chapter_count=2,
            chapter_ids=["chapter-5", "chapter-6"],
            status="completed",
            total_chapters=2,
            completed_chapters=2,
        )
        other_user_task = BatchGenerationTask(
            project_id=other_project.id,
            user_id="other-user",
            start_chapter_number=1,
            chapter_count=2,
            chapter_ids=["x-1", "x-2"],
            status="running",
            total_chapters=2,
            completed_chapters=0,
        )
        session.add_all([user_single_task, user_batch_task, user_completed_task, other_user_task])
        await session.commit()
        await session.refresh(user_single_task)
        await session.refresh(user_batch_task)

    response = await chapters_client.get("/api/chapters/batch-generate/active-tasks?limit=10")
    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 2

    items_by_id = {item["batch_id"]: item for item in body["items"]}
    assert set(items_by_id.keys()) == {user_single_task.id, user_batch_task.id}
    assert items_by_id[user_single_task.id]["task_type"] == "chapter_single_generate"
    assert items_by_id[user_batch_task.id]["task_type"] == "chapters_batch_generate"
    assert items_by_id[user_batch_task.id]["stage_code"] == "6.writing.loading"
    assert items_by_id[user_batch_task.id]["execution_mode"] == "interactive"
    assert items_by_id[user_batch_task.id]["checkpoint"]["current_chapter_number"] is None
    assert items_by_id[user_batch_task.id]["project_id"] == project.id


async def test_should_require_login_when_listing_active_batch_generation_tasks(
    chapters_client,
):
    response = await chapters_client.get(
        "/api/chapters/batch-generate/active-tasks",
        headers={"x-test-user-id": "__none__"},
    )
    assert response.status_code == 401


async def test_should_reject_stream_subscription_from_other_user(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="订阅权限测试章节",
        content=None,
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    response = await chapters_client.get(
        f"/api/chapters/batch-generate/{task_id}/stream",
        headers={"x-test-user-id": "other-user"},
    )
    assert response.status_code == 403


async def test_should_return_404_when_batch_status_task_missing(chapters_client):
    response = await chapters_client.get("/api/chapters/batch-generate/missing/status")
    assert response.status_code == 404


async def test_should_regenerate_chapter_stream_and_persist_regeneration_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待重写章节",
        content="这是旧内容",
        status="completed",
    )

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            yield {"type": "progress", "progress": 35, "message": "准备中"}
            yield {"type": "chunk", "content": "新"}
            yield {"type": "chunk", "content": "内容"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 12.5, "difference": 87.5}

    monkeypatch.setattr(chapters_api, "ChapterRegenerator", FakeRegenerator)

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "优化节奏",
            "target_word_count": 500,
            "focus_areas": ["pacing"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    task_id = result_event["data"]["task_id"]
    assert result_event["data"]["word_count"] > 0
    assert "diff_stats" in result_event["data"]

    async with chapters_session_factory() as session:
        task = await session.get(RegenerationTask, task_id)
        assert task is not None
        assert task.status == "completed"
        assert task.regenerated_content == "新内容"



async def test_should_sanitize_regenerated_content_before_persisting_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="\u6574\u7ae0\u91cd\u5199\u6e05\u6d17\u6d4b\u8bd5",
        content="\u539f\u6587",
        status="completed",
    )

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            yield {
                "type": "chunk",
                "content": "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n",
            }
            yield {
                "type": "chunk",
                "content": "\u4e0b\u4e00\u79d2\uff0c\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002\n",
            }
            yield {
                "type": "chunk",
                "content": "\u5730\u4e0a\u7684\u6c34\u75d5\u50cf\u6709\u4ec0\u4e48\u4e1c\u897f\u62d6\u8fc7\u53bb\u3002",
            }

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 10.0, "difference": 90.0}

    monkeypatch.setattr(chapters_api, "ChapterRegenerator", FakeRegenerator)

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "\u4f18\u5316\u8282\u594f",
            "target_word_count": 500,
            "focus_areas": ["pacing"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    task_id = result_event["data"]["task_id"]

    async with chapters_session_factory() as session:
        task = await session.get(RegenerationTask, task_id)
        assert task is not None
        assert task.regenerated_content == (
            "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
            "\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002\n"
            "\u5730\u4e0a\u7684\u6c34\u75d5\u50cf\u6709\u4e1c\u897f\u62d6\u8fc7\u53bb\u3002"
        )
async def test_should_return_400_when_regenerate_chapter_content_is_empty(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="空内容章节",
        content="",
        status="draft",
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "优化节奏",
            "target_word_count": 500,
            "auto_apply": False,
        },
    )
    assert response.status_code == 400


async def test_should_return_analysis_checker_and_auto_revision_payloads(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="分析闭环章节",
        content="旧正文",
        status="completed",
    )

    analysis = PlotAnalysis(
        project_id=project.id,
        chapter_id=chapter.id,
        plot_stage="发展",
        conflict_level=7,
        conflict_types=["人与人"],
        emotional_tone="紧张",
        hooks=[{"type": "悬念", "content": "门后有异响", "strength": 8, "position": "结尾"}],
        hooks_count=1,
        foreshadows=[{"content": "镜面异光", "type": "planted", "strength": 7}],
        foreshadows_planted=1,
        plot_points=[{"content": "主角决定独自断后", "importance": 0.9, "type": "conflict"}],
        plot_points_count=1,
        character_states=[],
        scenes=[{"location": "走廊", "atmosphere": "压抑"}],
        pacing="fast",
        overall_quality_score=8.6,
        pacing_score=8.1,
        engagement_score=8.8,
        coherence_score=8.2,
        analysis_report="分析报告",
        suggestions=["增加回收"],
        word_count=len(chapter.content or ""),
        dialogue_ratio=0.22,
        description_ratio=0.41,
        created_at=datetime.utcnow() - timedelta(minutes=1),
    )
    checker_result = {
        "overall_assessment": "存在关键断裂",
        "severity_counts": {"critical": 1, "warning": 2, "info": 0},
        "issues": [
            {
                "severity": "critical",
                "title": "冲突动机不足",
                "evidence": "转折过快",
                "suggestion": "补足主角犹豫过程",
            }
        ],
        "priority_actions": ["补冲突因果"],
        "revision_suggestions": ["补一段心理描写"],
    }
    reviser_result = {
        "critical_count": 1,
        "major_count": 0,
        "priority_issue_count": 1,
        "applied_critical_count": 1,
        "applied_issue_count": 1,
        "change_summary": "补足心理与动作承接",
        "revised_text": "修订后正文，门后异响逼近，他终于承认自己在害怕。",
        "revised_text_preview": "修订后正文，门后异响逼近",
        "revised_word_count": 28,
        "unresolved_issues": [],
    }

    async with chapters_session_factory() as session:
        session.add(analysis)
        session.add(
            StoryMemory(
                project_id=project.id,
                chapter_id=chapter.id,
                memory_type="hook",
                title="结尾悬念",
                content="门后有异响",
                related_characters=[],
                related_locations=["走廊"],
                tags=["悬念"],
                importance_score=0.9,
                story_timeline=1,
                chapter_position=6,
                text_length=5,
                is_foreshadow=0,
                vector_id=f"vec-{chapter.id}",
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="checker",
                generated_content=chapters_api._build_checker_history_payload(checker_result),
                model="chapter_text_checker_v1",
                created_at=datetime.utcnow() - timedelta(minutes=2),
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="reviser",
                generated_content=chapters_api._build_reviser_history_payload(reviser_result),
                model="chapter_text_reviser_v1",
                created_at=datetime.utcnow() - timedelta(minutes=1),
            )
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/{chapter.id}/analysis")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["analysis"]["plot_stage"] == "发展"
    assert body["memories"][0]["title"] == "结尾悬念"
    assert body["checker_result"]["severity_counts"]["critical"] == 1
    assert body["auto_revision_draft"]["critical_count"] == 1
    assert body["auto_revision_draft"]["major_count"] == 0
    assert body["auto_revision_draft"]["priority_issue_count"] == 1
    assert body["auto_revision_draft"]["applied_issue_count"] == 1
    assert body["auto_revision_draft"]["is_stale"] is True
    assert body["auto_revision_draft"].get("revised_text") is None

    full_response = await chapters_client.get(f"/api/chapters/{chapter.id}/analysis?include_full_draft=true")
    assert full_response.status_code == 200
    assert (
        full_response.json()["auto_revision_draft"]["revised_text"]
        == reviser_result["revised_text"]
    )


async def test_should_get_and_apply_auto_revision_draft(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待应用草稿章节",
        content="旧正文",
        status="completed",
    )

    reviser_result = {
        "critical_count": 2,
        "major_count": 1,
        "priority_issue_count": 3,
        "applied_critical_count": 2,
        "applied_issue_count": 3,
        "change_summary": "修复关键断裂",
        "revised_text": "新正文已经覆盖旧正文，并补足了承接。",
        "revised_text_preview": "新正文已经覆盖旧正文",
        "revised_word_count": 18,
        "unresolved_issues": [],
    }

    async with chapters_session_factory() as session:
        reviser_history = GenerationHistory(
            project_id=project.id,
            chapter_id=chapter.id,
            prompt="reviser",
            generated_content=chapters_api._build_reviser_history_payload(reviser_result),
            model="chapter_text_reviser_v1",
        )
        session.add(reviser_history)
        await session.commit()
        await session.refresh(reviser_history)
        history_id = reviser_history.id

    draft_response = await chapters_client.get(
        f"/api/chapters/{chapter.id}/analysis/auto-revision-draft"
    )
    assert draft_response.status_code == 200
    draft = draft_response.json()["auto_revision_draft"]
    assert draft["history_id"] == history_id
    assert draft["priority_issue_count"] == 3
    assert draft["major_count"] == 1
    assert draft["applied_issue_count"] == 3
    assert draft["revised_text"] == reviser_result["revised_text"]
    assert draft["is_stale"] is False

    apply_response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/analysis/auto-revision-draft/apply",
        json={"history_id": history_id},
    )
    assert apply_response.status_code == 200
    apply_body = apply_response.json()
    assert apply_body["success"] is True
    assert apply_body["draft_history_id"] == history_id
    assert apply_body["word_count"] == len(reviser_result["revised_text"])

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.content == reviser_result["revised_text"]

        history_result = await session.execute(
            select(GenerationHistory)
            .where(GenerationHistory.chapter_id == chapter.id)
            .order_by(GenerationHistory.created_at.desc())
        )
        histories = history_result.scalars().all()
        assert any(history.model == "chapter_text_reviser_apply_v1" for history in histories)


async def test_should_generate_auto_revision_draft_when_only_major_issues_exist(
    chapters_session_factory,
    monkeypatch,
):
    captured_prompt: dict[str, str] = {}

    class StubAIService:
        async def call_with_json_retry(self, **kwargs):
            captured_prompt["prompt"] = kwargs["prompt"]
            return {
                "revised_text": "她把门把手握得更紧，呼吸也跟着停了一拍。门外没有再响，可那份迟疑终于落在了动作上。",
                "applied_issues": ["补足人物迟疑过程", "把异响的即时反应落到动作里"],
                "unresolved_issues": ["结尾悬念还能再收紧一点"],
                "change_summary": "已补足 major 级承接与动作反应",
            }

    async def fake_get_template(*args, **kwargs):
        return chapters_api.PromptService.CHAPTER_TEXT_REVISER

    monkeypatch.setattr(chapters_api.PromptService, "get_template", fake_get_template)

    checker_result = {
        "severity_counts": {"critical": 0, "major": 2, "minor": 1},
        "issues": [
            {
                "severity": "major",
                "category": "衔接",
                "location": "开头第2段",
                "impact": "人物从听见异响到决定开门缺少迟疑过程",
                "suggestion": "补足人物迟疑过程",
            },
            {
                "severity": "major",
                "category": "表现",
                "location": "结尾第1段",
                "impact": "异响出现后缺少即时动作反馈",
                "suggestion": "把异响的即时反应落到动作里",
            },
            {
                "severity": "minor",
                "category": "文风",
                "location": "结尾句",
                "impact": "个别表达偏模板化",
                "suggestion": "压缩套句",
            },
        ],
    }

    async with chapters_session_factory() as session:
        reviser_result = await chapters_api._run_chapter_text_reviser(
            ai_service=StubAIService(),
            db_session=session,
            user_id="test-user",
            chapter_number=1,
            chapter_title="凌晨三点半的多余顾客",
            chapter_content="门外的异响又响了一次，她把手从门把上挪开，又按了回去。",
            checker_result=checker_result,
        )

    assert reviser_result is not None
    assert reviser_result["critical_count"] == 0
    assert reviser_result["major_count"] == 2
    assert reviser_result["priority_issue_count"] == 2
    assert reviser_result["applied_issue_count"] == 2
    assert "高优先问题清单" in captured_prompt["prompt"]
    assert "补足人物迟疑过程" in captured_prompt["prompt"]
    assert "把异响的即时反应落到动作里" in captured_prompt["prompt"]


async def test_should_auto_recover_stale_analysis_status_and_keep_none_compatible(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_none = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="无任务状态章节",
        content="正文",
        status="completed",
    )
    chapter_active = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="活跃分析章节",
        content="正文",
        status="completed",
    )
    chapter_stale_running = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="超时运行章节",
        content="正文",
        status="completed",
    )
    chapter_stale_pending = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=4,
        title="超时待启动章节",
        content="正文",
        status="completed",
    )

    none_response = await chapters_client.get(f"/api/chapters/{chapter_none.id}/analysis/status")
    assert none_response.status_code == 200
    none_body = none_response.json()
    assert none_body["has_task"] is False
    assert none_body["status"] == "none"
    assert none_body["task_id"] is None

    active_running_time = datetime.now() - timedelta(minutes=4)
    stale_running_time = datetime.now() - timedelta(minutes=11)
    stale_pending_time = datetime.now() - timedelta(minutes=4)

    async with chapters_session_factory() as session:
        session.add(
            AnalysisTask(
                chapter_id=chapter_active.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="running",
                progress=20,
                started_at=active_running_time,
                created_at=active_running_time,
            )
        )
        session.add(
            AnalysisTask(
                chapter_id=chapter_stale_running.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="running",
                progress=56,
                started_at=stale_running_time,
                created_at=stale_running_time,
            )
        )
        session.add(
            AnalysisTask(
                chapter_id=chapter_stale_pending.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="pending",
                progress=0,
                created_at=stale_pending_time,
            )
        )
        await session.commit()

    active_running = await chapters_client.get(f"/api/chapters/{chapter_active.id}/analysis/status")
    assert active_running.status_code == 200
    active_running_body = active_running.json()
    assert active_running_body["has_task"] is True
    assert active_running_body["status"] == "running"
    assert active_running_body["auto_recovered"] is False
    assert active_running_body["progress"] == 20

    recovered_running = await chapters_client.get(f"/api/chapters/{chapter_stale_running.id}/analysis/status")
    assert recovered_running.status_code == 200
    running_body = recovered_running.json()
    assert running_body["has_task"] is True
    assert running_body["status"] == "failed"
    assert running_body["auto_recovered"] is True
    assert running_body["error_code"] == "timeout"
    assert "自动恢复" in (running_body["error_message"] or "")

    recovered_pending = await chapters_client.get(f"/api/chapters/{chapter_stale_pending.id}/analysis/status")
    assert recovered_pending.status_code == 200
    pending_body = recovered_pending.json()
    assert pending_body["status"] == "failed"
    assert pending_body["auto_recovered"] is True
    assert pending_body["error_code"] == "timeout"
    assert "启动超时" in (pending_body["error_message"] or "")


async def test_should_trigger_manual_analysis_task_creation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="手动分析章节",
        content="正文存在，可分析。",
        status="completed",
    )

    calls: list[dict[str, Any]] = []

    async def fake_analyze_chapter_background(**kwargs):
        calls.append(kwargs)
        return True

    async def fake_sleep(_seconds):
        return None

    monkeypatch.setattr(chapters_api, "analyze_chapter_background", fake_analyze_chapter_background)
    monkeypatch.setattr(chapters_api.asyncio, "sleep", fake_sleep)

    response = await chapters_client.post(f"/api/chapters/{chapter.id}/analyze")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["status"] == "pending"
    assert body["task_id"]

    assert calls
    assert calls[0]["chapter_id"] == chapter.id
    assert calls[0]["project_id"] == project.id
    assert calls[0]["task_id"] == body["task_id"]


async def test_should_restore_deferred_analysis_quality_snapshot_and_regeneration_compatibility(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="后台分析恢复章节",
        content="正文内容",
        status="completed",
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
            current_chapter_id=chapter.id,
            current_chapter_number=1,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    await chapters_api.publish_task_stream_event(
        task_id,
        {
            "type": "analysis_started",
            "chapter_id": chapter.id,
            "chapter_number": 1,
            "message": "章节分析开始",
            "progress": 85,
            "phase": "parsing",
        },
    )
    await chapters_api._record_task_quality_metrics(
        task_id,
        {
            "chapter_id": chapter.id,
            "chapter_number": 1,
            "overall_score": 88.0,
            "conflict_chain_hit_rate": 80.0,
            "rule_grounding_hit_rate": 84.0,
            "opening_hook_rate": 90.0,
            "payoff_chain_rate": 76.0,
            "cliffhanger_rate": 92.0,
        },
    )

    status_response = await chapters_client.get(f"/api/chapters/batch-generate/{task_id}/status")
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["stage_code"] == "6.writing.parsing"
    assert status_body["checkpoint"]["last_event"] == "analysis_started"
    assert status_body["latest_quality_metrics"]["overall_score"] == 88.0
    assert status_body["quality_metrics_summary"]["chapter_count"] == 1
    assert status_body["quality_metrics_summary"]["avg_overall_score"] == 88.0

    active_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/batch-generate/active"
    )
    assert active_response.status_code == 200
    active_body = active_response.json()
    assert active_body["has_active_task"] is True
    assert active_body["task"]["batch_id"] == task_id
    assert active_body["task"]["checkpoint"]["progress_phase"] == "parsing"
    assert active_body["task"]["latest_quality_metrics"]["overall_score"] == 88.0
    assert active_body["task"]["quality_metrics_summary"]["avg_cliffhanger_rate"] == 92.0

    can_generate_response = await chapters_client.get(f"/api/chapters/{chapter.id}/can-generate")
    assert can_generate_response.status_code == 200
    assert can_generate_response.json()["can_generate"] is True
