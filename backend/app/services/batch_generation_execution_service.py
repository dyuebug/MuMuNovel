"""批量生成执行门面冻结 shim。

该文件保留给 rollback/source-map 和测试 patch surface 使用，
真实 owner 已分别下沉到 runtime / prompt / candidate / workflow /
single-chapter wiring 模块。
"""
from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation execution/runtime facade chain; "
    "this Python module is retained only as frozen rollback/source-map "
    "material after the batch retired-execution-facade closeout review."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/health.rs; "
    "backend-rs/src/services/chapter_generation_runtime_service.rs; "
    "backend-rs/src/services/chapter_generation_execution_contract_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "aggregate_chapters_python_source_map; "
    "legacy_batch_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.batch_generation_candidate_service import (
        BatchGenerationCandidateExecution,
        BatchGenerationCandidateFlowResult,
        BatchGenerationCandidateQualityHooks,
    )
    from app.services.batch_generation_prompt_service import (
        BatchGenerationPrompt,
        BatchGenerationPromptStageResult,
        BatchGenerationRequestPayload,
    )
    from app.services.batch_generation_runtime_service import (
        BatchGenerationBuiltContext,
        BatchGenerationResolvedRuntime,
        BatchGenerationRuntimePreparation,
    )
    from app.services.batch_generation_single_chapter_wiring_service import (
        BatchGenerationSingleChapterDependencies,
        BatchGenerationSingleChapterRequest,
    )


def _batch_generation_candidate_service():
    from app.services import batch_generation_candidate_service

    return batch_generation_candidate_service


def _batch_generation_prompt_service():
    from app.services import batch_generation_prompt_service

    return batch_generation_prompt_service


def _batch_generation_runtime_service():
    from app.services import batch_generation_runtime_service

    return batch_generation_runtime_service


def _batch_generation_workflow_service():
    from app.services import batch_generation_workflow_service

    return batch_generation_workflow_service


def _batch_generation_single_chapter_wiring_service():
    from app.services import batch_generation_single_chapter_wiring_service

    return batch_generation_single_chapter_wiring_service


def __getattr__(name: str):
    if hasattr(_batch_generation_prompt_service(), name):
        return getattr(_batch_generation_prompt_service(), name)
    if hasattr(_batch_generation_runtime_service(), name):
        return getattr(_batch_generation_runtime_service(), name)
    if hasattr(_batch_generation_candidate_service(), name):
        return getattr(_batch_generation_candidate_service(), name)
    if hasattr(_batch_generation_workflow_service(), name):
        return getattr(_batch_generation_workflow_service(), name)
    if hasattr(_batch_generation_single_chapter_wiring_service(), name):
        return getattr(_batch_generation_single_chapter_wiring_service(), name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


def publish_task_stream_event(*args, **kwargs):
    return _batch_generation_candidate_service().publish_task_stream_event(*args, **kwargs)


def calculate_estimated_time(*args, **kwargs):
    return _batch_generation_workflow_service().calculate_estimated_time(*args, **kwargs)


def enqueue_batch_generation_execution(*args, **kwargs):
    return _batch_generation_workflow_service().enqueue_batch_generation_execution(*args, **kwargs)


def build_batch_generation_request_payload(*args, **kwargs) -> "BatchGenerationRequestPayload":
    return _batch_generation_prompt_service().build_batch_generation_request_payload(*args, **kwargs)


async def build_batch_generation_prompt(*args, **kwargs) -> "BatchGenerationPrompt":
    return await _batch_generation_prompt_service().build_batch_generation_prompt(*args, **kwargs)


async def execute_batch_generation_prompt_stage(*args, **kwargs) -> "BatchGenerationPromptStageResult":
    return await _batch_generation_prompt_service().execute_batch_generation_prompt_stage(*args, **kwargs)


async def prepare_batch_generation_runtime(*args, **kwargs) -> "BatchGenerationRuntimePreparation":
    return await _batch_generation_runtime_service().prepare_batch_generation_runtime(*args, **kwargs)


def finalize_batch_generation_runtime(*args, **kwargs) -> "BatchGenerationResolvedRuntime":
    return _batch_generation_runtime_service().finalize_batch_generation_runtime(*args, **kwargs)


async def build_batch_generation_context(*args, **kwargs) -> "BatchGenerationBuiltContext":
    return await _batch_generation_runtime_service().build_batch_generation_context(*args, **kwargs)


async def resolve_batch_generation_chapter_runtime(*args, **kwargs) -> "BatchGenerationResolvedRuntime":
    return await _batch_generation_runtime_service().resolve_batch_generation_chapter_runtime(*args, **kwargs)


def build_batch_generation_candidate_quality_hooks(
    *args,
    **kwargs,
) -> "BatchGenerationCandidateQualityHooks":
    return _batch_generation_candidate_service().build_batch_generation_candidate_quality_hooks(*args, **kwargs)


def build_batch_generation_candidate_runtime_state(*args, **kwargs):
    return _batch_generation_candidate_service().build_batch_generation_candidate_runtime_state(*args, **kwargs)


def create_batch_generation_candidate_execution(
    *args,
    **kwargs,
) -> "BatchGenerationCandidateExecution":
    return _batch_generation_candidate_service().create_batch_generation_candidate_execution(*args, **kwargs)


async def wait_for_batch_generation_candidate(*args, **kwargs):
    if "publish_stream_event_fn" not in kwargs or kwargs["publish_stream_event_fn"] is None:
        kwargs["publish_stream_event_fn"] = publish_task_stream_event
    return await _batch_generation_candidate_service().wait_for_batch_generation_candidate(*args, **kwargs)


async def emit_batch_generation_selected_candidate_events(*args, **kwargs):
    if "publish_stream_event_fn" not in kwargs or kwargs["publish_stream_event_fn"] is None:
        kwargs["publish_stream_event_fn"] = publish_task_stream_event
    return await _batch_generation_candidate_service().emit_batch_generation_selected_candidate_events(*args, **kwargs)


def build_batch_generation_selected_candidate_result(*args, **kwargs):
    return _batch_generation_candidate_service().build_batch_generation_selected_candidate_result(*args, **kwargs)


async def execute_batch_generation_candidate_flow(
    *args,
    **kwargs,
) -> "BatchGenerationCandidateFlowResult":
    if "emit_selected_candidate_events_fn" not in kwargs or kwargs["emit_selected_candidate_events_fn"] is None:
        kwargs["emit_selected_candidate_events_fn"] = emit_batch_generation_selected_candidate_events
    return await _batch_generation_candidate_service().execute_batch_generation_candidate_flow(*args, **kwargs)


async def execute_batch_generation_generation_stage(*args, **kwargs):
    if "publish_stream_event_fn" not in kwargs or kwargs["publish_stream_event_fn"] is None:
        kwargs["publish_stream_event_fn"] = publish_task_stream_event
    if "execute_candidate_flow_fn" not in kwargs or kwargs["execute_candidate_flow_fn"] is None:
        kwargs["execute_candidate_flow_fn"] = execute_batch_generation_candidate_flow
    return await _batch_generation_candidate_service().execute_batch_generation_generation_stage(*args, **kwargs)


def build_batch_generation_single_chapter_request(**kwargs) -> "BatchGenerationSingleChapterRequest":
    return _batch_generation_single_chapter_wiring_service().build_batch_generation_single_chapter_request(**kwargs)


def build_batch_generation_single_chapter_dependencies(
    **kwargs,
) -> "BatchGenerationSingleChapterDependencies":
    return _batch_generation_single_chapter_wiring_service().build_batch_generation_single_chapter_dependencies(**kwargs)


async def generate_single_chapter_for_batch_workflow(
    *,
    request: "BatchGenerationSingleChapterRequest",
    dependencies: "BatchGenerationSingleChapterDependencies",
):
    return await _batch_generation_single_chapter_wiring_service().generate_single_chapter_for_batch_workflow(
        request=request,
        dependencies=dependencies,
    )


def build_default_batch_generation_single_chapter_dependencies(*args, **kwargs):
    return _batch_generation_single_chapter_wiring_service().build_default_batch_generation_single_chapter_dependencies(
        *args,
        **kwargs,
    )


async def generate_single_chapter_for_batch_with_default_wiring(*args, **kwargs):
    return await _batch_generation_single_chapter_wiring_service().generate_single_chapter_for_batch_with_default_wiring(
        *args,
        **kwargs,
    )
