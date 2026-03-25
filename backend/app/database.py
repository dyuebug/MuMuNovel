"""Database connection and session management."""

import asyncio
import copy
from datetime import datetime
import time
from typing import Any, Dict

from fastapi import HTTPException, Request
from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.orm import declarative_base

from app.config import settings
from app.logger import get_logger

logger = get_logger(__name__)

Base = declarative_base()

from app.models import (  # noqa: E402
    AnalysisTask,
    BatchGenerationTask,
    BatchGenerationSnapshot,
    Career,
    Character,
    CharacterCareer,
    CharacterRelationship,
    Chapter,
    ChapterDraftAttempt,
    GenerationHistory,
    MCPPlugin,
    Organization,
    OrganizationMember,
    Outline,
    PlotAnalysis,
    Project,
    ProjectDefaultStyle,
    PromptTemplate,
    RegenerationTask,
    RelationshipType,
    Settings,
    StoryMemory,
    User,
    WritingStyle,
)

_engine_cache: Dict[str, Any] = {}
_session_factory_cache: Dict[str, Any] = {}
_cache_lock = asyncio.Lock()
_health_check_cache: Dict[str, Dict[str, Any]] = {}
_health_check_lock = asyncio.Lock()

_session_stats = {
    "created": 0,
    "closed": 0,
    "active": 0,
    "errors": 0,
    "generator_exits": 0,
    "last_check": None,
}

AsyncSessionLocal = async_sessionmaker(class_=AsyncSession, expire_on_commit=False)


def _cache_key(_: str) -> str:
    return "shared_postgres"


def _build_engine_args() -> Dict[str, Any]:
    is_sqlite = "sqlite" in settings.database_url.lower()
    engine_args: Dict[str, Any] = {
        "echo": settings.database_echo_pool,
        "echo_pool": settings.database_echo_pool,
        "future": True,
    }

    if is_sqlite:
        engine_args["connect_args"] = {
            "check_same_thread": False,
            "timeout": 30.0,
        }
        engine_args["pool_pre_ping"] = True
        return engine_args

    connect_args = {
        "server_settings": {
            "application_name": settings.app_name,
            "jit": "off",
            "search_path": "public",
        },
        "command_timeout": 60,
        "statement_cache_size": 500,
    }
    engine_args.update({
        "pool_size": settings.database_pool_size,
        "max_overflow": settings.database_max_overflow,
        "pool_timeout": settings.database_pool_timeout,
        "pool_pre_ping": settings.database_pool_pre_ping,
        "pool_recycle": settings.database_pool_recycle,
        "pool_use_lifo": settings.database_pool_use_lifo,
        "pool_reset_on_return": settings.database_pool_reset_on_return,
        "max_identifier_length": settings.database_max_identifier_length,
        "connect_args": connect_args,
    })
    return engine_args


async def get_engine(user_id: str):
    """Return the shared async engine."""
    cache_key = _cache_key(user_id)
    engine = _engine_cache.get(cache_key)
    if engine is not None:
        return engine

    async with _cache_lock:
        engine = _engine_cache.get(cache_key)
        if engine is not None:
            return engine

        engine = create_async_engine(settings.database_url, **_build_engine_args())
        _engine_cache[cache_key] = engine
        _session_factory_cache[cache_key] = async_sessionmaker(
            engine,
            class_=AsyncSession,
            expire_on_commit=False,
        )

        if "sqlite" in settings.database_url.lower():
            try:
                from sqlalchemy import event

                @event.listens_for(engine.sync_engine, "connect")
                def set_sqlite_pragma(dbapi_conn, _connection_record):
                    cursor = dbapi_conn.cursor()
                    cursor.execute("PRAGMA journal_mode=WAL")
                    cursor.execute("PRAGMA synchronous=NORMAL")
                    cursor.execute("PRAGMA cache_size=-64000")
                    cursor.execute("PRAGMA busy_timeout=30000")
                    cursor.close()
            except Exception as e:
                logger.warning(f"Failed to configure SQLite pragmas: {e}")

        return engine


async def get_session_factory(user_id: str):
    """Return cached async sessionmaker for the shared engine."""
    cache_key = _cache_key(user_id)
    session_factory = _session_factory_cache.get(cache_key)
    if session_factory is not None:
        return session_factory

    await get_engine(user_id)

    async with _cache_lock:
        session_factory = _session_factory_cache.get(cache_key)
        if session_factory is None:
            engine = _engine_cache[cache_key]
            session_factory = async_sessionmaker(
                engine,
                class_=AsyncSession,
                expire_on_commit=False,
            )
            _session_factory_cache[cache_key] = session_factory

    return session_factory


async def get_db(request: Request):
    """FastAPI dependency that yields a database session."""
    user_id = getattr(request.state, "user_id", None)
    if not user_id:
        raise HTTPException(status_code=401, detail="未登录或用户ID缺失")

    session_factory = await get_session_factory(user_id)
    session = session_factory()
    session_id = id(session)

    _session_stats["created"] += 1
    _session_stats["active"] += 1

    try:
        yield session
        if session.in_transaction():
            await session.rollback()
    except GeneratorExit:
        _session_stats["generator_exits"] += 1
        try:
            if session.in_transaction():
                await session.rollback()
        except Exception as rollback_error:
            _session_stats["errors"] += 1
            logger.error(f"Rollback failed after GeneratorExit [ID:{session_id}]: {rollback_error}")
    except Exception as e:
        _session_stats["errors"] += 1
        try:
            if session.in_transaction():
                await session.rollback()
        except Exception as rollback_error:
            logger.error(f"Rollback failed after session error [ID:{session_id}]: {rollback_error}")
        logger.error(f"Database session error [ID:{session_id}]: {e}")
        raise
    finally:
        try:
            if session.in_transaction():
                await session.rollback()

            await session.close()
            _session_stats["closed"] += 1
            _session_stats["active"] -= 1
            _session_stats["last_check"] = datetime.now().isoformat()
        except Exception as close_error:
            _session_stats["errors"] += 1
            logger.error(f"Failed to close DB session [ID:{session_id}]: {close_error}", exc_info=True)
            try:
                await session.close()
            except Exception:
                pass


async def init_db(user_id: str = None):
    """Deprecated compatibility stub."""
    logger.warning("init_db() is deprecated; use Alembic migrations and lazy settings bootstrap instead")


async def close_db():
    """Dispose cached engines and clear caches."""
    engines = list(_engine_cache.values())
    _engine_cache.clear()
    _session_factory_cache.clear()
    _health_check_cache.clear()

    for engine in engines:
        try:
            await engine.dispose()
        except Exception as e:
            logger.warning(f"Failed to dispose engine: {e}")


async def get_database_stats():
    """Return database pool and session statistics."""
    pool_stats: Dict[str, Any] = {}
    cache_key = _cache_key("stats")
    engine = _engine_cache.get(cache_key)

    if engine is not None:
        try:
            pool = engine.pool
            total_connections = max(settings.database_pool_size + settings.database_max_overflow, 1)
            checked_out = pool.checkedout() if hasattr(pool, "checkedout") else 0
            pool_stats = {
                "size": pool.size() if hasattr(pool, "size") else None,
                "checked_in": pool.checkedin() if hasattr(pool, "checkedin") else None,
                "checked_out": checked_out,
                "overflow": pool.overflow() if hasattr(pool, "overflow") else None,
                "usage_percent": (checked_out / total_connections) * 100,
            }
        except Exception as e:
            pool_stats = {"error": str(e)}

    error_rate = (_session_stats["errors"] / max(_session_stats["created"], 1)) * 100
    health_status = "healthy"
    warnings = []
    errors = []

    if _session_stats["active"] > settings.database_session_leak_threshold:
        health_status = "critical"
        errors.append(
            f"active sessions {_session_stats['active']} exceeded leak threshold {settings.database_session_leak_threshold}"
        )
    elif _session_stats["active"] > settings.database_session_max_active:
        health_status = "warning"
        warnings.append(
            f"active sessions {_session_stats['active']} exceeded warning threshold {settings.database_session_max_active}"
        )

    usage_percent = pool_stats.get("usage_percent")
    if isinstance(usage_percent, (int, float)):
        if usage_percent > 95:
            health_status = "critical"
            errors.append(f"pool usage is too high: {usage_percent:.1f}%")
        elif usage_percent > 90 and health_status == "healthy":
            health_status = "warning"
            warnings.append(f"pool usage is high: {usage_percent:.1f}%")

    if error_rate > 5 and health_status == "healthy":
        health_status = "warning"
        warnings.append(f"session error rate is high: {error_rate:.2f}%")

    return {
        "session_stats": {
            **_session_stats,
            "error_rate": f"{error_rate:.2f}%",
        },
        "pool_stats": pool_stats,
        "engine_cache": {
            "total_engines": len(_engine_cache),
            "engine_keys": list(_engine_cache.keys()),
        },
        "config": {
            "database_type": "SQLite" if "sqlite" in settings.database_url.lower() else "PostgreSQL",
            "pool_size": settings.database_pool_size,
            "max_overflow": settings.database_max_overflow,
            "pool_timeout": settings.database_pool_timeout,
            "pool_recycle": settings.database_pool_recycle,
            "session_max_active_threshold": settings.database_session_max_active,
            "session_leak_threshold": settings.database_session_leak_threshold,
        },
        "health": {
            "status": health_status,
            "warnings": warnings,
            "errors": errors,
        },
    }


def _health_check_cache_key(user_id: str | None) -> str:
    return user_id or "_health_check_"


def _clone_health_check_result(result: Dict[str, Any]) -> Dict[str, Any]:
    return copy.deepcopy(result)


async def _execute_database_health_probe(session_factory: async_sessionmaker[AsyncSession]) -> None:
    async with session_factory() as session:
        await session.execute(text("SELECT 1"))


async def check_database_health(user_id: str = None, *, force_refresh: bool = False) -> dict:
    """Run a lightweight database health check with timeout and short-lived caching."""
    cache_key = _health_check_cache_key(user_id)
    cache_ttl = max(float(settings.database_health_cache_ttl_seconds or 0.0), 0.0)
    timeout_seconds = max(float(settings.database_health_timeout_seconds or 0.0), 0.1)
    now = time.monotonic()

    if not force_refresh and cache_ttl > 0:
        cached_entry = _health_check_cache.get(cache_key)
        if cached_entry and float(cached_entry.get("expires_at") or 0.0) > now:
            cached_result = _clone_health_check_result(cached_entry["result"])
            cached_result["cached"] = True
            cached_result["cache_age_ms"] = round((now - float(cached_entry.get("stored_at") or now)) * 1000, 1)
            return cached_result

    async with _health_check_lock:
        now = time.monotonic()
        if not force_refresh and cache_ttl > 0:
            cached_entry = _health_check_cache.get(cache_key)
            if cached_entry and float(cached_entry.get("expires_at") or 0.0) > now:
                cached_result = _clone_health_check_result(cached_entry["result"])
                cached_result["cached"] = True
                cached_result["cache_age_ms"] = round((now - float(cached_entry.get("stored_at") or now)) * 1000, 1)
                return cached_result

        result = {
            "healthy": True,
            "checks": {},
            "timestamp": datetime.now().isoformat(),
            "cached": False,
        }
        started_at = time.perf_counter()

        try:
            engine = await get_engine(cache_key)
            session_factory = await get_session_factory(cache_key)
            await asyncio.wait_for(_execute_database_health_probe(session_factory), timeout=timeout_seconds)
            latency_ms = round((time.perf_counter() - started_at) * 1000, 1)
            result["latency_ms"] = latency_ms
            result["checks"]["connection"] = {
                "status": "ok",
                "healthy": True,
                "latency_ms": latency_ms,
            }

            pool = engine.pool
            if hasattr(pool, "size"):
                pool_status = {
                    "size": pool.size() if hasattr(pool, "size") else None,
                    "checked_in": pool.checkedin() if hasattr(pool, "checkedin") else None,
                    "checked_out": pool.checkedout() if hasattr(pool, "checkedout") else None,
                    "overflow": pool.overflow() if hasattr(pool, "overflow") else None,
                    "healthy": True,
                }
                if hasattr(pool, "overflow") and pool.overflow() >= settings.database_max_overflow:
                    pool_status["healthy"] = False
                    pool_status["warning"] = "connection pool overflow reached limit"
                    result["healthy"] = False
                result["checks"]["pool"] = pool_status
        except asyncio.TimeoutError:
            latency_ms = round((time.perf_counter() - started_at) * 1000, 1)
            result["healthy"] = False
            result["latency_ms"] = latency_ms
            result["checks"]["connection"] = {
                "status": "timeout",
                "healthy": False,
                "latency_ms": latency_ms,
                "timeout_seconds": timeout_seconds,
            }
            result["checks"]["error"] = {
                "status": "error",
                "message": f"database health check timed out after {timeout_seconds:.1f}s",
                "healthy": False,
            }
            logger.warning("Database health check timed out after %.1fs", timeout_seconds)
        except Exception as e:
            latency_ms = round((time.perf_counter() - started_at) * 1000, 1)
            result["healthy"] = False
            result["latency_ms"] = latency_ms
            result["checks"]["error"] = {
                "status": "error",
                "message": str(e),
                "healthy": False,
            }
            logger.error(f"Database health check failed: {e}", exc_info=True)

        stored_at = time.monotonic()
        if cache_ttl > 0:
            _health_check_cache[cache_key] = {
                "result": _clone_health_check_result(result),
                "stored_at": stored_at,
                "expires_at": stored_at + cache_ttl,
            }

        return result


async def reset_session_stats():
    """Reset session statistics."""
    _session_stats.update({
        "created": 0,
        "closed": 0,
        "active": 0,
        "errors": 0,
        "generator_exits": 0,
        "last_check": datetime.now().isoformat(),
    })
    return _session_stats