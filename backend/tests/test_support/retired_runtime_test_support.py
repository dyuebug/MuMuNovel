"""Test-only support for retired Python runtime config and logging contracts."""

from __future__ import annotations

import json
import logging
import os
import sys
import uuid
from pathlib import Path
from typing import Annotated, Optional
from urllib.parse import urlparse

from dotenv import load_dotenv
from pydantic import field_validator
from pydantic_settings import BaseSettings, NoDecode
from logging.handlers import RotatingFileHandler

PROJECT_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = PROJECT_ROOT.parent
DATA_DIR = PROJECT_ROOT / "data"
DATA_DIR.mkdir(exist_ok=True)

load_dotenv(REPO_ROOT / ".env", override=False)
load_dotenv(PROJECT_ROOT / ".env", override=False)

_logging_configured = False


def _build_default_database_url() -> str:
    postgres_user = os.getenv("POSTGRES_USER", "mumuai")
    postgres_password = os.getenv("POSTGRES_PASSWORD", "password")
    postgres_host = os.getenv("POSTGRES_HOST", os.getenv("DB_HOST", "localhost"))
    postgres_port = os.getenv("POSTGRES_PORT", os.getenv("DB_PORT", "5432"))
    postgres_db = os.getenv("POSTGRES_DB", "mumuai_novel")
    return f"postgresql+asyncpg://{postgres_user}:{postgres_password}@{postgres_host}:{postgres_port}/{postgres_db}"


DATABASE_URL = os.getenv("DATABASE_URL") or _build_default_database_url()


class Settings(BaseSettings):
    """Application settings used by test support."""

    app_name: str = "MuMuNovel"
    app_version: str = "1.3.9"
    app_host: str = "0.0.0.0"
    app_port: int = 8005
    debug: bool = True

    log_level: str = "INFO"
    log_to_file: bool = True
    log_file_path: str = str(PROJECT_ROOT / "logs" / "app.log")
    log_max_bytes: int = 10 * 1024 * 1024
    log_backup_count: int = 30

    cors_origins: Annotated[list[str], NoDecode] = [
        "http://localhost:8000",
        "http://127.0.0.1:8000",
    ]

    database_url: str = DATABASE_URL
    database_pool_size: int = 50
    database_max_overflow: int = 30
    database_pool_timeout: int = 90
    database_pool_recycle: int = 1800
    database_pool_pre_ping: bool = True
    database_pool_use_lifo: bool = True
    database_echo_pool: bool = False
    database_pool_reset_on_return: str = "rollback"
    database_max_identifier_length: int = 128
    database_session_max_active: int = 50
    database_session_leak_threshold: int = 100
    database_enable_slow_query_log: bool = True
    database_slow_query_threshold: float = 1.0
    database_enable_metrics: bool = True
    database_health_cache_ttl_seconds: float = 3.0
    database_health_timeout_seconds: float = 2.5

    openai_api_key: Optional[str] = None
    openai_base_url: Optional[str] = None
    gemini_api_key: Optional[str] = None
    gemini_base_url: Optional[str] = None
    anthropic_api_key: Optional[str] = None
    anthropic_base_url: Optional[str] = None
    default_ai_provider: str = "openai"
    default_model: str = "gpt-4"
    default_temperature: float = 0.7
    default_max_tokens: int = 32000

    mcp_max_rounds: int = 3

    pre_generation_web_research_enabled: bool = False
    pre_generation_web_research_skill_repo_path: str = str(PROJECT_ROOT.parent.parent / "openclaw-dae-skills")
    pre_generation_web_research_timeout_seconds: int = 90
    pre_generation_web_research_max_assets: int = 4
    pre_generation_web_research_exa_enabled: bool = True
    pre_generation_web_research_grok_enabled: bool = True
    pre_generation_web_research_grok_search_enabled: bool = False

    LINUXDO_CLIENT_ID: Optional[str] = None
    LINUXDO_CLIENT_SECRET: Optional[str] = None
    LINUXDO_REDIRECT_URI: Optional[str] = None

    FRONTEND_URL: str = "http://localhost:8005"
    INITIAL_ADMIN_LINUXDO_ID: Optional[str] = None

    LOCAL_AUTH_ENABLED: bool = True
    LOCAL_AUTH_USERNAME: Optional[str] = None
    LOCAL_AUTH_PASSWORD: Optional[str] = None
    LOCAL_AUTH_DISPLAY_NAME: str = "本地用户"

    SESSION_EXPIRE_MINUTES: int = 120
    SESSION_REFRESH_THRESHOLD_MINUTES: int = 30

    WORKSHOP_MODE: str = "client"
    WORKSHOP_CLOUD_URL: str = "https://mumuverse.space:1566"
    WORKSHOP_API_TIMEOUT: int = 30
    WORKSHOP_PROXY_SHARED_SECRET: Optional[str] = None

    class Config:
        env_file = ".env"
        case_sensitive = False
        extra = "ignore"

    @field_validator("cors_origins", mode="before")
    @classmethod
    def _parse_cors_origins(cls, value: object) -> list[str] | object:
        if isinstance(value, list):
            return value
        if not isinstance(value, str):
            return value

        raw = value.strip()
        if not raw:
            return []
        if raw.startswith("["):
            parsed = json.loads(raw)
            if isinstance(parsed, list):
                return [str(item).strip() for item in parsed if str(item).strip()]
            raise ValueError("CORS_ORIGINS JSON 格式必须是数组")

        return [item.strip() for item in raw.split(",") if item.strip()]

    @staticmethod
    def _normalize_origin(origin: Optional[str]) -> Optional[str]:
        if not origin:
            return None
        normalized = origin.strip().rstrip("/")
        return normalized or None

    @staticmethod
    def _is_local_origin(origin: Optional[str]) -> bool:
        normalized = Settings._normalize_origin(origin)
        if not normalized:
            return False
        parsed = urlparse(normalized)
        return parsed.hostname in {"localhost", "127.0.0.1", "0.0.0.0"}

    def get_effective_cors_origins(self) -> list[str]:
        origins = {
            origin
            for origin in (
                self._normalize_origin(item)
                for item in self.cors_origins
            )
            if origin
        }

        frontend_origin = self._normalize_origin(self.FRONTEND_URL)
        if frontend_origin:
            origins.add(frontend_origin)

        if self.debug or self._is_local_origin(frontend_origin):
            for host in ("localhost", "127.0.0.1"):
                for port in (4173, 5173, 8000, 8003, 8005):
                    origins.add(f"http://{host}:{port}")
            origins.add("null")

        return sorted(origins)


settings = Settings()


def _configure_third_party_loggers() -> None:
    logging.getLogger("sqlalchemy.engine").setLevel(logging.WARNING)
    logging.getLogger("sqlalchemy.pool").setLevel(logging.WARNING)
    logging.getLogger("sqlalchemy.dialects").setLevel(logging.WARNING)
    logging.getLogger("sqlalchemy.orm").setLevel(logging.WARNING)
    logging.getLogger("aiosqlite").setLevel(logging.WARNING)
    logging.getLogger("watchfiles").setLevel(logging.WARNING)
    logging.getLogger("httpx").setLevel(logging.WARNING)
    logging.getLogger("httpcore").setLevel(logging.WARNING)
    logging.getLogger("openai").setLevel(logging.WARNING)
    logging.getLogger("anthropic").setLevel(logging.WARNING)
    logging.getLogger("tests.test_support.ai_gateway.ai_service").setLevel(logging.WARNING)
    logging.getLogger("app.api.wizard").setLevel(logging.WARNING)


class UvicornFormatter(logging.Formatter):
    COLORS = {
        "DEBUG": "\033[36m",
        "INFO": "\033[32m",
        "WARNING": "\033[33m",
        "ERROR": "\033[31m",
        "CRITICAL": "\033[35m",
    }
    RESET = "\033[0m"

    def __init__(self, use_colors: bool = True):
        super().__init__()
        self.use_colors = use_colors

    def format(self, record):
        levelname = record.levelname
        if self.use_colors and sys.stderr.isatty():
            colored_level = f"{self.COLORS.get(levelname, '')}{levelname}{self.RESET}"
        else:
            colored_level = levelname

        request_id = getattr(record, "request_id", None)
        request_id_str = f" [{request_id}]" if request_id else ""
        return f"{colored_level}:     {record.name}{request_id_str} - {record.getMessage()}"


def setup_logging(
    level: str = "INFO",
    log_to_file: bool = False,
    log_file_path: Optional[str] = None,
    max_bytes: int = 10 * 1024 * 1024,
    backup_count: int = 30,
):
    global _logging_configured
    if _logging_configured:
        return logging.getLogger()

    root_logger = logging.getLogger()
    root_logger.setLevel(getattr(logging, level.upper()))
    root_logger.handlers.clear()

    console_handler = logging.StreamHandler(sys.stderr)
    console_handler.setLevel(getattr(logging, level.upper()))
    console_handler.setFormatter(UvicornFormatter(use_colors=True))
    root_logger.addHandler(console_handler)

    if log_to_file and log_file_path:
        log_file = Path(log_file_path)
        log_file.parent.mkdir(parents=True, exist_ok=True)
        file_handler = RotatingFileHandler(
            filename=log_file_path,
            maxBytes=max_bytes,
            backupCount=backup_count,
            encoding="utf-8",
        )
        file_handler.setLevel(getattr(logging, level.upper()))
        file_handler.setFormatter(UvicornFormatter(use_colors=False))
        root_logger.addHandler(file_handler)
        root_logger.info("日志文件输出已启用: %s", log_file_path)
        root_logger.info(
            "日志轮转配置: 单文件最大%.1fMB, 保留%s个备份",
            max_bytes / 1024 / 1024,
            backup_count,
        )

    _configure_third_party_loggers()
    _logging_configured = True
    return root_logger


def get_logger(name: str) -> logging.Logger:
    return logging.getLogger(name)


def get_or_create_instance_id() -> str:
    if settings.WORKSHOP_MODE.lower() == "server":
        return "server"

    instance_file = PROJECT_ROOT / ".instance_id"
    if instance_file.exists():
        with open(instance_file, "r", encoding="utf-8") as f:
            instance_id = f.read().strip()
            if instance_id and instance_id != "server":
                return instance_id

    instance_id = str(uuid.uuid4())[:12]
    try:
        with open(instance_file, "w", encoding="utf-8") as f:
            f.write(instance_id)
    except Exception:
        pass
    return instance_id


INSTANCE_ID = get_or_create_instance_id()


def is_workshop_server() -> bool:
    return settings.WORKSHOP_MODE.lower() == "server"
