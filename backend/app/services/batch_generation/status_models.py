from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch task-view and status payload chain; this "
    "Python module is kept only as frozen rollback/source-map material for "
    "legacy batch route and aggregate fallback flows."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"


@dataclass(frozen=True)
class BatchGenerationTaskViewContext:
    task: Any
    quality_snapshot: Dict[str, Any]
    workflow_snapshot: Dict[str, Any]
