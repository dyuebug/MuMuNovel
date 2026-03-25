from app.api.outlines import _merge_outline_requirements
from app.services.prompt_service import (
    QUALITY_PREFERENCE_SPECS,
    build_story_acceptance_card_block,
    build_story_character_arc_card_block,
    build_story_cliffhanger_card_block,
    build_story_creation_brief_block,
    build_quality_preference_block,
    build_story_dialogue_advancement_card_block,
    build_story_emotion_landing_card_block,
    build_story_execution_checklist_block,
    build_story_information_release_card_block,
    build_story_opening_hook_card_block,
    build_story_payoff_chain_card_block,
    build_story_action_rendering_card_block,
    build_story_repetition_control_card_block,
    build_story_rule_grounding_card_block,
    build_story_scene_anchor_card_block,
    build_story_scene_density_card_block,
    build_story_summary_tone_control_card_block,
    build_story_viewpoint_discipline_card_block,
    PromptService,
    build_story_objective_card_block,
    build_story_repair_target_block,
    build_story_repetition_risk_block,
    build_story_result_card_block,
    build_story_long_term_goal_block,
    build_story_character_focus_anchor_block,
    build_story_foreshadow_payoff_plan_block,
    build_story_pacing_budget_block,
    build_volume_pacing_block,
)


def test_should_inject_narrative_blueprint_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
    )

    assert "【结构蓝图】" in blocks["narrative_blueprint_block"]
    assert "钩子优先" in blocks["narrative_blueprint_block"]
    assert "主线推进" in blocks["narrative_blueprint_block"]
    assert "【结构蓝图】" in blocks["quality_contract_block"]


def test_should_build_story_objective_card_block_with_stage_hint():
    block = build_story_objective_card_block(
        "suspense",
        "relationship_shift",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节目标卡】" in block
    assert "- 目标：" in block
    assert "- 阻力：" in block
    assert "- 转折：" in block
    assert "- 钩子：" in block
    assert "高潮阶段" in block


def test_should_inject_story_objective_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="relationship_shift",
        plot_stage="climax",
    )

    assert "【章节目标卡】" in blocks["story_objective_card_block"]
    assert "高潮阶段" in blocks["story_objective_card_block"]
    assert "【章节目标卡】" in blocks["quality_contract_block"]



def test_should_build_story_creation_brief_block():
    block = build_story_creation_brief_block(
        "本轮按「钩子优先 / 高潮阶段」创作，目标是把正面对撞写实。"
    )

    assert "【本轮创作总控】" in block
    assert "执行摘要" in block
    assert "钩子优先" in block


def test_should_inject_story_creation_brief_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="climax",
        story_creation_brief="本轮先把目标、阻力和收束写扎实。",
    )

    assert "【本轮创作总控】" in blocks["story_creation_brief_block"]
    assert "目标、阻力和收束" in blocks["story_creation_brief_block"]
    assert "【本轮创作总控】" in blocks["quality_contract_block"]


def test_should_build_quality_preference_block_with_notes():
    notes = "add more echo and tactile detail"
    block = build_quality_preference_block(
        "immersive",
        notes,
        scene="chapter",
    )

    assert QUALITY_PREFERENCE_SPECS["immersive"]["label"] in block
    assert QUALITY_PREFERENCE_SPECS["immersive"]["chapter"][0] in block
    assert notes in block



def test_should_split_multi_line_quality_notes_into_prompt_bullets():
    notes = "减少解释句；保留动作反馈\n强化章尾牵引"
    block = build_quality_preference_block(
        "plot_drive",
        notes,
        scene="chapter",
    )

    assert "- 补充偏好：" in block
    assert "  - 减少解释句" in block
    assert "  - 保留动作反馈" in block
    assert "  - 强化章尾牵引" in block
def test_should_inject_quality_preference_block_into_chapter_quality_contract():
    notes = "make endings snap harder"
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        quality_preset="plot_drive",
        quality_notes=notes,
    )

    assert QUALITY_PREFERENCE_SPECS["plot_drive"]["label"] in blocks["quality_preference_block"]
    assert notes in blocks["quality_preference_block"]
    assert blocks["quality_preference_block"] in blocks["quality_contract_block"]


def test_should_build_story_repair_target_block():
    block = build_story_repair_target_block(
        "优先补强冲突抬压",
        ["写实受阻", "升级代价"],
        ["保留对白辨识度"],
    )

    assert "【修复目标卡】" in block
    assert "优先补强冲突抬压" in block
    assert "写实受阻" in block
    assert "保留对白辨识度" in block


def test_should_inject_story_repair_target_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="climax",
        story_repair_summary="下一轮先修复章尾牵引",
        story_repair_targets=["补强章尾钩子", "让下一步更具体"],
        story_preserve_strengths=["保留人物火花"],
    )

    assert "【修复目标卡】" in blocks["story_repair_target_block"]
    assert "补强章尾钩子" in blocks["story_repair_target_block"]
    assert "保留人物火花" in blocks["story_repair_target_block"]
    assert "【修复目标卡】" in blocks["quality_contract_block"]


def test_should_build_story_result_card_block_with_stage_hint():
    block = build_story_result_card_block(
        "payoff",
        "foreshadow_payoff",
        scene="chapter",
        plot_stage="ending",
    )

    assert "【章节结果卡】" in block
    assert "- 推进：" in block
    assert "- 揭示：" in block
    assert "- 关系：" in block
    assert "- 余波：" in block
    assert "结局阶段" in block


def test_should_inject_story_result_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="ending",
    )

    assert "【章节结果卡】" in blocks["story_result_card_block"]
    assert "结局阶段" in blocks["story_result_card_block"]
    assert "【章节结果卡】" in blocks["quality_contract_block"]


def test_should_build_story_execution_checklist_block_with_stage_hint():
    block = build_story_execution_checklist_block(
        "hook",
        "advance_plot",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节执行清单】" in block
    assert "- 开场：" in block
    assert "- 加压：" in block
    assert "- 转折：" in block
    assert "- 收束：" in block
    assert "高潮阶段" in block


def test_should_inject_story_execution_checklist_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="climax",
    )

    assert "【章节执行清单】" in blocks["story_execution_checklist_block"]
    assert "高潮阶段" in blocks["story_execution_checklist_block"]
    assert "【章节执行清单】" in blocks["quality_contract_block"]


def test_should_build_story_scene_anchor_card_block_with_stage_hint():
    block = build_story_scene_anchor_card_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节场景调度卡】" in block
    assert "- 入场锚点：" in block
    assert "- 镜头重心：" in block
    assert "- 信息投放：" in block
    assert "- 切换规则：" in block
    assert "高潮阶段" in block


def test_should_inject_story_scene_anchor_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="climax",
    )

    assert "【章节场景调度卡】" in blocks["story_scene_anchor_card_block"]
    assert "高潮阶段" in blocks["story_scene_anchor_card_block"]
    assert "【章节场景调度卡】" in blocks["quality_contract_block"]


def test_should_build_story_repetition_risk_block_with_stage_hint():
    block = build_story_repetition_risk_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="ending",
    )

    assert "【章节重复风险卡】" in block
    assert "- 开场风险：" in block
    assert "- 加压风险：" in block
    assert "- 转折风险：" in block
    assert "- 收尾风险：" in block
    assert "结局阶段" in block


def test_should_inject_story_repetition_risk_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="ending",
    )

    assert "【章节重复风险卡】" in blocks["story_repetition_risk_block"]
    assert "结局阶段" in blocks["story_repetition_risk_block"]
    assert "【章节重复风险卡】" in blocks["quality_contract_block"]


def test_should_build_story_acceptance_card_block_with_stage_hint():
    block = build_story_acceptance_card_block(
        "payoff",
        "foreshadow_payoff",
        scene="chapter",
        plot_stage="ending",
    )

    assert "【章节验收卡】" in block
    assert "- 任务命中：" in block
    assert "- 变化落地：" in block
    assert "- 新鲜度：" in block
    assert "- 收束质量：" in block
    assert "结局阶段" in block


def test_should_inject_story_acceptance_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="ending",
    )

    assert "【章节验收卡】" in blocks["story_acceptance_card_block"]
    assert "结局阶段" in blocks["story_acceptance_card_block"]
    assert "【章节验收卡】" in blocks["quality_contract_block"]


def test_should_build_story_rule_grounding_card_block_with_stage_hint():
    block = build_story_rule_grounding_card_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="development",
    )

    assert "【章节设定落地卡】" in block
    assert "- 规则着陆：" in block
    assert "- 触发条件：" in block
    assert "- 代价/限制：" in block
    assert "- 场景表现：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "发展阶段" in block


def test_should_inject_story_rule_grounding_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="development",
    )

    assert "【章节设定落地卡】" in blocks["story_rule_grounding_card_block"]
    assert "发展阶段" in blocks["story_rule_grounding_card_block"]
    assert "【章节设定落地卡】" in blocks["quality_contract_block"]


def test_should_build_story_information_release_card_block_with_stage_hint():
    block = build_story_information_release_card_block(
        "emotion",
        "deepen_character",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节信息投放卡】" in block
    assert "- 本轮信息：" in block
    assert "- 承载方式：" in block
    assert "- 解释上限：" in block
    assert "- 读者抓手：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_information_release_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="emotion",
        story_focus="deepen_character",
        plot_stage="climax",
    )

    assert "【章节信息投放卡】" in blocks["story_information_release_card_block"]
    assert "高潮阶段" in blocks["story_information_release_card_block"]
    assert "【章节信息投放卡】" in blocks["quality_contract_block"]


def test_should_build_story_emotion_landing_card_block_with_stage_hint():
    block = build_story_emotion_landing_card_block(
        "emotion",
        "deepen_character",
        scene="chapter",
        plot_stage="ending",
    )

    assert "【章节情绪落点卡】" in block
    assert "- 触发点：" in block
    assert "- 外显反应：" in block
    assert "- 关系余波：" in block
    assert "- 层次推进：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "结局阶段" in block


def test_should_inject_story_emotion_landing_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="emotion",
        story_focus="deepen_character",
        plot_stage="ending",
    )

    assert "【章节情绪落点卡】" in blocks["story_emotion_landing_card_block"]
    assert "结局阶段" in blocks["story_emotion_landing_card_block"]
    assert "【章节情绪落点卡】" in blocks["quality_contract_block"]


def test_should_build_story_action_rendering_card_block_with_stage_hint():
    block = build_story_action_rendering_card_block(
        "hook",
        "escalate_conflict",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节动作显影卡】" in block
    assert "- 起手动作：" in block
    assert "- 碰撞反馈：" in block
    assert "- 局面变化：" in block
    assert "- 镜头优先：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_action_rendering_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="escalate_conflict",
        plot_stage="climax",
    )

    assert "【章节动作显影卡】" in blocks["story_action_rendering_card_block"]
    assert "高潮阶段" in blocks["story_action_rendering_card_block"]
    assert "【章节动作显影卡】" in blocks["quality_contract_block"]


def test_should_build_story_summary_tone_control_card_block_with_stage_hint():
    block = build_story_summary_tone_control_card_block(
        "payoff",
        "foreshadow_payoff",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节总结腔抑制卡】" in block
    assert "- 结论克制：" in block
    assert "- 替代表现：" in block
    assert "- 留白位置：" in block
    assert "- 句式控制：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_summary_tone_control_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="climax",
    )

    assert "【章节总结腔抑制卡】" in blocks["story_summary_tone_control_card_block"]
    assert "高潮阶段" in blocks["story_summary_tone_control_card_block"]
    assert "【章节总结腔抑制卡】" in blocks["quality_contract_block"]


def test_should_build_story_repetition_control_card_block_with_stage_hint():
    block = build_story_repetition_control_card_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="development",
    )

    assert "【章节重复压缩卡】" in block
    assert "- 重复对象：" in block
    assert "- 首次命中：" in block
    assert "- 后续处理：" in block
    assert "- 删并原则：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "发展阶段" in block


def test_should_inject_story_repetition_control_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="development",
    )

    assert "【章节重复压缩卡】" in blocks["story_repetition_control_card_block"]
    assert "发展阶段" in blocks["story_repetition_control_card_block"]
    assert "【章节重复压缩卡】" in blocks["quality_contract_block"]


def test_should_build_story_viewpoint_discipline_card_block_with_stage_hint():
    block = build_story_viewpoint_discipline_card_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节视角纪律卡】" in block
    assert "- 主镜头：" in block
    assert "- 可见边界：" in block
    assert "- 内心准入：" in block
    assert "- 切换条件：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_viewpoint_discipline_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="climax",
    )

    assert "【章节视角纪律卡】" in blocks["story_viewpoint_discipline_card_block"]
    assert "高潮阶段" in blocks["story_viewpoint_discipline_card_block"]
    assert "【章节视角纪律卡】" in blocks["quality_contract_block"]


def test_should_build_story_dialogue_advancement_card_block_with_stage_hint():
    block = build_story_dialogue_advancement_card_block(
        "relationship",
        "relationship_shift",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节对白推进卡】" in block
    assert "- 对话任务：" in block
    assert "- 信息落差：" in block
    assert "- 声线区分：" in block
    assert "- 动作陪跑：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_dialogue_advancement_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="relationship",
        story_focus="relationship_shift",
        plot_stage="climax",
    )

    assert "【章节对白推进卡】" in blocks["story_dialogue_advancement_card_block"]
    assert "高潮阶段" in blocks["story_dialogue_advancement_card_block"]
    assert "【章节对白推进卡】" in blocks["quality_contract_block"]


def test_should_build_story_scene_density_card_block_with_stage_hint():
    block = build_story_scene_density_card_block(
        "hook",
        "advance_plot",
        scene="chapter",
        plot_stage="development",
    )

    assert "【章节场景密度卡】" in block
    assert "- 场景任务：" in block
    assert "- 现场化：" in block
    assert "- 装载方式：" in block
    assert "- 节奏呼吸：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "发展阶段" in block


def test_should_inject_story_scene_density_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
    )

    assert "【章节场景密度卡】" in blocks["story_scene_density_card_block"]
    assert "发展阶段" in blocks["story_scene_density_card_block"]
    assert "【章节场景密度卡】" in blocks["quality_contract_block"]


def test_should_build_story_payoff_chain_card_block_with_stage_hint():
    block = build_story_payoff_chain_card_block(
        "payoff",
        "foreshadow_payoff",
        scene="chapter",
        plot_stage="ending",
    )

    assert "【章节爽点回收卡】" in block
    assert "- 预埋点：" in block
    assert "- 兑现点：" in block
    assert "- 反馈链：" in block
    assert "- 读者回报：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "结局阶段" in block


def test_should_inject_story_payoff_chain_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="ending",
    )

    assert "【章节爽点回收卡】" in blocks["story_payoff_chain_card_block"]
    assert "结局阶段" in blocks["story_payoff_chain_card_block"]
    assert "【章节爽点回收卡】" in blocks["quality_contract_block"]


def test_should_build_story_opening_hook_card_block_with_stage_hint():
    block = build_story_opening_hook_card_block(
        "hook",
        "advance_plot",
        scene="chapter",
        plot_stage="development",
    )

    assert "【章节开篇抓力卡】" in block
    assert "- 第一击：" in block
    assert "- 麻烦种子：" in block
    assert "- 未决问题：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "发展阶段" in block


def test_should_inject_story_opening_hook_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
    )

    assert "【章节开篇抓力卡】" in blocks["story_opening_hook_card_block"]
    assert "发展阶段" in blocks["story_opening_hook_card_block"]
    assert "【章节开篇抓力卡】" in blocks["quality_contract_block"]


def test_should_build_story_character_arc_card_block_with_stage_hint():
    block = build_story_character_arc_card_block(
        "emotion",
        "deepen_character",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节角色弧光卡】" in block
    assert "- 外在线：" in block
    assert "- 内在线：" in block
    assert "- 关系线：" in block
    assert "- 落点：" in block
    assert "高潮阶段" in block


def test_should_inject_story_character_arc_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="emotion",
        story_focus="deepen_character",
        plot_stage="climax",
    )

    assert "【章节角色弧光卡】" in blocks["story_character_arc_card_block"]
    assert "高潮阶段" in blocks["story_character_arc_card_block"]
    assert "【章节角色弧光卡】" in blocks["quality_contract_block"]


def test_should_build_story_cliffhanger_card_block_with_stage_hint():
    block = build_story_cliffhanger_card_block(
        "suspense",
        "reveal_mystery",
        scene="chapter",
        plot_stage="climax",
    )

    assert "【章节结尾悬停卡】" in block
    assert "- 未决点：" in block
    assert "- 下一步逼力：" in block
    assert "- 余味：" in block
    assert "- 阶段提醒：" in block
    assert "- 避免：" in block
    assert "高潮阶段" in block


def test_should_inject_story_cliffhanger_card_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="suspense",
        story_focus="reveal_mystery",
        plot_stage="climax",
    )

    assert "【章节结尾悬停卡】" in blocks["story_cliffhanger_card_block"]
    assert "高潮阶段" in blocks["story_cliffhanger_card_block"]
    assert "【章节结尾悬停卡】" in blocks["quality_contract_block"]


def test_should_merge_outline_requirements_with_stage_aware_blueprint():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "relationship_shift",
        "climax",
        12,
    )

    assert "保留双线叙事" in merged
    assert "【结构蓝图】" in merged
    assert "【卷级节奏】" in merged
    assert "悬念拉满" in merged
    assert "关系转折" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_objective_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "relationship_shift",
        "ending",
        12,
    )

    assert "【大纲目标卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_result_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "payoff",
        "foreshadow_payoff",
        "ending",
        12,
    )

    assert "【大纲结果卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_execution_checklist():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "hook",
        "advance_plot",
        "climax",
        12,
    )

    assert "【大纲执行清单】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_scene_anchor_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "climax",
        12,
    )

    assert "【大纲场景调度卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_repetition_risk_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "ending",
        12,
    )

    assert "【大纲重复风险卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_acceptance_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "payoff",
        "foreshadow_payoff",
        "ending",
        12,
    )

    assert "【大纲验收卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_rule_grounding_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "development",
        12,
    )

    assert "【大纲设定落地卡】" in merged
    assert "发展阶段" in merged


def test_should_merge_outline_requirements_with_story_information_release_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "emotion",
        "deepen_character",
        "climax",
        12,
    )

    assert "【大纲信息投放卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_emotion_landing_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "emotion",
        "deepen_character",
        "ending",
        12,
    )

    assert "【大纲情绪落点卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_action_rendering_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "hook",
        "escalate_conflict",
        "climax",
        12,
    )

    assert "【大纲动作显影卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_summary_tone_control_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "payoff",
        "foreshadow_payoff",
        "climax",
        12,
    )

    assert "【大纲总结腔抑制卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_repetition_control_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "development",
        12,
    )

    assert "【大纲重复压缩卡】" in merged
    assert "发展阶段" in merged


def test_should_merge_outline_requirements_with_quality_preference_block():
    notes = "stretch the emotional aftershock by one beat"
    expected_block = build_quality_preference_block(
        "emotion_drama",
        notes,
        scene="outline",
    )
    merged = _merge_outline_requirements(
        "keep dual timeline",
        "emotion",
        "deepen_character",
        "development",
        12,
        quality_preset="emotion_drama",
        quality_notes=notes,
    )

    assert expected_block in merged


def test_should_merge_outline_requirements_with_story_viewpoint_discipline_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "climax",
        12,
    )

    assert "【大纲视角纪律卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_dialogue_advancement_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "relationship",
        "relationship_shift",
        "climax",
        12,
    )

    assert "【大纲对白推进卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_scene_density_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "hook",
        "advance_plot",
        "development",
        12,
    )

    assert "【大纲场景密度卡】" in merged
    assert "发展阶段" in merged


def test_should_merge_outline_requirements_with_story_payoff_chain_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "payoff",
        "foreshadow_payoff",
        "ending",
        12,
    )

    assert "【大纲爽点回收卡】" in merged
    assert "结局阶段" in merged


def test_should_merge_outline_requirements_with_story_opening_hook_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "hook",
        "advance_plot",
        "development",
        12,
    )

    assert "【大纲开篇抓力卡】" in merged
    assert "发展阶段" in merged


def test_should_merge_outline_requirements_with_story_cliffhanger_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "suspense",
        "reveal_mystery",
        "climax",
        12,
    )

    assert "【大纲结尾悬停卡】" in merged
    assert "高潮阶段" in merged


def test_should_merge_outline_requirements_with_story_character_arc_card():
    merged = _merge_outline_requirements(
        "保留双线叙事",
        "emotion",
        "deepen_character",
        "climax",
        12,
    )

    assert "【大纲角色弧光卡】" in merged
    assert "高潮阶段" in merged


def test_should_build_volume_pacing_block_with_stage_hint():
    block = build_volume_pacing_block(10, plot_stage="ending")

    assert "【卷级节奏】" in block
    assert "第1-" in block
    assert "结局阶段" in block





def test_should_inject_story_repair_diagnostic_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        story_repair_diagnostic_block="\u3010\u8bca\u65ad\u4f18\u5148\u7ea7\u5361\u3011\n- \u5f53\u524d\u6700\u5f31\u9879\uff1a\u56de\u62a5\u5151\u73b0\uff08\u5f53\u524d\u503c\uff1a58\uff09",
    )

    assert "\u3010\u8bca\u65ad\u4f18\u5148\u7ea7\u5361\u3011" in blocks["story_repair_diagnostic_block"]
    assert "\u56de\u62a5\u5151\u73b0" in blocks["quality_contract_block"]


def test_should_merge_outline_quality_repair_guidance_into_requirements():
    merged = _merge_outline_requirements(
        "保持主要人物关系线清晰",
        "hook",
        story_focus="advance_plot",
        quality_repair_guidance="【诊断优先级卡】\n- 当前最弱项：章尾牵引（当前值：61）",
    )

    assert "保持主要人物关系线清晰" in merged
    assert "【诊断优先级卡】" in merged
    assert "当前最弱项：章尾牵引（当前值：61）" in merged



def test_should_build_story_blueprint_blocks():
    assert "【长线目标锚点】" in build_story_long_term_goal_block(
        "The lead must seize the capital before the enemy closes in."
    )
    assert "【章节角色焦点锚点】" in build_story_character_focus_anchor_block(
        ["Lin", "Su"],
        scene="chapter",
    )
    assert "【章节伏笔兑现计划】" in build_story_foreshadow_payoff_plan_block(
        ["recover the hidden key"],
        scene="chapter",
    )
    assert "【章节节奏预算】" in build_story_pacing_budget_block(
        12,
        current_chapter_number=5,
        target_word_count=2600,
        plot_stage="climax",
        scene="chapter",
    )


def test_should_inject_story_blueprint_blocks_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        story_long_term_goal="The lead must seize the capital before the enemy closes in.",
        story_character_focus=["Lin", "Su"],
        story_foreshadow_payoff_plan=["recover the hidden key"],
        chapter_count=12,
        current_chapter_number=5,
        target_word_count=2600,
        plot_stage="climax",
    )

    assert "【长线目标锚点】" in blocks["story_long_term_goal_block"]
    assert "【章节角色焦点锚点】" in blocks["story_character_focus_anchor_block"]
    assert "【章节伏笔兑现计划】" in blocks["story_foreshadow_payoff_plan_block"]
    assert "【章节节奏预算】" in blocks["story_pacing_budget_block"]
    assert "【长线目标锚点】" in blocks["quality_contract_block"]
    assert "【章节节奏预算】" in blocks["quality_contract_block"]
