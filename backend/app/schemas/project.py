"""项目相关的 Pydantic 模型"""
from datetime import datetime
from typing import Literal, Optional

from pydantic import BaseModel, ConfigDict, Field, field_validator

from app.schemas.generation_preferences import (
    CreativeModeValue,
    PlotStageValue,
    QualityPresetValue,
    StoryFocusValue,
    normalize_optional_choice,
    normalize_optional_text,
)


class ProjectBase(BaseModel):
    """项目基础模型"""

    title: str = Field(..., description="项目标题")
    description: Optional[str] = Field(None, description="项目描述")
    theme: Optional[str] = Field(None, description="项目主题")
    genre: Optional[str] = Field(None, description="小说类型")
    target_words: Optional[int] = Field(None, description="目标字数")
    default_creative_mode: Optional[str] = Field(None, description="默认创作模式")
    default_story_focus: Optional[str] = Field(None, description="默认结构侧重点")
    default_plot_stage: Optional[str] = Field(None, description="默认剧情阶段")
    default_story_creation_brief: Optional[str] = Field(None, description="默认创作总控摘要")
    default_quality_preset: Optional[str] = Field(None, description="默认质量预设")
    default_quality_notes: Optional[str] = Field(None, description="默认质量补充偏好")
    outline_mode: Literal["one-to-one", "one-to-many"] = Field(
        default="one-to-many",
        description="大纲章节模式: one-to-one(一章一纲) 或 one-to-many(一纲多章)",
    )


class ProjectCreate(ProjectBase):
    """创建项目的请求模型"""

    model_config = ConfigDict(extra="forbid")

    default_creative_mode: Optional[CreativeModeValue] = Field(None, description="默认创作模式")
    default_story_focus: Optional[StoryFocusValue] = Field(None, description="默认结构侧重点")
    default_plot_stage: Optional[PlotStageValue] = Field(None, description="默认剧情阶段")
    default_story_creation_brief: Optional[str] = Field(None, description="默认创作总控摘要", max_length=1200)
    default_quality_preset: Optional[QualityPresetValue] = Field(None, description="默认质量预设")
    default_quality_notes: Optional[str] = Field(None, description="默认质量补充偏好", max_length=600)

    @field_validator(
        "default_creative_mode",
        "default_story_focus",
        "default_plot_stage",
        "default_quality_preset",
        mode="before",
    )
    @classmethod
    def normalize_generation_choices(cls, value):
        return normalize_optional_choice(value)

    @field_validator("default_story_creation_brief", "default_quality_notes", mode="before")
    @classmethod
    def normalize_generation_texts(cls, value):
        return normalize_optional_text(value)


class ProjectUpdate(BaseModel):
    """更新项目的请求模型"""

    model_config = ConfigDict(extra="forbid")

    title: Optional[str] = None
    description: Optional[str] = None
    theme: Optional[str] = None
    genre: Optional[str] = None
    target_words: Optional[int] = None
    status: Optional[str] = None
    world_time_period: Optional[str] = None
    world_location: Optional[str] = None
    world_atmosphere: Optional[str] = None
    world_rules: Optional[str] = None
    chapter_count: Optional[int] = None
    narrative_perspective: Optional[str] = None
    character_count: Optional[int] = None
    default_creative_mode: Optional[CreativeModeValue] = Field(None, description="默认创作模式")
    default_story_focus: Optional[StoryFocusValue] = Field(None, description="默认结构侧重点")
    default_plot_stage: Optional[PlotStageValue] = Field(None, description="默认剧情阶段")
    default_story_creation_brief: Optional[str] = Field(None, description="默认创作总控摘要", max_length=1200)
    default_quality_preset: Optional[QualityPresetValue] = Field(None, description="默认质量预设")
    default_quality_notes: Optional[str] = Field(None, description="默认质量补充偏好", max_length=600)

    @field_validator(
        "default_creative_mode",
        "default_story_focus",
        "default_plot_stage",
        "default_quality_preset",
        mode="before",
    )
    @classmethod
    def normalize_generation_choices(cls, value):
        return normalize_optional_choice(value)

    @field_validator("default_story_creation_brief", "default_quality_notes", mode="before")
    @classmethod
    def normalize_generation_texts(cls, value):
        return normalize_optional_text(value)


class ProjectResponse(ProjectBase):
    """项目响应模型"""

    id: str
    status: str
    current_words: int
    wizard_status: Optional[str] = None
    wizard_step: Optional[int] = None
    world_time_period: Optional[str] = None
    world_location: Optional[str] = None
    world_atmosphere: Optional[str] = None
    world_rules: Optional[str] = None
    chapter_count: Optional[int] = None
    narrative_perspective: Optional[str] = None
    character_count: Optional[int] = None
    outline_mode: str
    created_at: datetime
    updated_at: datetime

    model_config = ConfigDict(from_attributes=True)


class ProjectListResponse(BaseModel):
    """项目列表响应模型"""

    total: int
    items: list[ProjectResponse]


class ProjectWizardRequest(BaseModel):
    """项目创建向导请求模型"""

    title: str = Field(..., description="书名")
    theme: str = Field(..., description="主题")
    genre: Optional[str] = Field(None, description="类型")
    chapter_count: int = Field(..., ge=1, description="章节数量")
    narrative_perspective: str = Field(..., description="叙事视角")
    character_count: int = Field(5, ge=5, description="角色数量（至少 5 个）")
    target_words: Optional[int] = Field(None, description="目标字数")
    outline_mode: Literal["one-to-one", "one-to-many"] = Field(
        default="one-to-many",
        description="大纲章节模式",
    )


class WorldBuildingResponse(BaseModel):
    """世界构建响应模型"""

    time_period: str = Field(..., description="时间背景")
    location: str = Field(..., description="地理位置")
    atmosphere: str = Field(..., description="氛围基调")
    rules: str = Field(..., description="世界规则")
