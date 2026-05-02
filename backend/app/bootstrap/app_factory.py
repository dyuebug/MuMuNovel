from __future__ import annotations

from fastapi import FastAPI, Request, status
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from sqlalchemy.exc import OperationalError

from app.config import settings as config_settings
from app.logger import get_logger, setup_logging
from app.middleware import RequestIDMiddleware
from app.middleware.auth_middleware import AuthMiddleware

from app.bootstrap.lifespan import (
    create_lifespan,
    recover_startup_status_if_possible,
)
from app.bootstrap.router_registry import register_api_routers
from app.bootstrap.static_assets import register_static_assets


def create_app(*, startup_state: dict | None = None, register_health_routes: bool = True) -> FastAPI:
    setup_logging(
        level=config_settings.log_level,
        log_to_file=config_settings.log_to_file,
        log_file_path=config_settings.log_file_path,
        max_bytes=config_settings.log_max_bytes,
        backup_count=config_settings.log_backup_count,
    )

    logger = get_logger(__name__)

    if startup_state is None:
        startup_state = {}

    app = FastAPI(
        title=config_settings.app_name,
        version=config_settings.app_version,
        description="AI写小说工具 - 智能小说创作助手",
        lifespan=create_lifespan(startup_state),
    )

    @app.exception_handler(RequestValidationError)
    async def validation_exception_handler(request: Request, exc: RequestValidationError):
        logger.error(f"请求验证失败: {exc.errors()}")
        return JSONResponse(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            content={"detail": "请求参数验证失败", "errors": exc.errors()},
        )

    @app.exception_handler(Exception)
    async def global_exception_handler(request: Request, exc: Exception):
        logger.error(f"未处理异常: {type(exc).__name__}: {str(exc)}", exc_info=True)

        if isinstance(exc, (ConnectionRefusedError, ConnectionError, TimeoutError, OperationalError)):
            return JSONResponse(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                content={
                    "detail": "服务暂时不可用，请确认 PostgreSQL 已启动后重试",
                    "message": str(exc) if config_settings.debug else "服务暂不可用",
                },
            )

        return JSONResponse(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            content={
                "detail": "服务内部异常",
                "message": str(exc) if config_settings.debug else "服务内部异常",
            },
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

    if register_health_routes:
        from app.database import _session_stats, check_database_health

        @app.get("/health")
        async def health_check():
            return {"status": "ok"}

        @app.get("/livez")
        async def liveness_check():
            return {"status": "ok"}

        @app.get("/readyz")
        async def readiness_check():
            database_status = await check_database_health()
            startup_status = recover_startup_status_if_possible(startup_state, database_status)
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
            return {
                "status": "ok",
                "session_stats": _session_stats,
                "warning": "活跃会话数过多" if _session_stats["active"] > 10 else None,
            }

    register_api_routers(app)
    register_static_assets(app)

    return app
