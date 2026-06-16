"""Lazy rollback/source-map facade for legacy chapter generation route defaults.

Active single-generation traffic is owned by the Rust route/workflow chain.
This module intentionally keeps the old Python names importable for rollback
and tests without importing the route-wiring runtime graph during app startup.
"""

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active chapter generation route and workflow chain; this "
    "compat facade is kept only as frozen rollback/source-map material after "
    "its business callers were reduced to manifest, tests, and explicit "
    "rollback import paths."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_generation_routes.rs"
SOURCE_MAP_ROLLBACK_FLAG = "aggregate_chapter_generation_route_compat_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


def _route_wiring_attr(name: str):
    from app.services.chapter_generation import route_wiring_service

    return getattr(route_wiring_service, name)


class _LazyRouteWiringFactory:
    _target_name: str

    def __new__(cls, *args, **kwargs):
        return _route_wiring_attr(cls._target_name)(*args, **kwargs)


class OneToOneContextBuilder(_LazyRouteWiringFactory):
    _target_name = "OneToOneContextBuilder"


class OneToManyContextBuilder(_LazyRouteWiringFactory):
    _target_name = "OneToManyContextBuilder"


async def get_template(*args, **kwargs):
    return await _route_wiring_attr("get_template")(*args, **kwargs)


def format_prompt(*args, **kwargs):
    return _route_wiring_attr("format_prompt")(*args, **kwargs)


def apply_style_to_prompt(*args, **kwargs):
    return _route_wiring_attr("apply_style_to_prompt")(*args, **kwargs)


def build_chapter_runtime_system_prompt(*args, **kwargs):
    return _route_wiring_attr("build_chapter_runtime_system_prompt")(*args, **kwargs)


def compute_story_quality_metrics(*args, **kwargs):
    return _route_wiring_attr("compute_story_quality_metrics")(*args, **kwargs)


def detect_style_profile(*args, **kwargs):
    return _route_wiring_attr("detect_style_profile")(*args, **kwargs)


async def execute_chapter_analysis_background(*args, **kwargs):
    return await _route_wiring_attr("execute_chapter_analysis_background")(*args, **kwargs)


def get_db(*args, **kwargs):
    return _route_wiring_attr("get_db")(*args, **kwargs)


def resolve_generation_temperature(*args, **kwargs):
    return _route_wiring_attr("resolve_generation_temperature")(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    return _route_wiring_attr("resolve_quality_gate_execution_plan")(*args, **kwargs)


async def generate_chapter_content_stream_with_default_route_wiring(*args, **kwargs):
    return await _route_wiring_attr(
        "generate_chapter_content_stream_with_default_route_wiring"
    )(*args, **kwargs)


async def generate_chapter_content_background_with_default_route_wiring(*args, **kwargs):
    return await _route_wiring_attr(
        "generate_chapter_content_background_with_default_route_wiring"
    )(*args, **kwargs)


__all__ = [
    "CHAPTER_CANDIDATE_RERANK_LIMIT",
    "CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS",
    "OneToManyContextBuilder",
    "OneToOneContextBuilder",
    "apply_style_to_prompt",
    "build_chapter_runtime_system_prompt",
    "compute_story_quality_metrics",
    "detect_style_profile",
    "execute_chapter_analysis_background",
    "format_prompt",
    "generate_chapter_content_background_with_default_route_wiring",
    "generate_chapter_content_stream_with_default_route_wiring",
    "get_db",
    "get_template",
    "resolve_generation_temperature",
    "resolve_quality_gate_execution_plan",
]
