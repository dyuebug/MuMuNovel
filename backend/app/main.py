"""FastAPI应用主入口"""

from app.bootstrap.app_factory import create_app
from app.config import settings as config_settings

app = create_app()

if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "app.main:app",
        host=config_settings.app_host,
        port=config_settings.app_port,
        reload=config_settings.debug,
    )
