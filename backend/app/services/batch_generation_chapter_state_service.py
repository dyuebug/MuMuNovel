"""批量生成章节状态门面。"""
from __future__ import annotations

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch chapter state-transition chain; this Python "
    "facade is kept only as frozen rollback/source-map material for legacy "
    "batch fallback wiring."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.services.batch_generation_chapter_failure_state_service import (
    fail_batch_generation_after_analysis,
    fail_batch_generation_after_max_retries,
    fail_batch_generation_for_manual_review,
)
from app.services.batch_generation_chapter_success_state_service import (
    BatchGenerationAppliedChapterState,
    BatchGenerationQualityGateRetryPreparation,
    apply_successful_batch_generation_chapter,
    finalize_successful_batch_generation_chapter,
    handle_batch_generation_quality_gate_retry,
)
