"""Background chapter analysis execution service."""

from __future__ import annotations

import asyncio
import json
from datetime import datetime
from typing import Any, Dict, Optional

from sqlalchemy import select

from app.database import get_session_factory
from app.logger import get_logger
from app.models.analysis_task import AnalysisTask
from app.models.chapter import Chapter
from app.models.character import Character
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.outline import Outline
from app.models.project import Project
from app.services.ai_service import AIService
from app.services.character_context_service import build_characters_info_with_careers
from app.services.chapter_analysis_support_service import (
    build_checker_history_payload,
    build_checker_report_text,
    build_reviser_history_payload,
    get_chapter_analysis_write_lock,
    merge_checker_suggestions,
    run_chapter_text_checker,
    run_chapter_text_reviser,
)
from app.services.chapter_quality_context_service import (
    StoryGenerationGuidance,
    StoryPacket,
    build_story_generation_packet_with_project_continuity,
    resolve_chapter_quality_profile,
)
from app.services.foreshadow_service import foreshadow_service
from app.services.memory_service import memory_service
from app.services.plot_analyzer import PlotAnalyzer
from app.services.story_repair_payload_service import (
    StoryRepairPayload,
    resolve_story_repair_prompt_kwargs,
)

logger = get_logger(__name__)


async def execute_chapter_analysis_background(
    chapter_id: str,
    user_id: str,
    project_id: str,
    task_id: str,
    ai_service: AIService,
    quality_profile: Optional[Dict[str, Any]] = None,
    story_packet: Optional[StoryPacket] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
    chapter_content_override: Optional[str] = None,
    chapter_word_count_override: Optional[int] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
) -> bool:
    """
    后台异步分析章节（支持并发，使用锁保护数据库写入）
    
    Args:
        chapter_id: 章节ID
        user_id: 用户ID
        project_id: 项目ID
        task_id: 任务ID
        ai_service: AI服务实例
        
    Returns:
        bool: True表示分析成功，False表示分析失败
    """
    db_session = None
    write_lock = await get_chapter_analysis_write_lock(user_id)
    resolved_story_repair_kwargs = resolve_story_repair_prompt_kwargs(
        story_repair_payload,
        summary=story_repair_summary,
        targets=story_repair_targets,
        strengths=story_preserve_strengths,
    )
    story_repair_summary = resolved_story_repair_kwargs.get("story_repair_summary")
    story_repair_targets = resolved_story_repair_kwargs.get("story_repair_targets")
    story_preserve_strengths = resolved_story_repair_kwargs.get("story_preserve_strengths")
    
    try:
        logger.info(f"🔍 开始分析章节: {chapter_id}, 任务ID: {task_id}")
        
        # 创建独立数据库会话
        AsyncSessionLocal = await get_session_factory(user_id)
        db_session = AsyncSessionLocal()
        
        # 1. 获取任务（读操作）
        task_result = await db_session.execute(
            select(AnalysisTask).where(AnalysisTask.id == task_id)
        )
        task = task_result.scalar_one_or_none()
        
        if not task:
            logger.error(f"❌ 任务不存在: {task_id}")
            return False
        
        # 更新任务状态（写操作，需要锁）
        async with write_lock:
            task.status = 'running'
            task.started_at = datetime.now()
            task.progress = 10
            await db_session.commit()
        
        # 2. 获取章节信息（读操作）
        chapter_result = await db_session.execute(
            select(Chapter).where(Chapter.id == chapter_id)
        )
        chapter = chapter_result.scalar_one_or_none()
        effective_chapter_content = chapter_content_override if chapter_content_override is not None else (chapter.content if chapter else None)
        if not chapter or not effective_chapter_content:
            async with write_lock:
                task.status = 'failed'
                task.error_message = '章节不存在或正文为空'
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ 章节不存在或正文为空: {chapter_id}")
            return False
        effective_chapter_word_count = int(chapter_word_count_override or chapter.word_count or len(effective_chapter_content))
        async with write_lock:
            task.progress = 20
            await db_session.commit()

        # 获取已埋入的伏笔列表（用于回收匹配，传入当前章节号以启用智能标记）
        project_result = await db_session.execute(
            select(Project).where(Project.id == project_id)
        )
        project = project_result.scalar_one_or_none()
        if not project:
            async with write_lock:
                task.status = 'failed'
                task.error_message = '项目不存在'
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ 项目不存在: {project_id}")
            return False

        chapter_outline_record = None
        chapter_outline_text = ""
        if chapter.outline_id:
            outline_result = await db_session.execute(
                select(Outline).where(Outline.id == chapter.outline_id)
            )
            chapter_outline_record = outline_result.scalar_one_or_none()
            if chapter_outline_record:
                chapter_outline_text = (chapter_outline_record.content or chapter_outline_record.title or "").strip()

        existing_foreshadows = await foreshadow_service.get_planted_foreshadows_for_analysis(
            db=db_session,
            project_id=project_id,
            current_chapter_number=chapter.chapter_number  # 传入当前章节号以启用智能标记
        )
        logger.info(f"📋 后台分析 - 已获取{len(existing_foreshadows)}个已埋入伏笔用于匹配（含智能回收标记）")
        
        # 获取项目角色信息（根据大纲/展开规划筛选本章相关角色）
        filter_character_names = None
        
        # 1-N模式：从expansion_plan中提取character_focus
        if chapter.expansion_plan:
            try:
                plan = json.loads(chapter.expansion_plan)
                focus_names = plan.get('character_focus', [])
                if focus_names:
                    filter_character_names = focus_names
                    logger.info(f"📋 从expansion_plan提取角色焦点: {filter_character_names}")
            except (json.JSONDecodeError, Exception):
                pass
        
        # 1-1模式：从outline.structure中提取characters
        if not filter_character_names and chapter_outline_record and chapter_outline_record.structure:
            try:
                structure = json.loads(chapter_outline_record.structure)
                raw_characters = structure.get('characters', [])
                if raw_characters:
                    filter_character_names = [
                        c['name'] if isinstance(c, dict) else c
                        for c in raw_characters
                    ]
                    logger.info(f"📋 从outline.structure提取角色: {filter_character_names}")
            except (json.JSONDecodeError, Exception):
                pass
        
        # 查询角色（根据筛选名单或全部）
        characters_query = select(Character).where(Character.project_id == project_id)
        if filter_character_names:
            characters_query = characters_query.where(Character.name.in_(filter_character_names))
        characters_result = await db_session.execute(characters_query)
        project_characters = characters_result.scalars().all()
        
        # 如果筛选后无角色，降级为全部角色
        if not project_characters and filter_character_names:
            logger.warning(f"⚠️ 筛选后无匹配角色，降级为全部角色")
            characters_result = await db_session.execute(
                select(Character).where(Character.project_id == project_id)
            )
            project_characters = characters_result.scalars().all()
            filter_character_names = None
        
        characters_info = await build_characters_info_with_careers(
            db=db_session,
            project_id=project_id,
            characters=project_characters,
            filter_character_names=filter_character_names
        )
        logger.info(f"📋 后台分析 - 已获取{len(project_characters)}个角色信息用于分析")

        analysis_quality_profile = quality_profile or await resolve_chapter_quality_profile(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=None,
            enable_mcp=True,
            prefer_project_default_style=True,
            log_prefix="章节分析",
        )
        analysis_story_packet = story_packet
        if analysis_story_packet is None and generation_guidance is not None:
            analysis_story_packet = StoryPacket.from_guidance(
                generation_guidance,
                source="legacy-analysis-guidance",
            )
        if analysis_story_packet is None:
            analysis_story_packet = await build_story_generation_packet_with_project_continuity(
                db_session,
                project,
                source_label="chapter-analysis-defaults",
            )
        analysis_guidance = analysis_story_packet.guidance

        # 定义重试回调函数，用于在重试时更新任务状态
        async def on_retry_callback(attempt: int, max_retries: int, wait_time: int, error_reason: str):
            """重试时更新任务状态，让前端能感知到重试进度"""
            try:
                async with write_lock:
                    # 重新获取任务（确保获取最新状态）
                    task_result_retry = await db_session.execute(
                        select(AnalysisTask).where(AnalysisTask.id == task_id)
                    )
                    task_retry = task_result_retry.scalar_one_or_none()
                    if task_retry:
                        # 更新任务状态，保持 running 但更新 started_at 以重置超时计时器
                        task_retry.status = 'running'
                        task_retry.started_at = datetime.now()  # 重置开始时间，防止超时检测误判
                        task_retry.progress = min(70, 35 + attempt * 15)  # 根据重试次数更新进度
                        task_retry.error_message = f"正在重试({attempt}/{max_retries})：{error_reason[:100]}"
                        await db_session.commit()
                        logger.info(f"🔄 分析任务重试状态已更新: 尝试 {attempt}/{max_retries}, 等待 {wait_time}s, 原因: {error_reason[:50]}...")
            except Exception as callback_error:
                logger.warning(f"⚠️ 更新重试状态失败: {callback_error}")
        
        # 3. 使用PlotAnalyzer分析章节（传入已有伏笔列表、角色信息和重试回调）
        async with write_lock:
            task.progress = 30
            task.error_message = '正在调用AI分析章节...'
            await db_session.commit()

        analyzer = PlotAnalyzer(ai_service)
        analysis_result = await analyzer.analyze_chapter(
            chapter_number=chapter.chapter_number,
            title=chapter.title,
            content=effective_chapter_content,
            word_count=effective_chapter_word_count,
            existing_foreshadows=existing_foreshadows,
            on_retry=on_retry_callback,
            characters_info=characters_info,
            **analysis_story_packet.build_analysis_quality_kwargs(analysis_quality_profile),
        )
        
        if not analysis_result:
            analysis_error_message = analyzer.last_error_message or '章节分析失败，请稍后重试'
            async with write_lock:
                task.status = 'failed'
                task.error_message = analysis_error_message[:500]
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ AI分析失败: {chapter_id}, 原因: {analysis_error_message}")
            return False
        
        skip_followup_enrichment = analysis_result.get("analysis_mode") == "heuristic_fallback"
        checker_result = None
        reviser_result = None
        if skip_followup_enrichment:
            logger.warning(
                "⚠️ 当前分析使用启发式回退，跳过后续检查与润色补强: %s",
                analysis_result.get("fallback_reason") or "unknown",
            )
        else:
            checker_result = await run_chapter_text_checker(
                ai_service=ai_service,
                db_session=db_session,
                user_id=user_id,
                chapter_number=chapter.chapter_number,
                chapter_title=chapter.title or "",
                chapter_content=effective_chapter_content,
                chapter_outline=chapter_outline_text,
                characters_info=characters_info,
                world_rules=project.world_rules or "",
                quality_profile=analysis_quality_profile,
                generation_guidance=analysis_guidance,
            )
            reviser_result = await run_chapter_text_reviser(
                ai_service=ai_service,
                db_session=db_session,
                user_id=user_id,
                chapter_number=chapter.chapter_number,
                chapter_title=chapter.title or "",
                chapter_content=effective_chapter_content,
                checker_result=checker_result or {},
                quality_profile=analysis_quality_profile,
                generation_guidance=analysis_guidance,
            )

        analysis_report_text = analyzer.generate_analysis_summary(analysis_result)
        checker_report_text = build_checker_report_text(checker_result)
        if checker_report_text:
            analysis_report_text = f"{analysis_report_text}\n\n{checker_report_text}"
        if reviser_result:
            draft_priority_issue_count = int(
                reviser_result.get("priority_issue_count")
                or (
                    int(reviser_result.get("critical_count") or 0)
                    + int(reviser_result.get("major_count") or 0)
                )
            )
            draft_applied_issue_count = int(
                reviser_result.get("applied_issue_count")
                or reviser_result.get("applied_critical_count")
                or 0
            )
            reviser_summary_lines = [
                "【第三版自动修订草稿】",
                f"- 高优先问题数：{draft_priority_issue_count}（严重{reviser_result.get('critical_count', 0)} / 中等{reviser_result.get('major_count', 0)}）",
                f"- 已处理问题数：{draft_applied_issue_count}",
                f"- 草稿字数：{reviser_result.get('revised_word_count', 0)}",
                f"- 说明：{reviser_result.get('change_summary', '已生成草稿')}",
            ]
            analysis_report_text = f"{analysis_report_text}\n\n" + "\n".join(reviser_summary_lines)

        merged_suggestions = merge_checker_suggestions(
            analysis_suggestions=analysis_result.get('suggestions', []),
            checker_result=checker_result,
        )
        if reviser_result:
            for unresolved in (reviser_result.get("unresolved_issues") or []):
                if isinstance(unresolved, str) and unresolved.strip():
                    merged_suggestions.append(f"【修订未完成】{unresolved.strip()[:200]}")
                if len(merged_suggestions) >= 16:
                    break
            merged_suggestions = merged_suggestions[:16]

        async with write_lock:
            task.progress = 60
            await db_session.commit()
        
        # 4. 保存分析结果到数据库（写操作，需要锁）
        async with write_lock:
            existing_analysis_result = await db_session.execute(
                select(PlotAnalysis).where(PlotAnalysis.chapter_id == chapter_id)
            )
            existing_analysis = existing_analysis_result.scalar_one_or_none()
            
            if existing_analysis:
                # 更新现有记录
                logger.info(f"  更新现有分析记录: {existing_analysis.id}")
                existing_analysis.plot_stage = analysis_result.get('plot_stage', '发展')
                existing_analysis.conflict_level = analysis_result.get('conflict', {}).get('level', 0)
                existing_analysis.conflict_types = analysis_result.get('conflict', {}).get('types', [])
                existing_analysis.emotional_tone = analysis_result.get('emotional_arc', {}).get('primary_emotion', '')
                existing_analysis.emotional_intensity = analysis_result.get('emotional_arc', {}).get('intensity', 0) / 10.0
                existing_analysis.hooks = analysis_result.get('hooks', [])
                existing_analysis.hooks_count = len(analysis_result.get('hooks', []))
                existing_analysis.foreshadows = analysis_result.get('foreshadows', [])
                existing_analysis.foreshadows_planted = sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'planted')
                existing_analysis.foreshadows_resolved = sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'resolved')
                existing_analysis.plot_points = analysis_result.get('plot_points', [])
                existing_analysis.plot_points_count = len(analysis_result.get('plot_points', []))
                existing_analysis.character_states = analysis_result.get('character_states', [])
                existing_analysis.scenes = analysis_result.get('scenes', [])
                existing_analysis.pacing = analysis_result.get('pacing', 'moderate')
                existing_analysis.overall_quality_score = analysis_result.get('scores', {}).get('overall', 0)
                existing_analysis.pacing_score = analysis_result.get('scores', {}).get('pacing', 0)
                existing_analysis.engagement_score = analysis_result.get('scores', {}).get('engagement', 0)
                existing_analysis.coherence_score = analysis_result.get('scores', {}).get('coherence', 0)
                existing_analysis.analysis_report = analysis_report_text
                existing_analysis.suggestions = merged_suggestions
                existing_analysis.dialogue_ratio = analysis_result.get('dialogue_ratio', 0)
                existing_analysis.description_ratio = analysis_result.get('description_ratio', 0)
            else:
                # 创建新记录
                logger.info(f"  创建新的分析记录")
                plot_analysis = PlotAnalysis(
                    chapter_id=chapter_id,
                    project_id=project_id,
                    plot_stage=analysis_result.get('plot_stage', '发展'),
                    conflict_level=analysis_result.get('conflict', {}).get('level', 0),
                    conflict_types=analysis_result.get('conflict', {}).get('types', []),
                    emotional_tone=analysis_result.get('emotional_arc', {}).get('primary_emotion', ''),
                    emotional_intensity=analysis_result.get('emotional_arc', {}).get('intensity', 0) / 10.0,
                    hooks=analysis_result.get('hooks', []),
                    hooks_count=len(analysis_result.get('hooks', [])),
                    foreshadows=analysis_result.get('foreshadows', []),
                    foreshadows_planted=sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'planted'),
                    foreshadows_resolved=sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'resolved'),
                    plot_points=analysis_result.get('plot_points', []),
                    plot_points_count=len(analysis_result.get('plot_points', [])),
                    character_states=analysis_result.get('character_states', []),
                    scenes=analysis_result.get('scenes', []),
                    pacing=analysis_result.get('pacing', 'moderate'),
                    overall_quality_score=analysis_result.get('scores', {}).get('overall', 0),
                    pacing_score=analysis_result.get('scores', {}).get('pacing', 0),
                    engagement_score=analysis_result.get('scores', {}).get('engagement', 0),
                    coherence_score=analysis_result.get('scores', {}).get('coherence', 0),
                    analysis_report=analysis_report_text,
                    suggestions=merged_suggestions,
                    dialogue_ratio=analysis_result.get('dialogue_ratio', 0),
                    description_ratio=analysis_result.get('description_ratio', 0)
                )
                db_session.add(plot_analysis)

            if checker_result:
                checker_history = GenerationHistory(
                    project_id=project_id,
                    chapter_id=chapter_id,
                    prompt=f"章节质检: 第{chapter.chapter_number}章 {chapter.title or ''}",
                    generated_content=build_checker_history_payload(checker_result),
                    model="chapter_text_checker_v1",
                )
                db_session.add(checker_history)
            if reviser_result:
                reviser_history = GenerationHistory(
                    project_id=project_id,
                    chapter_id=chapter_id,
                    prompt=f"自动修订草稿: 第{chapter.chapter_number}章 {chapter.title or ''}",
                    generated_content=build_reviser_history_payload(reviser_result),
                    model="chapter_text_reviser_v1",
                )
                db_session.add(reviser_history)
            
            await db_session.commit()
            
            task.progress = 80
            await db_session.commit()
        
        # 5. 清理旧的分析伏笔（重新分析时需要先清理）
        try:
            async with write_lock:
                clean_result = await foreshadow_service.clean_chapter_analysis_foreshadows(
                    db=db_session,
                    project_id=project_id,
                    chapter_id=chapter_id
                )
            if clean_result['cleaned_count'] > 0:
                logger.info(f"🧹 重新分析前清理了 {clean_result['cleaned_count']} 个旧伏笔")
        except Exception as clean_error:
            logger.warning(f"⚠️ 清理旧伏笔失败（继续分析）: {str(clean_error)}")
        
        # 6. 提取记忆并保存到向量数据库（传入章节内容用于计算位置）
        memories = analyzer.extract_memories_from_analysis(
            analysis=analysis_result,
            chapter_id=chapter_id,
            chapter_number=chapter.chapter_number,
            chapter_content=effective_chapter_content,
            chapter_title=chapter.title or ""
        )
        
        # 先删除该章节的旧记忆（写操作，需要锁）
        async with write_lock:
            old_memories_result = await db_session.execute(
                select(StoryMemory).where(StoryMemory.chapter_id == chapter_id)
            )
            old_memories = old_memories_result.scalars().all()
            for old_mem in old_memories:
                await db_session.delete(old_mem)
            await db_session.commit()
            logger.info(f"  删除旧记忆: {len(old_memories)}条")
        
        # 准备批量添加的记忆数据（不需要锁）
        memory_records = []
        for mem in memories:
            memory_id = f"{chapter_id}_{mem['type']}_{len(memory_records)}"
            memory_records.append({
                'id': memory_id,
                'content': mem['content'],
                'type': mem['type'],
                'metadata': mem['metadata']
            })
            
        # 保存到关系数据库（写操作，需要锁）
        async with write_lock:
            for mem in memories:
                memory_id = memory_records[memories.index(mem)]['id']
                text_position = mem['metadata'].get('text_position', -1)
                text_length = mem['metadata'].get('text_length', 0)
                
                story_memory = StoryMemory(
                    id=memory_id,
                    project_id=project_id,
                    chapter_id=chapter_id,
                    memory_type=mem['type'],
                    content=mem['content'],
                    title=mem['title'],
                    importance_score=mem['metadata'].get('importance_score', 0.5),
                    tags=mem['metadata'].get('tags', []),
                    is_foreshadow=mem['metadata'].get('is_foreshadow', 0),
                    story_timeline=chapter.chapter_number,
                    chapter_position=text_position,
                    text_length=text_length,
                    related_characters=mem['metadata'].get('related_characters', []),
                    related_locations=mem['metadata'].get('related_locations', [])
                )
                db_session.add(story_memory)
                
                if text_position >= 0:
                    logger.debug(f"  保存记忆 {memory_id}: position={text_position}, length={text_length}")
            
            await db_session.commit()
        
        # 批量添加到向量数据库
        if memory_records:
            added_count = await memory_service.batch_add_memories(
                user_id=user_id,
                project_id=project_id,
                memories=memory_records
            )
            logger.info(f"✅ 添加{added_count}条记忆到向量库")
        
        # 💼 更新角色职业（根据分析结果）
        if analysis_result.get('character_states'):
            try:
                from app.services.career_update_service import CareerUpdateService
                
                logger.info(f"💼 开始根据分析结果更新角色职业...")
                career_update_result = await CareerUpdateService.update_careers_from_analysis(
                    db=db_session,
                    project_id=project_id,
                    character_states=analysis_result.get('character_states', []),
                    chapter_id=chapter_id,
                    chapter_number=chapter.chapter_number
                )
                
                if career_update_result['updated_count'] > 0:
                    logger.info(
                        f"✅ 更新了 {career_update_result['updated_count']} 个角色的职业信息"
                    )
                    if career_update_result['changes']:
                        for change in career_update_result['changes']:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无角色职业变化")
                    
            except Exception as career_error:
                # 职业更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新角色职业失败: {str(career_error)}", exc_info=True)
        else:
            logger.debug("📋 分析结果中无角色状态信息，跳过职业更新")
        
        # 👤 更新角色心理状态和关系（根据分析结果）
        if analysis_result.get('character_states'):
            try:
                from app.services.character_state_update_service import CharacterStateUpdateService
                
                logger.info(f"👤 开始根据分析结果更新角色状态、关系和组织成员...")
                async with write_lock:
                    state_update_result = await CharacterStateUpdateService.update_from_analysis(
                        db=db_session,
                        project_id=project_id,
                        character_states=analysis_result.get('character_states', []),
                        chapter_id=chapter_id,
                        chapter_number=chapter.chapter_number
                    )
                
                total_state_changes = (
                    state_update_result['state_updated_count'] +
                    state_update_result['relationship_created_count'] +
                    state_update_result['relationship_updated_count'] +
                    state_update_result.get('org_updated_count', 0)
                )
                if total_state_changes > 0:
                    logger.info(
                        f"✅ 角色状态更新: 心理状态{state_update_result['state_updated_count']}个, "
                        f"新建关系{state_update_result['relationship_created_count']}个, "
                        f"更新关系{state_update_result['relationship_updated_count']}个, "
                        f"组织变动{state_update_result.get('org_updated_count', 0)}个"
                    )
                    if state_update_result['changes']:
                        for change in state_update_result['changes'][:8]:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无角色状态、关系或组织变化")
                    
            except Exception as state_error:
                # 角色状态更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新角色状态、关系和组织失败: {str(state_error)}", exc_info=True)
        
        # 🏛️ 更新组织自身状态（根据分析结果）
        if analysis_result.get('organization_states'):
            try:
                from app.services.character_state_update_service import CharacterStateUpdateService
                
                logger.info(f"🏛️ 开始根据分析结果更新组织自身状态...")
                async with write_lock:
                    org_state_result = await CharacterStateUpdateService.update_organization_states(
                        db=db_session,
                        project_id=project_id,
                        organization_states=analysis_result.get('organization_states', []),
                        chapter_number=chapter.chapter_number
                    )
                
                if org_state_result['updated_count'] > 0:
                    logger.info(
                        f"✅ 组织状态更新: {org_state_result['updated_count']}个组织"
                    )
                    if org_state_result['changes']:
                        for change in org_state_result['changes'][:5]:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无组织自身状态变化")
                    
            except Exception as org_state_error:
                # 组织状态更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新组织自身状态失败: {str(org_state_error)}", exc_info=True)
        
        # 🔮 自动更新伏笔状态（根据分析结果）
        if analysis_result.get('foreshadows'):
            try:
                logger.info(f"🔮 开始根据分析结果自动更新伏笔状态...")
                async with write_lock:
                    foreshadow_stats = await foreshadow_service.auto_update_from_analysis(
                        db=db_session,
                        project_id=project_id,
                        chapter_id=chapter_id,
                        chapter_number=chapter.chapter_number,
                        analysis_foreshadows=analysis_result.get('foreshadows', [])
                    )
                
                if foreshadow_stats['planted_count'] > 0 or foreshadow_stats['resolved_count'] > 0:
                    logger.info(
                        f"✅ 伏笔自动更新: 埋入{foreshadow_stats['planted_count']}个, "
                        f"回收{foreshadow_stats['resolved_count']}个"
                    )
                else:
                    logger.info("ℹ️ 本章节无新的伏笔状态变化")
                    
            except Exception as foreshadow_error:
                # 伏笔更新失败不应影响整个分析流程
                logger.error(f"⚠️ 自动更新伏笔失败: {str(foreshadow_error)}", exc_info=True)
        else:
            logger.debug("📋 分析结果中无伏笔信息，跳过伏笔自动更新")
        
        # 最终更新任务状态（写操作，需要锁）- 增加重试机制
        update_success = False
        for retry in range(3):
            try:
                async with write_lock:
                    task.progress = 100
                    task.status = 'completed'
                    task.error_message = None
                    task.completed_at = datetime.now()
                    await db_session.commit()
                    update_success = True
                    logger.info(f"✅ 章节分析完成: {chapter_id}, 提取{len(memories)}条记忆")
                    break
            except Exception as commit_error:
                logger.error(f"❌ 提交任务完成状态失败(重试{retry+1}/3): {str(commit_error)}")
                if retry < 2:
                    await asyncio.sleep(0.1)
                else:
                    logger.error(f"❌ 无法更新任务为completed状态: {task_id}")
                    # 即使失败也不抛出异常，因为分析本身已经完成
        
        if not update_success:
            logger.warning(f"⚠️  章节分析完成但状态更新失败: {chapter_id}")
        
        # 返回成功状态
        return True
        
    except Exception as e:
        logger.error(f"❌ 后台分析异常: {str(e)}", exc_info=True)
        # 确保任务状态被更新为failed（写操作，需要锁）
        if db_session:
            # 多次重试更新任务状态
            for retry in range(3):
                try:
                    async with write_lock:
                        # 重新获取任务（可能是旧会话导致的问题）
                        task_result = await db_session.execute(
                            select(AnalysisTask).where(AnalysisTask.id == task_id)
                        )
                        task = task_result.scalar_one_or_none()
                        if task:
                            task.status = 'failed'
                            task.error_message = str(e)[:500]
                            task.completed_at = datetime.now()
                            task.progress = 0
                            await db_session.commit()
                            logger.info(f"✅ 任务状态已更新为failed: {task_id} (重试{retry+1}次)")
                            break
                        else:
                            logger.error(f"❌ 无法找到任务进行状态更新: {task_id}")
                            break
                except Exception as update_error:
                    logger.error(f"❌ 更新任务状态失败(重试{retry+1}/3): {str(update_error)}")
                    if retry < 2:
                        await asyncio.sleep(0.1)  # 短暂等待后重试
                    else:
                        logger.error(f"❌ 任务状态更新失败，已达到最大重试次数: {task_id}")
        
        # 返回失败状态
        return False
        
    finally:
        if db_session:
            await db_session.close()
