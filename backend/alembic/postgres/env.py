"""Alembic 环境配置文件 - PostgreSQL"""
import asyncio
import os
import sys
from logging.config import fileConfig
from pathlib import Path

from sqlalchemy import pool
from sqlalchemy.engine import Connection
from sqlalchemy.ext.asyncio import async_engine_from_config

from alembic import context
from alembic.ddl.impl import DefaultImpl
from dotenv import load_dotenv

from tools.alembic_versioning import (
    ensure_version_table_column_capacity,
    patch_default_impl_version_table,
)

# 导入 Base 和所有模型
backend_root = Path(__file__).resolve().parents[2]
repo_root = backend_root.parent
for dotenv_path in (repo_root / ".env", backend_root / ".env"):
    load_dotenv(dotenv_path, override=False)

if str(backend_root) not in sys.path:
    sys.path.insert(0, str(backend_root))

from migrator_app.models import Base, load_all_models

load_all_models()


def _build_default_database_url() -> str:
    postgres_user = os.getenv("POSTGRES_USER", "mumuai")
    postgres_password = os.getenv("POSTGRES_PASSWORD", "password")
    postgres_host = os.getenv("POSTGRES_HOST", os.getenv("DB_HOST", "localhost"))
    postgres_port = os.getenv("POSTGRES_PORT", os.getenv("DB_PORT", "5432"))
    postgres_db = os.getenv("POSTGRES_DB", "mumuai_novel")
    return f"postgresql+asyncpg://{postgres_user}:{postgres_password}@{postgres_host}:{postgres_port}/{postgres_db}"


DATABASE_URL = os.getenv("DATABASE_URL") or _build_default_database_url()

# Alembic Config 对象
config = context.config

# 设置数据库连接字符串（从环境变量读取）
config.set_main_option("sqlalchemy.url", DATABASE_URL)

# 配置日志
if config.config_file_name is not None:
    fileConfig(config.config_file_name)

# 设置 target_metadata 为应用的 Base.metadata
target_metadata = Base.metadata
patch_default_impl_version_table(DefaultImpl)


def run_migrations_offline() -> None:
    """在'离线'模式下运行迁移"""
    url = config.get_main_option("sqlalchemy.url")
    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
        compare_type=True,
        compare_server_default=True,
    )

    with context.begin_transaction():
        context.run_migrations()


def do_run_migrations(connection: Connection) -> None:
    ensure_version_table_column_capacity(connection)
    """执行迁移的核心函数 - PostgreSQL 专用"""
    context.configure(
        connection=connection,
        target_metadata=target_metadata,
        compare_type=True,
        compare_server_default=True,
        render_as_batch=False,  # PostgreSQL 不需要批处理模式
    )

    with context.begin_transaction():
        context.run_migrations()


async def run_async_migrations() -> None:
    """在'在线'模式下运行异步迁移"""
    configuration = config.get_section(config.config_ini_section, {})
    configuration["sqlalchemy.url"] = DATABASE_URL
    
    connectable = async_engine_from_config(
        configuration,
        prefix="sqlalchemy.",
        poolclass=pool.NullPool,
    )

    async with connectable.begin() as connection:
        await connection.run_sync(do_run_migrations)

    await connectable.dispose()


def run_migrations_online() -> None:
    """在'在线'模式下运行迁移"""
    asyncio.run(run_async_migrations())


# 根据上下文选择运行模式
if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
