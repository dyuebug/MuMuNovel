from types import SimpleNamespace

import pytest
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api import background_tasks as background_tasks_api
from app.services.background_task_manager import BackgroundTaskRecord


pytestmark = pytest.mark.asyncio


async def test_should_default_enable_mcp_true_for_career_background_task(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    async def fake_generate_career_system(**kwargs):
        captured.update(kwargs)
        return SimpleNamespace(body_iterator=fake_stream())

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.careers as careers_api

    monkeypatch.setattr(careers_api, "generate_career_system", fake_generate_career_system)

    await background_tasks_api._run_generation_task(
        task_id="task-1",
        user_id="user-1",
        task_type="careers_generate_system",
        project_id="project-1",
        payload={
            "main_career_count": 2,
            "sub_career_count": 4,
        },
    )

    assert captured["enable_mcp"] is True
    assert captured["main_career_count"] == 2
    assert captured["sub_career_count"] == 4


async def test_should_pass_mapping_payload_to_outline_generate_background_task(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    async def fake_generate_outline_stream(**kwargs):
        captured.update(kwargs)
        return SimpleNamespace(body_iterator=fake_stream())

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.outlines as outlines_api

    monkeypatch.setattr(outlines_api, "generate_outline_stream", fake_generate_outline_stream)

    await background_tasks_api._run_generation_task(
        task_id="task-outline-1",
        user_id="user-1",
        task_type="outline_generate",
        project_id="project-1",
        payload={
            "theme": "fated showdown",
            "chapter_count": 6,
            "narrative_perspective": "third_person",
            "mode": "new",
            "enable_web_research": True,
            "web_research_query": "late qing trade customs",
        },
    )

    assert isinstance(captured["data"], dict)
    assert captured["data"]["project_id"] == "project-1"
    assert captured["data"]["enable_web_research"] is True
    assert captured["data"]["web_research_query"] == "late qing trade customs"
    assert "user_id" not in captured["data"]


async def test_should_strip_user_id_from_outline_generate_background_payload(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    async def fake_generate_outline_stream(**kwargs):
        captured.update(kwargs)
        return SimpleNamespace(body_iterator=fake_stream())

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.outlines as outlines_api

    monkeypatch.setattr(outlines_api, "generate_outline_stream", fake_generate_outline_stream)

    await background_tasks_api._run_generation_task(
        task_id="task-outline-2",
        user_id="local_21232f297a57a5a7",
        task_type="outline_generate",
        project_id="project-1",
        payload={
            "theme": "fated showdown",
            "chapter_count": 6,
            "narrative_perspective": "third_person",
            "mode": "new",
            "user_id": "local_21232f297a57a5a7",
        },
    )

    assert isinstance(captured["data"], dict)
    assert captured["data"]["project_id"] == "project-1"
    assert "user_id" not in captured["data"]
    assert captured["data"]["mode"] == "new"
    assert captured["request"].state.user_id == "local_21232f297a57a5a7"


async def test_should_apply_outline_expand_schema_defaults_for_background_task(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    async def fake_expand_outline_to_chapters_stream(**kwargs):
        captured.update(kwargs)
        return SimpleNamespace(body_iterator=fake_stream())

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.outlines as outlines_api

    monkeypatch.setattr(outlines_api, "expand_outline_to_chapters_stream", fake_expand_outline_to_chapters_stream)

    await background_tasks_api._run_generation_task(
        task_id="task-outline-expand-1",
        user_id="user-1",
        task_type="outline_expand",
        project_id="project-1",
        payload={
            "outline_id": "outline-1",
            "user_id": "local_21232f297a57a5a7",
        },
    )

    assert captured["outline_id"] == "outline-1"
    assert "user_id" not in captured["data"]
    assert captured["data"]["enable_scene_analysis"] is True
    assert captured["data"]["auto_create_chapters"] is False


async def test_should_apply_outline_batch_expand_schema_defaults_for_background_task(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    async def fake_batch_expand_outlines_stream(**kwargs):
        captured.update(kwargs)
        return SimpleNamespace(body_iterator=fake_stream())

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.outlines as outlines_api

    monkeypatch.setattr(outlines_api, "batch_expand_outlines_stream", fake_batch_expand_outlines_stream)

    await background_tasks_api._run_generation_task(
        task_id="task-outline-batch-1",
        user_id="user-1",
        task_type="outline_batch_expand",
        project_id="project-1",
        payload={
            "user_id": "local_21232f297a57a5a7",
        },
    )

    assert captured["data"]["project_id"] == "project-1"
    assert "user_id" not in captured["data"]
    assert captured["data"]["enable_scene_analysis"] is True
    assert captured["data"]["auto_create_chapters"] is False


@pytest.mark.parametrize(
    ("task_type", "target_name", "expect_project_id"),
    [
        ("wizard_world_building", "world_building_generator", False),
        ("wizard_career_system", "career_system_generator", True),
        ("wizard_characters", "characters_generator", True),
        ("wizard_outline", "outline_generator", True),
    ],
)
async def test_should_inject_user_id_into_wizard_background_task_payload(
    monkeypatch,
    test_db: AsyncSession,
    task_type: str,
    target_name: str,
    expect_project_id: bool,
):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    def fake_target(data, db, user_ai_service):
        captured["data"] = data
        captured["db"] = db
        captured["user_ai_service"] = user_ai_service
        return fake_stream()

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.wizard_stream as wizard_stream_api

    monkeypatch.setattr(wizard_stream_api, target_name, fake_target)

    await background_tasks_api._run_generation_task(
        task_id=f"task-{task_type}",
        user_id="user-1",
        task_type=task_type,
        project_id="project-1",
        payload={
            "topic": "seed",
            "enable_web_research": True,
            "web_research_query": "harbor guild rumors",
            "reference_research_assets": [
                {
                    "title": "carried note",
                    "source": "https://example.com/note",
                    "summary": "Keep the hook visible in the opening scene.",
                }
            ],
        },
    )

    assert captured["data"]["user_id"] == "user-1"
    assert captured["data"]["enable_web_research"] is True
    assert captured["data"]["web_research_query"] == "harbor guild rumors"
    assert captured["data"]["reference_research_assets"][0]["title"] == "carried note"
    if expect_project_id:
        assert captured["data"]["project_id"] == "project-1"


async def test_should_not_override_known_metadata_when_background_task_missing(monkeypatch):
    async def fake_get_task(task_id: str, user_id: str):
        return None

    monkeypatch.setattr(background_tasks_api.background_task_manager, "get_task", fake_get_task)

    request = SimpleNamespace(state=SimpleNamespace(user_id="user-1"))
    result = await background_tasks_api.get_background_task_status("missing-task", request)

    assert result["task_id"] == "missing-task"
    assert result["task_type"] is None
    assert result["project_id"] is None
    assert result["execution_mode"] is None
    assert result["created_at"] is None
    assert result["status"] == "cancelled"
    assert result["error"] == "task_missing"


async def test_should_inject_user_id_into_world_regenerate_background_task(monkeypatch, test_db: AsyncSession):
    captured = {}
    session_maker = async_sessionmaker(
        test_db.bind,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async def fake_get_session_factory(user_id: str):
        return session_maker

    async def fake_build_user_ai_service(user_id: str, db: AsyncSession):
        return SimpleNamespace()

    async def fake_stream():
        if False:
            yield ""

    def fake_generator(project_id, data, db, user_ai_service):
        captured["project_id"] = project_id
        captured["data"] = data
        captured["db"] = db
        captured["user_ai_service"] = user_ai_service
        return fake_stream()

    async def fake_consume_sse_stream(task_id: str, stream):
        return None

    monkeypatch.setattr(background_tasks_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(background_tasks_api, "_build_user_ai_service", fake_build_user_ai_service)
    monkeypatch.setattr(background_tasks_api.background_task_manager, "consume_sse_stream", fake_consume_sse_stream)

    import app.api.wizard_stream as wizard_stream_api

    monkeypatch.setattr(wizard_stream_api, "world_building_regenerate_generator", fake_generator)

    await background_tasks_api._run_generation_task(
        task_id="task-world-regenerate",
        user_id="user-1",
        task_type="world_regenerate",
        project_id="project-1",
        payload={"topic": "seed"},
    )

    assert captured["project_id"] == "project-1"
    assert captured["data"]["user_id"] == "user-1"


@pytest.mark.parametrize(
    ("task_type", "project_id", "payload", "existing_task_id"),
    [
        (
            "wizard_world_building",
            "",
            {
                "title": "Smoke Novel",
                "description": "test setup",
                "theme": "urban",
            },
            "task-existing-wizard-world",
        ),
        (
            "world_regenerate",
            "project-1",
            {},
            "task-existing-world-regenerate",
        ),
        (
            "outline_generate",
            "project-1",
            {
                "title": "Smoke Outline",
                "theme": "urban",
                "target_chapters": 12,
            },
            "task-existing-outline-generate",
        ),
        (
            "outline_expand",
            "project-1",
            {
                "outline_id": "outline-1",
                "chapter_count": 8,
            },
            "task-existing-outline-expand",
        ),
        (
            "outline_batch_expand",
            "project-1",
            {
                "outline_ids": ["outline-1", "outline-2"],
                "auto_create_chapters": False,
            },
            "task-existing-outline-batch-expand",
        ),
    ],
)
async def test_should_reuse_existing_active_background_task_on_duplicate_create(
    monkeypatch,
    test_db: AsyncSession,
    task_type: str,
    project_id: str,
    payload: dict,
    existing_task_id: str,
):
    captured = {}
    existing = BackgroundTaskRecord(
        task_id=existing_task_id,
        task_type=task_type,
        user_id="user-1",
        project_id=project_id,
        status="running",
        progress=20,
        message="Generating...",
    )

    async def fake_find_active_task(**kwargs):
        captured.update(kwargs)
        return existing

    async def fake_create_task(**kwargs):
        raise AssertionError('create_task should not be called for duplicate active task')

    async def fake_attach_runner(task_id: str, runner_task):
        raise AssertionError('attach_runner should not be called for duplicate active task')

    async def fake_verify_project_access(project_id: str, user_id: str, db: AsyncSession):
        return None

    monkeypatch.setattr(background_tasks_api.background_task_manager, 'find_active_task', fake_find_active_task)
    monkeypatch.setattr(background_tasks_api.background_task_manager, 'create_task', fake_create_task)
    monkeypatch.setattr(background_tasks_api.background_task_manager, 'attach_runner', fake_attach_runner)
    monkeypatch.setattr(background_tasks_api, 'verify_project_access', fake_verify_project_access)

    request = SimpleNamespace(state=SimpleNamespace(user_id='user-1'))
    data = background_tasks_api.BackgroundTaskCreateRequest(
        task_type=task_type,
        project_id=project_id or None,
        payload=payload,
    )

    result = await background_tasks_api.create_background_task(data, request, test_db)

    assert result['task_id'] == existing_task_id
    assert captured['user_id'] == 'user-1'
    assert captured['task_type'] == task_type
    assert captured['project_id'] == project_id
    assert captured['payload_fingerprint'] == background_tasks_api._build_task_dedupe_fingerprint(
        task_type,
        project_id,
        payload,
    )
