from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation create workflow and payload "
    "assembly; this Python module is kept only as frozen rollback/source-map "
    "material after its remaining callers were reduced to frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; "
    "backend-rs/src/services/chapter_generation_execution_contract_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "legacy_batch_generation_python_routes_enabled; "
    "legacy_single_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.batch_generation_orchestration_service import (
        BatchGenerationCreatePreparation,
    )


async def prepare_batch_generation_create(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        prepare_batch_generation_create as prepare_batch_generation_create_service,
    )

    return await prepare_batch_generation_create_service(*args, **kwargs)


async def create_batch_generation_and_enqueue(*args, **kwargs):
    from app.services.batch_generation_orchestration_service import (
        create_batch_generation_and_enqueue as create_batch_generation_and_enqueue_service,
    )

    return await create_batch_generation_and_enqueue_service(*args, **kwargs)
