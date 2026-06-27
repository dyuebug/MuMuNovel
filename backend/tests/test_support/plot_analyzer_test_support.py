"""剧情分析服务 - 自动分析章节的钩子、伏笔、冲突等元素"""
from typing import Dict, Any, List, Optional, Callable, Awaitable
from collections import Counter
from functools import lru_cache
from pathlib import Path
from sqlalchemy.ext.asyncio import AsyncSession
from tests.test_support.ai_gateway.ai_service import AIService
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)
from tests.test_support.schemas.novel_quality_profile_service import novel_quality_profile_service
from tests.test_support.retired_runtime_test_support import get_logger
import json
import re
import asyncio
import httpx

logger = get_logger(__name__)
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)

ANALYSIS_CONTENT_CHAR_LIMIT = 6000
ANALYSIS_TOKEN_BUFFER = 600
ANALYSIS_MIN_MAX_TOKENS = 2200
ANALYSIS_MAX_MAX_TOKENS = 3200
ANALYSIS_ATTEMPT_TIMEOUT_SECONDS = 75
ANALYSIS_TRANSPORT_READ_TIMEOUT_SECONDS = 45
ANALYSIS_TRANSPORT_MAX_RETRIES = 1
ANALYSIS_FALLBACK_SUMMARY_CHAR_LIMIT = 180
ANALYSIS_FALLBACK_POINT_LIMIT = 3
ANALYSIS_FALLBACK_SENTENCE_LIMIT = 4


@lru_cache(maxsize=1)
def _load_plot_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = ("PLOT_ANALYSIS",)
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(
                f"plot analyzer test support 未找到模板常量: {template_key}"
            )
        templates[template_key] = match.group(1)
    return templates


def _plot_template_lookup(template_key: str) -> Optional[str]:
    return _load_plot_prompt_template_map().get(template_key)


async def _default_get_plot_template(
    template_key: str,
    user_id: str,
    db: AsyncSession,
):
    return await get_template_for_owner(
        template_key,
        user_id,
        db,
        template_lookup=_plot_template_lookup,
    )


def _default_format_plot_prompt(template: str, **kwargs) -> str:
    return _facade_format_prompt(template, **kwargs)


class PromptService:
    PLOT_ANALYSIS = _load_plot_prompt_template_map()["PLOT_ANALYSIS"]
    get_template = staticmethod(_default_get_plot_template)
    format_prompt = staticmethod(_default_format_plot_prompt)


async def get_template(*args, **kwargs):
    return await _default_get_plot_template(*args, **kwargs)


def format_prompt(*args, **kwargs):
    return _default_format_plot_prompt(*args, **kwargs)


_ORIGINAL_PROMPTSERVICE_GET_TEMPLATE = PromptService.get_template
_ORIGINAL_PROMPTSERVICE_FORMAT_PROMPT = PromptService.format_prompt

# 重试回调类型定义
OnRetryCallback = Callable[[int, int, int, str], Awaitable[None]]
# 参数: (当前重试次数, 最大重试次数, 等待时间秒数, 错误原因)


async def _get_plot_template(template_key: str, user_id: str, db: AsyncSession):
    patched_impl = globals().get("get_template")
    if patched_impl is None:
        raise RuntimeError("plot analyzer get_template 未定义")
    return await patched_impl(template_key, user_id, db)


def _format_plot_prompt(template: str, **kwargs) -> str:
    patched_impl = globals().get("format_prompt")
    if patched_impl is None:
        raise RuntimeError("plot analyzer format_prompt 未定义")
    return patched_impl(template, **kwargs)


def build_chapter_quality_prompt_context(
    *,
    genre: Optional[str] = None,
    style_name: Optional[str] = None,
    style_preset_id: Optional[str] = None,
    style_content: str = "",
    external_assets: Optional[List[Dict[str, Any]]] = None,
    reference_assets: Optional[List[Dict[str, Any]]] = None,
    mcp_references: str = "",
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> Dict[str, Any]:
    """构建分析、质检、修订共用的质量画像上下文。"""
    resolved_assets = external_assets or reference_assets or ()
    payload = {
        "genre": genre,
        "style_name": style_name,
        "style_preset_id": style_preset_id,
        "style_content": style_content,
        "external_assets": resolved_assets,
    }
    profile = novel_quality_profile_service.build_profile_dict(payload)
    return {
        "genre": genre,
        "style_name": style_name,
        "style_preset_id": style_preset_id,
        "style_content": style_content,
        "external_assets": resolved_assets,
        "reference_assets": resolved_assets,
        "mcp_references": mcp_references or "",
        "creative_mode": creative_mode or "",
        "story_focus": story_focus or "",
        "plot_stage": plot_stage or "",
        "story_creation_brief": story_creation_brief or "",
        "quality_preset": quality_preset or "",
        "quality_notes": quality_notes or "",
        "quality_profile": profile,
    }


class PlotAnalyzer:
    """剧情分析器 - 使用AI分析章节内容"""
    
    def __init__(self, ai_service: AIService):
        """
        初始化剧情分析器
        
        Args:
            ai_service: AI服务实例
        """
        self.ai_service = ai_service
        self.last_error_message: Optional[str] = None
        logger.info("✅ PlotAnalyzer初始化成功")

    def _set_last_error(self, message: str) -> str:
        self.last_error_message = message
        return message

    @staticmethod
    def _describe_exception(error: Exception) -> str:
        message = str(error).strip()
        if message:
            return message
        return type(error).__name__
    
    @staticmethod
    def _build_retry_prompt(prompt: str, last_error: str, attempt: int) -> str:
        """构建 JSON 解析失败后的重试提示词。"""
        retry_reason = (last_error or "返回内容不是有效 JSON")[:200]
        return (
            f"{prompt}\n\n"
            f"第 {attempt} 次重试，上一轮失败原因：{retry_reason}\n"
            "请严格返回合法 JSON：\n"
            "- 不要使用 markdown 代码块\n"
            "- 不要添加解释文字\n"
            "- 不要省略必填字段\n"
            "- 不要截断或输出不完整 JSON"
        )

    @staticmethod
    def _split_sentences(content: str) -> List[str]:
        normalized = re.sub(r"\s+", " ", (content or "").strip())
        if not normalized:
            return []
        parts = re.split(r"(?<=[。！？!?])\s*", normalized)
        return [part.strip() for part in parts if part.strip()]

    @staticmethod
    def _split_paragraphs(content: str) -> List[str]:
        if not content:
            return []
        return [part.strip() for part in re.split(r"\n\s*\n+", content) if part.strip()]

    @staticmethod
    def _trim_excerpt(text: str, limit: int = 80) -> str:
        normalized = re.sub(r"\s+", " ", (text or "").strip())
        if len(normalized) <= limit:
            return normalized
        return normalized[:limit].rstrip("，。！？!?；;：:、 ") + "…"

    @staticmethod
    def _pick_keyword(text: str, limit: int = 18) -> str:
        normalized = re.sub(r"\s+", "", (text or "").strip())
        return normalized[:limit]

    @staticmethod
    def _safe_score(value: float, *, minimum: int = 4, maximum: int = 8) -> int:
        bounded = max(float(minimum), min(float(maximum), value))
        return int(round(bounded))

    def _build_heuristic_fallback_analysis(
        self,
        *,
        chapter_number: int,
        title: str,
        content: str,
        word_count: int,
        failure_reason: str,
    ) -> Dict[str, Any]:
        paragraphs = self._split_paragraphs(content)
        sentences = self._split_sentences(content)
        effective_units = paragraphs or sentences or ([content.strip()] if (content or '').strip() else [])
        point_units = effective_units[:ANALYSIS_FALLBACK_POINT_LIMIT]
        summary_source = "；".join(self._trim_excerpt(unit, 56) for unit in point_units if unit)
        summary = summary_source[:ANALYSIS_FALLBACK_SUMMARY_CHAR_LIMIT] or self._trim_excerpt(content, 120)

        hook_candidates: List[Dict[str, Any]] = []
        seen_keywords: set[str] = set()
        sentence_pool: List[tuple[str, str]] = []
        if sentences:
            sentence_pool.append(("开篇", sentences[0]))
            if len(sentences) > 1:
                sentence_pool.append(("结尾", sentences[-1]))
            for sentence in sentences[1:ANALYSIS_FALLBACK_SENTENCE_LIMIT]:
                if re.search(r"(秘密|异变|异常|线索|危机|警讯|封街|失踪|未知|预兆|真相|反常|忽然|突然|竟然)", sentence):
                    sentence_pool.append(("中段", sentence))
        for position, sentence in sentence_pool:
            keyword = self._pick_keyword(sentence)
            if not keyword or keyword in seen_keywords:
                continue
            seen_keywords.add(keyword)
            hook_candidates.append({
                "type": "悬念",
                "content": self._trim_excerpt(sentence, 70),
                "strength": 6 if position != "结尾" else 7,
                "position": position,
                "keyword": keyword,
            })
            if len(hook_candidates) >= 2:
                break

        foreshadow_candidates: List[Dict[str, Any]] = []
        for sentence in sentences[: max(ANALYSIS_FALLBACK_SENTENCE_LIMIT + 2, 6)]:
            if not re.search(r"(似乎|隐约|预示|线索|秘密|异变|异常|征兆|不祥|钟声|灰潮|账册|裂口)", sentence):
                continue
            keyword = self._pick_keyword(sentence)
            if not keyword:
                continue
            foreshadow_candidates.append({
                "type": "planted",
                "content": self._trim_excerpt(sentence, 80),
                "keyword": keyword,
                "strength": 5,
                "reference_foreshadow_id": None,
            })
            if len(foreshadow_candidates) >= 2:
                break

        plot_points: List[Dict[str, Any]] = []
        for idx, unit in enumerate(point_units, 1):
            keyword = self._pick_keyword(unit)
            plot_points.append({
                "type": "事件推进",
                "content": self._trim_excerpt(unit, 90),
                "keyword": keyword,
                "importance": 6 if idx == 1 else 5,
            })

        conflict_markers = re.findall(r"(冲突|对峙|争执|阻止|拒绝|威胁|危机|封街|拦下|失败|代价|受阻|追赶|敌意|质疑|反击)", content or "")
        conflict_level = self._safe_score(4.8 + min(len(conflict_markers), 4) * 0.6)
        conflict_types = [marker for marker, _count in Counter(conflict_markers).most_common(3)]
        if not conflict_types and conflict_level >= 6:
            conflict_types = ["外部阻碍"]

        emotion_keywords = {
            "紧张": ["紧张", "警讯", "危机", "追赶", "封街", "灰潮"],
            "压迫": ["压迫", "失控", "封锁", "威胁", "代价"],
            "希望": ["希望", "机会", "转机", "合作", "成功"],
            "疑惧": ["秘密", "异变", "异常", "未知", "不祥"],
        }
        emotion_scores = {
            emotion: sum((content or "").count(token) for token in tokens)
            for emotion, tokens in emotion_keywords.items()
        }
        primary_emotion = max(emotion_scores, key=emotion_scores.get) if any(emotion_scores.values()) else "紧张"
        emotion_intensity = self._safe_score(5.0 + min(sum(1 for char in (content or "") if char in "！？!?"), 6) * 0.35)

        dialogue_chars = sum((content or "").count(mark) for mark in ['“', '”', '「', '」', '『', '』', '"'])
        dialogue_ratio = round(min(0.55, max(0.05, dialogue_chars / max(len(content or ""), 1) * 6.0)), 3)
        description_ratio = round(max(0.2, min(0.9, 1 - dialogue_ratio)), 3)

        pacing = "moderate"
        if len(sentences) >= 10 or conflict_level >= 7:
            pacing = "fast"
        elif len(sentences) <= 3 and word_count <= 600:
            pacing = "slow"

        scores = {
            "overall": self._safe_score(5.8 + min(len(plot_points), 3) * 0.25 + (0.4 if conflict_level >= 6 else 0)),
            "pacing": self._safe_score(5.5 + (0.5 if pacing == "moderate" else 0.2 if pacing == "fast" else -0.2)),
            "engagement": self._safe_score(5.6 + min(len(hook_candidates), 2) * 0.5),
            "coherence": self._safe_score(5.8 + (0.4 if len(plot_points) >= 2 else 0.1)),
        }

        scenes = [
            {
                "summary": self._trim_excerpt(unit, 80),
                "purpose": "推进情节",
                "index": idx,
            }
            for idx, unit in enumerate(point_units[:2], 1)
        ]

        suggestions = [
            f"【快速分析】上游 AI 分析未在时限内完成，已自动切换为规则摘要模式；建议稍后补跑深度分析。原因：{failure_reason[:120]}",
        ]
        if conflict_level < 6:
            suggestions.append("补强本章的直接阻力与代价，让主角目标与阻碍发生更明确的正面碰撞。")
        if dialogue_ratio < 0.08:
            suggestions.append("可适当加入高信息密度对话，用角色交锋承载设定与冲突，减少纯说明段。")
        if word_count > 1400:
            suggestions.append("正文明显偏长，可压缩重复铺陈，把关键事件控制在 2-3 个连续动作单元内。")
        elif word_count < 600:
            suggestions.append("正文偏短，可补一段推进结果或代价反馈，增强章节回报感。")
        suggestions = suggestions[:4]

        return {
            "analysis_mode": "heuristic_fallback",
            "fallback_reason": failure_reason,
            "plot_stage": "发展" if chapter_number > 1 else "开篇",
            "summary": summary or f"第{chapter_number}章《{title or f'第{chapter_number}章'}》的快速分析摘要",
            "hooks": hook_candidates,
            "foreshadows": foreshadow_candidates,
            "plot_points": plot_points,
            "conflict": {
                "level": conflict_level,
                "types": conflict_types,
                "description": self._trim_excerpt(summary or content, 90),
                "parties": [],
                "resolution_progress": 0.3 if chapter_number <= 1 else 0.5,
            },
            "emotional_arc": {
                "primary_emotion": primary_emotion,
                "intensity": emotion_intensity,
            },
            "character_states": [],
            "organization_states": [],
            "scenes": scenes,
            "pacing": pacing,
            "scores": scores,
            "dialogue_ratio": dialogue_ratio,
            "description_ratio": description_ratio,
            "suggestions": suggestions,
        }

    async def analyze_chapter(
        self,
        chapter_number: int,
        title: str,
        content: str,
        word_count: int,
        user_id: str = None,
        db: AsyncSession = None,
        max_retries: int = 2,
        existing_foreshadows: Optional[List[Dict[str, Any]]] = None,
        on_retry: Optional[OnRetryCallback] = None,
        characters_info: str = "",
        genre: Optional[str] = None,
        style_name: Optional[str] = None,
        style_preset_id: Optional[str] = None,
        style_content: str = "",
        external_assets: Optional[List[Dict[str, Any]]] = None,
        reference_assets: Optional[List[Dict[str, Any]]] = None,
        mcp_references: str = "",
        creative_mode: Optional[str] = None,
        story_focus: Optional[str] = None,
        plot_stage: Optional[str] = None,
        story_creation_brief: Optional[str] = None,
        quality_preset: Optional[str] = None,
        quality_notes: Optional[str] = None,
    ) -> Optional[Dict[str, Any]]:
        """
        分析单章内容（带重试机制）
        """
        logger.info(f"🔍 开始分析第{chapter_number}章: {title}")

        analysis_content = content[:ANALYSIS_CONTENT_CHAR_LIMIT] if len(content) > ANALYSIS_CONTENT_CHAR_LIMIT else content

        try:
            if user_id and db:
                template = await _get_plot_template("PLOT_ANALYSIS", user_id, db)
            else:
                template = PromptService.PLOT_ANALYSIS
        except Exception as e:
            logger.warning(f"⚠️ 获取提示词模板失败，使用默认模板: {str(e)}")
            template = PromptService.PLOT_ANALYSIS

        foreshadows_text = self._format_existing_foreshadows(existing_foreshadows)
        quality_context = build_chapter_quality_prompt_context(
            genre=genre,
            style_name=style_name,
            style_preset_id=style_preset_id,
            style_content=style_content,
            external_assets=external_assets,
            reference_assets=reference_assets,
            mcp_references=mcp_references,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
        )
        prompt = _format_plot_prompt(
            template,
            chapter_number=chapter_number,
            title=title,
            word_count=word_count,
            content=analysis_content,
            existing_foreshadows=foreshadows_text,
            characters_info=characters_info if characters_info else "（暂无角色信息）",
            _template_key="PLOT_ANALYSIS",
            **quality_context,
        )

        self.last_error_message = None
        last_error = None
        analysis_max_tokens = max(
            ANALYSIS_MIN_MAX_TOKENS,
            min(max(word_count or 0, len(analysis_content)) + ANALYSIS_TOKEN_BUFFER, ANALYSIS_MAX_MAX_TOKENS),
        )
        logger.debug(f"章节分析提示词: {prompt}")
        logger.info(f"章节分析 max_tokens: {analysis_max_tokens}")

        for attempt in range(1, max_retries + 1):
            try:
                current_prompt = prompt if attempt == 1 else self._build_retry_prompt(prompt, last_error or "", attempt)
                logger.info(f"  调用AI分析(内容长度: {len(analysis_content)}字, 尝试 {attempt}/{max_retries})...")
                response = await asyncio.wait_for(
                    self.ai_service.generate_text(
                        prompt=current_prompt,
                        temperature=0.2,
                        max_tokens=analysis_max_tokens,
                        auto_mcp=False,
                        handle_tool_calls=False,
                        request_options={
                            "read_timeout": ANALYSIS_TRANSPORT_READ_TIMEOUT_SECONDS,
                            "transport_max_retries": ANALYSIS_TRANSPORT_MAX_RETRIES,
                            "prefer_chat_completions": True,
                        },
                    ),
                    timeout=ANALYSIS_ATTEMPT_TIMEOUT_SECONDS,
                )
                accumulated_text = response.get("content", "") or ""
                if not accumulated_text or len(accumulated_text.strip()) < 10:
                    last_error = self._set_last_error("AI响应为空或过短")
                    if attempt < max_retries:
                        wait_time = min(2 ** attempt, 10)
                        logger.info(f"  ⏳ 等待 {wait_time} 秒后重试...")
                        if on_retry:
                            try:
                                await on_retry(attempt, max_retries, wait_time, last_error)
                            except Exception as callback_error:
                                logger.warning(f"⚠️ 重试回调执行失败: {callback_error}")
                        await asyncio.sleep(wait_time)
                        continue
                    fallback_reason = self._set_last_error("AI响应为空或过短，已切换快速规则分析")
                    logger.warning(f"⚠️ 第{chapter_number}章分析失败: AI响应为空，已切换快速规则分析")
                    return self._build_heuristic_fallback_analysis(
                        chapter_number=chapter_number,
                        title=title,
                        content=analysis_content,
                        word_count=word_count,
                        failure_reason=fallback_reason,
                    )

                response_text = accumulated_text
                analysis_result = self._parse_analysis_response(response_text)
                if analysis_result:
                    logger.info(f"✅ 第{chapter_number}章分析完成 (尝试 {attempt}/{max_retries})")
                    logger.info(f"  - 钩子: {len(analysis_result.get('hooks', []))}个")
                    logger.info(f"  - 伏笔: {len(analysis_result.get('foreshadows', []))}个")
                    logger.info(f"  - 情节点: {len(analysis_result.get('plot_points', []))}个")
                    logger.info(f"  - 整体评分: {analysis_result.get('scores', {}).get('overall', 'N/A')}")
                    return analysis_result

                last_error = self._set_last_error("AI返回格式异常，章节分析JSON解析失败")
                if attempt < max_retries:
                    wait_time = min(2 ** attempt, 10)
                    logger.info(f"  ⏳ 等待 {wait_time} 秒后重试...")
                    if on_retry:
                        try:
                            await on_retry(attempt, max_retries, wait_time, last_error)
                        except Exception as callback_error:
                            logger.warning(f"⚠️ 重试回调执行失败: {callback_error}")
                    await asyncio.sleep(wait_time)
                    continue
                fallback_reason = self._set_last_error("AI返回格式异常，已切换快速规则分析")
                logger.warning(f"⚠️ 第{chapter_number}章分析失败: JSON解析错误，已切换快速规则分析")
                return self._build_heuristic_fallback_analysis(
                    chapter_number=chapter_number,
                    title=title,
                    content=analysis_content,
                    word_count=word_count,
                    failure_reason=fallback_reason,
                )

            except (asyncio.TimeoutError, httpx.TimeoutException):
                last_error = self._set_last_error("章节分析请求超时（上游响应过慢）")
                logger.error(f"❌ 章节分析超时(尝试 {attempt}/{max_retries}): {last_error}")
                fallback_reason = self._set_last_error(f"{last_error}，已切换快速规则分析")
                logger.warning(f"⚠️ 第{chapter_number}章分析超时，直接切换快速规则分析: {fallback_reason}")
                return self._build_heuristic_fallback_analysis(
                    chapter_number=chapter_number,
                    title=title,
                    content=analysis_content,
                    word_count=word_count,
                    failure_reason=fallback_reason,
                )

            except Exception as e:
                last_error = self._set_last_error(self._describe_exception(e))
                logger.error(f"❌ 章节分析异常(尝试 {attempt}/{max_retries}): {last_error}")
                if attempt < max_retries:
                    wait_time = min(2 ** attempt, 10)
                    logger.info(f"  ⏳ 等待 {wait_time} 秒后重试...")
                    if on_retry:
                        try:
                            await on_retry(attempt, max_retries, wait_time, last_error)
                        except Exception as callback_error:
                            logger.warning(f"⚠️ 重试回调执行失败: {callback_error}")
                    await asyncio.sleep(wait_time)
                    continue
                fallback_reason = self._set_last_error(f"{last_error}，已切换快速规则分析")
                logger.warning(f"⚠️ 第{chapter_number}章分析失败: {last_error}，已切换快速规则分析")
                return self._build_heuristic_fallback_analysis(
                    chapter_number=chapter_number,
                    title=title,
                    content=analysis_content,
                    word_count=word_count,
                    failure_reason=fallback_reason,
                )

        fallback_reason = self._set_last_error((last_error or "章节分析失败，未获取到有效结果") + "，已切换快速规则分析")
        logger.warning(f"⚠️ 第{chapter_number}章分析未拿到有效结果，改用快速规则分析: {fallback_reason}")
        return self._build_heuristic_fallback_analysis(
            chapter_number=chapter_number,
            title=title,
            content=analysis_content,
            word_count=word_count,
            failure_reason=fallback_reason,
        )

    def _format_existing_foreshadows(self, foreshadows: Optional[List[Dict[str, Any]]]) -> str:
        """
        格式化已有伏笔列表，用于注入到分析提示词中
        
        核心策略（重构版）：
        - 分层展示所有已埋入伏笔，让AI能识别"自然回收"
        - 第1层：本章必须回收的伏笔（最详细）
        - 第2层：超期伏笔（较详细）
        - 第3层：其他已埋入伏笔（精简信息，供AI判断是否自然回收了）
        
        Args:
            foreshadows: 伏笔列表，每个包含 id, title, content, plant_chapter_number, resolve_status 等
        
        Returns:
            格式化的文本
        """
        if not foreshadows:
            return "（暂无已埋入的伏笔）"
        
        # 分类伏笔
        must_resolve = [fs for fs in foreshadows if fs.get('resolve_status') == 'must_resolve_now']
        overdue = [fs for fs in foreshadows if fs.get('resolve_status') == 'overdue']
        others = [fs for fs in foreshadows if fs.get('resolve_status') not in ('must_resolve_now', 'overdue')]
        
        lines = []
        
        # === 第1层：本章必须回收的伏笔（最详细）===
        if must_resolve:
            lines.append("=" * 40)
            lines.append("【🎯 本章必须回收的伏笔】")
            lines.append("=" * 40)
            for i, fs in enumerate(must_resolve, 1):
                fs_id = fs.get('id', 'unknown')
                fs_title = fs.get('title', '未命名伏笔')
                fs_content = fs.get('content', '')[:200]
                plant_chapter = fs.get('plant_chapter_number', '?')
                hint_text = fs.get('hint_text', '')
                
                lines.append(f"{i}. 【ID: {fs_id}】{fs_title}")
                lines.append(f"   埋入章节：第{plant_chapter}章")
                lines.append(f"   伏笔内容：{fs_content}{'...' if len(fs.get('content', '')) > 200 else ''}")
                if hint_text:
                    lines.append(f"   埋入暗示：{hint_text[:100]}")
                lines.append(f"   ⚠️ 回收时 reference_foreshadow_id 填写: {fs_id}")
                lines.append("")
        
        # === 第2层：超期伏笔 ===
        if overdue:
            lines.append("【⚠️ 超期未回收伏笔 - 如章节内容回收了请标记】")
            for fs in overdue[:5]:
                fs_id = fs.get('id', 'unknown')
                fs_title = fs.get('title', '')
                plant_chapter = fs.get('plant_chapter_number', '?')
                lines.append(f"- 【ID: {fs_id}】{fs_title}（第{plant_chapter}章埋入）")
            lines.append("")
        
        # === 第3层：其他已埋入伏笔（精简）===
        if others:
            lines.append("【📋 其他已埋入伏笔 - 如章节内容自然回收了请标记】")
            for fs in others[:10]:
                fs_id = fs.get('id', 'unknown')
                fs_title = fs.get('title', '')
                plant_chapter = fs.get('plant_chapter_number', '?')
                lines.append(f"- 【ID: {fs_id}】{fs_title}（第{plant_chapter}章埋入）")
            if len(others) > 10:
                lines.append(f"  ... 还有{len(others) - 10}个伏笔未列出")
            lines.append("")
        
        # 操作指引
        lines.append("提示：如果章节内容回收了上述任一伏笔，请在 foreshadows 数组中")
        lines.append("添加 type='resolved' 的记录，并在 reference_foreshadow_id 填写对应ID。")
        
        return "\n".join(lines)
    
    def _parse_analysis_response(self, response: str) -> Optional[Dict[str, Any]]:
        """
        解析AI返回的分析结果（使用统一的JSON清洗方法）
        
        Args:
            response: AI返回的文本
        
        Returns:
            解析后的字典,失败返回None
        """
        try:
            # 使用统一的JSON清洗方法
            cleaned = self.ai_service._clean_json_response(response)
            
            # 尝试解析JSON
            result = json.loads(cleaned)
            
            # 验证必要字段
            required_fields = ['hooks', 'plot_points', 'scores']
            for field in required_fields:
                if field not in result:
                    logger.warning(f"⚠️ 分析结果缺少字段: {field}")
                    result[field] = [] if field != 'scores' else {}
            
            logger.info("✅ 成功解析分析结果")
            return result
            
        except json.JSONDecodeError as e:
            self._set_last_error("AI返回格式异常，章节分析JSON解析失败")
            logger.error(f"❌ JSON解析失败: {str(e)}")
            logger.error(f"  原始响应(前500字): {response[:500]}")
            return None
        except Exception as e:
            self._set_last_error(f"章节分析结果解析异常: {str(e)}")
            logger.error(f"❌ 解析异常: {str(e)}")
            return None
    
    def extract_memories_from_analysis(
        self,
        analysis: Dict[str, Any],
        chapter_id: str,
        chapter_number: int,
        chapter_content: str = "",
        chapter_title: str = ""
    ) -> List[Dict[str, Any]]:
        """
        从分析结果中提取记忆片段
        
        Args:
            analysis: 分析结果
            chapter_id: 章节ID
            chapter_number: 章节号
            chapter_content: 章节完整内容(用于计算位置)
            chapter_title: 章节标题
        
        Returns:
            记忆片段列表
        """
        memories = []
        
        try:
            # 【新增】0. 提取章节摘要作为记忆（用于语义检索相关章节）
            chapter_summary = ""
            
            # 尝试从分析结果获取摘要
            if analysis.get('summary'):
                chapter_summary = analysis.get('summary')
            # 或者从情节点组合生成摘要
            elif analysis.get('plot_points'):
                plot_summaries = [p.get('content', '') for p in analysis.get('plot_points', [])[:3]]
                chapter_summary = "；".join(plot_summaries)
            # 或者使用内容前300字
            elif chapter_content:
                chapter_summary = chapter_content[:300] + ("..." if len(chapter_content) > 300 else "")
            
            # 如果有摘要，添加到记忆中
            if chapter_summary:
                memories.append({
                    'type': 'chapter_summary',
                    'content': chapter_summary,
                    'title': f"第{chapter_number}章《{chapter_title}》摘要",
                    'metadata': {
                        'chapter_id': chapter_id,
                        'chapter_number': chapter_number,
                        'importance_score': 0.6,  # 中等重要性
                        'tags': ['摘要', '章节概览', chapter_title],
                        'is_foreshadow': 0,
                        'text_position': 0,
                        'text_length': len(chapter_summary)
                    }
                })
                logger.info(f"  ✅ 添加章节摘要记忆: {len(chapter_summary)}字")
            
            # 1. 提取钩子作为记忆
            for i, hook in enumerate(analysis.get('hooks', [])):
                if hook.get('strength', 0) >= 6:  # 只保存强度>=6的钩子
                    keyword = hook.get('keyword', '')
                    position, length = self._find_text_position(chapter_content, keyword)
                    
                    logger.info(f"  钩子位置: keyword='{keyword[:30]}...', pos={position}, len={length}")
                    
                    memories.append({
                        'type': 'hook',
                        'content': f"[{hook.get('type', '未知')}钩子] {hook.get('content', '')}",
                        'title': f"{hook.get('type', '钩子')} - {hook.get('position', '')}",
                        'metadata': {
                            'chapter_id': chapter_id,
                            'chapter_number': chapter_number,
                            'importance_score': min(hook.get('strength', 5) / 10, 1.0),
                            'tags': [hook.get('type', '钩子'), hook.get('position', '')],
                            'is_foreshadow': 0,
                            'keyword': keyword,
                            'text_position': position,
                            'text_length': length,
                            'strength': hook.get('strength', 5),
                            'position_desc': hook.get('position', '')
                        }
                    })
            
            # 2. 提取伏笔作为记忆
            for i, foreshadow in enumerate(analysis.get('foreshadows', [])):
                is_planted = foreshadow.get('type') == 'planted'
                keyword = foreshadow.get('keyword', '')
                position, length = self._find_text_position(chapter_content, keyword)
                
                logger.info(f"  伏笔位置: keyword='{keyword[:30]}...', pos={position}, len={length}")
                
                memories.append({
                    'type': 'foreshadow',
                    'content': foreshadow.get('content', ''),
                    'title': f"{'埋下伏笔' if is_planted else '回收伏笔'}",
                    'metadata': {
                        'chapter_id': chapter_id,
                        'chapter_number': chapter_number,
                        'importance_score': min(foreshadow.get('strength', 5) / 10, 1.0),
                        'tags': ['伏笔', foreshadow.get('type', 'planted')],
                        'is_foreshadow': 1 if is_planted else 2,
                        'reference_chapter': foreshadow.get('reference_chapter'),
                        'keyword': keyword,
                        'text_position': position,
                        'text_length': length,
                        'foreshadow_type': foreshadow.get('type', 'planted'),
                        'strength': foreshadow.get('strength', 5)
                    }
                })
            
            # 3. 提取关键情节点
            for i, plot_point in enumerate(analysis.get('plot_points', [])):
                if plot_point.get('importance', 0) >= 0.6:  # 只保存重要性>=0.6的情节点
                    keyword = plot_point.get('keyword', '')
                    position, length = self._find_text_position(chapter_content, keyword)
                    
                    logger.info(f"  情节点位置: keyword='{keyword[:30]}...', pos={position}, len={length}")
                    
                    memories.append({
                        'type': 'plot_point',
                        'content': f"{plot_point.get('content', '')}。影响: {plot_point.get('impact', '')}",
                        'title': f"情节点 - {plot_point.get('type', '未知')}",
                        'metadata': {
                            'chapter_id': chapter_id,
                            'chapter_number': chapter_number,
                            'importance_score': plot_point.get('importance', 0.5),
                            'tags': ['情节点', plot_point.get('type', '未知')],
                            'is_foreshadow': 0,
                            'keyword': keyword,
                            'text_position': position,
                            'text_length': length
                        }
                    })
            
            # 4. 提取角色状态变化
            for i, char_state in enumerate(analysis.get('character_states', [])):
                char_name = char_state.get('character_name', '未知角色')
                memories.append({
                    'type': 'character_event',
                    'content': f"{char_name}的状态变化: {char_state.get('state_before', '')} → {char_state.get('state_after', '')}。{char_state.get('psychological_change', '')}",
                    'title': f"{char_name}的变化",
                    'metadata': {
                        'chapter_id': chapter_id,
                        'chapter_number': chapter_number,
                        'importance_score': 0.7,
                        'tags': ['角色', char_name, '状态变化'],
                        'related_characters': [char_name],
                        'is_foreshadow': 0
                    }
                })
            
            # 5. 如果有重要冲突,也记录下来
            conflict = analysis.get('conflict', {})
            
            if conflict and conflict.get('level', 0) >= 7:
                # 确保 parties 和 types 都是字符串列表
                parties = conflict.get('parties', [])
                if parties and isinstance(parties, list):
                    parties = [str(p) for p in parties]
                
                types = conflict.get('types', [])
                if types and isinstance(types, list):
                    types = [str(t) for t in types]
                
                memories.append({
                    'type': 'plot_point',
                    'content': f"重要冲突: {conflict.get('description', '')}。冲突各方: {', '.join(parties)}",
                    'title': f"冲突 - 强度{conflict.get('level', 0)}",
                    'metadata': {
                        'chapter_id': chapter_id,
                        'chapter_number': chapter_number,
                        'importance_score': min(conflict.get('level', 5) / 10, 1.0),
                        'tags': ['冲突'] + types,
                        'is_foreshadow': 0
                    }
                })
            
            logger.info(f"📝 从分析中提取了{len(memories)}条记忆")
            return memories
            
        except Exception as e:
            logger.error(f"❌ 提取记忆失败: {str(e)}")
            return []
    
    def _find_text_position(self, full_text: str, keyword: str) -> tuple[int, int]:
        """
        在全文中查找关键词位置
        
        Args:
            full_text: 完整文本
            keyword: 关键词
        
        Returns:
            (起始位置, 长度) 如果未找到返回(-1, 0)
        """
        if not keyword or not full_text:
            return (-1, 0)
        
        try:
            # 1. 精确匹配
            pos = full_text.find(keyword)
            if pos != -1:
                return (pos, len(keyword))
            
            # 2. 去除标点符号后匹配
            import re
            clean_keyword = re.sub(r'[，。！？、；：""''（）《》【】]', '', keyword)
            clean_text = re.sub(r'[，。！？、；：""''（）《》【】]', '', full_text)
            pos = clean_text.find(clean_keyword)
            
            if pos != -1:
                # 反向映射到原文位置（简化处理）
                return (pos, len(clean_keyword))
            
            # 3. 模糊匹配：查找关键词的前半部分
            if len(keyword) > 10:
                partial = keyword[:min(15, len(keyword))]
                pos = full_text.find(partial)
                if pos != -1:
                    return (pos, len(partial))
            
            # 4. 未找到
            logger.debug(f"未找到关键词位置: {keyword[:30]}...")
            return (-1, 0)
            
        except Exception as e:
            logger.error(f"查找位置失败: {str(e)}")
            return (-1, 0)
    
    def generate_analysis_summary(self, analysis: Dict[str, Any]) -> str:
        """
        生成分析摘要文本
        
        Args:
            analysis: 分析结果
        
        Returns:
            格式化的摘要文本
        """
        try:
            lines = ["=== 章节分析报告 ===\n"]
            if analysis.get('analysis_mode') == 'heuristic_fallback':
                fallback_reason = str(analysis.get('fallback_reason') or '上游 AI 分析未完成')
                lines.append("【分析模式】快速规则分析（自动降级）")
                lines.append(f"  原因: {fallback_reason[:160]}")
                lines.append("")
            
            # 整体评分
            scores = analysis.get('scores', {})
            lines.append(f"【整体评分】")
            lines.append(f"  整体质量: {scores.get('overall', 'N/A')}/10")
            lines.append(f"  节奏把控: {scores.get('pacing', 'N/A')}/10")
            lines.append(f"  吸引力: {scores.get('engagement', 'N/A')}/10")
            lines.append(f"  连贯性: {scores.get('coherence', 'N/A')}/10\n")
            
            # 剧情阶段
            lines.append(f"【剧情阶段】{analysis.get('plot_stage', '未知')}\n")
            
            # 钩子统计
            hooks = analysis.get('hooks', [])
            if hooks:
                lines.append(f"【钩子分析】共{len(hooks)}个")
                for hook in hooks[:3]:  # 只显示前3个
                    lines.append(f"  • [{hook.get('type')}] {hook.get('content', '')[:50]}... (强度:{hook.get('strength', 0)})")
                lines.append("")
            
            # 伏笔统计
            foreshadows = analysis.get('foreshadows', [])
            if foreshadows:
                planted = sum(1 for f in foreshadows if f.get('type') == 'planted')
                resolved = sum(1 for f in foreshadows if f.get('type') == 'resolved')
                lines.append(f"【伏笔分析】埋下{planted}个, 回收{resolved}个\n")
            
            # 冲突分析
            conflict = analysis.get('conflict', {})
            if conflict:
                lines.append(f"【冲突分析】")
                lines.append(f"  类型: {', '.join(conflict.get('types', []))}")
                lines.append(f"  强度: {conflict.get('level', 0)}/10")
                lines.append(f"  进度: {int(conflict.get('resolution_progress', 0) * 100)}%\n")
            
            # 改进建议
            suggestions = analysis.get('suggestions', [])
            if suggestions:
                lines.append(f"【改进建议】")
                for i, sug in enumerate(suggestions, 1):
                    lines.append(f"  {i}. {sug}")
            
            return "\n".join(lines)
            
        except Exception as e:
            logger.error(f"❌ 生成摘要失败: {str(e)}")
            return "分析摘要生成失败"


# 创建全局实例(需要时手动初始化)
_plot_analyzer_instance = None

def get_plot_analyzer(ai_service: AIService) -> PlotAnalyzer:
    """获取剧情分析器实例"""
    global _plot_analyzer_instance
    if _plot_analyzer_instance is None:
        _plot_analyzer_instance = PlotAnalyzer(ai_service)
    return _plot_analyzer_instance





