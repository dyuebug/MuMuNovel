"""构建章节生成 runtime prompt 片段。"""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from app.models.project import Project
from app.services.outline_requirement_service import extract_outline_anchor_lines


def detect_style_profile(
    style_name: Optional[str],
    style_preset_id: Optional[str],
    style_content: Optional[str] = None,
) -> str:
    """识别写作风格画像，用于运行时护栏和采样参数调整。"""
    preset = (style_preset_id or "").strip().lower()
    name = (style_name or "").strip().lower()
    content = (style_content or "").strip()

    if preset == "low_ai_serial" or "低ai连载感" in name or "连载感" in content:
        return "low_ai_serial"
    if preset == "low_ai_life" or "低ai生活化" in name or "生活化" in content:
        return "low_ai_life"
    return "default"


def resolve_generation_temperature(style_profile: str) -> float:
    """根据写作风格返回更合适的生成温度。"""
    if style_profile == "low_ai_serial":
        return 0.82
    if style_profile == "low_ai_life":
        return 0.78
    return 0.72


def build_chapter_runtime_system_prompt(
    project: Project,
    style_content: str,
    chapter_outline: Optional[str],
    previous_summary: Optional[str] = None,
    style_name: Optional[str] = None,
    style_preset_id: Optional[str] = None,
    target_word_count: Optional[int] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
    web_research_grounding_block: Optional[str] = None,
) -> str:
    """构建章节生成运行时系统提示词（风格、世界锚点、剧情锚点与护栏）。"""
    style_profile = detect_style_profile(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )

    style_block = (
        f"【🎨 写作风格参考】\n\n{style_content}\n\n"
        if style_content else ""
    )

    outline_anchor_lines = extract_outline_anchor_lines(chapter_outline)
    outline_anchor_block = (
        "【🧭 本章剧情锚点（需覆盖）】\n"
        + "\n".join(f"- {line}" for line in outline_anchor_lines)
        + "\n\n"
        if outline_anchor_lines else ""
    )

    previous_summary_block = (
        f"【📋 上章回执】\n- {previous_summary[:200]}\n\n"
        if previous_summary else ""
    )

    organization_ledger_source: List[str] = []
    if isinstance(story_runtime_contract, dict):
        runtime_blueprint = story_runtime_contract.get("blueprint")
        if isinstance(runtime_blueprint, dict):
            organization_ledger_source = runtime_blueprint.get("organization_state_ledger") or []
        if not organization_ledger_source:
            organization_ledger_source = story_runtime_contract.get("organization_state_ledger") or []

    organization_state_ledger = [
        str(item).strip()
        for item in organization_ledger_source
        if str(item).strip()
    ][:4]
    organization_continuity_block = (
        "【🏛️ 组织连续性（本章必须落地）】\n"
        + "\n".join(f"- {entry}" for entry in organization_state_ledger)
        + "\n- 以上组织不能只停留在背景设定里，至少让其中 1-2 个组织通过命令、权限、封锁、资源调度、公开通报或现场约束进入当前冲突。\n"
        + "- 若组织已在本章大纲里激活，就必须在正文中体现其动作、压力来源或实际影响，不能只由角色转述。\n\n"
        if organization_state_ledger
        else ""
    )

    safe_target_word_count = max(200, int(target_word_count or 0)) if target_word_count else None
    target_lower_bound = (
        max(200, min(safe_target_word_count - 120, int(safe_target_word_count * 0.9)))
        if safe_target_word_count
        else None
    )
    target_upper_bound = (
        max(
            (target_lower_bound or 200) + 80,
            min(safe_target_word_count + 150, int(safe_target_word_count * 1.15)),
        )
        if safe_target_word_count
        else None
    )

    guard_lines = [
        (
            f"- 字数是硬约束：目标约{safe_target_word_count}字，理想范围 {target_lower_bound}-{target_upper_bound} 字；一旦接近上限就立刻收束。"
            if safe_target_word_count and target_lower_bound and target_upper_bound
            else "- 字数控制优先于铺陈，避免越写越散、越收越晚。"
        ),
        "- 当主冲突结果和章尾钩子已经成立时立即停笔，不补尾声、复盘、世界观讲解或重复心理总结。",
        "- 若素材过多，优先保留“目标→阻力→选择→代价/钩子”主链，砍掉支线说明和背景补课。",
        "- 只输出章节正文，不输出流程说明、调度术语、策略说明或自我评注",
        "- 发生信息冲突时按优先级处理：本章大纲 > 上章回执 > 最近上下文 > 相关记忆",
        "- 信息不足时先补最小动作闭环：目标→阻力→选择→即时后果，避免空泛总结",
        "- 先写正在发生的动作与人物反应，再补必要解释；让读者先“看到”，再“理解”",
        "- 对话要区分人物声线：同一信息由不同角色说出来，词汇和语气必须有差别",
        "- 情绪要有层次：至少体现“触发→压住/回避→外露”中的两个阶段，避免一步到位喊口号",
        "- 遇到设定术语时，用角色追问、吐槽或误解补一句人话解释，不要硬塞定义",
        "- 关键桥段尽量写成“动作→反馈→余波/代价”，避免整段概述",
        "- 同一自然段尽量只保留1个有效比喻；慎用“像……/仿佛/像……一样”，能直写动作结果就别先比喻",
        "- 少用“下一秒/那一瞬/忽然/不是……而是……”等固定推进句式，连续出现会削弱真人感",
        "- 疼痛、惊惧和异常优先写身体反应、动作受阻、物件变化和现场声响，不要每次都靠抽象意象撑气氛",
        "- 允许出现朴素、直接、略笨的过渡句，不要把每一句都写成有设计感的“好句子”",
        "- 保留少量口语颗粒和不完美句，不要把每句都修成工整书面句",
        "- 避免模板化开头和总结腔，如“总之/综上/值得注意的是/在这个过程中”",
        "- 禁止出现“执行X.X/调用Agent/方案A-B/复盘”这类流程文本",
        "- Mid-scene must include one obstruction or misread that forces a choice and immediately pays a cost; do not let the chapter glide forward without friction.",
        "- If character and relationship ledgers exist, explicitly carry at least one character-state item and one relationship-state item into on-page action, stance, dialogue, or cost.",
        "- The final paragraph must leave a fresh unresolved push: an information gap, approaching danger, identity shift, or pending choice.",
        "- Avoid soft landing endings like 'in short', 'everything will be fine', 'the story continues', or 'he finally understood'; the last line must pin down next-step pressure.",
    ]

    if style_profile == "low_ai_serial":
        guard_lines.extend(
            [
                "- 连载感优先：中段要有一次小波折或误判，结尾留“自然未完感”，不要生硬反转",
                "- 主角和至少1名核心配角都要出现可见情绪反差（嘴硬、迟疑、硬撑、破防等）",
                "- 配角不能只附和主角，至少让一名配角做出会改变局面的主动选择",
            ]
        )
    elif style_profile == "low_ai_life":
        guard_lines.extend(
            [
                "- 生活化优先：通过细小动作、语气词、场景噪声传递情绪，不堆抽象形容词",
                "- 对白允许打断、改口和留白，避免角色轮流端着讲道理",
            ]
        )

    web_research_block = web_research_grounding_block or ''

    return f"""{style_block}【🌍 世界观锚点】
- 时间背景：{project.world_time_period or '未设定'}
- 地理位置：{project.world_location or '未设定'}
- 氛围基调：{project.world_atmosphere or '未设定'}
- 世界规则：{project.world_rules or '未设定'}

{outline_anchor_block}{previous_summary_block}{organization_continuity_block}{web_research_block}【创作护栏】
{chr(10).join(guard_lines)}
"""
