"""??????????????????"""

from typing import Any, Literal, Optional

CreativeModeValue = Literal["balanced", "hook", "emotion", "suspense", "relationship", "payoff"]
StoryFocusValue = Literal[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
]
PlotStageValue = Literal["development", "climax", "ending"]
QualityPresetValue = Literal["balanced", "plot_drive", "immersive", "emotion_drama", "clean_prose"]
OutlineGenerateModeValue = Literal["auto", "new", "continue"]


def normalize_optional_choice(value: Any) -> Any:
    if value is None:
        return None
    if isinstance(value, str):
        cleaned = value.strip()
        return cleaned or None
    return value


def normalize_optional_text(value: Any) -> Any:
    if value is None:
        return None
    if isinstance(value, str):
        cleaned = value.strip()
        return cleaned or None
    return value
