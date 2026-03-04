import pytest
from pathlib import Path
from uuid import uuid4

from app.services.background_task_manager import BackgroundTaskManager

pytestmark = pytest.mark.asyncio


def _build_persistence_path() -> str:
    base_dir = Path.cwd() / "data" / "runtime" / "test-artifacts"
    base_dir.mkdir(parents=True, exist_ok=True)
    return str(base_dir / f"background_tasks_{uuid4().hex}.json")


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

    await manager.create_task(
        task_id="task-restart-recover",
        task_type="outline_generate",
        user_id="user-1",
        project_id="project-1",
    )
    await manager.mark_running("task-restart-recover", "执行中")

    restored = BackgroundTaskManager(
        ttl_seconds=3600,
        max_tasks=100,
        persistence_path=str(persistence_file),
    )
    tasks = await restored.list_tasks(user_id="user-1", limit=20)
    assert len(tasks) == 1
    assert tasks[0].task_id == "task-restart-recover"
    assert tasks[0].status == "failed"
    assert tasks[0].error == "服务重启导致任务中断"


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
