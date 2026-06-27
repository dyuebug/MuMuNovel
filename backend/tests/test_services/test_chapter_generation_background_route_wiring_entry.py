import pytest

from tests.test_support import (
    single_generation_background_entry_test_adapter as entry_service,
)


class _ScalarResult:
    def __init__(self, value):
        self._value = value

    def scalar_one_or_none(self):
        return self._value


@pytest.mark.asyncio
async def test_should_generate_chapter_content_background_with_explicit_wiring():
    captured = {}
    chapter = type("Chapter", (), {"project_id": "project-1"})()
    project = object()
    request = type("Request", (), {"state": type("State", (), {"user_id": "user-1"})()})()

    async def fake_load_chapter(*, db, chapter_id, user_id):
        captured["load_kwargs"] = {
            "db": db,
            "chapter_id": chapter_id,
            "user_id": user_id,
        }
        return chapter

    def fake_require_authenticated_user_id(request_obj):
        assert request_obj is request
        return "user-1"

    class FakeDbSession:
        async def execute(self, stmt):
            captured["project_stmt"] = stmt
            return _ScalarResult(project)

    class FakeProjectModel:
        class id:
            def __eq__(self, other):
                return ("project-id-eq", other)

    class FakeSelectStmt:
        def __init__(self, model):
            self.model = model

        def where(self, condition):
            captured["where_condition"] = condition
            return self

    async def fake_orchestrate(*args, **kwargs):
        captured["orchestrate_args"] = args
        captured["orchestrate_kwargs"] = kwargs
        return {"ok": True}

    async def fake_sync(*_args, **_kwargs):
        return {}

    original_select = entry_service.select
    entry_service.select = lambda model: FakeSelectStmt(model)

    try:
        result = await entry_service.generate_chapter_content_background_with_explicit_wiring(
            chapter_id="chapter-1",
            request=request,
            background_tasks="bg",
            generate_request="request",
            db_session=FakeDbSession(),
            user_ai_service="ai",
            require_authenticated_user_id_fn=fake_require_authenticated_user_id,
            load_accessible_chapter_or_404_fn=fake_load_chapter,
            project_model=FakeProjectModel,
            check_prerequisites_fn="check-fn",
            build_workflow_snapshot_fn="snapshot-fn",
            resolve_story_repair_state_fn="repair-state-fn",
            sync_task_story_repair_state_fn=fake_sync,
            orchestrate_background_generation_fn=fake_orchestrate,
            execution_callable="execute-fn",
        )
    finally:
        entry_service.select = original_select

    assert result == {"ok": True}
    assert captured["load_kwargs"]["chapter_id"] == "chapter-1"
    assert captured["load_kwargs"]["user_id"] == "user-1"
    assert captured["orchestrate_args"]
    assert captured["orchestrate_kwargs"]["chapter_id"] == "chapter-1"
    assert captured["orchestrate_kwargs"]["chapter"] is chapter
    assert captured["orchestrate_kwargs"]["project"] is project
    assert captured["orchestrate_kwargs"]["user_id"] == "user-1"
    assert captured["orchestrate_kwargs"]["generate_request"] == "request"
    assert captured["orchestrate_kwargs"]["background_tasks"] == "bg"
    assert captured["orchestrate_kwargs"]["ai_service"] == "ai"
    assert captured["orchestrate_kwargs"]["check_prerequisites_fn"] == "check-fn"
    assert captured["orchestrate_kwargs"]["build_workflow_snapshot_fn"] == "snapshot-fn"
    assert captured["orchestrate_kwargs"]["resolve_story_repair_state_fn"] == "repair-state-fn"
    assert captured["orchestrate_kwargs"]["sync_task_story_repair_state_fn"] is fake_sync
    assert captured["orchestrate_kwargs"]["execution_callable"] == "execute-fn"


@pytest.mark.asyncio
async def test_should_raise_when_project_missing_during_background_generation_explicit_wiring():
    chapter = type("Chapter", (), {"project_id": "project-1"})()
    request = type("Request", (), {"state": type("State", (), {"user_id": "user-1"})()})()

    async def fake_load_chapter(*, db, chapter_id, user_id):
        return chapter

    def fake_require_authenticated_user_id(request_obj):
        assert request_obj is request
        return "user-1"

    class FakeDbSession:
        async def execute(self, _stmt):
            return _ScalarResult(None)

    class FakeProjectModel:
        class id:
            def __eq__(self, other):
                return ("project-id-eq", other)

    class FakeSelectStmt:
        def __init__(self, model):
            self.model = model

        def where(self, _condition):
            return self

    original_select = entry_service.select
    entry_service.select = lambda model: FakeSelectStmt(model)

    try:
        with pytest.raises(Exception) as exc_info:
            await entry_service.generate_chapter_content_background_with_explicit_wiring(
                chapter_id="chapter-1",
                request=request,
                background_tasks="bg",
                generate_request="request",
                db_session=FakeDbSession(),
                user_ai_service="ai",
                require_authenticated_user_id_fn=fake_require_authenticated_user_id,
                load_accessible_chapter_or_404_fn=fake_load_chapter,
                project_model=FakeProjectModel,
                check_prerequisites_fn="check-fn",
                build_workflow_snapshot_fn="snapshot-fn",
                resolve_story_repair_state_fn="repair-state-fn",
                sync_task_story_repair_state_fn="sync-task-state-fn",
                orchestrate_background_generation_fn="orchestrate-fn",
                execution_callable="execute-fn",
            )
    finally:
        entry_service.select = original_select

    assert exc_info.value.status_code == 404
    assert exc_info.value.detail == "Project not found"


@pytest.mark.asyncio
async def test_should_generate_chapter_content_background_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_generate_with_explicit_wiring(**kwargs):
        captured.update(kwargs)
        return {"ok": True}

    monkeypatch.setattr(
        entry_service,
        "generate_chapter_content_background_with_explicit_wiring",
        fake_generate_with_explicit_wiring,
    )
    monkeypatch.setattr(entry_service, "require_authenticated_user_id", "require-user-fn")
    monkeypatch.setattr(entry_service, "load_accessible_chapter_or_404", "load-chapter-fn")
    monkeypatch.setattr(entry_service, "check_chapter_generation_prerequisites", "check-fn")
    monkeypatch.setattr(entry_service, "build_batch_task_workflow_snapshot", "snapshot-fn")
    monkeypatch.setattr(entry_service, "resolve_generation_story_repair_state_for_chapter", "repair-state-fn")
    monkeypatch.setattr(entry_service, "sync_task_story_repair_state", "sync-task-state-fn")
    monkeypatch.setattr(entry_service, "orchestrate_single_chapter_background_generation", "orchestrate-fn")
    monkeypatch.setattr(entry_service, "execute_batch_generation_in_order_with_default_wiring", "execute-fn")

    project_model = object()
    monkeypatch.setattr(entry_service, "_project_model", lambda: project_model)

    result = await entry_service.generate_chapter_content_background_with_default_wiring(
        chapter_id="chapter-1",
        request="request",
        background_tasks="bg",
        generate_request="generate-request",
        db_session="db-session",
        user_ai_service="ai-service",
    )

    assert result == {"ok": True}
    assert captured["chapter_id"] == "chapter-1"
    assert captured["request"] == "request"
    assert captured["background_tasks"] == "bg"
    assert captured["generate_request"] == "generate-request"
    assert captured["db_session"] == "db-session"
    assert captured["user_ai_service"] == "ai-service"
    assert captured["require_authenticated_user_id_fn"] == "require-user-fn"
    assert captured["load_accessible_chapter_or_404_fn"] == "load-chapter-fn"
    assert captured["project_model"] is project_model
    assert captured["check_prerequisites_fn"] == "check-fn"
    assert captured["build_workflow_snapshot_fn"] == "snapshot-fn"
    assert captured["resolve_story_repair_state_fn"] == "repair-state-fn"
    assert captured["sync_task_story_repair_state_fn"] == "sync-task-state-fn"
    assert captured["orchestrate_background_generation_fn"] == "orchestrate-fn"
    assert "execution_callable" not in captured
