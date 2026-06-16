from __future__ import annotations

from typing import Any, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active single-generation and batch-generation runtime "
    "transport policy chain; this Python module is kept only as frozen "
    "rollback/source-map material after its remaining callers were reduced to "
    "repointed or frozen source-map shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_generation_runtime_service.rs; "
    "backend-rs/src/api/health.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_single_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

RESPONSES_TEXT_GENERATION_PROVIDERS = {"sub2api", "openai_responses"}
CHAPTER_GENERATION_TRANSPORT_RETRY_CAP = 2
CHAPTER_GENERATION_FIRST_CHUNK_TIMEOUT = 20.0


def _calculate_chapter_generation_max_tokens(target_word_count: int) -> int:
    safe_target = max(200, int(target_word_count or 0))
    calculated_max_tokens = int(safe_target * 0.6)
    return max(700, min(calculated_max_tokens, 8000))


def _build_chapter_generation_request_options(ai_service: Any) -> Optional[Dict[str, Any]]:
    normalized_provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    if normalized_provider not in RESPONSES_TEXT_GENERATION_PROVIDERS:
        return None

    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(
        getattr(retry_cfg, "max_retries", CHAPTER_GENERATION_TRANSPORT_RETRY_CAP)
        or CHAPTER_GENERATION_TRANSPORT_RETRY_CAP
    )
    transport_max_retries = max(1, min(configured_retry_budget, CHAPTER_GENERATION_TRANSPORT_RETRY_CAP))
    return {
        "prefer_chat_completions": True,
        "transport_max_retries": transport_max_retries,
        "first_chunk_timeout": CHAPTER_GENERATION_FIRST_CHUNK_TIMEOUT,
        "allow_non_stream_fallback": False,
    }
