from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List

from fastapi import BackgroundTasks, HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter import Chapter
from app.models.project import Project
from app.services.ai_service import AIService
from app.services.batch_generation_workflow_service import (
    calculate_estimated_time,
    create_batch_generation_task_record,
    enqueue_batch_generation_execution,
)


@dataclass(frozen=True)
class BatchGenerationCreatePreparation:
    chapters_to_generate: List[Chapter]
    batch_quality_profile: Dict[str, Any]
    batch_story_packet: Any
    batch_story_repair_state: Dict[str, Any]


async def prepare_batch_generation_create(
    db_session: AsyncSession,
    *,
    project_id: str,
    project: Project,
    user_id: str,
    batch_request: Any,
    check_prerequisites_fn,
    resolve_quality_profile_fn,
    build_story_packet_fn,
    resolve_story_repair_state_fn,
) -> BatchGenerationCreatePreparation:
    result = await db_session.execute(
        select(Chapter)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number)
    )
    all_chapters = result.scalars().all()
    if not all_chapters:
        raise HTTPException(status_code=404, detail='项目下暂无章节')

    start_number = batch_request.start_chapter_number
    end_number = start_number + batch_request.count - 1
    chapters_to_generate = [
        chapter for chapter in all_chapters
        if start_number <= chapter.chapter_number <= end_number
    ]
    if not chapters_to_generate:
        raise HTTPException(status_code=404, detail='未找到指定范围内的章节')

    first_chapter = chapters_to_generate[0]
    can_generate, error_msg, _ = await check_prerequisites_fn(db_session, first_chapter)
    if not can_generate:
        raise HTTPException(status_code=400, detail=f'批量生成前置检查未通过：{error_msg}')

    batch_quality_profile = await resolve_quality_profile_fn(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=batch_request.style_id,
        enable_mcp=True,
        prefer_project_default_style=not bool(batch_request.style_id),
        log_prefix='批量生成',
    )
    batch_story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        project_id=project_id,
        before_chapter_number=start_number,
        story_repair_summary=batch_request.story_repair_summary,
        story_repair_targets=batch_request.story_repair_targets,
        story_preserve_strengths=batch_request.story_preserve_strengths,
    )
    batch_story_packet = await build_story_packet_fn(
        db_session,
        project,
        source=batch_request,
        source_label='batch-generate-request',
    )
    return BatchGenerationCreatePreparation(
        chapters_to_generate=chapters_to_generate,
        batch_quality_profile=(dict(batch_quality_profile) if isinstance(batch_quality_profile, dict) else {}),
        batch_story_packet=batch_story_packet,
        batch_story_repair_state=(dict(batch_story_repair_state) if isinstance(batch_story_repair_state, dict) else {}),
    )


async def create_batch_generation_and_enqueue(
    db_session: AsyncSession,
    *,
    project_id: str,
    user_id: str,
    batch_request: Any,
    preparation: BatchGenerationCreatePreparation,
    background_tasks: BackgroundTasks,
    ai_service: AIService,
    execution_callable: Callable[..., Any],
    sync_task_story_repair_state_fn,
) -> Dict[str, Any]:
    batch_task = await create_batch_generation_task_record(
        db_session,
        project_id=project_id,
        user_id=user_id,
        start_chapter_number=batch_request.start_chapter_number,
        chapter_ids=[chapter.id for chapter in preparation.chapters_to_generate],
        style_id=preparation.batch_quality_profile.get('resolved_style_id'),
        target_word_count=batch_request.target_word_count,
        enable_analysis=batch_request.enable_analysis,
        max_retries=batch_request.max_retries,
    )
    batch_id = batch_task.id

    enqueue_batch_generation_execution(
        background_tasks,
        execution_callable,
        batch_id=batch_id,
        user_id=user_id,
        ai_service=ai_service,
        custom_model=batch_request.model,
        story_packet=preparation.batch_story_packet,
        base_quality_profile=preparation.batch_quality_profile,
        enable_web_research=batch_request.enable_web_research,
        web_research_query=batch_request.web_research_query,
        story_repair_payload=preparation.batch_story_repair_state.get('payload'),
    )
    await sync_task_story_repair_state_fn(
        batch_id,
        story_repair_state=preparation.batch_story_repair_state,
        db_session=db_session,
    )
    estimated_time = calculate_estimated_time(
        chapter_count=len(preparation.chapters_to_generate),
        target_word_count=batch_request.target_word_count,
        enable_analysis=batch_request.enable_analysis,
    )
    return {
        'batch_id': batch_id,
        'message': f'已创建批量生成任务，共 {len(preparation.chapters_to_generate)} 章',
        'chapters_to_generate': [
            {
                'id': chapter.id,
                'chapter_number': chapter.chapter_number,
                'title': chapter.title,
            }
            for chapter in preparation.chapters_to_generate
        ],
        'estimated_time_minutes': estimated_time,
    }
