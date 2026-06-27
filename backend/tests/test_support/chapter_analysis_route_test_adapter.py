"""Test-only chapter analysis router after production route-shell closeout."""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Sequence

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request
from pydantic import BaseModel

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.chapter import Chapter
    from migrator_app.models import ChapterDraftAttempt, GenerationHistory
    from migrator_app.models import PlotAnalysis, StoryMemory
    from migrator_app.models.project import Project
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_packet_test_support import StoryPacket

router = APIRouter(prefix="/chapters", tags=["章节管理"])
logger = get_logger(__name__)


class BatchAnalysisStatusRequest(BaseModel):
    chapter_ids: List[str]


@dataclass(frozen=True)
class ManualChapterAnalysisPreparation:
    task_id: str
    quality_profile: Dict[str, Any]
    story_packet: "StoryPacket"


async def prepare_manual_chapter_analysis_with_default_route_wiring(
    db_session: "AsyncSession",
    *,
    chapter: "Chapter",
    project: "Project",
    user_id: str,
) -> Optional[ManualChapterAnalysisPreparation]:
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity,
    )
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )
    from tests.test_support.analysis_task_test_support import (
        create_analysis_task_safely,
    )

    analysis_quality_profile = await resolve_chapter_quality_profile(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=None,
        enable_mcp=True,
        prefer_project_default_style=True,
        log_prefix="章节分析",
    )
    analysis_story_packet = await build_story_generation_packet_with_project_continuity(
        db_session,
        project,
        source_label="manual-analysis-request",
    )
    analysis_task = await create_analysis_task_safely(
        db_session,
        chapter_id=chapter.id,
        user_id=user_id,
        project_id=project.id,
        log_context="manual-analysis",
    )
    if analysis_task is None:
        return None

    return ManualChapterAnalysisPreparation(
        task_id=analysis_task.id,
        quality_profile=analysis_quality_profile,
        story_packet=analysis_story_packet,
    )


async def get_analysis_task_status_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    db_session: "AsyncSession",
):
    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404,
        require_authenticated_user_id,
    )
    from tests.test_support.analysis_task_query_test_support import (
        load_latest_analysis_task_for_chapter,
    )
    from tests.test_support.analysis_task_status_test_support import (
        build_analysis_task_status_payload,
    )

    user_id = require_authenticated_user_id(request)
    await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    task = await load_latest_analysis_task_for_chapter(
        db_session,
        chapter_id=chapter_id,
    )
    status_result = build_analysis_task_status_payload(chapter_id, task)
    if status_result.changed:
        await db_session.commit()

    return status_result.payload


async def get_batch_analysis_task_status_with_default_route_wiring(
    *,
    chapter_ids_input,
    request: Request | None,
    db_session: "AsyncSession" | None,
):
    from tests.test_support.chapter_route_helpers_test_support import (
        require_authenticated_user_id,
    )
    from tests.test_support.api_common_test_support import verify_project_access
    from tests.test_support.analysis_task_query_test_support import (
        build_empty_batch_analysis_status_response,
        load_batch_analysis_status_query_context,
        normalize_batch_analysis_chapter_ids,
    )
    from tests.test_support.analysis_task_status_test_support import (
        build_batch_analysis_status_items,
    )

    chapter_ids = normalize_batch_analysis_chapter_ids(chapter_ids_input)
    if not chapter_ids:
        return build_empty_batch_analysis_status_response()

    if request is None or db_session is None:
        raise ValueError("request and db_session are required when chapter_ids are provided")

    query_context = await load_batch_analysis_status_query_context(
        db_session,
        chapter_ids=chapter_ids,
    )

    user_id = require_authenticated_user_id(request)
    for project_id in {chapter.project_id for chapter in query_context.chapters}:
        await verify_project_access(project_id, user_id, db_session)

    status_items_result = build_batch_analysis_status_items(
        chapter_ids,
        latest_tasks_by_chapter_id=query_context.latest_tasks_by_chapter_id,
    )
    if status_items_result.changed:
        await db_session.commit()

    return {
        "project_id": query_context.response_project_id,
        "total": len(status_items_result.items),
        "items": status_items_result.items,
    }


async def check_can_generate_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    db_session: "AsyncSession",
):
    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404,
        require_authenticated_user_id,
    )
    from tests.test_support.chapter_query_test_support import (
        check_chapter_generation_prerequisites,
    )

    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    can_generate, error_message, previous_chapters = await check_chapter_generation_prerequisites(
        db_session,
        chapter,
    )
    previous_info = [
        {
            "id": previous_chapter.id,
            "chapter_number": previous_chapter.chapter_number,
            "title": previous_chapter.title,
            "has_content": bool(previous_chapter.content and previous_chapter.content.strip()),
            "word_count": previous_chapter.word_count or 0,
        }
        for previous_chapter in previous_chapters
    ]

    return {
        "can_generate": can_generate,
        "reason": error_message if not can_generate else "",
        "previous_chapters": previous_info,
        "chapter_number": chapter.chapter_number,
    }


async def trigger_chapter_analysis_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db_session: "AsyncSession",
    user_ai_service: "AIService",
):
    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404,
        require_authenticated_user_id,
    )
    from tests.test_support.api_common_test_support import verify_project_access

    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    if not chapter.content or chapter.content.strip() == "":
        raise HTTPException(status_code=400, detail="章节不存在或内容为空")

    project = await verify_project_access(chapter.project_id, user_id, db_session)
    manual_analysis = await prepare_manual_chapter_analysis_with_default_route_wiring(
        db_session,
        chapter=chapter,
        project=project,
        user_id=user_id,
    )
    if manual_analysis is None:
        raise HTTPException(
            status_code=409,
            detail="Chapter or project was deleted before analysis task creation",
        )

    logger.info("Created analysis task: %s, chapter=%s", manual_analysis.task_id, chapter_id)
    await asyncio.sleep(3)

    background_tasks.add_task(
        execute_chapter_analysis_background,
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=project.id,
        task_id=manual_analysis.task_id,
        ai_service=user_ai_service,
        quality_profile=manual_analysis.quality_profile,
        story_packet=manual_analysis.story_packet,
    )

    return {
        "task_id": manual_analysis.task_id,
        "chapter_id": chapter_id,
        "status": "pending",
        "message": "章节分析任务已创建",
    }


def parse_checker_result_from_history(generated_content: Optional[str]) -> Optional[Dict[str, Any]]:
    if not generated_content:
        return None
    try:
        payload = json.loads(generated_content)
        if not isinstance(payload, dict):
            return None
        if payload.get("log_type") != "chapter_text_checker_v1":
            return None
        checker_result = payload.get("checker_result")
        if isinstance(checker_result, dict):
            return checker_result
    except Exception:
        return None
    return None


def _serialize_memories(memories: Sequence["StoryMemory"]) -> List[Dict[str, Any]]:
    return [
        {
            "id": memory.id,
            "type": memory.memory_type,
            "title": memory.title,
            "content": memory.content,
            "importance": memory.importance_score,
            "tags": memory.tags,
            "is_foreshadow": memory.is_foreshadow,
            "position": memory.chapter_position,
            "related_characters": memory.related_characters,
        }
        for memory in memories
    ]


def build_chapter_analysis_payload(
    *,
    chapter: "Chapter",
    analysis: "PlotAnalysis",
    memories: Sequence["StoryMemory"],
    histories: Sequence["GenerationHistory"],
    candidate_attempt: Optional["ChapterDraftAttempt"],
    include_full_draft: bool = False,
) -> Dict[str, Any]:
    from tests.test_support.chapter_generation_history_test_support import (
        _build_candidate_draft_payload as build_candidate_draft_payload,
        build_auto_revision_draft_payload,
        parse_reviser_result_from_history,
    )
    from tests.test_support.chapter_quality_metrics_query_test_support import (
        extract_quality_metrics_from_history_payload,
    )
    from tests.test_support.story_repair_payload_test_support import (
        build_batch_quality_metrics_summary,
    )

    latest_checker_result: Optional[Dict[str, Any]] = None
    latest_reviser_result: Optional[Dict[str, Any]] = None
    checker_created_at: Optional[str] = None
    latest_reviser_created_at: Optional[datetime] = None
    latest_reviser_history_id: Optional[str] = None

    for history in histories:
        if latest_checker_result is None:
            parsed_checker = parse_checker_result_from_history(history.generated_content)
            if parsed_checker:
                latest_checker_result = parsed_checker
                checker_created_at = history.created_at.isoformat() if history.created_at else None
        if latest_reviser_result is None:
            parsed_reviser = parse_reviser_result_from_history(history.generated_content)
            if parsed_reviser:
                latest_reviser_result = parsed_reviser
                latest_reviser_created_at = history.created_at
                latest_reviser_history_id = history.id
        if latest_checker_result is not None and latest_reviser_result is not None:
            break

    auto_revision_draft = None
    if latest_reviser_result:
        auto_revision_draft = build_auto_revision_draft_payload(
            reviser_result=latest_reviser_result,
            history_id=latest_reviser_history_id,
            created_at=latest_reviser_created_at,
            chapter_updated_at=chapter.updated_at,
            include_full_text=include_full_draft,
        )

    quality_metrics_history = [
        metrics
        for metrics in (
            extract_quality_metrics_from_history_payload(history.generated_content)
            for history in histories
        )
        if metrics
    ]
    latest_quality_metrics = quality_metrics_history[0] if quality_metrics_history else None
    quality_metrics_summary = build_batch_quality_metrics_summary(quality_metrics_history)

    candidate_draft = (
        build_candidate_draft_payload(
            draft_attempt=candidate_attempt,
            chapter_updated_at=chapter.updated_at,
            include_full_text=include_full_draft,
        )
        if candidate_attempt is not None
        else None
    )

    return {
        "chapter_id": chapter.id,
        "analysis": analysis.to_dict(),
        "memories": _serialize_memories(memories),
        "checker_result": latest_checker_result,
        "checker_created_at": checker_created_at,
        "auto_revision_draft": auto_revision_draft,
        "candidate_draft": candidate_draft,
        "quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "created_at": analysis.created_at.isoformat() if analysis.created_at else None,
    }


async def get_db(request: Request):
    from tests.test_support.database_test_support import get_db as app_get_db

    async for session in app_get_db(request):
        yield session


async def get_user_ai_service(request: Request, db=Depends(get_db)):
    from tests.test_support.ai_dependencies_test_support import (
        get_user_ai_service as app_get_user_ai_service,
        require_login,
    )

    return await app_get_user_ai_service(user=require_login(request), db=db)


async def execute_chapter_analysis_background(*args, **kwargs):
    from tests.test_support.manual_chapter_analysis_execution_test_support import (
        execute_chapter_analysis_background as execute_chapter_analysis_background_service,
    )

    return await execute_chapter_analysis_background_service(*args, **kwargs)


async def get_chapter_analysis_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    include_full_draft: bool,
    db_session: "AsyncSession",
):
    from sqlalchemy import select

    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404,
        require_authenticated_user_id,
    )
    from migrator_app.models import PlotAnalysis, StoryMemory
    from migrator_app.models import GenerationHistory
    from tests.test_support.chapter_generation_history_test_support import (
        _load_latest_candidate_draft_attempt,
    )

    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    analysis_result = await db_session.execute(
        select(PlotAnalysis)
        .where(PlotAnalysis.chapter_id == chapter_id)
        .order_by(PlotAnalysis.created_at.desc())
        .limit(1)
    )
    analysis = analysis_result.scalar_one_or_none()
    if analysis is None:
        raise HTTPException(status_code=404, detail="未找到章节分析结果")

    memories_result = await db_session.execute(
        select(StoryMemory)
        .where(StoryMemory.chapter_id == chapter_id)
        .order_by(StoryMemory.importance_score.desc())
    )
    memories = memories_result.scalars().all()

    history_result = await db_session.execute(
        select(GenerationHistory)
        .where(GenerationHistory.chapter_id == chapter_id)
        .order_by(GenerationHistory.created_at.desc())
        .limit(30)
    )
    histories = history_result.scalars().all()

    candidate_attempt = await _load_latest_candidate_draft_attempt(db_session, chapter_id)
    return build_chapter_analysis_payload(
        chapter=chapter,
        analysis=analysis,
        memories=memories,
        histories=histories,
        candidate_attempt=candidate_attempt,
        include_full_draft=include_full_draft,
    )


@router.get("/{chapter_id}/analysis", summary="获取章节分析")
async def get_chapter_analysis(
    chapter_id: str,
    request: Request,
    include_full_draft: bool = Query(False, description="是否包含完整草稿"),
    db: "AsyncSession" = Depends(get_db),
):
    return await get_chapter_analysis_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        include_full_draft=include_full_draft,
        db_session=db,
    )


@router.get("/{chapter_id}/analysis/status", summary="查询章节分析任务状态")
async def get_analysis_task_status(
    chapter_id: str,
    request: Request,
    db: "AsyncSession" = Depends(get_db),
):
    return await get_analysis_task_status_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        db_session=db,
    )


@router.post("/analysis/status/batch", summary="批量查询章节分析任务状态")
async def get_batch_analysis_task_status(
    data: BatchAnalysisStatusRequest,
    request: Request,
    db: "AsyncSession" = Depends(get_db),
):
    return await get_batch_analysis_task_status_with_default_route_wiring(
        chapter_ids_input=data.chapter_ids,
        request=request,
        db_session=db,
    )


@router.get("/{chapter_id}/can-generate", summary="检查章节是否可以生成")
async def check_can_generate(
    chapter_id: str,
    request: Request,
    db: "AsyncSession" = Depends(get_db),
):
    return await check_can_generate_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        db_session=db,
    )


@router.post("/{chapter_id}/analyze", summary="手动触发章节分析")
async def trigger_chapter_analysis(
    chapter_id: str,
    request: Request,
    background_tasks: BackgroundTasks,
    db: "AsyncSession" = Depends(get_db),
    user_ai_service: "AIService" = Depends(get_user_ai_service),
):
    return await trigger_chapter_analysis_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        background_tasks=background_tasks,
        db_session=db,
        user_ai_service=user_ai_service,
    )




