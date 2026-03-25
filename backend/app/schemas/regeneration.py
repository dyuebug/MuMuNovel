"""章节重新生成相关的Schema定义"""
from datetime import datetime
from typing import Optional, List, Dict, Any

from pydantic import BaseModel, Field, field_validator

from app.schemas.generation_preferences import (
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
    preserve_dialogues: List[str] = Field(default_factory=list, description="需要保留的对话片段关键词")
    preserve_plot_points: List[str] = Field(default_factory=list, description="需要保留的情节点关键词")
    preserve_character_traits: bool = Field(True, description="保持角色性格一致")


class ChapterRegenerateRequest(BaseModel):
    """章节重新生成请求"""

    modification_source: str = Field("custom", description="修改来源: custom/analysis_suggestions/mixed")
    selected_suggestion_indices: Optional[List[int]] = Field(None, description="选中的建议索引列表")
    custom_instructions: Optional[str] = Field(None, description="用户自定义的修改要求")
    preserve_elements: Optional[PreserveElementsConfig] = Field(None, description="保留元素配置")

    style_id: Optional[int] = Field(None, description="写作风格ID")
    target_word_count: int = Field(3000, description="目标字数", ge=500, le=10000)
    focus_areas: List[str] = Field(default_factory=list, description="重点优化方向")
    creative_mode: Optional[CreativeModeValue] = Field(None, description="创作模式覆盖")
    story_focus: Optional[StoryFocusValue] = Field(None, description="结构侧重点覆盖")
    plot_stage: Optional[PlotStageValue] = Field(None, description="剧情阶段覆盖")
    story_creation_brief: Optional[str] = Field(None, description="本轮创作总控", max_length=1200)
    quality_preset: Optional[QualityPresetValue] = Field(None, description="质量预设覆盖")
    quality_notes: Optional[str] = Field(None, description="质量补充偏好", max_length=600)
    story_repair_summary: Optional[str] = Field(None, description="剧情质量修复摘要")
    story_repair_targets: List[str] = Field(default_factory=list, description="剧情质量修复目标")
    story_preserve_strengths: List[str] = Field(default_factory=list, description="需要保留的既有优势")

    save_as_version: bool = Field(True, description="是否保存为新版本")
    version_note: Optional[str] = Field(None, description="版本说明", max_length=500)
    auto_apply: bool = Field(False, description="是否自动应用（替换当前内容）")

    @field_validator(
        "creative_mode",
        "story_focus",
        "plot_stage",
        "quality_preset",
        mode="before",
    )
    @classmethod
    def normalize_generation_choices(cls, value):
        return normalize_optional_choice(value)

    @field_validator(
        "custom_instructions",
        "story_creation_brief",
        "quality_notes",
        "story_repair_summary",
        "version_note",
        mode="before",
    )
    @classmethod
    def normalize_generation_texts(cls, value):
        return normalize_optional_text(value)


class RegenerationTaskResponse(BaseModel):
    """重新生成任务响应"""
    task_id: str
    chapter_id: str
    status: str
    message: str
    estimated_time_seconds: int = 120


class RegenerationTaskStatus(BaseModel):
    """重新生成任务状态"""
    task_id: str
    chapter_id: str
    status: str
    progress: int
    error_message: Optional[str] = None
    created_at: Optional[datetime] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None

    original_word_count: Optional[int] = None
    regenerated_word_count: Optional[int] = None
    version_number: Optional[int] = None