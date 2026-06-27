from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import TYPE_CHECKING, Any, Dict, List, Mapping, Optional

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from migrator_app.models.analysis_task import AnalysisTask

logger = get_logger(__name__)


@dataclass(frozen=True)
class AnalysisTaskStatusBuildResult:
    payload: Dict[str, Any]
    changed: bool


@dataclass(frozen=True)
class BatchAnalysisStatusItemsBuildResult:
    items: Dict[str, Dict[str, Any]]
    changed: bool


def build_empty_analysis_task_status(chapter_id: str) -> Dict[str, Any]:
    return {
        "has_task": False,
        "chapter_id": chapter_id,
        "status": "none",
        "progress": 0,
        "error_message": None,
        "auto_recovered": False,
        "task_id": None,
        "created_at": None,
        "started_at": None,
        "completed_at": None,
    }


def classify_analysis_error_code(error_message: Optional[str]) -> Optional[str]:
    if not error_message:
        return None

    if "正在重试(" in error_message:
        return "retrying"
    if "JSON解析失败" in error_message or "AI返回格式异常" in error_message:
        return "json_parse_failed"
    if "AI响应为空或过短" in error_message:
        return "ai_empty"
    if "流式响应中断" in error_message or "流式生成出错" in error_message:
        return "stream_interrupted"
    if "任务超时" in error_message or "启动超时" in error_message:
        return "timeout"
    if "章节不存在或内容为空" in error_message:
        return "chapter_empty"
    if "项目不存在" in error_message:
        return "project_missing"
    return "unknown"


def recover_analysis_task_if_needed(task: AnalysisTask) -> bool:
    current_time = datetime.now()
    auto_recovered = False

    if task.status == "running":
        is_retrying = bool(task.error_message and "重试" in task.error_message)
        timeout_minutes = 15 if is_retrying else 10

        if task.started_at and (current_time - task.started_at) > timedelta(minutes=timeout_minutes):
            task.status = "failed"
            task.error_message = f"任务超时（超过{timeout_minutes}分钟未完成，已自动恢复）"
            task.completed_at = current_time
            task.progress = 0
            auto_recovered = True
            logger.warning(
                "Auto recovered stale running analysis task: %s, chapter=%s",
                task.id,
                task.chapter_id,
            )

    elif task.status == "pending":
        if task.created_at and (current_time - task.created_at) > timedelta(minutes=3):
            task.status = "failed"
            task.error_message = "任务启动超时（超过3分钟未启动，已自动恢复）"
            task.completed_at = current_time
            task.progress = 0
            auto_recovered = True
            logger.warning(
                "Auto recovered stale pending analysis task: %s, chapter=%s",
                task.id,
                task.chapter_id,
            )

    return auto_recovered


def serialize_analysis_task_status(
    chapter_id: str,
    task: Optional[AnalysisTask],
    *,
    auto_recovered: bool = False,
) -> Dict[str, Any]:
    if not task:
        return build_empty_analysis_task_status(chapter_id)

    return {
        "has_task": True,
        "task_id": task.id,
        "chapter_id": task.chapter_id,
        "status": task.status,
        "progress": task.progress,
        "error_message": task.error_message,
        "error_code": classify_analysis_error_code(task.error_message),
        "auto_recovered": auto_recovered,
        "created_at": task.created_at.isoformat() if task.created_at else None,
        "started_at": task.started_at.isoformat() if task.started_at else None,
        "completed_at": task.completed_at.isoformat() if task.completed_at else None,
    }


def build_analysis_task_status_payload(
    chapter_id: str,
    task: Optional[AnalysisTask],
) -> AnalysisTaskStatusBuildResult:
    if not task:
        return AnalysisTaskStatusBuildResult(
            payload=build_empty_analysis_task_status(chapter_id),
            changed=False,
        )

    auto_recovered = recover_analysis_task_if_needed(task)
    return AnalysisTaskStatusBuildResult(
        payload=serialize_analysis_task_status(
            chapter_id,
            task,
            auto_recovered=auto_recovered,
        ),
        changed=auto_recovered,
    )


def build_batch_analysis_status_items(
    chapter_ids: List[str],
    *,
    latest_tasks_by_chapter_id: Mapping[str, AnalysisTask],
) -> BatchAnalysisStatusItemsBuildResult:
    items: Dict[str, Dict[str, Any]] = {}
    changed = False

    for chapter_id in chapter_ids:
        status_result = build_analysis_task_status_payload(
            chapter_id,
            latest_tasks_by_chapter_id.get(chapter_id),
        )
        changed = changed or status_result.changed
        items[chapter_id] = status_result.payload

    return BatchAnalysisStatusItemsBuildResult(items=items, changed=changed)


