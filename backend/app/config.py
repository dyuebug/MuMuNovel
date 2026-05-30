"""应用配置管理"""
import json
from typing import Annotated, Optional

from pydantic import field_validator
from pydantic_settings import BaseSettings, NoDecode
from dotenv import load_dotenv
from pathlib import Path
import logging
import os
import uuid
from urllib.parse import urlparse

# 基于 backend 目录解析路径
PROJECT_ROOT = Path(__file__).parent.parent
REPO_ROOT = PROJECT_ROOT.parent
DATA_DIR = PROJECT_ROOT / "data"
DATA_DIR.mkdir(exist_ok=True)

# 优先加载仓库根目录与 backend 目录的 .env
load_dotenv(REPO_ROOT / ".env", override=False)
load_dotenv(PROJECT_ROOT / ".env", override=False)

# 配置模块使用标准logging（在logger.py初始化之前）
config_logger = logging.getLogger(__name__)

# 默认使用 PostgreSQL
# 若未显式配置 DATABASE_URL，则回退为 POSTGRES_* 环境变量组装
def _build_default_database_url() -> str:
    postgres_user = os.getenv("POSTGRES_USER", "mumuai")
    postgres_password = os.getenv("POSTGRES_PASSWORD", "password")
    postgres_host = os.getenv("POSTGRES_HOST", os.getenv("DB_HOST", "localhost"))
    postgres_port = os.getenv("POSTGRES_PORT", os.getenv("DB_PORT", "5432"))
    postgres_db = os.getenv("POSTGRES_DB", "mumuai_novel")
    return f"postgresql+asyncpg://{postgres_user}:{postgres_password}@{postgres_host}:{postgres_port}/{postgres_db}"

DATABASE_URL = os.getenv("DATABASE_URL") or _build_default_database_url()

config_logger.debug(f"数据库类型: {'SQLite' if 'sqlite' in DATABASE_URL.lower() else 'PostgreSQL'}")
config_logger.debug(f"数据库 URL: {DATABASE_URL}")

class Settings(BaseSettings):
    """应用配置"""
    
    # 应用配置
    app_name: str = "MuMuNovel"
    app_version: str = "1.3.9"
    app_host: str = "0.0.0.0"
    app_port: int = 8000
    debug: bool = True
    
    # 日志配置
    log_level: str = "INFO"  # DEBUG, INFO, WARNING, ERROR, CRITICAL
    log_to_file: bool = True  # 是否输出到文件
    log_file_path: str = str(PROJECT_ROOT / "logs" / "app.log")
    log_max_bytes: int = 10 * 1024 * 1024  # 10MB
    log_backup_count: int = 30  # 保留30个备份文件
    
    # CORS配置
    cors_origins: Annotated[list[str], NoDecode] = [
        "http://localhost:8000",
        "http://127.0.0.1:8000",
    ]
    
    # 数据库配置 - PostgreSQL
    database_url: str = DATABASE_URL
    
    # PostgreSQL连接池配置（优化后支持150-200并发用户）
    database_pool_size: int = 50  # 核心连接池大小（优化：从30提升到50）
    database_max_overflow: int = 30  # 最大溢出连接数（优化：从20提升到30）
    database_pool_timeout: int = 90  # 连接池超时秒数（优化：从60提升到90）
    database_pool_recycle: int = 1800  # 连接回收时间秒数（30分钟，防止长时间连接失效）
    database_pool_pre_ping: bool = True  # 连接前ping检测，确保连接有效
    database_pool_use_lifo: bool = True  # 使用LIFO策略提高连接复用率
    
    # 连接池高级配置
    database_echo_pool: bool = False  # 是否记录连接池日志（调试用）
    database_pool_reset_on_return: str = "rollback"  # 连接归还时的重置策略：rollback/commit/none
    database_max_identifier_length: int = 128  # PostgreSQL标识符最大长度
    
    # 会话监控配置
    database_session_max_active: int = 50  # 活跃会话警告阈值（从100降低到50）
    database_session_leak_threshold: int = 100  # 会话泄漏严重告警阈值
    
    # 数据库监控配置
    database_enable_slow_query_log: bool = True  # 启用慢查询日志
    database_slow_query_threshold: float = 1.0  # 慢查询阈值（秒）
    database_enable_metrics: bool = True  # 启用性能指标收集
    database_health_cache_ttl_seconds: float = 3.0  # Health probe cache TTL to reduce readyz jitter
    database_health_timeout_seconds: float = 2.5  # Health probe timeout to avoid blocking readyz
    
    # AI服务配置
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
    
    # MCP配置
    mcp_max_rounds: int = 3  # MCP工具调用最大轮数（全局统一控制）

    # 生成前外部检索配置
    pre_generation_web_research_enabled: bool = False
    pre_generation_web_research_skill_repo_path: str = str(PROJECT_ROOT.parent.parent / "openclaw-dae-skills")
    pre_generation_web_research_timeout_seconds: int = 90
    pre_generation_web_research_max_assets: int = 4
    pre_generation_web_research_exa_enabled: bool = True
    pre_generation_web_research_grok_enabled: bool = True
    pre_generation_web_research_grok_search_enabled: bool = False
    
    # LinuxDO OAuth2 配置
    LINUXDO_CLIENT_ID: Optional[str] = None
    LINUXDO_CLIENT_SECRET: Optional[str] = None
    # 回调地址：Docker部署时必须使用实际域名或服务器IP，不能使用localhost
    # 本地开发: http://localhost:8000/api/auth/callback
    # 生产环境: https://your-domain.com/api/auth/callback 或 http://your-ip:8000/api/auth/callback
    LINUXDO_REDIRECT_URI: Optional[str] = None
    
    # 前端URL配置（用于OAuth回调后重定向）
    # 本地开发: http://localhost:8000
    # 生产环境: https://your-domain.com 或 http://your-ip:8000
    FRONTEND_URL: str = "http://localhost:8000"
    
    # 初始管理员配置（LinuxDO user_id）
    INITIAL_ADMIN_LINUXDO_ID: Optional[str] = None
    
    # 本地账户登录配置
    LOCAL_AUTH_ENABLED: bool = True  # 是否启用本地账户登录
    LOCAL_AUTH_USERNAME: Optional[str] = None  # 本地登录用户名
    LOCAL_AUTH_PASSWORD: Optional[str] = None  # 本地登录密码
    LOCAL_AUTH_DISPLAY_NAME: str = "本地用户"  # 本地用户显示名称
    
    # 会话配置
    SESSION_EXPIRE_MINUTES: int = 120  # 会话过期时间（分钟），默认2小时
    SESSION_REFRESH_THRESHOLD_MINUTES: int = 30  # 会话刷新阈值（分钟），剩余时间少于此值时可刷新
    
    # 提示词工坊配置
    WORKSHOP_MODE: str = "client"  # client: 本地部署实例, server: 云端中央服务器
    WORKSHOP_CLOUD_URL: str = "https://mumuverse.space:1566"  # 云端服务地址
    WORKSHOP_API_TIMEOUT: int = 30  # 云端API请求超时时间（秒）
    WORKSHOP_PROXY_SHARED_SECRET: Optional[str] = None  # 代理请求共享密钥，服务端校验后才信任代理用户身份
    
    class Config:
        env_file = ".env"
        case_sensitive = False
        extra = "ignore"  # 忽略未定义的环境变量，避免验证错误

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


# 创建全局配置实例
settings = Settings()
config_logger.info(f"配置加载完成: {settings.app_name} v{settings.app_version}")
config_logger.debug(f"调试模式: {settings.debug}")
config_logger.debug(f"AI提供商: {settings.default_ai_provider}")


# ==================== 提示词工坊实例标识 ====================

def get_or_create_instance_id() -> str:
    """获取或创建实例唯一标识
    
    - Server 模式：固定使用 "server" 作为标识，确保与所有 Client 实例区分
    - Client 模式：从 .instance_id 文件读取或自动生成唯一标识
    """
    # Server 模式使用固定标识
    if settings.WORKSHOP_MODE.lower() == "server":
        config_logger.info("Server 模式：使用固定实例标识 'server'")
        return "server"
    
    # Client 模式：从文件读取或生成
    instance_file = PROJECT_ROOT / ".instance_id"
    if instance_file.exists():
        with open(instance_file, 'r') as f:
            instance_id = f.read().strip()
            if instance_id and instance_id != "server":  # 确保不与 server 冲突
                return instance_id
    
    # 生成新的实例ID
    instance_id = str(uuid.uuid4())[:12]
    try:
        with open(instance_file, 'w') as f:
            f.write(instance_id)
        config_logger.info(f"生成新的实例标识: {instance_id}")
    except Exception as e:
        config_logger.warning(f"无法保存实例标识到文件: {e}")
    
    return instance_id

INSTANCE_ID = get_or_create_instance_id()

def is_workshop_server() -> bool:
    """判断当前实例是否为工坊服务端"""
    return settings.WORKSHOP_MODE.lower() == "server"

config_logger.info(f"提示词工坊模式: {settings.WORKSHOP_MODE}, 实例ID: {INSTANCE_ID}")
