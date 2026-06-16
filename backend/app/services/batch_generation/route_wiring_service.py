"""Legacy batch-generation route wiring source map.

This module owns the remaining Python rollback wiring for batch-generation
routes. The active production route owner is Rust; this file stays frozen so
the legacy Python route shell can remain thin and explicit.
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, List, Optional

from fastapi import HTTPException

from app.logger import get_logger

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route and runtime chain; this Python "
    "route wiring module remains only as frozen rollback/source-map material "
    "after the batch legacy route shell closeout review."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

logger = get_logger(__name__)

if TYPE_CHECKING:
    pass


async def verify_project_access(*args, **kwargs):
    from app.services.batch_generation_route_default_wiring_service import (
        verify_project_access as verify_project_access_service,
    )

    return await verify_project_access_service(*args, **kwargs)


async def orchestrate_batch_generation_create_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_default_wiring_service import (
        orchestrate_batch_generation_create_with_default_wiring as orchestrate_batch_generation_create_with_default_wiring_service,
    )

    return await orchestrate_batch_generation_create_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def orchestrate_batch_generation_resume_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_default_wiring_service import (
        orchestrate_batch_generation_resume_with_default_wiring as orchestrate_batch_generation_resume_with_default_wiring_service,
    )

    return await orchestrate_batch_generation_resume_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def validate_batch_generation_stream_access(*args, **kwargs):
    from app.services.batch_generation_stream_service import (
        validate_batch_generation_stream_access as validate_batch_generation_stream_access_service,
    )

    return await validate_batch_generation_stream_access_service(*args, **kwargs)


def build_batch_generation_event_stream(*args, **kwargs):
    from app.services.batch_generation_stream_service import (
        build_batch_generation_event_stream as build_batch_generation_event_stream_service,
    )

    return build_batch_generation_event_stream_service(*args, **kwargs)


def recover_stale_batch_generation_task_if_needed(task: "BatchGenerationTask") -> bool:
    from app.services.batch_generation_status_read_owner_service import (
        recover_stale_batch_generation_task_if_needed as recover_stale_batch_generation_task_if_needed_service,
    )

    return recover_stale_batch_generation_task_if_needed_service(task)


async def recover_stale_batch_generation_tasks(
    *args,
    **kwargs,
) -> bool:
    from app.services.batch_generation_status_read_owner_service import (
        recover_stale_batch_generation_tasks as recover_stale_batch_generation_tasks_service,
    )

    return await recover_stale_batch_generation_tasks_service(*args, **kwargs)


def build_batch_task_terminal_status(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_task_terminal_status as build_batch_task_terminal_status_service,
    )

    return build_batch_task_terminal_status_service(*args, **kwargs)


def _default_batch_progress_phase(task: "BatchGenerationTask") -> str:
    from app.services.batch_generation_status_read_owner_service import (
        _default_batch_progress_phase as _default_batch_progress_phase_service,
    )

    return _default_batch_progress_phase_service(task)


def _compose_batch_stage_code(base: str, phase: Optional[str]) -> str:
    from app.services.batch_generation_status_read_owner_service import (
        _compose_batch_stage_code as _compose_batch_stage_code_service,
    )

    return _compose_batch_stage_code_service(base, phase)


async def build_batch_task_workflow_snapshot(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_task_workflow_snapshot as build_batch_task_workflow_snapshot_service,
    )

    return await build_batch_task_workflow_snapshot_service(*args, **kwargs)


async def build_batch_generation_task_view_context(
    *args,
    **kwargs,
) -> BatchGenerationTaskViewContext:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_generation_task_view_context as build_batch_generation_task_view_context_service,
    )

    return await build_batch_generation_task_view_context_service(*args, **kwargs)


async def load_batch_generation_task_view_context(
    *args,
    **kwargs,
) -> Optional[BatchGenerationTaskViewContext]:
    from app.services.batch_generation_status_read_owner_service import (
        load_batch_generation_task_view_context as load_batch_generation_task_view_context_service,
    )

    return await load_batch_generation_task_view_context_service(*args, **kwargs)


async def load_active_project_batch_generation_task_view_context(
    *args,
    **kwargs,
) -> Optional[BatchGenerationTaskViewContext]:
    from app.services.batch_generation_status_read_owner_service import (
        load_active_project_batch_generation_task_view_context as load_active_project_batch_generation_task_view_context_service,
    )

    return await load_active_project_batch_generation_task_view_context_service(
        *args,
        **kwargs,
    )


async def load_active_user_batch_generation_task_view_contexts(
    *args,
    **kwargs,
) -> List[BatchGenerationTaskViewContext]:
    from app.services.batch_generation_status_read_owner_service import (
        load_active_user_batch_generation_task_view_contexts as load_active_user_batch_generation_task_view_contexts_service,
    )

    return await load_active_user_batch_generation_task_view_contexts_service(
        *args,
        **kwargs,
    )


def build_batch_generation_status_response(
    *args,
    **kwargs,
) -> BatchGenerateStatusResponse:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_generation_status_response as build_batch_generation_status_response_service,
    )

    return build_batch_generation_status_response_service(*args, **kwargs)


def build_active_batch_generation_payload(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_status_read_owner_service import (
        build_active_batch_generation_payload as build_active_batch_generation_payload_service,
    )

    return build_active_batch_generation_payload_service(*args, **kwargs)


def _resolve_batch_generation_task_type(task: "BatchGenerationTask") -> str:
    from app.services.batch_generation_status_read_owner_service import (
        _resolve_batch_generation_task_type as _resolve_batch_generation_task_type_service,
    )

    return _resolve_batch_generation_task_type_service(task)


def build_batch_generation_task_list_item(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_generation_task_list_item as build_batch_generation_task_list_item_service,
    )

    return build_batch_generation_task_list_item_service(*args, **kwargs)


async def stream_batch_generation_events_with_default_route_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_transport_service import (
        stream_batch_generation_events_with_default_route_wiring as stream_batch_generation_events_with_default_route_wiring_service,
    )

    return await stream_batch_generation_events_with_default_route_wiring_service(
        *args,
        **kwargs,
    )


async def cancel_batch_generation_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_transport_service import (
        cancel_batch_generation_with_default_wiring as cancel_batch_generation_with_default_wiring_service,
    )

    return await cancel_batch_generation_with_default_wiring_service(*args, **kwargs)


async def load_batch_generation_status_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_default_wiring_service import (
        load_batch_generation_status_with_default_wiring as load_batch_generation_status_with_default_wiring_service,
    )

    return await load_batch_generation_status_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def load_active_project_batch_generation_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_default_wiring_service import (
        load_active_project_batch_generation_with_default_wiring as load_active_project_batch_generation_with_default_wiring_service,
    )

    return await load_active_project_batch_generation_with_default_wiring_service(
        *args,
        **kwargs,
    )


async def load_active_batch_generation_task_list_with_default_wiring(
    *args,
    **kwargs,
):
    from app.services.batch_generation_route_default_wiring_service import (
        load_active_batch_generation_task_list_with_default_wiring as load_active_batch_generation_task_list_with_default_wiring_service,
    )

    return await load_active_batch_generation_task_list_with_default_wiring_service(
        *args,
        **kwargs,
    )
