from __future__ import annotations

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.regeneration_task import RegenerationTask


async def load_regeneration_tasks_payload(
    *,
    db_session: AsyncSession,
    chapter_id: str,
    limit: int,
) -> dict[str, object]:
    result = await db_session.execute(
        select(RegenerationTask)
        .where(RegenerationTask.chapter_id == chapter_id)
        .order_by(RegenerationTask.created_at.desc())
        .limit(limit)
    )
    tasks = result.scalars().all()
    return {
        'chapter_id': chapter_id,
        'total': len(tasks),
        'tasks': [
            {
                'task_id': task.id,
                'status': task.status,
                'version_number': task.version_number,
                'version_note': task.version_note,
                'original_word_count': task.original_word_count,
                'regenerated_word_count': task.regenerated_word_count,
                'created_at': task.created_at.isoformat() if task.created_at else None,
                'completed_at': task.completed_at.isoformat() if task.completed_at else None,
            }
            for task in tasks
        ],
    }
