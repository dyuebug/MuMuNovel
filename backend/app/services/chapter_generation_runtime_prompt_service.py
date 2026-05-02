"""Compatibility shim for chapter generation runtime prompt helpers.

Deprecated: import from app.services.chapter_generation.runtime.prompt_service instead.
"""

from app.services.chapter_generation.runtime import prompt_service as _impl

build_chapter_runtime_system_prompt = _impl.build_chapter_runtime_system_prompt
resolve_generation_temperature = _impl.resolve_generation_temperature
detect_style_profile = _impl.detect_style_profile

__all__ = [
    "build_chapter_runtime_system_prompt",
    "resolve_generation_temperature",
    "detect_style_profile",
]
