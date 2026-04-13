from __future__ import annotations

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import chapters as chapters_api
from app.api.outlines import cancel_outline_postprocess_tasks
from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.schemas.chapter import ChapterGenerateRequest
from app.services.ai_service import AIService
from app.services.batch_generation_orchestration_service import (
    orchestrate_single_chapter_background_generation,
)
from app.services.chapter_generation_background_entry_service import (
    generate_chapter_content_background_with_default_wiring,
)
from app.services.batch_generation_query_service import (
    build_batch_task_workflow_snapshot,
)
from app.services.chapter_generation_prerequisite_service import (
    check_chapter_generation_prerequisites,
)
from app.services.chapter_generation_stream_entry_service import (
    generate_chapter_content_stream_with_default_wiring,
)
from app.services.story_repair_payload_service import (
    resolve_generation_story_repair_state_for_chapter,
)
from app.services.task_workflow_runtime_service import (
    sync_task_story_repair_state,
)

router = APIRouter(prefix="/chapters", tags=["chapters"])


@router.post("/{chapter_id}/generate-stream", summary="AI stream chapter generation")
async def generate_chapter_content_stream(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    return await generate_chapter_content_stream_with_default_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        generate_request=generate_request,
        user_ai_service=user_ai_service,
        get_db_fn=chapters_api.get_db,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
        candidate_generator_fn=chapters_api._generate_best_ranked_candidate,
        candidate_rerank_limit=chapters_api.CHAPTER_CANDIDATE_RERANK_LIMIT,
        one_to_one_builder_cls=chapters_api.OneToOneContextBuilder,
        one_to_many_builder_cls=chapters_api.OneToManyContextBuilder,
        build_runtime_system_prompt_fn=chapters_api._build_chapter_runtime_system_prompt,
        detect_style_profile_fn=chapters_api._detect_style_profile,
        resolve_generation_temperature_fn=chapters_api._resolve_generation_temperature,
        compute_story_quality_metrics_fn=chapters_api.compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=chapters_api._resolve_quality_gate_execution_plan,
        analyze_chapter_background_fn=chapters_api.analyze_chapter_background,
        heartbeat_interval_seconds=chapters_api.CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
    )


@router.post("/{chapter_id}/generate-background", summary="AI background chapter generation")
async def generate_chapter_content_background(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    user_id = require_authenticated_user_id(request)

    return await generate_chapter_content_background_with_default_wiring(
        db_session=db,
        chapter_id=chapter_id,
        user_id=user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        ai_service=user_ai_service,
        load_accessible_chapter_or_404_fn=load_accessible_chapter_or_404,
        check_prerequisites_fn=check_chapter_generation_prerequisites,
        build_workflow_snapshot_fn=build_batch_task_workflow_snapshot,
        resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_chapter,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        execution_callable=chapters_api.execute_batch_generation_in_order,
    )
