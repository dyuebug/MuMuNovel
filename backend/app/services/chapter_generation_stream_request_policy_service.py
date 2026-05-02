"""Compatibility shim.

Deprecated: import from app.services.chapter_generation.stream.request_policy_service instead.
"""

from app.services.chapter_generation.stream import request_policy_service as _impl

_build_chapter_generation_request_options = _impl._build_chapter_generation_request_options
_calculate_chapter_generation_max_tokens = _impl._calculate_chapter_generation_max_tokens

__all__ = [
    "_build_chapter_generation_request_options",
    "_calculate_chapter_generation_max_tokens",
]
