"""批量生成编排 service。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, List, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route, runtime, and single-chapter "
    "background execution chain; this Python orchestration module is kept "
    "only as frozen rollback/source-map material after its remaining callers "
    "were reduced to frozen shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_generation_execution_contract_service.rs; "
    "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs; "
    "backend-rs/src/services/chapter_access_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "legacy_batch_generation_python_routes_enabled; "
    "legacy_single_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.services.batch_generation_create_orchestration_service import (
    BatchGenerationCreatePreparation,
)

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.chapter import Chapter
    from app.models.project import Project
    from app.services.ai_service import AIService
    from app.services.single_chapter_background_context_service import (
        SingleChapterBackgroundExecutionContext,
    )
    from app.services.story_repair_payload_service import StoryRepairPayload


@dataclass(frozen=True)
class BatchGenerationResumePreparation:
    source_task: "BatchGenerationTask"
    remaining_chapter_ids: List[str]
    remaining_chapters: List["Chapter"]
    first_chapter: "Chapter"
    resumed_story_repair_payload: Optional["StoryRepairPayload"]
    active_story_repair_payload_snapshot: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class BatchGenerationResumeExecutionResult:
    resumed_task: "BatchGenerationTask"
    response_payload: Dict[str, Any]


def recover_stale_single_chapter_background_task_if_needed(
    *args,
    **kwargs,
) -> bool:
    from app.services.single_chapter_background_task_helper_service import (
        recover_stale_single_chapter_background_task_if_needed as recover_stale_single_chapter_background_task_if_needed_service,
    )

    return recover_stale_single_chapter_background_task_if_needed_service(
        *args,
        **kwargs,
    )


def single_chapter_background_task_contains_chapter(
    *args,
    **kwargs,
) -> bool:
    from app.services.single_chapter_background_task_helper_service import (
        single_chapter_background_task_contains_chapter as single_chapter_background_task_contains_chapter_service,
    )

    return single_chapter_background_task_contains_chapter_service(*args, **kwargs)


async def load_existing_single_chapter_background_task_payload(*args, **kwargs):
    from app.services.single_chapter_background_generation_service import (
        load_existing_single_chapter_background_task_payload as load_existing_single_chapter_background_task_payload_service,
    )

    return await load_existing_single_chapter_background_task_payload_service(
        *args,
        **kwargs,
    )


async def prepare_single_chapter_background_generation(*args, **kwargs):
    from app.services.single_chapter_background_generation_service import (
        prepare_single_chapter_background_generation as prepare_single_chapter_background_generation_service,
    )

    return await prepare_single_chapter_background_generation_service(*args, **kwargs)


async def create_single_chapter_background_generation_and_enqueue(*args, **kwargs):
    from app.services.single_chapter_background_generation_service import (
        create_single_chapter_background_generation_and_enqueue as create_single_chapter_background_generation_and_enqueue_service,
    )

    return await create_single_chapter_background_generation_and_enqueue_service(
        *args,
        **kwargs,
    )


async def prepare_batch_generation_create(*args, **kwargs):
    from app.services.batch_generation_create_orchestration_service import (
        prepare_batch_generation_create as prepare_batch_generation_create_service,
    )

    return await prepare_batch_generation_create_service(*args, **kwargs)


async def create_batch_generation_and_enqueue(*args, **kwargs):
    from app.services.batch_generation_create_orchestration_service import (
        create_batch_generation_and_enqueue as create_batch_generation_and_enqueue_service,
    )

    return await create_batch_generation_and_enqueue_service(*args, **kwargs)


async def orchestrate_single_chapter_background_generation(*args, **kwargs):
    from app.services.single_chapter_background_generation_service import (
        orchestrate_single_chapter_background_generation as orchestrate_single_chapter_background_generation_service,
    )

    return await orchestrate_single_chapter_background_generation_service(
        *args,
        **kwargs,
    )


async def orchestrate_batch_generation_create(*args, **kwargs) -> Dict[str, Any]:
    from app.services.batch_generation_create_orchestration_service import (
        orchestrate_batch_generation_create as orchestrate_batch_generation_create_service,
    )

    return await orchestrate_batch_generation_create_service(*args, **kwargs)


def build_batch_task_terminal_status(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_status_read_owner_service import (
        build_batch_task_terminal_status as build_batch_task_terminal_status_service,
    )

    return build_batch_task_terminal_status_service(*args, **kwargs)


async def orchestrate_batch_generation_resume(
    *args,
    **kwargs,
) -> Dict[str, Any]:
    from app.services.batch_generation_resume_orchestration_service import (
        orchestrate_batch_generation_resume as orchestrate_batch_generation_resume_service,
    )

    return await orchestrate_batch_generation_resume_service(
        *args,
        build_batch_task_terminal_status_fn=build_batch_task_terminal_status,
        **kwargs,
    )


async def prepare_batch_generation_resume(*args, **kwargs):
    from app.services.batch_generation_resume_orchestration_service import (
        prepare_batch_generation_resume as prepare_batch_generation_resume_service,
    )

    return await prepare_batch_generation_resume_service(
        *args,
        build_batch_task_terminal_status_fn=build_batch_task_terminal_status,
        **kwargs,
    )


def build_resumed_batch_generation_runtime_snapshot(*args, **kwargs):
    from app.services.batch_generation_resume_orchestration_service import (
        build_resumed_batch_generation_runtime_snapshot as build_resumed_batch_generation_runtime_snapshot_service,
    )

    return build_resumed_batch_generation_runtime_snapshot_service(*args, **kwargs)


def build_resumed_batch_generation_response(*args, **kwargs):
    from app.services.batch_generation_resume_orchestration_service import (
        build_resumed_batch_generation_response as build_resumed_batch_generation_response_service,
    )

    return build_resumed_batch_generation_response_service(*args, **kwargs)


async def create_resumed_batch_generation_and_enqueue(*args, **kwargs):
    from app.services.batch_generation_resume_orchestration_service import (
        create_resumed_batch_generation_and_enqueue as create_resumed_batch_generation_and_enqueue_service,
    )

    return await create_resumed_batch_generation_and_enqueue_service(*args, **kwargs)
