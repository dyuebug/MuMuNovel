"""?????Pydantic??"""
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
    """??????"""
    title: str = Field(..., description="????")
    description: Optional[str] = Field(None, description="????")
    theme: Optional[str] = Field(None, description="??")
    genre: Optional[str] = Field(None, description="????")
    target_words: Optional[int] = Field(None, description="????")
    default_creative_mode: Optional[str] = Field(None, description="??????")
    default_story_focus: Optional[str] = Field(None, description="???????")
    default_plot_stage: Optional[str] = Field(None, description="??????")
    default_story_creation_brief: Optional[str] = Field(None, description="????????")
    default_quality_preset: Optional[str] = Field(None, description="??????")
    default_quality_notes: Optional[str] = Field(None, description="????????")
    outline_mode: Literal["one-to-one", "one-to-many"] = Field(
        default="one-to-many",
        description="??????: one-to-one(????,1???1??) ? one-to-many(????,1???N??)",
    )


class ProjectCreate(ProjectBase):
    """?????????"""

    model_config = ConfigDict(extra="forbid")

    default_creative_mode: Optional[CreativeModeValue] = Field(None, description="??????")
    default_story_focus: Optional[StoryFocusValue] = Field(None, description="???????")
    default_plot_stage: Optional[PlotStageValue] = Field(None, description="??????")
    default_story_creation_brief: Optional[str] = Field(None, description="????????", max_length=1200)
    default_quality_preset: Optional[QualityPresetValue] = Field(None, description="??????")
    default_quality_notes: Optional[str] = Field(None, description="????????", max_length=600)

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
    """?????????"""
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
    default_creative_mode: Optional[CreativeModeValue] = Field(None, description="??????")
    default_story_focus: Optional[StoryFocusValue] = Field(None, description="???????")
    default_plot_stage: Optional[PlotStageValue] = Field(None, description="??????")
    default_story_creation_brief: Optional[str] = Field(None, description="????????", max_length=1200)
    default_quality_preset: Optional[QualityPresetValue] = Field(None, description="??????")
    default_quality_notes: Optional[str] = Field(None, description="????????", max_length=600)

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
    """??????"""
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
    """????????"""
    total: int
    items: list[ProjectResponse]


class ProjectWizardRequest(BaseModel):
    """??????????"""
    title: str = Field(..., description="??")
    theme: str = Field(..., description="??")
    genre: Optional[str] = Field(None, description="??")
    chapter_count: int = Field(..., ge=1, description="????")
    narrative_perspective: str = Field(..., description="????")
    character_count: int = Field(5, ge=5, description="???????5??")
    target_words: Optional[int] = Field(None, description="????")
    outline_mode: Literal["one-to-one", "one-to-many"] = Field(
        default="one-to-many",
        description="??????",
    )


class WorldBuildingResponse(BaseModel):
    """????????"""
    time_period: str = Field(..., description="????")
    location: str = Field(..., description="????")
    atmosphere: str = Field(..., description="????")
    rules: str = Field(..., description="????")
