from __future__ import annotations

from typing import Optional

from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.analysis_task import AnalysisTask
from app.models.chapter import Chapter
from app.models.project import Project

logger = get_logger(__name__)


async def create_analysis_task_safely(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    user_id: str,
    project_id: str,
    log_context: str,
) -> Optional[AnalysisTask]:
    chapter_exists_result = await db_session.execute(
        select(Chapter.id).where(Chapter.id == chapter_id)
    )
    if chapter_exists_result.scalar_one_or_none() is None:
        logger.info(f"Skip analysis task creation because chapter no longer exists: {chapter_id} ({log_context})")
        return None

    project_exists_result = await db_session.execute(
        select(Project.id).where(Project.id == project_id)
    )
    if project_exists_result.scalar_one_or_none() is None:
        logger.info(f"Skip analysis task creation because project no longer exists: {project_id} ({log_context})")
        return None

    analysis_task = AnalysisTask(
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=project_id,
        status='pending',
        progress=0,
    )
    db_session.add(analysis_task)
    try:
        await db_session.commit()
    except IntegrityError:
        await db_session.rollback()
        logger.info(
            f"Skip analysis task creation because chapter/project disappeared during commit: "
            f"chapter={chapter_id}, project={project_id}, context={log_context}"
        )
        return None
    await db_session.refresh(analysis_task)
    return analysis_task
