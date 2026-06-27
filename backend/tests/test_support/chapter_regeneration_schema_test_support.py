"""Test-only regeneration request schemas kept outside production chapter.py."""

from typing import List, Optional

from pydantic import BaseModel, ConfigDict, Field, field_validator

from tests.test_support.schemas.generation_preferences import (
    CreativeModeValue,
    PlotStageValue,
    QualityPresetValue,
    StoryFocusValue,
    normalize_optional_choice,
    normalize_optional_text,
)


class PreserveElementsConfig(BaseModel):
    """保留元素配置"""

    preserve_structure: bool = Field(False, description="是否保留整体结构")
    preserve_dialogues: List[str] = Field(
        default_factory=list,
        description="需要保留的对话片段关键词",
    )
    preserve_plot_points: List[str] = Field(
        default_factory=list,
        description="需要保留的情节点关键词",
    )
    preserve_character_traits: bool = Field(True, description="保持角色性格一致")


class ChapterRegenerateRequest(BaseModel):
    """章节重新生成请求"""

    modification_source: str = Field(
        "custom",
        description="修改来源: custom/analysis_suggestions/mixed",
    )
    selected_suggestion_indices: Optional[List[int]] = Field(
        None,
        description="选中的建议索引列表",
    )
    custom_instructions: Optional[str] = Field(None, description="用户自定义修改要求")
    preserve_elements: Optional[PreserveElementsConfig] = Field(
        None,
        description="保留元素配置",
    )

    style_id: Optional[int] = Field(None, description="写作风格 ID")
    target_word_count: int = Field(3000, description="目标字数", ge=500, le=10000)
    focus_areas: List[str] = Field(default_factory=list, description="重点优化方向")
    creative_mode: Optional[CreativeModeValue] = Field(None, description="创作模式覆盖")
    story_focus: Optional[StoryFocusValue] = Field(None, description="故事重心覆盖")
    plot_stage: Optional[PlotStageValue] = Field(None, description="剧情阶段覆盖")
    story_creation_brief: Optional[str] = Field(
        None,
        description="本轮创作总控",
        max_length=1200,
    )
    quality_preset: Optional[QualityPresetValue] = Field(
        None,
        description="质量预设覆盖",
    )
    quality_notes: Optional[str] = Field(None, description="质量补充偏好", max_length=600)
    enable_web_research: Optional[bool] = Field(None, description="是否启用联网搜索辅助")
    web_research_query: Optional[str] = Field(
        None,
        description="联网搜索自定义查询",
        max_length=500,
    )
    story_repair_summary: Optional[str] = Field(None, description="剧情质量修复摘要")
    story_repair_targets: List[str] = Field(
        default_factory=list,
        description="剧情质量修复目标",
    )
    story_preserve_strengths: List[str] = Field(
        default_factory=list,
        description="需要保留的既有优势",
    )

    save_as_version: bool = Field(True, description="是否保存为新版本")
    version_note: Optional[str] = Field(None, description="版本说明", max_length=500)
    auto_apply: bool = Field(False, description="是否自动应用")

    @field_validator(
        "creative_mode",
        "story_focus",
        "plot_stage",
        "quality_preset",
        mode="before",
    )
    @classmethod
    def normalize_regeneration_choices(cls, value):
        return normalize_optional_choice(value)

    @field_validator(
        "custom_instructions",
        "story_creation_brief",
        "quality_notes",
        "web_research_query",
        "story_repair_summary",
        "version_note",
        mode="before",
    )
    @classmethod
    def normalize_regeneration_texts(cls, value):
        return normalize_optional_text(value)


class PartialRegenerateRequest(BaseModel):
    """局部重写请求"""

    selected_text: str = Field(..., description="用户选中的原文片段")
    start_position: int = Field(..., description="选中片段在章节全文中的起始位置", ge=0)
    end_position: int = Field(..., description="选中片段在章节全文中的结束位置", ge=0)
    user_instructions: str = Field(..., description="用户的补充指令", min_length=1, max_length=1000)

    context_chars: int = Field(
        500,
        description="用于重写时拼接的前后文字符数",
        ge=100,
        le=2000,
    )
    style_id: Optional[int] = Field(None, description="风格模板 ID，不传时使用当前默认风格")
    length_mode: Optional[str] = Field(
        "similar",
        description="重写长度模式：similar/expand/condense/custom",
    )
    target_word_count: Optional[int] = Field(
        None,
        description="目标字数，仅在 length_mode 为 custom 时生效",
        ge=10,
        le=5000,
    )
    enable_web_research: Optional[bool] = Field(None, description="是否启用联网搜索")
    web_research_query: Optional[str] = Field(None, description="联网搜索查询词", max_length=500)

    @field_validator("user_instructions", "web_research_query", mode="before")
    @classmethod
    def normalize_partial_texts(cls, value):
        return normalize_optional_text(value)

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "selected_text": "这一段冲突张力不够，需要重写",
                "start_position": 1234,
                "end_position": 1260,
                "user_instructions": "请强化冲突压迫感，补足人物心理",
                "context_chars": 500,
                "length_mode": "expand",
                "enable_web_research": True,
                "web_research_query": "late qing harbor guild rules",
            }
        }
    )

