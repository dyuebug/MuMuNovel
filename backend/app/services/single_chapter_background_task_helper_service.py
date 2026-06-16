"""单章后台任务匹配与过期恢复 helper service。"""
from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation background access and launch "
    "workflow chain; this Python helper module is kept only as frozen "
    "rollback/source-map material after the remaining batch orchestration "
    "owner was split into narrower shells."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/services/chapter_access_service.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask


def recover_stale_single_chapter_background_task_if_needed(
    task: "BatchGenerationTask",
) -> bool:
    current_time = datetime.now(timezone.utc).replace(tzinfo=None)
    auto_recovered = False

    if task.status == "running":
        if task.started_at and (current_time - task.started_at) > timedelta(minutes=15):
            task.status = "failed"
            task.error_message = "任务超时（超过15分钟未完成，已自动恢复）"
            task.completed_at = current_time
            auto_recovered = True
    elif task.status == "pending":
        if task.created_at and (current_time - task.created_at) > timedelta(minutes=3):
            task.status = "failed"
            task.error_message = "任务启动超时（超过3分钟未启动，已自动恢复）"
            task.completed_at = current_time
            auto_recovered = True

    return auto_recovered


def single_chapter_background_task_contains_chapter(
    task: "BatchGenerationTask",
    chapter_id: str,
) -> bool:
    chapter_ids = task.chapter_ids or []
    for item in chapter_ids:
        if item == chapter_id:
            return True
        if isinstance(item, dict) and item.get("id") == chapter_id:
            return True
    return False
