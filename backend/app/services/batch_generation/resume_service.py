from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation resume selection, reset, dispatch, "
    "and response contract; this Python module is kept only as frozen "
    "rollback/source-map material after its remaining callers were reduced to "
    "frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; "
    "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.batch_generation_orchestration_service import (
        BatchGenerationResumeExecutionResult,
        BatchGenerationResumePreparation,
    )


async def prepare_batch_generation_resume(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        prepare_batch_generation_resume as prepare_batch_generation_resume_service,
    )

    return await prepare_batch_generation_resume_service(*args, **kwargs)


def build_resumed_batch_generation_runtime_snapshot(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        build_resumed_batch_generation_runtime_snapshot as build_resumed_batch_generation_runtime_snapshot_service,
    )

    return build_resumed_batch_generation_runtime_snapshot_service(*args, **kwargs)


def build_resumed_batch_generation_response(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        build_resumed_batch_generation_response as build_resumed_batch_generation_response_service,
    )

    return build_resumed_batch_generation_response_service(*args, **kwargs)


async def create_resumed_batch_generation_and_enqueue(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        create_resumed_batch_generation_and_enqueue as create_resumed_batch_generation_and_enqueue_service,
    )

    return await create_resumed_batch_generation_and_enqueue_service(*args, **kwargs)

