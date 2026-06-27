"""API common helper test support.

Historical Python API helper behavior is kept here for tests only. Production
API traffic is owned by Rust through the strangler gateway.
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Optional

from fastapi import HTTPException, Request

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.project import Project

logger = get_logger(__name__)


async def verify_project_access(
    project_id: str,
    user_id: Optional[str],
    db: AsyncSession,
) -> Project:
    """
    验证用户是否有权访问指定项目。

    这是历史 Python API helper 的测试支撑副本，生产路由已由 Rust 接管。
    """
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录")

    from sqlalchemy import select

    from migrator_app.models.project import Project

    result = await db.execute(
        select(Project).where(
            Project.id == project_id,
            Project.user_id == user_id,
        )
    )
    project = result.scalar_one_or_none()

    if not project:
        logger.warning(f"项目访问被拒绝: project_id={project_id}, user_id={user_id}")
        raise HTTPException(status_code=404, detail="项目不存在或无权访问")

    return project


def get_user_id(request: Request) -> Optional[str]:
    """从请求中获取用户 ID。"""
    return getattr(request.state, "user_id", None)


def raise_auth_service_unavailable_if_needed(request: Request) -> None:
    """在鉴权依赖的数据库不可用时，统一返回 503。"""
    if getattr(request.state, "auth_backend_unavailable", False):
        detail = getattr(
            request.state,
            "auth_backend_unavailable_message",
            "认证服务暂时不可用，请确认 PostgreSQL 已启动后重试",
        )
        raise HTTPException(status_code=503, detail=detail)


def require_request_user(request: Request, detail: str = "需要登录"):
    """统一处理登录校验，并在鉴权后端不可用时优先返回 503。"""
    raise_auth_service_unavailable_if_needed(request)
    if not hasattr(request.state, "user") or not request.state.user:
        raise HTTPException(status_code=401, detail=detail)
    return request.state.user


async def verify_project_access_from_request(
    project_id: str,
    request: Request,
    db: AsyncSession,
) -> Project:
    """从请求中验证项目访问权限。"""
    user_id = get_user_id(request)
    return await verify_project_access(project_id, user_id, db)


