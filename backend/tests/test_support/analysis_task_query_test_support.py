from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Dict, Iterable, List

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

if TYPE_CHECKING:
    from migrator_app.models.analysis_task import AnalysisTask
    from migrator_app.models.chapter import Chapter


BATCH_ANALYSIS_STATUS_CHAPTER_LIMIT = 200


@dataclass(frozen=True)
class BatchAnalysisStatusQueryContext:
    chapters: List[Chapter]
    latest_tasks_by_chapter_id: Dict[str, AnalysisTask]
    response_project_id: str


def _analysis_query_models():
    from migrator_app.models.analysis_task import AnalysisTask
    from migrator_app.models.chapter import Chapter

    return AnalysisTask, Chapter


def normalize_batch_analysis_chapter_ids(
    raw_chapter_ids: Iterable[object] | None,
    *,
    limit: int = BATCH_ANALYSIS_STATUS_CHAPTER_LIMIT,
) -> List[str]:
    chapter_ids: List[str] = []
    seen_chapter_ids: set[str] = set()

    for raw_chapter_id in raw_chapter_ids or []:
        chapter_id = str(raw_chapter_id).strip()
        if not chapter_id or chapter_id in seen_chapter_ids:
            continue
        seen_chapter_ids.add(chapter_id)
        chapter_ids.append(chapter_id)
        if len(chapter_ids) >= max(int(limit or 0), 1):
            break

    return chapter_ids


def build_empty_batch_analysis_status_response() -> Dict[str, object]:
    return {
        "project_id": "",
        "total": 0,
        "items": {},
    }


async def load_batch_analysis_status_query_context(
    db: AsyncSession,
    *,
    chapter_ids: List[str],
) -> BatchAnalysisStatusQueryContext:
    AnalysisTask, Chapter = _analysis_query_models()

    chapter_result = await db.execute(select(Chapter).where(Chapter.id.in_(chapter_ids)))
    chapters = chapter_result.scalars().all()
    chapter_map = {chapter.id: chapter for chapter in chapters}

    latest_tasks_by_chapter_id: Dict[str, AnalysisTask] = {}
    if chapter_map:
        task_result = await db.execute(
            select(AnalysisTask)
            .where(AnalysisTask.chapter_id.in_(list(chapter_map.keys())))
            .order_by(AnalysisTask.chapter_id.asc(), AnalysisTask.created_at.desc())
        )
        for task in task_result.scalars().all():
            if task.chapter_id not in latest_tasks_by_chapter_id:
                latest_tasks_by_chapter_id[task.chapter_id] = task

    return BatchAnalysisStatusQueryContext(
        chapters=chapters,
        latest_tasks_by_chapter_id=latest_tasks_by_chapter_id,
        response_project_id=next(iter({chapter.project_id for chapter in chapters}), ""),
    )


async def load_latest_analysis_task_for_chapter(
    db: AsyncSession,
    *,
    chapter_id: str,
) -> AnalysisTask | None:
    AnalysisTask, _Chapter = _analysis_query_models()

    result = await db.execute(
        select(AnalysisTask)
        .where(AnalysisTask.chapter_id == chapter_id)
        .order_by(AnalysisTask.created_at.desc())
        .limit(1)
    )
    return result.scalar_one_or_none()
