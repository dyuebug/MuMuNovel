"""批量生成章节状态门面。"""
from __future__ import annotations

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
