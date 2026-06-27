from __future__ import annotations

from pathlib import Path

from fastapi import FastAPI
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles


def register_static_assets(app: FastAPI) -> None:
    from tests.test_support.retired_runtime_test_support import settings as config_settings
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)

    static_dir = Path(__file__).parents[3] / "static"
    resolved_static_dir = static_dir.resolve() if static_dir.exists() else static_dir

    if static_dir.exists():
        app.mount("/assets", StaticFiles(directory=str(static_dir / "assets")), name="assets")

        # SPA catch-all: only handles non-API paths.
        # API routes are registered *before* this catch-all and take priority,
        # but FastAPI/Starlette will also match {full_path:path} for API paths.
        # The guard below ensures unmatched API paths fall through to the
        # default 404 handler instead of being treated as SPA routes.
        @app.get("/{full_path:path}")
        async def serve_spa(full_path: str):
            if full_path.startswith("api/"):
                from fastapi import HTTPException
                raise HTTPException(status_code=404, detail="Not Found")

            requested_path = (resolved_static_dir / full_path).resolve(strict=False)
            if not requested_path.is_relative_to(resolved_static_dir):
                return JSONResponse(status_code=404, content={"detail": "资源不存在"})

            if requested_path.is_file():
                return FileResponse(requested_path)

            index_file = resolved_static_dir / "index.html"
            if index_file.exists():
                return FileResponse(index_file)

            return JSONResponse(status_code=404, content={"detail": "页面不存在"})

        return

    logger.warning("静态文件目录不存在，请先构建前端: cd frontend && npm run build")

    @app.get("/")
    async def root():
        return {
            "message": f"欢迎使用{config_settings.app_name}",
            "version": config_settings.app_version,
            "docs": "/docs",
            "notice": "请先构建前端: cd frontend && npm run build",
        }


