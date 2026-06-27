from __future__ import annotations

from typing import Optional

from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from migrator_app.models.analysis_task import AnalysisTask
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project

logger = get_logger(__name__)


def _analysis_task_models():
    from migrator_app.models.analysis_task import AnalysisTask
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project

    return AnalysisTask, Chapter, Project


async def create_analysis_task_safely(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    user_id: str,
    project_id: str,
    log_context: str,
) -> Optional[AnalysisTask]:
    AnalysisTask, Chapter, Project = _analysis_task_models()

    chapter_exists_result = await db_session.execute(
        select(Chapter.id).where(Chapter.id == chapter_id)
    )
    if chapter_exists_result.scalar_one_or_none() is None:
        logger.info(
            "Skip analysis task creation because chapter no longer exists: %s (%s)",
            chapter_id,
            log_context,
        )
        return None

    project_exists_result = await db_session.execute(
        select(Project.id).where(Project.id == project_id)
    )
    if project_exists_result.scalar_one_or_none() is None:
        logger.info(
            "Skip analysis task creation because project no longer exists: %s (%s)",
            project_id,
            log_context,
        )
        return None

    analysis_task = AnalysisTask(
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=project_id,
        status="pending",
        progress=0,
    )
    db_session.add(analysis_task)
    try:
        await db_session.commit()
    except IntegrityError:
        await db_session.rollback()
        logger.info(
            "Skip analysis task creation because chapter/project disappeared during commit: "
            "chapter=%s, project=%s, context=%s",
            chapter_id,
            project_id,
            log_context,
        )
        return None
    await db_session.refresh(analysis_task)
    return analysis_task


