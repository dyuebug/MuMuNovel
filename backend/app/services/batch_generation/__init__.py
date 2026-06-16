"""Frozen batch-generation domain source-map package.

This package remains importable only as explicit Python rollback/source-map
material. The active runtime owners live under `backend-rs`.
"""

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route/read/write/runtime chain; "
    "this Python package remains only as a frozen rollback/source-map package "
    "after its submodules were repointed or frozen."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = (
    "legacy_batch_generation_python_routes_enabled; "
    "legacy_single_generation_python_routes_enabled"
)
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"
