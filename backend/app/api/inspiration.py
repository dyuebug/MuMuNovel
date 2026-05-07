"""灵感模式 API - 通过对话式引导创建项目。"""
from fastapi import APIRouter, Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession
from typing import Dict, Any
from datetime import datetime
import json
import re

from app.database import get_db
from app.services.ai_service import AIService
from app.api.settings import get_user_ai_service
from app.services.prompt_service import PromptService
from app.services.chapter_web_research_service import chapter_web_research_service
from app.logger import get_logger

router = APIRouter(prefix="/inspiration", tags=["灵感模式"])
logger = get_logger(__name__)


def _build_generation_request_options(ai_service: AIService) -> dict[str, object]:
    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(getattr(retry_cfg, "max_retries", 2) or 2)
    provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    request_options: dict[str, object] = {
        "transport_max_retries": max(1, min(configured_retry_budget, 2)),
    }
    if provider in {"sub2api", "openai_responses"}:
        request_options.update(
            {
                "prefer_chat_completions": True,
                "prefer_normalized_v1_candidate": True,
                "first_chunk_timeout": 20.0,
                "allow_non_stream_fallback": False,
            }
        )
    return request_options


# 不同阶段的temperature设置（递减以保持一致性）
TEMPERATURE_SETTINGS = {
    "title": 0.9,
    "description": 0.78,
    "theme": 0.72,
    "genre": 0.62,
}

COMMON_INSPIRATION_STYLE_GUARD = """
【风格与可读性要求（必须遵守）】
1. 使用中文网文读者习惯的自然表达，避免公文腔、论文腔和模板腔。
2. 句子长短要有变化，避免整段都是同长度短句或流水账长句。
3. 可以少量借用网络语感，但必须克制，不能堆砌流行词。
4. 优先写人物目标、阻碍、代价和情绪，不要只堆设定名词。
5. 如果出现术语或特殊设定，要顺手补一句白话解释。
6. 避免高频模板开头，例如“这是一个关于……”或“故事围绕……”这类空泛引入。
7. 只输出当前任务结果，不输出流程说明、调度术语或自我评价。
8. 信息不足时，优先保住“目标 → 阻力 → 选择 → 后果”的最小冲突链。
9. 六个选项必须有明显区分，至少覆盖不同切入角度，不能只换同义词。
10. 描述要带具体场景感或动作感，避免只给抽象大词。
11. 避免“鸡汤式收尾”和“下章预告式空话”，优先保留具体冲突钩子。
12. 优先给可传播的记忆点：反常识信息、极端选择、倒计时压力，至少命中其一。
"""

STEP_EXTRA_STYLE_GUARD = {
    "title": """
    【书名专项】
    - 风格要拉开差异，避免同构词组批量改写。
    - 名称要顺口、好记，避免生造拗口长词。
    - 至少覆盖以下命名策略中的四种：身份反差、强事件、关系张力、情绪钩子、世界观异化、命运投择。
    """,
    "description": """
    【简介专项】
    - 每个选项都要体现：主角当前目标 + 关键阻碍或代价。
    - 冲突必须让读者感知得到，不能只写抽象观点。
    - 六个选项的开场方式要明显变化，比如动作切入、对话切入、结果倒叙、困境切入等。
    - 开头尽量尽快出现冲突触发、异常变化或高压任务，不要慢热铺垫。
    - 至少两个选项使用短句爆点开场，至少两个选项带明确转折连接词。
    """,
    "theme": """
    【主题专项】
    - 主题要先给人话结论，再落回角色冲突现场，避免高概念空转。
    - 保持情绪温度，不要写成教科书总结。
    - 每个主题都要包含一个价值冲突对撞点，避免全是正确废话。
    - 优先采用“命题句 → 冲突现场 → 情绪余震”的三拍结构。
    - 至少一个主题要体现“反常识但合理”的价值碰撞。
    """,
    "genre": """
    【类型专项】
    - 标签以读者常见认知为主，可以组合，但不要互相冲突。
    - 禁止生造难懂标签。
    - 至少体现“主赛道 + 冲突气质”两个维度。
    """,
}

INSPIRATION_ALLOWED_PERSPECTIVES = ("第一人称", "第三人称", "全知视角")
INSPIRATION_PERSPECTIVE_ALIASES = {
    "第一人称": "第一人称",
    "第三人称": "第三人称",
    "全知视角": "全知视角",
    "first_person": "第一人称",
    "third_person": "第三人称",
    "omniscient": "全知视角",
    "firstperson": "第一人称",
    "thirdperson": "第三人称",
}

_TEMPLATEY_PREFIXES = (
    "这是一个关于",
    "讲述了",
    "故事围绕",
    "在这个世界里",
    "主角是一个",
)
_SETTING_JARGON_WORDS = (
    "门影",
    "回灌",
    "锚点",
    "协议体",
    "模因",
    "阵列",
    "位面",
    "法则",
    "神性",
)
_EXPLANATION_HINTS = (
    "也就是",
    "简单说",
    "说白了",
    "换句话说",
    "直白点",
)
_TEMPLATEY_ENDING_HINTS = (
    "他终于明白了",
    "命运将会",
    "故事才刚刚开始",
    "一切都变了",
    "总之",
    "综上所述",
    "值得注意的是",
)
_WORKFLOW_META_PATTERNS = (
    r"执行\s*\d+(?:\.\d+)*",
    r"调用\s*agent",
    r"方案\s*[ab](?:\s*[/、]\s*[ab])?",
    r"流程(?:说明|总结|复盘)",
    r"步骤\s*\d+",
    r"(?:作为|身为)\s*(?:ai|助手|模型)",
)
_CONFLICT_CONNECTOR_WORDS = (
    "却",
    "偏偏",
    "但是",
    "结果",
    "直到",
    "否则",
    "代价",
)
_ACTION_HINT_WORDS = (
    "追",
    "问",
    "推开",
    "拦住",
    "逃",
    "退",
    "破",
    "开口",
    "撞",
    "掏出",
    "按下",
    "谈判",
    "对峙",
)
_ABSTRACT_THEME_WORDS = (
    "命运",
    "成长",
    "人性",
    "真相",
    "救赎",
    "信念",
    "希望",
    "黑暗",
    "善恶",
    "宿命",
)
_SCENE_HINT_WORDS = (
    "雨",
    "夜",
    "街",
    "门",
    "车",
    "血",
    "电话",
    "病房",
    "法庭",
    "会议室",
    "厂房",
    "码头",
    "天台",
    "巷子",
    "教室",
)


def _build_style_guard(step: str) -> str:
    extra = STEP_EXTRA_STYLE_GUARD.get(step, "")
    return f"{COMMON_INSPIRATION_STYLE_GUARD}\n{extra}".strip()


def _normalize_genre_list(value: Any) -> list[str]:
    if value is None:
        return []

    raw_items: list[str] = []
    if isinstance(value, str):
        raw_items = re.split(r"[，,、/|｜]+", value)
    elif isinstance(value, list):
        for item in value:
            if isinstance(item, str):
                raw_items.extend(re.split(r"[，,、/|｜]+", item))

    normalized: list[str] = []
    seen: set[str] = set()
    for item in raw_items:
        cleaned = item.strip()
        if not cleaned or cleaned in seen:
            continue
        seen.add(cleaned)
        normalized.append(cleaned)
    return normalized


def _normalize_narrative_perspective(value: Any) -> str:
    if value is None:
        return "第三人称"

    cleaned = str(value).strip()
    if not cleaned:
        return "第三人称"

    mapped = INSPIRATION_PERSPECTIVE_ALIASES.get(cleaned)
    if mapped:
        return mapped

    lowered = cleaned.lower().replace(" ", "")
    mapped = INSPIRATION_PERSPECTIVE_ALIASES.get(lowered)
    if mapped:
        return mapped

    return cleaned if cleaned in INSPIRATION_ALLOWED_PERSPECTIVES else "第三人称"


def _build_quick_generate_existing_text(data: Dict[str, Any]) -> str:
    existing_info: list[str] = []

    if data.get("title"):
        existing_info.append(f"- 书名：{data['title']}")
    if data.get("description"):
        existing_info.append(f"- 简介：{data['description']}")
    if data.get("theme"):
        existing_info.append(f"- 主题：{data['theme']}")

    genre_values = _normalize_genre_list(data.get("genre"))
    if genre_values:
        existing_info.append(f"- 类型：{', '.join(genre_values)}")

    if data.get("narrative_perspective"):
        existing_info.append(
            f"- 叙事视角：{_normalize_narrative_perspective(data.get('narrative_perspective'))}"
        )

    return "\n".join(existing_info) if existing_info else "暂无信息"


def _normalize_text(value: str) -> str:
    lowered = (value or "").strip().lower()
    return re.sub(r"[^\w\u4e00-\u9fff]+", "", lowered)


def _normalize_research_text(value: Any, limit: int = 180) -> str:
    text = re.sub(r"\s+", " ", str(value or "")).strip()
    if len(text) <= limit:
        return text
    return text[: limit - 3].rstrip() + "..."


def _serialize_inspiration_research_assets(value: Any) -> list[Dict[str, str]]:
    if not isinstance(value, list):
        return []

    assets: list[Dict[str, str]] = []
    for item in value[:4]:
        if not isinstance(item, dict):
            continue
        title = _normalize_research_text(item.get("title") or item.get("source") or "参考资料", 120)
        source = _normalize_research_text(item.get("source"), 300)
        summary = _normalize_research_text(item.get("summary") or item.get("raw_content") or item.get("title"), 220)
        if not title and not source and not summary:
            continue
        assets.append({
            "title": title or source or "参考资料",
            "source": source,
            "summary": summary,
        })
    return assets


def _compose_inspiration_research_seed(step: str, context: Dict[str, Any], feedback: str = "") -> str:
    step_label_map = {
        "title": "小说书名创意",
        "description": "小说简介与冲突设计",
        "theme": "小说主题与价值冲突",
        "genre": "小说类型定位与读者偏好",
    }
    parts = [
        step_label_map.get(step, step),
        _normalize_research_text(context.get("initial_idea"), 120),
        _normalize_research_text(context.get("title"), 80),
        _normalize_research_text(context.get("description"), 140),
        _normalize_research_text(context.get("theme"), 100),
        _normalize_research_text(feedback, 100),
    ]
    parts = [part for part in parts if part]
    return _normalize_research_text(" | ".join(parts), 320)


def _build_inspiration_research_note(bundle: Dict[str, Any]) -> str:
    query = _normalize_research_text(bundle.get("query"), 320)
    assets = _serialize_inspiration_research_assets(bundle.get("assets"))
    if not query and not assets:
        return ""

    lines = ["【联网检索参考（仅作灵感增强）】"]
    if query:
        lines.append(f"- 检索主题：{query}")
    for index, asset in enumerate(assets, start=1):
        title = asset.get("title") or "参考资料"
        source = asset.get("source") or ""
        summary = asset.get("summary") or ""
        title_line = f"- 资料{index}：{title}"
        if source:
            title_line += f" | {source}"
        lines.append(title_line)
        if summary:
            lines.append(f"  摘要：{summary}")
    lines.append("请只吸收可验证的趋势、职业细节、场景信息与语感方向，不要直接照抄外部文本。")
    return "\n".join(lines)


async def _build_inspiration_research_bundle(
    *,
    step: str,
    context: Dict[str, Any],
    user_id: str | None,
    db_session: AsyncSession,
    enable_web_research: Any,
    web_research_query: Any,
    feedback: str = "",
) -> Dict[str, Any]:
    custom_query = _normalize_research_text(web_research_query, 320)
    seed = custom_query or _compose_inspiration_research_seed(step, context, feedback)
    grok_context = _normalize_research_text(custom_query or seed, 260)
    if not seed and not grok_context:
        return {"enabled": False, "query": "", "assets": []}

    try:
        return await chapter_web_research_service.collect_assets(
            user_id=user_id,
            db_session=db_session,
            exa_query=seed,
            grok_query=(
                f"请围绕以下中文小说创作需求进行实时联网研究，重点整理题材趋势、读者关注点、职业与场景细节、可借鉴的语感方向，并尽量附带来源：{grok_context}"
            ) if grok_context else "",
            enable_web_research=enable_web_research,
            archive_scope=f"inspiration_{user_id or 'anonymous'}",
            archive_id=f"{step}_{datetime.now().strftime('%Y%m%d%H%M%S%f')}",
            metadata={"step": step},
        )
    except Exception as exc:
        logger.warning("灵感阶段联网检索失败: %s", exc, exc_info=True)
        return {"enabled": True, "query": custom_query or seed, "assets": [], "error": str(exc)}


def _attach_inspiration_research_payload(result: Dict[str, Any], bundle: Dict[str, Any]) -> Dict[str, Any]:
    payload = dict(result or {})
    payload["research_query"] = _normalize_research_text(bundle.get("query"), 320)
    payload["research_assets"] = _serialize_inspiration_research_assets(bundle.get("assets"))
    return payload


def _contains_unexplained_jargon(text: str) -> bool:
    jargon_hit_count = sum(1 for term in _SETTING_JARGON_WORDS if term in text)
    has_explanation = any(marker in text for marker in _EXPLANATION_HINTS)
    return jargon_hit_count >= 2 and not has_explanation


def _contains_workflow_meta_text(text: str) -> bool:
    if not text:
        return False
    return any(re.search(pattern, text, flags=re.IGNORECASE) for pattern in _WORKFLOW_META_PATTERNS)


def _opening_fingerprint(text: str) -> str:
    compact = re.sub(r"\s+", "", (text or "").strip())
    compact = re.sub(r"[，。！？、,.!?;；:：\"'“”‘’（）()【】\[\]<>《》\-—]", "", compact)
    return compact[:8].lower()


def _has_low_opening_diversity(options: list[str]) -> bool:
    if len(options) <= 1:
        return False
    fingerprints = [_opening_fingerprint(option) for option in options if option.strip()]
    unique_count = len(set(fp for fp in fingerprints if fp))
    # 至少保证大部分选项的开头结构是不同的
    return unique_count < max(3, len(options) - 2)


def _is_overly_abstract_text(option: str, step: str) -> bool:
    if step not in {"description", "theme"}:
        return False
    text = (option or "").strip()
    if not text:
        return False
    abstract_hits = sum(1 for w in _ABSTRACT_THEME_WORDS if w in text)
    scene_hits = sum(1 for w in _SCENE_HINT_WORDS if w in text)
    # 抽象词明显偏多且几乎没有场景词，判定为偏空泛
    return abstract_hits >= 3 and scene_hits == 0


def _has_templatey_ending(text: str) -> bool:
    compact = re.sub(r"\s+", "", (text or "").strip())
    if not compact:
        return False
    return any(compact.endswith(marker) for marker in _TEMPLATEY_ENDING_HINTS)


def _lacks_conflict_connector(text: str) -> bool:
    normalized = (text or "").strip()
    if not normalized:
        return False
    return not any(word in normalized for word in _CONFLICT_CONNECTOR_WORDS)


def _lacks_action_signal(text: str) -> bool:
    normalized = (text or "").strip()
    if not normalized:
        return False
    return not any(word in normalized for word in _ACTION_HINT_WORDS)


def validate_options_response(result: Dict[str, Any], step: str, max_retries: int = 3) -> tuple[bool, str]:
    """
    校验AI返回的选项格式是否正确
    
    Returns:
        (is_valid, error_message)
    """
    # 检查必需字段
    if "options" not in result:
        return False, "缺少options字段"
    
    options = result.get("options", [])
    
    # 检查options是否为数组
    if not isinstance(options, list):
        return False, "options必须是数组"

    prompt_value = result.get("prompt")
    if isinstance(prompt_value, str) and _contains_workflow_meta_text(prompt_value):
        return False, "prompt包含流程化元文本，请改为直接的引导语"
    
    # 检查数组长度
    if len(options) < 3:
        return False, f"选项数量不足，至少需要3个，当前只有{len(options)}个"
    
    if len(options) > 10:
        return False, f"选项数量过多，最多10个，当前有{len(options)}个"
    
    # 检查每个选项是否为字符串且不为空
    for i, option in enumerate(options):
        if not isinstance(option, str):
            return False, f"第{i+1}个选项不是字符串类型"
        if not option.strip():
            return False, f"第{i+1}个选项为空"
        if len(option) > 500:
            return False, f"第{i+1}个选项过长（超过500字符）"
        if _contains_workflow_meta_text(option):
            return False, f"第{i+1}个选项包含流程化元文本，请直接输出内容结果"
    
    # 根据不同步骤进行特定校验
    if step == "genre":
        # 类型标签应该比较短
        for i, option in enumerate(options):
            if len(option) > 10:
                return False, f"类型标签【{option}】过长，应该在2-10字之间"

    # 选项去重：避免同义改写导致“看起来有6个，实际没区别”
    normalized_options = [_normalize_text(option) for option in options]
    if len(set(normalized_options)) != len(normalized_options):
        return False, "选项存在重复或近似重复，请提升差异度"

    if _has_low_opening_diversity(options):
        return False, "选项开头结构过于雷同，请明显拉开表达方式"

    # 简介/主题的生动度与可读性兜底校验
    if step in {"description", "theme"}:
        min_len = 50 if step == "description" else 35
        for i, option in enumerate(options):
            if len(option.strip()) < min_len:
                return False, f"第{i+1}个选项过短，信息密度不足"

        templatey_count = sum(
            1 for option in options if option.strip().startswith(_TEMPLATEY_PREFIXES)
        )
        if templatey_count >= max(3, len(options) - 2):
            return False, "选项模板腔过重，请改成更自然的口语叙述"

        templatey_ending_count = sum(
            1 for option in options if _has_templatey_ending(option)
        )
        if templatey_ending_count >= max(3, len(options) - 2):
            return False, "选项结尾过于鸡汤或预告化，请收束到具体冲突钩子"

        weak_conflict_count = sum(
            1 for option in options if _lacks_conflict_connector(option)
        )
        if weak_conflict_count >= max(4, len(options) - 1):
            return False, "选项冲突连接词明显不足，请强化转折与代价表达"

        # 仅对简介做动作推进兜底，主题文本允许更高抽象度表达
        if step == "description":
            weak_action_count = sum(
                1 for option in options if _lacks_action_signal(option)
            )
            if weak_action_count >= max(4, len(options) - 1):
                return False, "选项动作推进不足，请补充可感知行动和现场变化"

        for i, option in enumerate(options):
            if _contains_unexplained_jargon(option):
                return False, f"第{i+1}个选项术语密度过高且缺少白话解释"
            if _is_overly_abstract_text(option, step):
                return False, f"第{i+1}个选项过于抽象，请补足可感知场景或动作"
    
    return True, ""


@router.post("/generate-options")
async def generate_options(
    data: Dict[str, Any],
    http_request: Request,
    db: AsyncSession = Depends(get_db),
    ai_service: AIService = Depends(get_user_ai_service)
) -> Dict[str, Any]:
    """
    根据当前收集的信息生成下一步的选项建议（带自动重试）
    
    Request:
        {
            "step": "title",  // title/description/theme/genre
            "context": {
                "title": "...",
                "description": "...",
                "theme": "..."
            }
        }
    
    Response:
        {
            "prompt": "引导语",
            "options": ["选项1", "选项2", ...]
        }
    """
    max_retries = 3
    
    for attempt in range(max_retries):
        try:
            step = data.get("step", "title")
            context = data.get("context", {})
            
            logger.info(f"灵感模式：生成{step}阶段的选项（第{attempt + 1}次尝试）")
            
            # 获取用户ID
            user_id = getattr(http_request.state, 'user_id', None)
            
            # 获取对应的提示词模板（根据step确定模板key）
            # 新结构：每个步骤有独立的 SYSTEM 和 USER 模板
            template_key_map = {
                "title": ("INSPIRATION_TITLE_SYSTEM", "INSPIRATION_TITLE_USER"),
                "description": ("INSPIRATION_DESCRIPTION_SYSTEM", "INSPIRATION_DESCRIPTION_USER"),
                "theme": ("INSPIRATION_THEME_SYSTEM", "INSPIRATION_THEME_USER"),
                "genre": ("INSPIRATION_GENRE_SYSTEM", "INSPIRATION_GENRE_USER")
            }
            template_keys = template_key_map.get(step)
            
            if not template_keys:
                return {
                    "error": f"不支持的步骤: {step}",
                    "prompt": "",
                    "options": []
                }
            
            system_key, user_key = template_keys
            
            # 获取自定义提示词模板（分别获取 system 和 user）
            system_template = await PromptService.get_template(system_key, user_id, db)
            user_template = await PromptService.get_template(user_key, user_id, db)
            
            # 准备格式化参数
            format_params = {
                "initial_idea": context.get("initial_idea", context.get("description", "")),
                "title": context.get("title", ""),
                "description": context.get("description", ""),
                "theme": context.get("theme", "")
            }
            research_bundle = await _build_inspiration_research_bundle(
                step=step,
                context=context,
                user_id=user_id,
                db_session=db,
                enable_web_research=data.get("enable_web_research"),
                web_research_query=data.get("web_research_query"),
            )
            
            # Build prompts
            system_prompt = system_template.format(**format_params)
            user_prompt = user_template.format(**format_params)
            style_guard = _build_style_guard(step)
            system_prompt = f"{system_prompt}\n\n{style_guard}"
            research_note = _build_inspiration_research_note(research_bundle)
            if research_note:
                system_prompt = f"{system_prompt}\n\n{research_note}"
            
            if attempt > 0:
                system_prompt += f"\n\n这是第{attempt + 1}次生成，请只返回合法JSON，并确保options里有6个有效选项。"
            
            # 调用AI生成选项
            # 关键改进：使用递减的temperature以保持后续阶段与前文的一致性
            temperature = TEMPERATURE_SETTINGS.get(step, 0.7)
            logger.info(f"调用AI生成{step}选项... (temperature={temperature})")
            request_options = _build_generation_request_options(ai_service)
            
            # 流式生成并累积文本
            accumulated_text = ""
            async for chunk in ai_service.generate_text_stream(
                prompt=user_prompt,
                system_prompt=system_prompt,
                temperature=temperature,
                request_options=request_options,
            ):
                accumulated_text += chunk
            
            response = {"content": accumulated_text}
            content = accumulated_text
            logger.info(f"AI返回内容长度: {len(content)}")
            
            # 解析JSON（使用统一的JSON清洗方法）
            try:
                # 使用统一的JSON清洗方法
                cleaned_content = ai_service._clean_json_response(content)
                
                result = json.loads(cleaned_content)
                
                # 校验返回格式
                is_valid, error_msg = validate_options_response(result, step)
                
                if not is_valid:
                    logger.warning(f"⚠️ 第{attempt + 1}次生成格式校验失败: {error_msg}")
                    if attempt < max_retries - 1:
                        logger.info("准备重试...")
                        continue  # 重试
                    else:
                        # 最后一次尝试也失败了
                        return {
                            "prompt": f"请为【{step}】提供内容：",
                            "options": ["让AI重新生成", "我自己输入"],
                            "error": f"AI生成格式错误（{error_msg}），已自动重试{max_retries}次，请手动重试或自己输入"
                        }
                
                logger.info(f"✅ 第{attempt + 1}次成功生成{len(result.get('options', []))}个有效选项")
                return _attach_inspiration_research_payload(result, research_bundle)
                
            except json.JSONDecodeError as e:
                logger.error(f"第{attempt + 1}次JSON解析失败: {e}")
                
                if attempt < max_retries - 1:
                    logger.info("JSON解析失败，准备重试...")
                    continue  # 重试
                else:
                    # 最后一次尝试也失败了
                    return {
                        "prompt": f"请为【{step}】提供内容：",
                        "options": ["让AI重新生成", "我自己输入"],
                        "error": f"AI返回格式错误，已自动重试{max_retries}次，请手动重试或自己输入"
                    }
        
        except Exception as e:
            logger.error(f"第{attempt + 1}次生成失败: {e}", exc_info=True)
            if attempt < max_retries - 1:
                logger.info("发生异常，准备重试...")
                continue
            else:
                return {
                    "error": str(e),
                    "prompt": "生成失败，请重试",
                    "options": ["重新生成", "我自己输入"]
                }
    
    # 理论上不会到这里
    return {
        "error": "生成失败",
        "prompt": "请重试",
        "options": []
    }


@router.post("/refine-options")
async def refine_options(
    data: Dict[str, Any],
    http_request: Request,
    db: AsyncSession = Depends(get_db),
    ai_service: AIService = Depends(get_user_ai_service)
) -> Dict[str, Any]:
    """
    基于用户反馈重新生成选项（支持多轮对话）
    
    Request:
        {
            "step": "title",  // 当前步骤
            "context": {
                "initial_idea": "...",
                "title": "...",
                "description": "...",
                "theme": "..."
            },
            "feedback": "我想要更悲剧一些的主题",  // 用户反馈
            "previous_options": ["选项1", "选项2", ...]  // 之前的选项（可选）
        }
    
    Response:
        {
            "prompt": "引导语",
            "options": ["新选项1", "新选项2", ...]
        }
    """
    max_retries = 3
    
    for attempt in range(max_retries):
        try:
            step = data.get("step", "title")
            context = data.get("context", {})
            feedback = data.get("feedback", "")
            previous_options = data.get("previous_options", [])
            
            logger.info(f"灵感模式：根据反馈重新生成{step}阶段的选项（第{attempt + 1}次尝试）")
            logger.info(f"用户反馈: {feedback}")
            
            # 获取用户ID
            user_id = getattr(http_request.state, 'user_id', None)
            
            # 获取对应的提示词模板
            template_key_map = {
                "title": ("INSPIRATION_TITLE_SYSTEM", "INSPIRATION_TITLE_USER"),
                "description": ("INSPIRATION_DESCRIPTION_SYSTEM", "INSPIRATION_DESCRIPTION_USER"),
                "theme": ("INSPIRATION_THEME_SYSTEM", "INSPIRATION_THEME_USER"),
                "genre": ("INSPIRATION_GENRE_SYSTEM", "INSPIRATION_GENRE_USER")
            }
            template_keys = template_key_map.get(step)
            
            if not template_keys:
                return {
                    "error": f"不支持的步骤: {step}",
                    "prompt": "",
                    "options": []
                }
            
            system_key, user_key = template_keys
            
            # 获取自定义提示词模板
            system_template = await PromptService.get_template(system_key, user_id, db)
            user_template = await PromptService.get_template(user_key, user_id, db)
            
            # 准备格式化参数
            format_params = {
                "initial_idea": context.get("initial_idea", context.get("description", "")),
                "title": context.get("title", ""),
                "description": context.get("description", ""),
                "theme": context.get("theme", "")
            }
            research_bundle = await _build_inspiration_research_bundle(
                step=step,
                context=context,
                user_id=user_id,
                db_session=db,
                enable_web_research=data.get("enable_web_research"),
                web_research_query=data.get("web_research_query"),
                feedback=feedback,
            )
            
            # Build prompts
            system_prompt = system_template.format(**format_params)
            user_prompt = user_template.format(**format_params)
            style_guard = _build_style_guard(step)
            system_prompt = f"{system_prompt}\n\n{style_guard}"
            research_note = _build_inspiration_research_note(research_bundle)
            if research_note:
                system_prompt = f"{system_prompt}\n\n{research_note}"
            
            feedback_instruction = f"""

用户对上一轮选项不满意，反馈如下：
【{feedback}】
上一轮选项：
{chr(10).join([f"- {opt}" for opt in previous_options]) if previous_options else "（无）"}

请根据反馈调整方向，给出更贴近用户预期的新选项。要求：
1. 先理解反馈意图，再改写方向。
2. 新选项要体现用户提出的偏好变化。
3. 与现有上下文保持一致，不跑题。
4. 返回 6 个有效选项。
5. 至少 2 个选项必须明显跳出上一轮表达结构，不能只做同义改写。
"""
            system_prompt += feedback_instruction
            
            # 如果是重试，强调格式要求
            if attempt > 0:
                system_prompt += f"\n\n这是第{attempt + 1}次生成，请只返回合法JSON。"
            
            # 调用AI生成选项
            temperature = TEMPERATURE_SETTINGS.get(step, 0.7)
            # 反馈生成时使用稍高的temperature以获得更多样化的结果
            temperature = min(temperature + 0.1, 0.9)
            logger.info(f"调用AI根据反馈生成{step}选项... (temperature={temperature})")
            request_options = _build_generation_request_options(ai_service)
            
            # 流式生成并累积文本
            accumulated_text = ""
            async for chunk in ai_service.generate_text_stream(
                prompt=user_prompt,
                system_prompt=system_prompt,
                temperature=temperature,
                request_options=request_options,
            ):
                accumulated_text += chunk
            
            content = accumulated_text
            logger.info(f"AI返回内容长度: {len(content)}")
            
            # 解析JSON
            try:
                cleaned_content = ai_service._clean_json_response(content)
                result = json.loads(cleaned_content)
                
                # 校验返回格式
                is_valid, error_msg = validate_options_response(result, step)
                
                if not is_valid:
                    logger.warning(f"⚠️ 第{attempt + 1}次生成格式校验失败: {error_msg}")
                    if attempt < max_retries - 1:
                        logger.info("准备重试...")
                        continue
                    else:
                        return {
                            "prompt": f"请为【{step}】提供内容：",
                            "options": ["让AI重新生成", "我自己输入"],
                            "error": f"AI生成格式错误（{error_msg}），已自动重试{max_retries}次"
                        }
                
                logger.info(f"✅ 第{attempt + 1}次根据反馈成功生成{len(result.get('options', []))}个有效选项")
                return _attach_inspiration_research_payload(result, research_bundle)
                
            except json.JSONDecodeError as e:
                logger.error(f"第{attempt + 1}次JSON解析失败: {e}")
                
                if attempt < max_retries - 1:
                    logger.info("JSON解析失败，准备重试...")
                    continue
                else:
                    return {
                        "prompt": f"请为【{step}】提供内容：",
                        "options": ["让AI重新生成", "我自己输入"],
                        "error": f"AI返回格式错误，已自动重试{max_retries}次"
                    }
        
        except Exception as e:
            logger.error(f"第{attempt + 1}次根据反馈生成失败: {e}", exc_info=True)
            if attempt < max_retries - 1:
                logger.info("发生异常，准备重试...")
                continue
            else:
                return {
                    "error": str(e),
                    "prompt": "生成失败，请重试",
                    "options": ["重新生成", "我自己输入"]
                }
    
    return {
        "error": "生成失败",
        "prompt": "请重试",
        "options": []
    }


@router.post("/quick-generate")
async def quick_generate(
    data: Dict[str, Any],
    http_request: Request,
    db: AsyncSession = Depends(get_db),
    ai_service: AIService = Depends(get_user_ai_service)
) -> Dict[str, Any]:
    """
    智能补全：根据用户已提供的部分信息，AI自动补全缺失字段
    
    Request:
        {
            "title": "书名（可选）",
            "description": "简介（可选）",
            "theme": "主题（可选）",
            "genre": ["类型1", "类型2"] 或 "类型1,类型2"（可选）, 
            "narrative_perspective": "第一人称/第三人称/全知视角"（可选）
        }
    
    Response:
        {
            "title": "补全的书名",
            "description": "补全的简介",
            "theme": "补全的主题",
            "genre": ["补全的类型"],
            "narrative_perspective": "叙事视角"
        }
    """
    try:
        logger.info("灵感模式：智能补全")

        # 获取用户ID
        user_id = getattr(http_request.state, 'user_id', None)

        provided_genre = _normalize_genre_list(data.get("genre"))
        provided_perspective = _normalize_narrative_perspective(data.get("narrative_perspective"))
        existing_text = _build_quick_generate_existing_text(data)

        # 获取自定义提示词模板
        system_template = await PromptService.get_template("INSPIRATION_QUICK_COMPLETE", user_id, db)

        # 格式化提示词
        prompts = {
            "system": (
                f"{PromptService.format_prompt(system_template, existing=existing_text)}\n\n"
                f"{_build_style_guard('description')}\n"
                "【智能补全专项】保证四个字段像同一部小说，人物语气自然，信息前后一致；"
                "仅返回JSON字段值，不输出流程说明或执行步骤；"
                "信息不足时先补目标→阻力→选择→后果链；"
                "如果用户没给叙事视角，请补一个最适合题材与冲突表达的视角。"
            ),
            "user": "请在不偏离现有信息的前提下补全缺失字段，只返回JSON。"
        }
        
        # 调用AI - 流式生成并累积文本
        accumulated_text = ""
        request_options = _build_generation_request_options(ai_service)
        async for chunk in ai_service.generate_text_stream(
            prompt=prompts["user"],
            system_prompt=prompts["system"],
            temperature=0.78,
            request_options=request_options,
        ):
            accumulated_text += chunk
        
        response = {"content": accumulated_text}
        content = accumulated_text
        
        # 解析JSON（使用统一的JSON清洗方法）
        try:
            # 使用统一的JSON清洗方法
            cleaned_content = ai_service._clean_json_response(content)
            
            result = json.loads(cleaned_content)

            result_genre = _normalize_genre_list(result.get("genre"))
            result_perspective = _normalize_narrative_perspective(
                result.get("narrative_perspective")
            )

            # 合并用户已提供的信息（用户输入优先）
            final_result = {
                "title": data.get("title") or result.get("title", ""),
                "description": data.get("description") or result.get("description", ""),
                "theme": data.get("theme") or result.get("theme", ""),
                "genre": provided_genre or result_genre,
                "narrative_perspective": (
                    provided_perspective
                    if data.get("narrative_perspective")
                    else result_perspective
                ),
            }
            
            logger.info(f"✅ 智能补全成功")
            return final_result
            
        except json.JSONDecodeError as e:
            logger.error(f"JSON解析失败: {e}")
            raise Exception("AI返回格式错误，请重试")
    
    except Exception as e:
        logger.error(f"智能补全失败: {e}", exc_info=True)
        return {
            "error": str(e)
        }
