"""Prompt template catalog owner for system template metadata and parameter shaping."""

from __future__ import annotations

from typing import Any, Callable, Dict, Optional

from tests.test_support.story_prompt_template_support_test_support import QUALITY_TEMPLATE_INSERTIONS


def augment_template_parameters(template_key: Optional[str], parameters: list) -> list:
    """Add quality-runtime parameters for templates that inject quality contracts."""

    augmented = list(parameters or [])
    if template_key not in QUALITY_TEMPLATE_INSERTIONS:
        return augmented

    for item in [
        "genre",
        "style_name",
        "style_preset_id",
        "style_content",
        "creative_mode",
        "creative_mode_block",
        "story_focus",
        "story_focus_block",
        "story_creation_brief",
        "story_creation_brief_block",
        "quality_metrics_summary",
        "story_quality_trend_block",
        "story_repair_summary",
        "story_repair_targets",
        "story_preserve_strengths",
        "story_repair_target_block",
        "story_repair_diagnostic_block",
        "external_assets",
        "reference_assets",
        "quality_generation_block",
        "quality_analysis_block",
        "quality_checker_block",
        "quality_reviser_block",
        "quality_regeneration_block",
        "quality_generation_protocol_block",
        "quality_json_protocol_block",
        "quality_mcp_guard_block",
        "mcp_guard",
        "quality_external_assets_block",
        "mcp_references",
    ]:
        if item not in augmented:
            augmented.append(item)
    return augmented


SYSTEM_TEMPLATE_DEFINITIONS: Dict[str, Dict[str, Any]] = {
    "WORLD_BUILDING": {
        "name": "世界构建",
        "category": "世界构建",
        "description": "用于生成小说世界观设定，包括时间背景、地理位置、氛围基调和世界规则",
        "parameters": ["title", "theme", "genre", "description"],
    },
    "CHARACTERS_BATCH_GENERATION": {
        "name": "批量角色生成",
        "category": "角色生成",
        "description": "批量生成多个角色和组织，建立角色关系网络",
        "parameters": ["count", "time_period", "location", "atmosphere", "rules", "theme", "genre", "requirements"],
    },
    "SINGLE_CHARACTER_GENERATION": {
        "name": "单个角色生成",
        "category": "角色生成",
        "description": "生成单个角色的详细设定",
        "parameters": ["project_context", "user_input"],
    },
    "SINGLE_ORGANIZATION_GENERATION": {
        "name": "组织生成",
        "category": "角色生成",
        "description": "生成组织/势力的详细设定",
        "parameters": ["project_context", "user_input"],
    },
    "OUTLINE_CREATE": {
        "name": "大纲生成",
        "category": "大纲生成",
        "description": "根据项目信息生成完整的章节大纲",
        "parameters": ["title", "theme", "genre", "chapter_count", "narrative_perspective", "target_words",
                     "time_period", "location", "atmosphere", "rules", "characters_info", "requirements", "mcp_references"],
    },
    "BOOK_IMPORT_REVERSE_PROJECT_SUGGESTION": {
        "name": "拆书导入-反向项目提炼",
        "category": "拆书导入",
        "description": "基于正文片段反向提炼项目简介、主题、类型、视角与目标字数",
        "parameters": ["title", "sampled_text"],
    },
    "BOOK_IMPORT_REVERSE_OUTLINES": {
        "name": "拆书导入-反向章节大纲",
        "category": "拆书导入",
        "description": "基于章节正文反向提炼章节大纲结构",
        "parameters": [
            "title", "genre", "theme", "narrative_perspective",
            "start_chapter", "end_chapter", "expected_count", "chapters_text",
        ],
    },
    "OUTLINE_CONTINUE": {
        "name": "大纲续写",
        "category": "大纲生成",
        "description": "基于已有章节续写大纲",
        "parameters": ["title", "theme", "genre", "narrative_perspective", "chapter_count", "time_period",
                     "location", "atmosphere", "rules", "characters_info", "current_chapter_count",
                     "all_chapters_brief", "recent_plot", "memory_context", "mcp_references",
                     "plot_stage_instruction", "start_chapter", "end_chapter", "story_direction", "requirements"],
    },
    "CHAPTER_GENERATION_ONE_TO_MANY": {
        "name": "章节创作-1-N模式（第1章）",
        "category": "章节创作",
        "description": "1-N模式：根据大纲创作章节内容（用于第1章，无前置章节）",
        "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                     "target_word_count", "narrative_perspective", "characters_info"],
    },
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": {
        "name": "章节创作-1-N模式（第2章及以后）",
        "category": "章节创作",
        "description": "1-N模式：基于前置章节内容创作新章节（用于第2章及以后）",
        "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                     "target_word_count", "narrative_perspective", "characters_info", "continuation_point",
                     "foreshadow_reminders", "relevant_memories", "story_skeleton", "previous_chapter_summary"],
    },
    "CHAPTER_GENERATION_ONE_TO_ONE": {
        "name": "章节创作-1-1模式（第1章）",
        "category": "章节创作",
        "description": "1-1模式：章节创作（用于第1章，无前置章节）",
        "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                     "target_word_count", "narrative_perspective", "characters_info", "chapter_careers"],
    },
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": {
        "name": "章节创作-1-1模式（第2章及以后）",
        "category": "章节创作",
        "description": "1-1模式：基于上一章内容创作新章节（用于第2章及以后）",
        "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                     "target_word_count", "narrative_perspective", "previous_chapter_content",
                     "characters_info", "chapter_careers", "foreshadow_reminders", "relevant_memories"],
    },
    "CHAPTER_REGENERATION_SYSTEM": {
        "name": "章节重写系统提示",
        "category": "章节重写",
        "description": "用于章节重写的系统提示词",
        "parameters": ["chapter_number", "title", "word_count", "content", "modification_instructions",
                     "project_context", "style_content", "target_word_count"],
    },
    "PARTIAL_REGENERATE": {
        "name": "局部重写",
        "category": "章节重写",
        "description": "根据用户修改要求重写选中的段落内容",
        "parameters": ["context_before", "original_word_count", "selected_text", "context_after",
                     "user_instructions", "length_requirement", "style_content"],
    },
    "PLOT_ANALYSIS": {
        "name": "情节分析",
        "category": "情节分析",
        "description": "深度分析章节的剧情、钩子、伏笔等",
        "parameters": ["chapter_number", "title", "content", "word_count"],
    },
    "CHAPTER_TEXT_CHECKER": {
        "name": "正文质量检查",
        "category": "情节分析",
        "description": "对章节正文进行结构化质量检查并输出可执行修订建议",
        "parameters": ["chapter_number", "chapter_title", "chapter_content", "chapter_outline", "characters_info", "world_rules"],
    },
    "CHAPTER_TEXT_REVISER": {
        "name": "正文自动修订",
        "category": "章节重写",
        "description": "根据质检结果自动生成修订草案（优先修复严重问题）",
        "parameters": ["chapter_number", "chapter_title", "chapter_content", "critical_issues_text", "checker_result_json"],
    },
    "OUTLINE_EXPAND_SINGLE": {
        "name": "大纲单批次展开",
        "category": "情节展开",
        "description": "将大纲节点展开为详细章节规划（单批次）",
        "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective",
                     "project_world_time_period", "project_world_location", "project_world_atmosphere",
                     "characters_info", "outline_order_index", "outline_title", "outline_content",
                     "context_info", "strategy_instruction", "target_chapter_count", "scene_instruction", "scene_field"],
    },
    "OUTLINE_EXPAND_MULTI": {
        "name": "大纲分批展开",
        "category": "情节展开",
        "description": "将大纲节点展开为详细章节规划（分批）",
        "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective",
                     "project_world_time_period", "project_world_location", "project_world_atmosphere",
                     "characters_info", "outline_order_index", "outline_title", "outline_content",
                     "context_info", "previous_context", "strategy_instruction", "start_index",
                     "end_index", "target_chapter_count", "scene_instruction", "scene_field"],
    },
    "MCP_TOOL_TEST": {
        "name": "MCP工具测试(用户提示词)",
        "category": "MCP测试",
        "description": "用于测试MCP插件功能的用户提示词",
        "parameters": ["plugin_name"],
    },
    "MCP_TOOL_TEST_SYSTEM": {
        "name": "MCP工具测试(系统提示词)",
        "category": "MCP测试",
        "description": "用于测试MCP插件功能的系统提示词",
        "parameters": [],
    },
    "MCP_WORLD_BUILDING_PLANNING": {
        "name": "MCP世界观规划",
        "category": "MCP增强",
        "description": "使用MCP工具搜索资料辅助世界观设计",
        "parameters": ["title", "genre", "theme", "description"],
    },
    "MCP_CHARACTER_PLANNING": {
        "name": "MCP角色规划",
        "category": "MCP增强",
        "description": "使用MCP工具搜索资料辅助角色设计",
        "parameters": ["title", "genre", "theme", "time_period", "location"],
    },
    "AUTO_CHARACTER_ANALYSIS": {
        "name": "自动角色分析",
        "category": "自动角色引入",
        "description": "分析新生成的大纲，判断是否需要引入新角色",
        "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                     "existing_characters", "new_outlines", "start_chapter", "end_chapter"],
    },
    "AUTO_CHARACTER_GENERATION": {
        "name": "自动角色生成",
        "category": "自动角色引入",
        "description": "根据剧情需求自动生成新角色的完整设定",
        "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                     "existing_characters", "plot_context", "character_specification", "mcp_references"],
    },
    "AUTO_ORGANIZATION_ANALYSIS": {
        "name": "自动组织分析",
        "category": "自动组织引入",
        "description": "分析新生成的大纲，判断是否需要引入新组织",
        "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                     "existing_organizations", "existing_characters", "all_chapters_brief", "start_chapter", "chapter_count", "plot_stage", "story_direction"],
    },
    "AUTO_ORGANIZATION_GENERATION": {
        "name": "自动组织生成",
        "category": "自动组织引入",
        "description": "根据剧情需求自动生成新组织的完整设定",
        "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                     "existing_organizations", "existing_characters", "plot_context", "organization_specification", "mcp_references"],
    },
    "CAREER_SYSTEM_GENERATION": {
        "name": "职业体系生成",
        "category": "世界构建",
        "description": "根据世界观和项目简介自动生成完整的职业体系，包括主职业和副职业",
        "parameters": ["title", "genre", "theme", "description", "time_period", "location", "atmosphere", "rules"],
    },
    "INSPIRATION_TITLE_SYSTEM": {
        "name": "灵感模式-书名生成(系统提示词)",
        "category": "灵感模式",
        "description": "根据用户的原始想法生成6个书名建议的系统提示词",
        "parameters": ["initial_idea"],
    },
    "INSPIRATION_TITLE_USER": {
        "name": "灵感模式-书名生成(用户提示词)",
        "category": "灵感模式",
        "description": "根据用户的原始想法生成6个书名建议的用户提示词",
        "parameters": ["initial_idea"],
    },
    "INSPIRATION_DESCRIPTION_SYSTEM": {
        "name": "灵感模式-简介生成(系统提示词)",
        "category": "灵感模式",
        "description": "根据用户想法和书名生成6个简介选项的系统提示词",
        "parameters": ["initial_idea", "title"],
    },
    "INSPIRATION_DESCRIPTION_USER": {
        "name": "灵感模式-简介生成(用户提示词)",
        "category": "灵感模式",
        "description": "根据用户想法和书名生成6个简介选项的用户提示词",
        "parameters": ["initial_idea", "title"],
    },
    "INSPIRATION_THEME_SYSTEM": {
        "name": "灵感模式-主题生成(系统提示词)",
        "category": "灵感模式",
        "description": "根据书名和简介生成6个深刻的主题选项的系统提示词",
        "parameters": ["initial_idea", "title", "description"],
    },
    "INSPIRATION_THEME_USER": {
        "name": "灵感模式-主题生成(用户提示词)",
        "category": "灵感模式",
        "description": "根据书名和简介生成6个深刻的主题选项的用户提示词",
        "parameters": ["initial_idea", "title", "description"],
    },
    "INSPIRATION_GENRE_SYSTEM": {
        "name": "灵感模式-类型生成(系统提示词)",
        "category": "灵感模式",
        "description": "根据小说信息生成6个合适的类型标签的系统提示词",
        "parameters": ["initial_idea", "title", "description", "theme"],
    },
    "INSPIRATION_GENRE_USER": {
        "name": "灵感模式-类型生成(用户提示词)",
        "category": "灵感模式",
        "description": "根据小说信息生成6个合适的类型标签的用户提示词",
        "parameters": ["initial_idea", "title", "description", "theme"],
    },
    "INSPIRATION_QUICK_COMPLETE": {
        "name": "灵感模式-智能补全",
        "category": "灵感模式",
        "description": "根据用户提供的部分信息智能补全完整的小说方案",
        "parameters": ["existing"],
    },
    "AI_DENOISING": {
        "name": "AI去味",
        "category": "文本润色",
        "description": "将文本改写为更自然的中文表达，降低模板腔和AI腔",
        "parameters": ["original_text", "focus_instruction", "structure_instruction", "style_hint_block"],
    },
}


def build_system_template_catalog(
    *,
    template_lookup: Callable[[str], Optional[str]],
    template_prepare: Callable[[Optional[str], Optional[str]], Optional[str]],
) -> list[dict[str, Any]]:
    """Build all system template metadata with prepared template content."""

    templates: list[dict[str, Any]] = []
    for key, info in SYSTEM_TEMPLATE_DEFINITIONS.items():
        template_content = template_lookup(key)
        if template_content:
            templates.append({
                "template_key": key,
                "template_name": info["name"],
                "category": info["category"],
                "description": info["description"],
                "parameters": augment_template_parameters(key, info["parameters"]),
                "content": template_prepare(key, template_content),
            })
    return templates


def get_system_template_info(template_key: str, templates: list[dict[str, Any]]) -> Optional[dict[str, Any]]:
    """Return one system template metadata record from the prepared catalog."""

    for template in templates:
        if template["template_key"] == template_key:
            return template
    return None
