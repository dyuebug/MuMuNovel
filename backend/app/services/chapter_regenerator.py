"""章节重新生成服务"""
from typing import Dict, Any, AsyncGenerator, Optional, List, Iterable
from sqlalchemy.ext.asyncio import AsyncSession
from app.services.ai_service import AIService
from app.services.prompt_service import prompt_service, PromptService
from app.models.chapter import Chapter
from app.models.memory import PlotAnalysis
from app.schemas.regeneration import ChapterRegenerateRequest, PreserveElementsConfig
from app.logger import get_logger
import difflib

logger = get_logger(__name__)


FOCUS_AREA_LABELS: Dict[str, str] = {
    "pacing": "节奏把控 - 调整叙事速度，避免拖沓或过快",
    "emotion": "情感渲染 - 深化人物情感表达，增强感染力",
    "description": "场景描写 - 丰富环境细节，增强画面感",
    "dialogue": "对话质量 - 让对话更自然真实，推动剧情",
    "conflict": "冲突强度 - 强化矛盾冲突，提升戏剧张力",
    "outline": "大纲贴合 - 确保当前章节命中本轮目标、变化与收束",
    "rule_grounding": "规则落地 - 把设定限制、代价与结果写进动作链",
    "opening": "开场钩子 - 开头尽快出现目标、异常或受阻",
    "payoff": "回报兑现 - 回收承诺、伏笔或阶段性爽点",
    "cliffhanger": "章尾钩子 - 章末保留更尖锐的未决问题或新失衡",
}

FOCUS_AREA_REPAIR_TARGETS: Dict[str, str] = {
    "pacing": "调整场景节拍，让推进、停顿和转折更均衡。",
    "emotion": "补强角色情绪触发与外露的连续变化。",
    "description": "用场景细节和动作反馈承载信息。",
    "dialogue": "删掉解释型对白，改成带潜台词和立场碰撞的说话方式。",
    "conflict": "补强正面冲突与升级代价，让人物付出真实后果。",
    "outline": "回扣本轮大纲关键任务、变化与收束。",
    "rule_grounding": "把规则限制、风险代价和结果反馈落到动作链里。",
    "opening": "把开头改成更快入场的异常 / 目标 / 受阻起手。",
    "payoff": "回收前文承诺、伏笔或阶段性期待。",
    "cliffhanger": "章尾保留未决选择、新失衡或更高一级的问题。",
}

AUTO_FOCUS_KEYWORDS: Dict[str, tuple[str, ...]] = {
    "pacing": ("节奏", "拖沓", "过快", "冗长", "跳切"),
    "emotion": ("情感", "情绪", "感染力", "共鸣"),
    "description": ("描写", "场景", "画面", "环境细节"),
    "dialogue": ("对白", "对话", "台词", "说话"),
    "conflict": ("冲突", "矛盾", "对抗", "张力", "代价"),
    "outline": ("大纲", "主线", "偏离", "跑题", "结构"),
    "rule_grounding": ("规则", "设定", "世界观", "逻辑", "约束"),
    "opening": ("开头", "开场", "起手", "前300字"),
    "payoff": ("回收", "回报", "兑现", "爽点", "伏笔回收"),
    "cliffhanger": ("章尾", "结尾", "悬念", "收束", "牵引"),
}


def _dedupe_text_items(items: Iterable[str], *, limit: int | None = None) -> List[str]:
    normalized_items: List[str] = []
    seen: set[str] = set()
    for item in items:
        text = str(item or "").strip()
        if not text:
            continue
        key = text.casefold()
        if key in seen:
            continue
        seen.add(key)
        normalized_items.append(text)
        if limit and len(normalized_items) >= limit:
            break
    return normalized_items


def _normalize_focus_areas(areas: Iterable[str]) -> List[str]:
    normalized: List[str] = []
    seen: set[str] = set()
    for area in areas:
        value = str(area or "").strip().lower()
        if not value or value not in FOCUS_AREA_LABELS or value in seen:
            continue
        seen.add(value)
        normalized.append(value)
    return normalized


def _infer_focus_areas_from_texts(texts: Iterable[str]) -> List[str]:
    combined = "\n".join(str(text or "").strip().lower() for text in texts if str(text or "").strip())
    if not combined:
        return []

    inferred: List[str] = []
    for area, keywords in AUTO_FOCUS_KEYWORDS.items():
        if any(keyword in combined for keyword in keywords):
            inferred.append(area)
    return _normalize_focus_areas(inferred)


class ChapterRegenerator:
    """章节重新生成服务"""
    
    def __init__(self, ai_service: AIService):
        self.ai_service = ai_service
        logger.info("✅ ChapterRegenerator初始化成功")
    
    async def regenerate_with_feedback(
        self,
        chapter: Chapter,
        analysis: Optional[PlotAnalysis],
        regenerate_request: ChapterRegenerateRequest,
        project_context: Dict[str, Any],
        style_content: str = "",
        user_id: str = None,
        db: AsyncSession = None
    ) -> AsyncGenerator[Dict[str, Any], None]:
        """
        根据反馈重新生成章节（流式）
        
        Args:
            chapter: 原始章节对象
            analysis: 分析结果（可选）
            regenerate_request: 重新生成请求参数
            project_context: 项目上下文（项目信息、角色、大纲等）
            style_content: 写作风格
            user_id: 用户ID（用于获取自定义提示词）
            db: 数据库会话（用于查询自定义提示词）
        
        Yields:
            包含类型和数据的字典: {'type': 'progress'/'chunk', 'data': ...}
        """
        try:
            logger.info(f"🔄 开始重新生成章节: 第{chapter.chapter_number}章")
            
            # 1. 构建修改指令
            yield {'type': 'progress', 'progress': 5, 'message': '正在构建修改指令...'}
            modification_instructions = self._build_modification_instructions(
                analysis=analysis,
                regenerate_request=regenerate_request
            )
            
            logger.info(f"📝 修改指令构建完成，长度: {len(modification_instructions)}字符")
            
            # 2. 构建完整提示词
            yield {'type': 'progress', 'progress': 10, 'message': '正在构建生成提示词...'}
            full_prompt = await self._build_regeneration_prompt(
                chapter=chapter,
                modification_instructions=modification_instructions,
                project_context=project_context,
                regenerate_request=regenerate_request,
                style_content=style_content,
                user_id=user_id,
                db=db
            )

            logger.info(f"🎯 提示词构建完成，开始AI生成")
            yield {'type': 'progress', 'progress': 15, 'message': '开始AI生成内容...'}
            
            # 3. 构建系统提示词（注入写作风格）
            system_prompt_with_style = None
            if style_content:
                extra_guard = ""
                if "连载感" in style_content:
                    extra_guard = (
                        "\n连载优化要求：\n"
                        "- 情绪推进至少经历“触发→压住/回避→外露”中的两个阶段\n"
                        "- 对话要区分角色声线，避免全员同一种书面表达\n"
                        "- 章末保留自然压力点，不要硬反转"
                    )
                elif "生活化" in style_content:
                    extra_guard = (
                        "\n生活化优化要求：\n"
                        "- 用动作和反应传达情绪，少写抽象总结\n"
                        "- 允许少量口语毛边，避免句句工整"
                    )

                system_prompt_with_style = f"""【🎨 写作风格参考】

{style_content}

请优先贴合上述写作风格进行重写。
整体语气尽量保持一致，自然表达，不要写成模板腔。{extra_guard}"""
                logger.info(f"✅ 已将写作风格注入系统提示词（{len(style_content)}字符）")
            
            # 4. 流式生成新内容，同时跟踪进度
            target_word_count = regenerate_request.target_word_count
            accumulated_length = 0
            
            async for chunk in self.ai_service.generate_text_stream(
                prompt=full_prompt,
                system_prompt=system_prompt_with_style,
                temperature=0.7
            ):
                # 发送内容块
                yield {'type': 'chunk', 'content': chunk}
                
                # 更新累积字数并计算进度（15%-95%）
                accumulated_length += len(chunk)
                # 进度从15%开始，到95%结束，为后处理预留5%
                generation_progress = min(15 + (accumulated_length / target_word_count) * 80, 95)
                yield {'type': 'progress', 'progress': int(generation_progress), 'word_count': accumulated_length}
            
            logger.info(f"✅ 章节重新生成完成，共生成 {accumulated_length} 字")
            yield {'type': 'progress', 'progress': 100, 'message': '生成完成'}
            
        except Exception as e:
            logger.error(f"❌ 重新生成失败: {str(e)}", exc_info=True)
            raise
    
    def _build_modification_instructions(
        self,
        analysis: Optional[PlotAnalysis],
        regenerate_request: ChapterRegenerateRequest
    ) -> str:
        """??????"""

        instructions: List[str] = []
        instructions.append("# ??????\n")

        selected_suggestions: List[str] = []
        if analysis and regenerate_request.selected_suggestion_indices and analysis.suggestions:
            for idx in regenerate_request.selected_suggestion_indices:
                if 0 <= idx < len(analysis.suggestions):
                    selected_suggestions.append(str(analysis.suggestions[idx]).strip())

        custom_instructions = str(regenerate_request.custom_instructions or "").strip()
        explicit_focus_areas = _normalize_focus_areas(regenerate_request.focus_areas)
        inferred_focus_areas = _infer_focus_areas_from_texts([
            *selected_suggestions,
            custom_instructions,
        ])

        if analysis:
            if getattr(analysis, "conflict_level", None) is not None and (analysis.conflict_level or 0) < 6:
                inferred_focus_areas.append("conflict")
            if getattr(analysis, "pacing_score", None) is not None and (analysis.pacing_score or 0) < 6.5:
                inferred_focus_areas.append("pacing")
            if getattr(analysis, "coherence_score", None) is not None and (analysis.coherence_score or 0) < 6.5:
                inferred_focus_areas.append("outline")
            if getattr(analysis, "dialogue_ratio", None) is not None and (analysis.dialogue_ratio or 0) < 0.18:
                inferred_focus_areas.append("dialogue")

        effective_focus_areas = _normalize_focus_areas([*explicit_focus_areas, *inferred_focus_areas])
        auto_added_focus_areas = [area for area in effective_focus_areas if area not in explicit_focus_areas]

        repair_summary = str(getattr(regenerate_request, "story_repair_summary", "") or "").strip()
        repair_targets = _dedupe_text_items(getattr(regenerate_request, "story_repair_targets", []) or [], limit=3)
        preserve_strengths = _dedupe_text_items(getattr(regenerate_request, "story_preserve_strengths", []) or [], limit=2)

        if not repair_targets and auto_added_focus_areas:
            repair_targets = [
                FOCUS_AREA_REPAIR_TARGETS[area]
                for area in auto_added_focus_areas
                if area in FOCUS_AREA_REPAIR_TARGETS
            ][:3]

        if not repair_summary and repair_targets:
            repair_summary = f"本轮优先修复：{repair_targets[0]}，不要只做表面润色。"

        if selected_suggestions:
            instructions.append("## ?? ??????????AI????\n")
            for idx, suggestion in enumerate(selected_suggestions, start=1):
                instructions.append(f"{idx}. {suggestion}")
            instructions.append("")

        if custom_instructions:
            instructions.append("## ?? ??????????\n")
            instructions.append(custom_instructions)
            instructions.append("")

        if repair_summary or repair_targets or preserve_strengths:
            instructions.append("## 🩺 剧情质量修复目标：\n")
            if repair_summary:
                instructions.append(f"- 修复摘要：{repair_summary}")
            if repair_targets:
                instructions.append("- 本轮优先修复：")
                for target in repair_targets:
                    instructions.append(f"  * {target}")
            if preserve_strengths:
                instructions.append("- 需要保留的优势：")
                for strength in preserve_strengths:
                    instructions.append(f"  * {strength}")
            instructions.append("")

        if effective_focus_areas:
            section_title = "## 🎯 重点优化方向（含自动补充）：\n" if auto_added_focus_areas else "## 🎯 重点优化方向：\n"
            instructions.append(section_title)
            for area in effective_focus_areas:
                focus_label = FOCUS_AREA_LABELS.get(area)
                if focus_label:
                    instructions.append(f"- {focus_label}")
            instructions.append("")

        if regenerate_request.preserve_elements:
            preserve = regenerate_request.preserve_elements
            instructions.append("## ?? ????????\n")

            if preserve.preserve_structure:
                instructions.append("- ???????????????")

            if preserve.preserve_dialogues:
                instructions.append("- ???????????")
                for dialogue in preserve.preserve_dialogues:
                    instructions.append(f"  * {dialogue}")

            if preserve.preserve_plot_points:
                instructions.append("- ????????????")
                for plot in preserve.preserve_plot_points:
                    instructions.append(f"  * {plot}")

            if preserve.preserve_character_traits:
                instructions.append("- ??????????????????")

            instructions.append("")

        return "\n".join(instructions)

    async def _build_regeneration_prompt(
        self,
        chapter: Chapter,
        modification_instructions: str,
        project_context: Dict[str, Any],
        regenerate_request: ChapterRegenerateRequest,
        style_content: str = "",
        user_id: str = None,
        db: AsyncSession = None
    ) -> str:
        """构建完整的重新生成提示词"""
        # 使用PromptService的get_chapter_regeneration_prompt方法
        # 该方法会处理自定义模板加载和完整提示词构建
        return await PromptService.get_chapter_regeneration_prompt(
            chapter_number=chapter.chapter_number,
            title=chapter.title,
            word_count=chapter.word_count,
            content=chapter.content,
            modification_instructions=modification_instructions,
            project_context=project_context,
            style_content=style_content,
            target_word_count=regenerate_request.target_word_count,
            user_id=user_id,
            db=db
        )
    
    def calculate_content_diff(
        self,
        original_content: str,
        new_content: str
    ) -> Dict[str, Any]:
        """
        计算两个版本的差异
        
        Returns:
            差异统计信息
        """
        # 基本统计
        diff_stats = {
            'original_length': len(original_content),
            'new_length': len(new_content),
            'length_change': len(new_content) - len(original_content),
            'length_change_percent': round((len(new_content) - len(original_content)) / len(original_content) * 100, 2) if len(original_content) > 0 else 0
        }
        
        # 计算相似度
        similarity = difflib.SequenceMatcher(None, original_content, new_content).ratio()
        diff_stats['similarity'] = round(similarity * 100, 2)
        diff_stats['difference'] = round((1 - similarity) * 100, 2)
        
        # 段落统计
        original_paragraphs = [p for p in original_content.split('\n\n') if p.strip()]
        new_paragraphs = [p for p in new_content.split('\n\n') if p.strip()]
        diff_stats['original_paragraph_count'] = len(original_paragraphs)
        diff_stats['new_paragraph_count'] = len(new_paragraphs)
        
        return diff_stats


# 全局实例
_regenerator_instance = None

def get_chapter_regenerator(ai_service: AIService) -> ChapterRegenerator:
    """获取章节重新生成器实例"""
    global _regenerator_instance
    if _regenerator_instance is None:
        _regenerator_instance = ChapterRegenerator(ai_service)
    return _regenerator_instance
