"""Compatibility shim for chapter regeneration routes.

Deprecated: import from app.services.compat.chapter_regeneration_route_compat_service instead.
"""

from app.services.compat import chapter_regeneration_route_compat_service as _impl

# Explicit re-exports to avoid import * omitting needed symbols.
get_db = _impl.get_db
REGENERATOR_FACTORY = _impl.REGENERATOR_FACTORY
regenerate_chapter_stream_with_default_route_wiring = _impl.regenerate_chapter_stream_with_default_route_wiring

# Make REGENERATOR_FACTORY patchable from this shim (tests monkeypatch this symbol).
_impl.REGENERATOR_FACTORY = REGENERATOR_FACTORY

__all__ = [
    "get_db",
    "REGENERATOR_FACTORY",
    "regenerate_chapter_stream_with_default_route_wiring",
]
