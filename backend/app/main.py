"""FastAPI应用主入口"""
from fastapi import FastAPI, Request, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import JSONResponse, FileResponse
from fastapi.exceptions import RequestValidationError
from contextlib import asynccontextmanager
from datetime import datetime
from pathlib import Path

from app.config import settings as config_settings
from app.database import close_db, _session_stats, check_database_health
from app.logger import setup_logging, get_logger
from app.middleware import RequestIDMiddleware
from app.middleware.auth_middleware import AuthMiddleware
from app.mcp import mcp_client, register_status_sync
from app.services.background_task_manager import background_task_manager

setup_logging(
    level=config_settings.log_level,
    log_to_file=config_settings.log_to_file,
    log_file_path=config_settings.log_file_path,
    max_bytes=config_settings.log_max_bytes,
    backup_count=config_settings.log_backup_count
)
logger = get_logger(__name__)


_startup_status = {
    "ready": False,
    "started_at": None,
    "completed_at": None,
    "steps": {},
}


def _build_default_startup_status() -> dict:
    return {
        "ready": False,
        "started_at": None,
        "completed_at": None,
        "steps": {
            "status_sync": {"healthy": False, "status": "pending", "detail": "waiting"},
            "background_tasks": {"healthy": False, "status": "pending", "detail": "waiting"},
            "database_warmup": {"healthy": False, "status": "pending", "detail": "waiting"},
        },
    }


def _reset_startup_status() -> dict:
    global _startup_status
    _startup_status = _build_default_startup_status()
    _startup_status["started_at"] = datetime.now().isoformat()
    return _startup_status


def _get_startup_status() -> dict:
    if not isinstance(_startup_status, dict) or not _startup_status:
        return _reset_startup_status()
    return _startup_status


def _mark_startup_step(step: str, *, healthy: bool, detail: str | None = None, payload: dict | None = None) -> dict:
    state = _get_startup_status()
    step_state = state["steps"].setdefault(step, {})
    step_state["healthy"] = bool(healthy)
    step_state["status"] = "ok" if healthy else "error"
    if detail:
        step_state["detail"] = detail
    if payload is not None:
        step_state["payload"] = payload
    return state


def _finalize_startup_status() -> dict:
    state = _get_startup_status()
    state["ready"] = all(bool(item.get("healthy")) for item in state["steps"].values())
    state["completed_at"] = datetime.now().isoformat()
    return state


def _set_startup_ready(ready: bool) -> dict:
    state = _get_startup_status()
    state["ready"] = bool(ready)
    if ready:
        state["completed_at"] = datetime.now().isoformat()
    else:
        state["completed_at"] = None
    return state


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan management."""
    _reset_startup_status()

    register_status_sync()
    _mark_startup_step("status_sync", healthy=True, detail="registered")

    await background_task_manager.ensure_loaded()
    _mark_startup_step("background_tasks", healthy=True, detail="loaded")

    database_warmup = await check_database_health(force_refresh=True)
    database_ready = bool(database_warmup.get("healthy"))
    _mark_startup_step(
        "database_warmup",
        healthy=database_ready,
        detail="warmup completed" if database_ready else "warmup failed",
        payload=database_warmup,
    )
    _finalize_startup_status()

    if database_ready:
        logger.info("Application startup completed")
    else:
        logger.warning("Application startup completed, but database warmup is still unhealthy; readyz will stay 503")

    yield

    await mcp_client.cleanup()

    from app.services.ai_service import cleanup_http_clients
    await cleanup_http_clients()

    await close_db()

    logger.info("Application shutdown completed")


app = FastAPI(
    title=config_settings.app_name,
    version=config_settings.app_version,
    description="AI写小说工具 - 智能小说创作助手",
    lifespan=lifespan
)

@app.exception_handler(RequestValidationError)
async def validation_exception_handler(request: Request, exc: RequestValidationError):
    """处理请求验证错误"""
    logger.error(f"请求验证失败: {exc.errors()}")
    return JSONResponse(
        status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
        content={
            "detail": "请求参数验证失败",
            "errors": exc.errors()
        }
    )

@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """处理所有未捕获的异常"""
    logger.error(f"未处理的异常: {type(exc).__name__}: {str(exc)}", exc_info=True)
    return JSONResponse(
        status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
        content={
            "detail": "服务器内部错误",
            "message": str(exc) if config_settings.debug else "请稍后重试"
        }
    )

app.add_middleware(RequestIDMiddleware)
app.add_middleware(AuthMiddleware)

if config_settings.debug:
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
else:
    app.add_middleware(
        CORSMiddleware,
        allow_origins=config_settings.cors_origins,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )


@app.get("/health")
async def health_check():
    """Compatibility health endpoint."""
    return {"status": "ok"}


@app.get("/livez")
async def liveness_check():
    """Liveness probe."""
    return {"status": "ok"}


@app.get("/readyz")
async def readiness_check():
    """Readiness probe with warmup status and database check."""
    startup_status = _get_startup_status()
    database_status = await check_database_health()
    startup_ready = bool(startup_status.get("ready"))
    database_ready = bool(database_status.get("healthy"))
    is_ready = startup_ready and database_ready
    return JSONResponse(
        status_code=status.HTTP_200_OK if is_ready else status.HTTP_503_SERVICE_UNAVAILABLE,
        content={
            "status": "ready" if is_ready else "not_ready",
            "checks": {
                "startup": startup_status,
                "database": database_status,
            },
        },
    )

@app.get("/health/db-sessions")
async def db_session_stats():
    """
    数据库会话统计（监控连接泄漏）
    
    返回：
    - created: 总创建会话数
    - closed: 总关闭会话数
    - active: 当前活跃会话数（应该接近0）
    - errors: 错误次数
    - generator_exits: SSE断开次数
    - last_check: 最后检查时间
    """
    return {
        "status": "ok",
        "session_stats": _session_stats,
        "warning": "活跃会话数过多" if _session_stats["active"] > 10 else None
    }


from app.api import (
    projects, outlines, characters, chapters,
    wizard_stream, relationships, organizations,
    auth, users, settings, writing_styles, memories,
    mcp_plugins, admin, inspiration, prompt_templates,
    changelog, careers, foreshadows, prompt_workshop,
    background_tasks, book_import
)

app.include_router(auth.router, prefix="/api")
app.include_router(users.router, prefix="/api")
app.include_router(settings.router, prefix="/api")
app.include_router(admin.router, prefix="/api")

app.include_router(projects.router, prefix="/api")
app.include_router(wizard_stream.router, prefix="/api")
app.include_router(inspiration.router, prefix="/api")
app.include_router(outlines.router, prefix="/api")
app.include_router(characters.router, prefix="/api")
app.include_router(careers.router, prefix="/api")  # 职业管理API
app.include_router(chapters.router, prefix="/api")
app.include_router(relationships.router, prefix="/api")
app.include_router(organizations.router, prefix="/api")
app.include_router(writing_styles.router, prefix="/api")
app.include_router(memories.router)  # 记忆管理API (已包含/api前缀)
app.include_router(foreshadows.router)  # 伏笔管理API (已包含/api前缀)
app.include_router(mcp_plugins.router, prefix="/api")  # MCP插件管理API
app.include_router(prompt_templates.router, prefix="/api")  # 提示词模板管理API
app.include_router(changelog.router, prefix="/api")  # 更新日志API
app.include_router(prompt_workshop.router, prefix="/api")  # 提示词工坊API
app.include_router(background_tasks.router, prefix="/api")  # Background task API
app.include_router(book_import.router, prefix="/api")  # 拆书导入API

static_dir = Path(__file__).parent.parent / "static"
if static_dir.exists():
    app.mount("/assets", StaticFiles(directory=str(static_dir / "assets")), name="assets")
    
    @app.get("/{full_path:path}")
    async def serve_spa(full_path: str):
        """服务单页应用，所有非API路径返回index.html"""
        if full_path.startswith("api/"):
            return JSONResponse(
                status_code=404,
                content={"detail": "API路径不存在"}
            )
        
        file_path = static_dir / full_path
        if file_path.is_file():
            return FileResponse(file_path)
        
        index_file = static_dir / "index.html"
        if index_file.exists():
            return FileResponse(index_file)
        
        return JSONResponse(
            status_code=404,
            content={"detail": "页面不存在"}
        )
else:
    logger.warning("静态文件目录不存在，请先构建前端: cd frontend && npm run build")
    
    @app.get("/")
    async def root():
        return {
            "message": f"欢迎使用{config_settings.app_name}",
            "version": config_settings.app_version,
            "docs": "/docs",
            "notice": "请先构建前端: cd frontend && npm run build"
        }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "app.main:app",
        host=config_settings.app_host,
        port=config_settings.app_port,
        reload=config_settings.debug
    )
