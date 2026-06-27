import asyncio
import pytest
from pathlib import Path
from uuid import uuid4
from fastapi import HTTPException

from tests.test_support.background_task_manager_test_support import BackgroundTaskManager
from tests.test_support.utils.exception_message import extract_exception_message

pytestmark = pytest.mark.asyncio


def _build_persistence_path() -> str:
    base_dir = Path.cwd() / "data" / "runtime" / "test-artifacts"
    base_dir.mkdir(parents=True, exist_ok=True)
    return str(base_dir / f"background_tasks_{uuid4().hex}.json")


async def test_should_not_force_persist_when_creating_task(monkeypatch):
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )
    persist_calls = []

    def fake_persist_locked(*, force: bool = False) -> None:
        persist_calls.append(force)

    monkeypatch.setattr(manager, '_persist_locked', fake_persist_locked)

    await manager.create_task(
        task_id='task-persist-create',
        task_type='outline_generate',
        user_id='user-1',
        project_id='project-1',
    )

    assert persist_calls == [False]


async def test_should_list_tasks_with_user_project_and_status_filters():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-u1-p1-running",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )
    await manager.mark_running("task-u1-p1-running", "生成中")

    await manager.create_task(
        task_id="task-u1-p2-completed",
        task_type="world_regenerate",
        user_id="user-1",
        project_id="project-2",
    )
    await manager.mark_completed("task-u1-p2-completed", "已完成")

    await manager.create_task(
        task_id="task-u2-p1-running",
        task_type="character_generate",
        user_id="user-2",
        project_id="project-1",
    )
    await manager.mark_running("task-u2-p1-running", "运行中")

    user_one_tasks = await manager.list_tasks(user_id="user-1", limit=20)
    assert {item.task_id for item in user_one_tasks} == {
        "task-u1-p2-completed",
        "task-u1-p1-running",
    }

    project_filtered = await manager.list_tasks(
        user_id="user-1",
        project_id="project-1",
        limit=20,
    )
    assert [item.task_id for item in project_filtered] == ["task-u1-p1-running"]

    status_filtered = await manager.list_tasks(
        user_id="user-1",
        statuses=["running", "pending"],
        limit=20,
    )
    assert [item.task_id for item in status_filtered] == ["task-u1-p1-running"]


async def test_should_respect_limit_when_listing_tasks():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    for index in range(5):
        task_id = f"task-{index}"
        await manager.create_task(
            task_id=task_id,
            task_type="outline_generate",
            user_id="user-1",
            project_id="project-1",
        )
        await manager.update_progress(task_id, index * 10, f"step-{index}")

    tasks = await manager.list_tasks(user_id="user-1", limit=3)
    assert len(tasks) == 3


async def test_should_record_checkpoint_snapshot_when_progress_updates():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )
    await manager.create_task(
        task_id="task-checkpoint-progress",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )

    await manager.update_progress("task-checkpoint-progress", 35, "生成进行中")
    record = await manager.get_task("task-checkpoint-progress", "user-1")
    assert record is not None
    assert isinstance(record.checkpoint, dict)
    assert record.checkpoint.get("event") == "progress"
    assert record.checkpoint.get("progress") == 35
    assert record.checkpoint.get("message") == "生成进行中"


async def test_should_recover_running_tasks_as_failed_after_restart():
    persistence_file = Path(_build_persistence_path())
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=str(persistence_file),
    )
    manager._progress_persist_interval_seconds = 0

    await manager.create_task(
        task_id="task-restart-recover",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline.generating",
    )
    await manager.mark_running("task-restart-recover", "任务执行中")
    await manager.update_progress("task-restart-recover", 35, "生成进行中")

    restored = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=str(persistence_file),
    )
    tasks = await restored.list_tasks(user_id="user-1", limit=20)
    assert len(tasks) == 1
    assert tasks[0].task_id == "task-restart-recover"
    assert tasks[0].status == "failed"
    assert tasks[0].error == "服务重启导致任务上下文丢失"
    assert isinstance(tasks[0].checkpoint, dict)
    assert tasks[0].checkpoint.get("event") == "failed"
    assert tasks[0].checkpoint.get("error") == "服务重启导致任务上下文丢失"
    assert tasks[0].checkpoint.get("stage_code") == "1.outline.generating"


async def test_should_keep_workflow_fields_when_creating_task():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    record = await manager.create_task(
        task_id="task-workflow-init",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline",
        execution_mode="auto",
        workflow_scope="第1卷",
        checkpoint={"cursor": "node-3"},
    )

    assert record.stage_code == "1.outline"
    assert record.execution_mode == "auto"
    assert record.workflow_scope == "第1卷"
    assert record.checkpoint == {"cursor": "node-3"}


async def test_should_update_workflow_state_and_persist_after_restart():
    persistence_file = Path(_build_persistence_path())
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=str(persistence_file),
    )

    await manager.create_task(
        task_id="task-workflow-update",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline",
        execution_mode="interactive",
    )

    updated = await manager.update_workflow_state(
        task_id="task-workflow-update",
        user_id="user-1",
        stage_code="2.volume",
        execution_mode="auto",
        workflow_scope="第2卷",
        checkpoint={"current_step": "2.3.1"},
        message="进入卷纲检查",
        progress=62,
    )
    assert updated is not None
    assert updated.stage_code == "2.volume"
    assert updated.execution_mode == "auto"
    assert updated.workflow_scope == "第2卷"
    assert updated.checkpoint == {"current_step": "2.3.1"}
    assert updated.progress == 62

    restored = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=str(persistence_file),
    )
    tasks = await restored.list_tasks(user_id="user-1", limit=20)
    assert len(tasks) == 1
    assert tasks[0].stage_code == "2.volume"
    assert tasks[0].execution_mode == "auto"
    assert tasks[0].workflow_scope == "第2卷"

async def test_should_infer_progress_phase_and_stage_code_from_updates():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )
    await manager.create_task(
        task_id="task-stage-progress",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline",
    )

    await manager.update_progress("task-stage-progress", 8, "loading context")
    record = await manager.get_task("task-stage-progress", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.loading"
    assert isinstance(record.checkpoint, dict)
    assert record.checkpoint.get("progress_phase") == "loading"
    assert record.checkpoint.get("stage_code") == "1.outline.loading"

    await manager.update_progress("task-stage-progress", 40, "generate draft")
    record = await manager.get_task("task-stage-progress", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.generating"

    await manager.update_progress("task-stage-progress", 90, "parse response")
    record = await manager.get_task("task-stage-progress", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.parsing"

    await manager.update_progress("task-stage-progress", 95, "save result")
    record = await manager.get_task("task-stage-progress", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.saving"

    await manager.mark_completed("task-stage-progress", "done")
    record = await manager.get_task("task-stage-progress", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.complete"
    assert isinstance(record.checkpoint, dict)
    assert record.checkpoint.get("progress_phase") == "complete"


async def test_should_keep_phase_monotonic_without_retry_hint():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )
    await manager.create_task(
        task_id="task-stage-monotonic",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline.parsing",
    )

    await manager.update_progress("task-stage-monotonic", 20, "loading context")
    record = await manager.get_task("task-stage-monotonic", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.parsing"

    await manager.update_progress("task-stage-monotonic", 20, "retry loading context")
    record = await manager.get_task("task-stage-monotonic", "user-1")
    assert record is not None
    assert record.stage_code == "1.outline.loading"


async def test_should_finalize_stage_code_when_run_job_completes():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-run-job-complete-stage",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline.parsing",
    )

    async def _job() -> None:
        return None

    await manager.run_job("task-run-job-complete-stage", _job())

    record = await manager.get_task("task-run-job-complete-stage", "user-1")
    assert record is not None
    assert record.status == "completed"
    assert record.stage_code == "1.outline.complete"
    assert isinstance(record.checkpoint, dict)
    assert record.checkpoint.get("event") == "completed"
    assert record.checkpoint.get("progress_phase") == "complete"
    assert record.checkpoint.get("stage_code") == "1.outline.complete"


async def test_should_not_run_job_when_task_was_cancelled_before_runner_started():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-cancel-before-runner",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )
    await manager.cancel_task("task-cancel-before-runner", "user-1")

    execution_state = {"started": False}

    async def _job() -> None:
        execution_state["started"] = True

    await manager.run_job("task-cancel-before-runner", _job())

    record = await manager.get_task("task-cancel-before-runner", "user-1")
    assert record is not None
    assert record.status == "cancelled"
    assert execution_state["started"] is False


async def test_should_close_stream_when_consume_sse_stream_returns_after_cancel():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-cancelled-stream-close",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )
    await manager.cancel_task("task-cancelled-stream-close", "user-1")

    class _ClosableStream:
        def __init__(self):
            self._yielded = False
            self.closed = False

        def __aiter__(self):
            return self

        async def __anext__(self):
            if self._yielded:
                raise StopAsyncIteration
            self._yielded = True
            return 'data: {"type":"progress","progress":10,"message":"step"}\n\n'

        async def aclose(self):
            self.closed = True

    stream = _ClosableStream()
    await manager.consume_sse_stream("task-cancelled-stream-close", stream)

    assert stream.closed is True


async def test_should_mark_task_completed_when_sse_stream_ends_after_result_without_done():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-result-without-done",
        task_type="wizard_world_building",
        user_id="user-1",
        project_id="",
    )
    await manager.mark_running("task-result-without-done", "running")

    class _ResultOnlyStream:
        def __init__(self):
            self._yielded = False
            self.closed = False

        def __aiter__(self):
            return self

        async def __anext__(self):
            if self._yielded:
                raise StopAsyncIteration
            self._yielded = True
            return 'data: {"type":"result","data":{"project_id":"project-created-1","message":"ok"}}\n\n'

        async def aclose(self):
            self.closed = True

    stream = _ResultOnlyStream()
    await manager.consume_sse_stream("task-result-without-done", stream)

    record = await manager.get_task("task-result-without-done", "user-1")
    assert record is not None
    assert record.status == "completed"
    assert record.progress == 100
    assert record.project_id == "project-created-1"
    assert record.result == {"project_id": "project-created-1", "message": "ok"}
    assert stream.closed is True


async def test_should_cancel_runner_when_attached_after_task_was_cancelled():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-attach-after-cancel",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )
    await manager.cancel_task("task-attach-after-cancel", "user-1")

    started = asyncio.Event()

    async def _runner() -> None:
        started.set()
        await asyncio.sleep(60)

    runner_task = asyncio.create_task(_runner())
    await started.wait()

    await manager.attach_runner("task-attach-after-cancel", runner_task)

    with pytest.raises(asyncio.CancelledError):
        await runner_task


async def test_should_update_task_project_id_from_result_payload():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-world-building-project-link",
        task_type="wizard_world_building",
        user_id="user-1",
        project_id="",
    )

    await manager.set_result(
        "task-world-building-project-link",
        {
            "project_id": "project-created-1",
            "message": "ok",
        },
    )

    record = await manager.get_task("task-world-building-project-link", "user-1")
    assert record is not None
    assert record.project_id == "project-created-1"

    project_tasks = await manager.list_tasks(
        user_id="user-1",
        project_id="project-created-1",
        limit=20,
    )
    assert [item.task_id for item in project_tasks] == ["task-world-building-project-link"]


def test_should_extract_http_exception_detail_when_string_representation_is_empty():
    exc = HTTPException(status_code=400, detail="没有可用的现有大纲，无法继续生成")

    assert extract_exception_message(exc) == "没有可用的现有大纲，无法继续生成"


async def test_should_use_http_exception_detail_when_background_job_fails():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-http-exception-detail",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
        stage_code="1.outline.generating",
    )

    async def _job() -> None:
        raise HTTPException(status_code=400, detail="没有可用的现有大纲，无法继续生成")

    await manager.run_job("task-http-exception-detail", _job())

    record = await manager.get_task("task-http-exception-detail", "user-1")
    assert record is not None
    assert record.status == "failed"
    assert record.error == "没有可用的现有大纲，无法继续生成"
    assert record.message == "任务执行失败"
    assert isinstance(record.checkpoint, dict)
    assert record.checkpoint.get("error") == "没有可用的现有大纲，无法继续生成"


async def test_should_find_active_task_by_matching_payload_fingerprint():
    manager = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=_build_persistence_path(),
    )

    await manager.create_task(
        task_id="task-dedupe-match-old",
        task_type="wizard_world_building",
        user_id="user-1",
        project_id="",
        payload_fingerprint="fp-match",
    )
    await manager.mark_running("task-dedupe-match-old", "running")

    await manager.create_task(
        task_id="task-dedupe-other",
        task_type="wizard_world_building",
        user_id="user-1",
        project_id="",
        payload_fingerprint="fp-other",
    )
    await manager.mark_running("task-dedupe-other", "running")

    await manager.create_task(
        task_id="task-dedupe-match-new",
        task_type="wizard_world_building",
        user_id="user-1",
        project_id="",
        payload_fingerprint="fp-match",
    )
    await manager.mark_running("task-dedupe-match-new", "running")
    await manager.update_progress("task-dedupe-match-new", 30, "step-new")

    record = await manager.find_active_task(
        user_id="user-1",
        task_type="wizard_world_building",
        project_id="",
        payload_fingerprint="fp-match",
    )

    assert record is not None
    assert record.task_id in {"task-dedupe-match-old", "task-dedupe-match-new"}
    assert record.payload_fingerprint == "fp-match"

    missing = await manager.find_active_task(
        user_id="user-1",
        task_type="wizard_world_building",
        project_id="",
        payload_fingerprint="fp-missing",
    )
    assert missing is None
