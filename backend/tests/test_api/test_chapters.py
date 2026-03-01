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
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
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
    assert status_body["latest_quality_metrics"] is None
    assert status_body["quality_metrics_summary"] is None

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, batch_id)
        assert task is not None
        assert task.chapter_count == 2


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
