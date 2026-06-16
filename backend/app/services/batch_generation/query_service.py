from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation read/query and status payload "
    "contract; this Python module is kept only as frozen rollback/source-map "
    "material after its remaining callers were reduced to frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; "
    "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "legacy_batch_generation_python_routes_enabled; "
    "legacy_single_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.batch_generation.status_models import BatchGenerationTaskViewContext


def recover_stale_batch_generation_task_if_needed(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        recover_stale_batch_generation_task_if_needed as recover_stale_batch_generation_task_if_needed_service,
    )

    return recover_stale_batch_generation_task_if_needed_service(*args, **kwargs)


async def recover_stale_batch_generation_tasks(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        recover_stale_batch_generation_tasks as recover_stale_batch_generation_tasks_service,
    )

    return await recover_stale_batch_generation_tasks_service(*args, **kwargs)


async def build_batch_generation_task_view_context(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_task_view_context as build_batch_generation_task_view_context_service,
    )

    return await build_batch_generation_task_view_context_service(*args, **kwargs)


async def load_batch_generation_task_view_context(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        load_batch_generation_task_view_context as load_batch_generation_task_view_context_service,
    )

    return await load_batch_generation_task_view_context_service(*args, **kwargs)


async def load_active_project_batch_generation_task_view_context(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        load_active_project_batch_generation_task_view_context as load_active_project_batch_generation_task_view_context_service,
    )

    return await load_active_project_batch_generation_task_view_context_service(
        *args,
        **kwargs,
    )


async def load_active_user_batch_generation_task_view_contexts(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        load_active_user_batch_generation_task_view_contexts as load_active_user_batch_generation_task_view_contexts_service,
    )

    return await load_active_user_batch_generation_task_view_contexts_service(
        *args,
        **kwargs,
    )

