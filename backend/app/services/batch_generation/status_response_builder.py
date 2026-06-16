from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation status and terminal payload "
    "semantics; this Python module is kept only as frozen rollback/source-map "
    "material after its remaining callers were reduced to frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; "
    "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask


def build_batch_task_terminal_status(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_task_terminal_status as build_batch_task_terminal_status_service,
    )

    return build_batch_task_terminal_status_service(*args, **kwargs)


def build_batch_generation_status_response(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_status_response as build_batch_generation_status_response_service,
    )

    return build_batch_generation_status_response_service(*args, **kwargs)


def build_active_batch_generation_payload(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_active_batch_generation_payload as build_active_batch_generation_payload_service,
    )

    return build_active_batch_generation_payload_service(*args, **kwargs)


def build_batch_generation_task_list_item(*args, **kwargs):
    from app.services.batch_generation.route_wiring_service import (
        build_batch_generation_task_list_item as build_batch_generation_task_list_item_service,
    )

    return build_batch_generation_task_list_item_service(*args, **kwargs)

