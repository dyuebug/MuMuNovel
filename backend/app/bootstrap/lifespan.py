from __future__ import annotations

from contextlib import asynccontextmanager
from datetime import datetime

from fastapi import FastAPI
from sqlalchemy.exc import OperationalError

from app.database import check_database_health, close_db
from app.logger import get_logger
from app.mcp import mcp_client, register_status_sync
from app.services.background_task_manager import background_task_manager

logger = get_logger(__name__)


def build_default_startup_status() -> dict:
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


def reset_startup_status(state: dict) -> dict:
    state.clear()
    state.update(build_default_startup_status())
    state["started_at"] = datetime.now().isoformat()
    return state


def get_startup_status(state: dict) -> dict:
    if not isinstance(state, dict) or not state:
        return reset_startup_status(state)
    return state


def mark_startup_step(
    state: dict,
    step: str,
    *,
    healthy: bool,
    detail: str | None = None,
    payload: dict | None = None,
) -> dict:
    current = get_startup_status(state)
    step_state = current.setdefault("steps", {}).setdefault(step, {})
    step_state["healthy"] = bool(healthy)
    step_state["status"] = "ok" if healthy else "error"
    if detail:
        step_state["detail"] = detail
    if payload is not None:
        step_state["payload"] = payload
    return current


def finalize_startup_status(state: dict) -> dict:
    current = get_startup_status(state)
    steps = current.get("steps") or {}
    current["ready"] = all(bool(item.get("healthy")) for item in steps.values())
    current["completed_at"] = datetime.now().isoformat()
    return current


def set_startup_ready(state: dict, ready: bool) -> dict:
    current = get_startup_status(state)
    current["ready"] = bool(ready)
    if ready:
        current["completed_at"] = datetime.now().isoformat()
    else:
        current["completed_at"] = None
    return current


def recover_startup_status_if_possible(state: dict, database_status: dict) -> dict:
    current = get_startup_status(state)
    if bool(current.get("ready")):
        return current

    steps = current.get("steps") or {}
    if not isinstance(steps, dict):
        return current

    non_database_steps_healthy = all(
        bool(item.get("healthy"))
        for step_name, item in steps.items()
        if step_name != "database_warmup"
    )
    database_ready = bool((database_status or {}).get("healthy"))
    if not (non_database_steps_healthy and database_ready):
        return current

    mark_startup_step(
        current,
        "database_warmup",
        healthy=True,
        detail="warmup recovered after startup",
        payload=database_status,
    )
    recovered = finalize_startup_status(current)
    logger.info("Application startup readiness recovered after database became healthy")
    return recovered


def create_lifespan(state: dict):
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        reset_startup_status(state)

        register_status_sync()
        mark_startup_step(state, "status_sync", healthy=True, detail="registered")

        await background_task_manager.ensure_loaded()
        mark_startup_step(state, "background_tasks", healthy=True, detail="loaded")

        database_warmup = await check_database_health(force_refresh=True)
        database_ready = bool(database_warmup.get("healthy"))
        mark_startup_step(
            state,
            "database_warmup",
            healthy=database_ready,
            detail="warmup completed" if database_ready else "warmup failed",
            payload=database_warmup,
        )
        finalize_startup_status(state)

        if database_ready:
            logger.info("Application startup completed")
        else:
            logger.warning(
                "Application startup completed, but database warmup is still unhealthy; readyz will stay 503"
            )

        yield

        await mcp_client.cleanup()

        from app.services.ai_service import cleanup_http_clients

        await cleanup_http_clients()
        await close_db()

        logger.info("Application shutdown completed")

    return lifespan
