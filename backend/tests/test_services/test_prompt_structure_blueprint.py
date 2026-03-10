from app.api.outlines import _merge_outline_requirements
from app.services.prompt_service import (
    build_story_acceptance_card_block,
    build_story_character_arc_card_block,
    build_story_creation_brief_block,
    build_story_execution_checklist_block,
    PromptService,
    build_story_objective_card_block,
    build_story_repair_target_block,
    build_story_repetition_risk_block,
    build_story_result_card_block,
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
