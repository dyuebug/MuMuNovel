"""Story writing style owner for prompt style guard composition."""

from __future__ import annotations


class WritingStyleManager:
    """写作风格管理器"""

    @staticmethod
    def apply_style_to_prompt(base_prompt: str, style_content: str) -> str:
        """
        将写作风格应用到基础提示词中

        Args:
            base_prompt: 基础提示词
            style_content: 风格要求内容

        Returns:
            组合后的提示词
        """
        style_profile = "default"
        normalized = (style_content or "").lower()
        if "连载感" in normalized:
            style_profile = "low_ai_serial"
        elif "生活化" in normalized:
            style_profile = "low_ai_life"
        elif "都市金融" in normalized or ("金融" in normalized and "商战" in normalized):
            style_profile = "urban_finance"
        elif "技术流修仙" in normalized or ("技术流" in normalized and "修仙" in normalized):
            style_profile = "tech_xianxia"
        elif "轻松幽默" in normalized or "幽默" in normalized:
            style_profile = "light_humor"
        elif "朴实年代" in normalized or "年代风" in normalized:
            style_profile = "era_plain"

        common_guard = (
            "写作执行要点："
            "你正在写长篇小说中段，不是开书导语，也不是全书终章。"
            "用中文母语者的自然表达写作，长短句穿插，读起来顺口。"
            "对话要像真人交流，少讲道理，多给反应和潜台词。"
            "出现设定术语时，尽量在场景中补一句通俗解释。"
            "比喻要克制：能直接写动作、表情、声音和即时结果，就不要先写抽象比喻。"
            "慎用高频定式句法，如“像……一样”“仿佛”“不是……而是……”“下一秒”“那一瞬”“忽然”。"
            "疼痛、恐惧和异常优先写身体反应、动作受阻、物件变化和现场声响，不要每次都靠意象包裹。"
            "允许保留少量朴素、直接、甚至略笨的过渡句，不要把每句话都打磨成有设计感的好句子。"
            "结尾禁止总结型/预告型/感悟型收束，优先停在动作、对话或突发事件上。"
            "直接输出章节正文，不要加章节标题和额外说明。"
        )

        serial_guard = (
            "连载强化要点："
            "保持追更节奏，中段给小波折，章末留自然未完感。"
            "人物情绪要有层次，不要开口就结论化表态。"
            "让配角有主动选择，避免只当信息传声筒。"
            "同一自然段尽量不要连续堆叠两个以上“像……”比喻；危险感先靠事件和反馈建立。"
        )

        life_guard = (
            "生活化强化要点："
            "优先用动作、表情和场景噪声传递情绪，别把解释写满。"
            "允许少量口语毛边，避免句句工整。"
            "少写漂亮空话和修辞连发，保留日常说话的停顿、改口和没那么圆的句子。"
        )

        urban_finance_guard = (
            "都市金融强化要点："
            "专业术语要落地到利益得失，避免术语堆砌。"
            "谈判和博弈要体现信息差与筹码变化，突出人物选择代价。"
        )

        tech_xianxia_guard = (
            "技术流修仙强化要点："
            "规则推演要清楚，但每段都要有行动反馈，避免连续讲义化解释。"
            "术法/阵法/功法术语出现后，尽量用角色互动补一句人话解释。"
        )

        light_humor_guard = (
            "轻松幽默强化要点："
            "笑点要服务剧情推进，不做连续段子堆叠。"
            "人物互怼要有立场差异，避免全员同口吻抖机灵。"
        )

        era_plain_guard = (
            "朴实年代强化要点："
            "时代细节要自然入戏，优先写可见的生活动作与人际压力。"
            "语言克制朴素，避免现代网络梗和悬浮金句。"
        )

        profile_guard = ""
        if style_profile == "low_ai_serial":
            profile_guard = serial_guard
        elif style_profile == "low_ai_life":
            profile_guard = life_guard
        elif style_profile == "urban_finance":
            profile_guard = urban_finance_guard
        elif style_profile == "tech_xianxia":
            profile_guard = tech_xianxia_guard
        elif style_profile == "light_humor":
            profile_guard = light_humor_guard
        elif style_profile == "era_plain":
            profile_guard = era_plain_guard

        return f"{base_prompt}\n\n{style_content}\n\n{common_guard}\n{profile_guard}".strip()
