from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation runtime snapshot and checkpoint "
    "projection contract; this Python module is kept only as frozen "
    "rollback/source-map material after its remaining callers were reduced to "
    "frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "legacy_batch_generation_python_routes_enabled; "
    "legacy_single_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask


async def build_batch_task_workflow_snapshot(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_task_workflow_snapshot as build_batch_task_workflow_snapshot_service,
    )

    return await build_batch_task_workflow_snapshot_service(*args, **kwargs)

