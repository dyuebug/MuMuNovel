"""数据模型导出。"""

from __future__ import annotations

from importlib import import_module
from typing import Any, Dict, Tuple

from sqlalchemy.orm import declarative_base

Base = declarative_base()

_MODEL_EXPORTS: Dict[str, Tuple[str, str]] = {
    "Project": ("migrator_app.models.project", "Project"),
    "Outline": ("migrator_app.models.outline", "Outline"),
    "Chapter": ("migrator_app.models.chapter", "Chapter"),
    "Character": ("migrator_app.models.character", "Character"),
    "CharacterRelationship": ("migrator_app.models.relationship", "CharacterRelationship"),
    "Organization": ("migrator_app.models.organization", "Organization"),
    "OrganizationMember": (
        "migrator_app.models.organization",
        "OrganizationMember",
    ),
    "RelationshipType": ("migrator_app.models.relationship", "RelationshipType"),
    "GenerationHistory": ("migrator_app.models.generation_history", "GenerationHistory"),
    "ChapterDraftAttempt": ("migrator_app.models.chapter_draft_attempt", "ChapterDraftAttempt"),
    "BatchGenerationSnapshot": ("migrator_app.models.batch_generation_snapshot", "BatchGenerationSnapshot"),
    "AnalysisTask": ("migrator_app.models.analysis_task", "AnalysisTask"),
    "BatchGenerationTask": ("migrator_app.models.batch_generation_task", "BatchGenerationTask"),
    "Settings": ("migrator_app.models.settings", "Settings"),
    "StoryMemory": ("migrator_app.models.memory_analysis", "StoryMemory"),
    "PlotAnalysis": ("migrator_app.models.memory_analysis", "PlotAnalysis"),
    "WritingStyle": ("migrator_app.models.writing_style", "WritingStyle"),
    "ProjectDefaultStyle": ("migrator_app.models.project_default_style", "ProjectDefaultStyle"),
    "MCPPlugin": ("migrator_app.models.mcp_plugin", "MCPPlugin"),
    "User": ("migrator_app.models.user", "User"),
    "UserPassword": ("migrator_app.models.user", "UserPassword"),
    "RegenerationTask": ("migrator_app.models.regeneration_task", "RegenerationTask"),
    "Career": ("migrator_app.models.career", "Career"),
    "CharacterCareer": ("migrator_app.models.career", "CharacterCareer"),
    "PromptTemplate": ("migrator_app.models.prompt_template", "PromptTemplate"),
}

__all__ = ["Base", *list(_MODEL_EXPORTS.keys())]


def __getattr__(name: str) -> Any:
    export = _MODEL_EXPORTS.get(name)
    if export is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")

    module_name, attribute_name = export
    module = import_module(module_name)
    value = getattr(module, attribute_name)
    globals()[name] = value
    return value


def load_all_models() -> Tuple[str, ...]:
    """Import every SQLAlchemy model so Alembic can populate Base.metadata."""
    model_names = tuple(name for name in __all__ if name != "Base")

    for name in model_names:
        if name not in globals():
            getattr(import_module(__name__), name)

    return model_names

