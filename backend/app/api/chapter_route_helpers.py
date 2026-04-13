"""章节路由共享辅助函数。"""

from fastapi import HTTPException, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.common import verify_project_access
from app.models.chapter import Chapter


def require_authenticated_user_id(request: Request) -> str:
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")
    return str(user_id)


async def load_accessible_chapter_or_404(
    *,
    db: AsyncSession,
    chapter_id: str,
    user_id: str,
) -> Chapter:
    chapter_result = await db.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    chapter = chapter_result.scalar_one_or_none()
    if chapter is None:
        raise HTTPException(status_code=404, detail="章节不存在")

    await verify_project_access(chapter.project_id, user_id, db)
    return chapter
