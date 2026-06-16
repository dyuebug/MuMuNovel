"""批量生成单章 service 冻结 shim。"""
from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch single-chapter request/runtime chain; this "
    "Python service is kept only as frozen rollback/source-map material for "
    "legacy batch single-chapter fallback execution."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_single_generation_prepare_service.rs; "
    "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.batch_generation_single_chapter_wiring_service import (
        BatchGenerationSingleChapterDependencies,
        BatchGenerationSingleChapterRequest,
    )


def build_batch_generation_single_chapter_request(**kwargs) -> BatchGenerationSingleChapterRequest:
    from app.services.batch_generation_single_chapter_wiring_service import (
        build_batch_generation_single_chapter_request as build_batch_generation_single_chapter_request_service,
    )

    return build_batch_generation_single_chapter_request_service(**kwargs)


def build_batch_generation_single_chapter_dependencies(**kwargs) -> BatchGenerationSingleChapterDependencies:
    from app.services.batch_generation_single_chapter_wiring_service import (
        build_batch_generation_single_chapter_dependencies as build_batch_generation_single_chapter_dependencies_service,
    )

    return build_batch_generation_single_chapter_dependencies_service(**kwargs)


async def generate_single_chapter_for_batch_workflow(
    *,
    request: BatchGenerationSingleChapterRequest,
    dependencies: BatchGenerationSingleChapterDependencies,
) -> Dict[str, Any]:
    from app.services.batch_generation_single_chapter_wiring_service import (
        generate_single_chapter_for_batch_workflow as generate_single_chapter_for_batch_workflow_service,
    )

    return await generate_single_chapter_for_batch_workflow_service(
        request=request,
        dependencies=dependencies,
    )
