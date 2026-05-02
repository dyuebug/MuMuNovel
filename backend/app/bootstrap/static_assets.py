from __future__ import annotations

from pathlib import Path

from fastapi import FastAPI
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles


def register_static_assets(app: FastAPI) -> None:
    from app.config import settings as config_settings
    from app.logger import get_logger

    logger = get_logger(__name__)

    static_dir = Path(__file__).parent.parent.parent / "static"
    resolved_static_dir = static_dir.resolve() if static_dir.exists() else static_dir

    if static_dir.exists():
        app.mount("/assets", StaticFiles(directory=str(static_dir / "assets")), name="assets")

        @app.get("/{full_path:path}")
        async def serve_spa(full_path: str):
            if full_path.startswith("api/"):
                return JSONResponse(status_code=404, content={"detail": "API路径不存在"})

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
