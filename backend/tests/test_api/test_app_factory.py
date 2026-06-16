import pytest
import sys
import importlib
from fastapi.testclient import TestClient

from app.bootstrap.app_factory import create_app
from app.main import app as main_app
from app.config import settings as config_settings
from app.middleware.auth_middleware import user_manager


pytestmark = pytest.mark.asyncio


LEGACY_SINGLE_GENERATION_MODULES = (
    "app.api.chapter_generation_routes",
    "app.services.chapter_generation.route_wiring_service",
    "app.services.compat.chapter_generation_route_compat_service",
)

LEGACY_SINGLE_GENERATION_ROUTE_REGISTRATION_MODULES = (
    "app.services.chapter_generation.route_wiring_service",
    "app.services.compat.chapter_generation_route_compat_service",
)

LEGACY_SINGLE_GENERATION_ROUTE_MODULE_IMPORT_GUARDS = (
    "app.api.settings",
    "app.database",
    "app.models",
    "app.services.ai_service",
    "sqlalchemy",
)

LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS = (
    "app.api.chapters",
    "app.api.outlines",
    "app.api.chapter_route_helpers",
    "app.database",
    "app.models",
    "app.services.ai_service",
    "app.services.chapter_context_service",
    "app.services.chapter_generation.runtime.prompt_service",
    "app.services.chapter_generation.stream.entry_service",
    "app.services.chapter_generation.background_entry_service",
    "app.services.prompt_service",
    "app.services.story_repair_payload_service",
    "app.services.story_quality_feedback_service",
    "app.services.task_workflow_runtime_service",
    "app.services.manual_chapter_analysis_execution_service",
    "sqlalchemy",
)

LEGACY_SINGLE_GENERATION_COMPAT_IMPORT_GUARDS = (
    "app.services.chapter_generation.route_wiring_service",
    *LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS,
)

LEGACY_SINGLE_GENERATION_STREAM_ENTRY_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.chapter_generation.runtime.prompt_service",
    "app.services.chapter_generation.stream.service",
    "app.services.chapter_generation.stream.wiring_service",
    "app.services.story_repair_payload_service",
    "app.services.manual_chapter_analysis_execution_service",
    "app.utils.sse_response",
)

LEGACY_SINGLE_GENERATION_STREAM_WIRING_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.schemas.generation_payload",
    "app.services.analysis_task_service",
    "app.services.chapter_context_service",
    "app.services.chapter_generation.history_service",
    "app.services.chapter_generation.runtime.prompt_service",
    "app.services.chapter_generation.runtime.service",
    "app.services.chapter_generation.stream.models",
    "app.services.foreshadow_service",
    "app.services.manual_chapter_analysis_execution_service",
    "app.services.memory_service",
    "app.services.outline_runtime_source_service",
    "app.services.prompt_service",
    "app.services.story_quality_feedback_service",
    "app.services.story_repair_payload_service",
    "app.services.story_runtime_serialization_service",
)

LEGACY_SINGLE_GENERATION_STREAM_FINALIZE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.chapter_generation.stream.models",
)

LEGACY_SINGLE_GENERATION_STREAM_CANDIDATE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.chapter_candidate_event_service",
    "app.services.chapter_generation.stream.models",
)

LEGACY_SINGLE_GENERATION_STREAM_SERVICE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.chapter_quality_context_service",
    "app.services.chapter_generation.stream.candidate_service",
    "app.services.chapter_generation.stream.execution_service",
    "app.services.chapter_generation.stream.finalize_service",
    "app.services.chapter_generation.stream.models",
    "app.utils.sse_response",
)

LEGACY_SINGLE_GENERATION_STREAM_EXECUTION_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "sqlalchemy",
    "app.services.chapter_quality_context_service",
    "app.services.chapter_generation.stream.models",
)

LEGACY_SINGLE_GENERATION_STREAM_MODELS_IMPORT_GUARDS = (
    "app.database",
    "app.models",
)

LEGACY_BATCH_GENERATION_MODULES = (
    "app.api.chapter_batch_generation_routes",
    "app.services.batch_generation.route_wiring_service",
    "app.services.batch_generation.create_service",
    "app.services.batch_generation.query_service",
    "app.services.batch_generation.resume_service",
    "app.services.batch_generation.status_response_builder",
    "app.services.batch_generation.task_workflow_snapshot_service",
    "app.services.batch_generation_orchestration_service",
    "app.services.batch_generation_run_wiring_service",
    "app.services.batch_generation_single_chapter_wiring_service",
    "app.services.batch_generation_stream_service",
    "app.services.batch_generation_execution_service",
    "app.services.batch_generation_analysis_service",
    "app.services.batch_generation_run_service",
)

LEGACY_BATCH_GENERATION_ROUTE_MODULE_IMPORT_GUARDS = (
    "app.api.common",
    "app.api.settings",
    "app.api.chapters",
    "app.database",
    "app.models",
    "app.services.ai_service",
    "app.services.batch_generation_orchestration_service",
    "app.services.batch_generation.query_service",
    "app.services.batch_generation_stream_service",
    "app.services.batch_generation.status_response_builder",
    "app.services.chapter_generation.prerequisite_service",
    "app.services.chapter_quality_context_service",
    "app.services.story_repair_payload_service",
    "app.services.task_workflow_runtime_service",
    "app.utils.sse_response",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS = (
    "app.api.common",
    "app.services.batch_generation_orchestration_service",
    "app.services.batch_generation.query_service",
    "app.services.batch_generation_stream_service",
    "app.services.batch_generation.status_response_builder",
    "app.services.chapter_generation.prerequisite_service",
    "app.services.chapter_quality_context_service",
    "app.services.story_repair_payload_service",
    "app.services.task_workflow_runtime_service",
    "app.utils.sse_response",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_STATUS_RESPONSE_IMPORT_GUARDS = (
    "app.models",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_TASK_WORKFLOW_SNAPSHOT_IMPORT_GUARDS = (
    "app.models",
    "app.services.task_workflow_runtime_service",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_QUERY_SERVICE_IMPORT_GUARDS = (
    "app.models",
    "app.services.batch_generation.task_workflow_snapshot_service",
    "app.services.task_quality_snapshot_service",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_CREATE_SERVICE_IMPORT_GUARDS = (
    "fastapi",
    "sqlalchemy",
    "app.models.chapter",
    "app.models.project",
    "app.services.batch_generation_orchestration_service",
)

LEGACY_BATCH_GENERATION_RESUME_SERVICE_IMPORT_GUARDS = (
    "fastapi",
    "sqlalchemy",
    "app.models.batch_generation_task",
    "app.models.chapter",
    "app.services.batch_generation_orchestration_service",
)

LEGACY_BATCH_GENERATION_ENTRY_SERVICE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.ai_service",
    "app.services.batch_generation_analysis_service",
    "app.services.batch_generation_run_service",
    "app.services.batch_generation_run_wiring_service",
    "app.services.chapter_quality_context_service",
    "app.services.manual_chapter_analysis_execution_service",
    "app.services.story_repair_payload_service",
    "app.services.task_workflow_runtime_service",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_WORKFLOW_SERVICE_IMPORT_GUARDS = (
    "fastapi",
    "sqlalchemy",
    "app.models",
    "app.services.ai_service",
    "app.services.chapter_quality_context_service",
    "app.services.story_repair_payload_service",
)

LEGACY_BATCH_GENERATION_SINGLE_CHAPTER_ENTRY_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.ai_service",
    "app.services.batch_generation_single_chapter_wiring_service",
    "app.services.chapter_candidate_runtime_state_service",
    "app.services.chapter_context_service",
    "app.services.chapter_quality_context_service",
    "app.services.chapter_web_research_service",
    "app.services.prompt_service",
    "app.services.story_repair_payload_service",
    "app.services.story_quality_feedback_service",
    "app.services.task_workflow_runtime_service",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_PACKAGE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "app.services.batch_generation.create_service",
    "app.services.batch_generation.query_service",
    "app.services.batch_generation.resume_service",
    "app.services.batch_generation.status_response_builder",
    "app.services.batch_generation.task_workflow_snapshot_service",
    "sqlalchemy",
)

BATCH_BATCH_GENERATION_FROZEN_SOURCE_MAPS = (
    (
        "app.services.batch_generation",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
        "legacy_batch_generation_python_routes_enabled; legacy_single_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.api.chapter_batch_generation_routes",
        "backend-rs/src/api/chapter_batch_generation.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.route_wiring_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/api/health.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.create_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; backend-rs/src/services/chapter_generation_execution_contract_service.rs",
        "legacy_batch_generation_python_routes_enabled; legacy_single_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.query_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
        "legacy_batch_generation_python_routes_enabled; legacy_single_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.resume_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.status_response_builder",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs; backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation.task_workflow_snapshot_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; backend-rs/src/api/health.rs",
        "legacy_batch_generation_python_routes_enabled; legacy_single_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_orchestration_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/services/chapter_generation_execution_contract_service.rs; backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs; backend-rs/src/services/chapter_access_service.rs",
        "legacy_batch_generation_python_routes_enabled; legacy_single_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_stream_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_read_context_service.rs; backend-rs/src/api/health.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_run_wiring_service",
        "backend-rs/src/api/health.rs",
        "aggregate_chapters_python_source_map",
        "freeze",
    ),
    (
        "app.services.batch_generation_analysis_service",
        "backend-rs/src/api/health.rs",
        "aggregate_chapters_python_source_map",
        "freeze",
    ),
    (
        "app.services.batch_generation_run_service",
        "backend-rs/src/api/health.rs",
        "aggregate_chapters_python_source_map",
        "freeze",
    ),
    (
        "app.services.batch_generation_execution_service",
        "backend-rs/src/api/health.rs; backend-rs/src/services/chapter_generation_runtime_service.rs; backend-rs/src/services/chapter_generation_execution_contract_service.rs; backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
        "aggregate_chapters_python_source_map; legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_entry_service",
        "backend-rs/src/api/chapter_batch_generation.rs; backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_workflow_service",
        "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
    (
        "app.services.batch_generation_single_chapter_entry_service",
        "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs; backend-rs/src/services/chapter_single_generation_runtime_state_service.rs; backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
        "legacy_batch_generation_python_routes_enabled",
        "freeze",
    ),
)

BATCH_B_DRAFT_ROUTE_PATHS = (
    "/api/chapters/{chapter_id}/analysis/auto-revision-draft",
    "/api/chapters/{chapter_id}/analysis/auto-revision-draft/apply",
    "/api/chapters/{chapter_id}/analysis/candidate-draft",
    "/api/chapters/{chapter_id}/analysis/candidate-draft/apply",
)

BATCH_B_ANALYSIS_ROUTE_PATHS = (
    "/api/chapters/{chapter_id}/analysis",
    "/api/chapters/{chapter_id}/analysis/status",
    "/api/chapters/analysis/status/batch",
    "/api/chapters/{chapter_id}/analyze",
)

BATCH_B_REGENERATION_ROUTE_PATHS = (
    "/api/chapters/{chapter_id}/regenerate-stream",
    "/api/chapters/{chapter_id}/regeneration/tasks",
    "/api/chapters/{chapter_id}/partial-regenerate-stream",
    "/api/chapters/{chapter_id}/apply-partial-regenerate",
)

BATCH_C_AGGREGATE_SOURCE_MAP_MODULES = (
    "app.api.chapters",
    "app.services.compat.chapter_generation_route_compat_service",
)

BATCH_C_AGGREGATE_SOURCE_MAP_IMPORT_GUARDS = (
    *BATCH_C_AGGREGATE_SOURCE_MAP_MODULES,
    "app.services.chapter_generation.route_wiring_service",
    "app.api.chapter_generation_routes",
    "app.api.chapter_batch_generation_routes",
)

BATCH_B_FROZEN_SOURCE_MAPS = (
    (
        "app.api.chapter_draft_routes",
        "backend-rs/src/api/chapter_draft_routes.rs",
        "legacy_chapter_draft_python_routes_enabled",
    ),
    (
        "app.api.chapter_analysis_routes",
        "backend-rs/src/api/chapter_analysis_routes.rs",
        "legacy_chapter_analysis_python_routes_enabled",
    ),
    (
        "app.api.chapter_analysis_task_routes",
        "backend-rs/src/api/chapter_analysis_routes.rs",
        "legacy_chapter_analysis_python_routes_enabled",
    ),
    (
        "app.api.chapter_regeneration_routes",
        "backend-rs/src/api/chapter_regeneration_routes.rs",
        "legacy_chapter_regeneration_python_routes_enabled",
    ),
    (
        "app.api.chapter_partial_regeneration_routes",
        "backend-rs/src/api/chapter_regeneration_routes.rs",
        "legacy_chapter_regeneration_python_routes_enabled",
    ),
)

BATCH_C_FROZEN_SOURCE_MAPS = (
    (
        "app.api.chapters",
        "backend-rs/src/api/chapter_generation_routes.rs; backend-rs/src/api/health.rs",
        "aggregate_chapters_python_source_map",
        "repoint",
    ),
    (
        "app.services.compat.chapter_generation_route_compat_service",
        "backend-rs/src/api/chapter_generation_routes.rs",
        "aggregate_chapter_generation_route_compat_source_map",
        "freeze",
    ),
)


@pytest.fixture(autouse=True)
def restore_sqlalchemy_modules():
    """Keep import-guard tests from leaking sys.modules surgery across files."""
    snapshot = {
        module_name: module
        for module_name, module in sys.modules.items()
        if module_name == "sqlalchemy" or module_name.startswith("sqlalchemy.")
    }
    yield

    for module_name in list(sys.modules):
        if module_name == "sqlalchemy" or module_name.startswith("sqlalchemy."):
            sys.modules.pop(module_name, None)
    sys.modules.update(snapshot)


def _clear_legacy_single_generation_modules():
    for module_name in LEGACY_SINGLE_GENERATION_MODULES:
        sys.modules.pop(module_name, None)


def _clear_module_prefixes(module_prefixes):
    for module_name in list(sys.modules):
        if any(
            module_name == module_prefix or module_name.startswith(f"{module_prefix}.")
            for module_prefix in module_prefixes
        ):
            sys.modules.pop(module_name, None)


def _clear_legacy_batch_generation_modules():
    for module_name in LEGACY_BATCH_GENERATION_MODULES:
        sys.modules.pop(module_name, None)


def _route_paths(app):
    return {route.path for route in app.routes}


def _assert_paths_present(route_paths, expected_paths):
    for expected_path in expected_paths:
        assert expected_path in route_paths


def _assert_paths_absent(route_paths, expected_paths):
    for expected_path in expected_paths:
        assert expected_path not in route_paths


def _assert_frozen_source_map(module, *, rust_owner, rollback_flag):
    assert module.SOURCE_MAP_FREEZE_STATUS == "frozen_source_map_rollback_only"
    assert module.SOURCE_MAP_RUST_OWNER == rust_owner
    assert module.SOURCE_MAP_ROLLBACK_FLAG == rollback_flag
    assert "Rust owns" in module.SOURCE_MAP_FREEZE_REASON


def _assert_source_map_closeout_action(module, *, action):
    assert module.SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION == action


async def test_should_create_app_and_register_health_routes():
    app = create_app()
    route_paths = _route_paths(app)

    assert "/health" in route_paths
    assert "/livez" in route_paths
    assert "/readyz" in route_paths


async def test_should_not_register_legacy_single_generation_python_routes_by_default():
    _clear_legacy_single_generation_modules()

    app = create_app()
    route_paths = _route_paths(app)

    assert "/api/chapters/{chapter_id}/generate-stream" not in route_paths
    assert "/api/chapters/{chapter_id}/generate-background" not in route_paths
    for module_name in LEGACY_SINGLE_GENERATION_MODULES:
        assert module_name not in sys.modules


async def test_should_not_register_legacy_batch_generation_python_routes_by_default():
    _clear_legacy_batch_generation_modules()

    app = create_app()
    route_paths = _route_paths(app)

    assert "/api/chapters/project/{project_id}/batch-generate" not in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/status" not in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/stream" not in route_paths
    for module_name in LEGACY_BATCH_GENERATION_MODULES:
        assert module_name not in sys.modules


async def test_should_not_import_batch_c_aggregate_source_maps_by_default():
    _clear_module_prefixes(BATCH_C_AGGREGATE_SOURCE_MAP_IMPORT_GUARDS)

    create_app()

    for module_name in BATCH_C_AGGREGATE_SOURCE_MAP_MODULES:
        assert module_name not in sys.modules


async def test_legacy_batch_generation_route_module_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.api.chapter_batch_generation_routes",
            *LEGACY_BATCH_GENERATION_ROUTE_MODULE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.api.chapter_batch_generation_routes")

    assert "app.api.chapter_batch_generation_routes" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_ROUTE_MODULE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.api.chapter_batch_generation_routes"],
        action="freeze",
    )


async def test_legacy_batch_generation_route_wiring_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.route_wiring_service",
            *LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.route_wiring_service")

    assert "app.services.batch_generation.route_wiring_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.route_wiring_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_status_response_builder_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.status_response_builder",
            *LEGACY_BATCH_GENERATION_STATUS_RESPONSE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.status_response_builder")

    assert "app.services.batch_generation.status_response_builder" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_STATUS_RESPONSE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.status_response_builder"],
        action="freeze",
    )


async def test_legacy_batch_generation_task_workflow_snapshot_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.task_workflow_snapshot_service",
            *LEGACY_BATCH_GENERATION_TASK_WORKFLOW_SNAPSHOT_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.task_workflow_snapshot_service")

    assert "app.services.batch_generation.task_workflow_snapshot_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_TASK_WORKFLOW_SNAPSHOT_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.task_workflow_snapshot_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_query_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.query_service",
            *LEGACY_BATCH_GENERATION_QUERY_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.query_service")

    assert "app.services.batch_generation.query_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_QUERY_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.query_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_create_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.create_service",
            *LEGACY_BATCH_GENERATION_CREATE_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.create_service")

    assert "app.services.batch_generation.create_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_CREATE_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.create_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_resume_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation.resume_service",
            *LEGACY_BATCH_GENERATION_RESUME_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation.resume_service")

    assert "app.services.batch_generation.resume_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_RESUME_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation.resume_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_entry_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation_entry_service",
            *LEGACY_BATCH_GENERATION_ENTRY_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation_entry_service")

    assert "app.services.batch_generation_entry_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_ENTRY_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation_entry_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_workflow_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation_workflow_service",
            *LEGACY_BATCH_GENERATION_WORKFLOW_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation_workflow_service")

    assert "app.services.batch_generation_workflow_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_WORKFLOW_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation_workflow_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_single_chapter_entry_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation_single_chapter_entry_service",
            *LEGACY_BATCH_GENERATION_SINGLE_CHAPTER_ENTRY_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation_single_chapter_entry_service")

    assert "app.services.batch_generation_single_chapter_entry_service" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_SINGLE_CHAPTER_ENTRY_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation_single_chapter_entry_service"],
        action="freeze",
    )


async def test_legacy_batch_generation_package_root_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation",
            *LEGACY_BATCH_GENERATION_PACKAGE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.batch_generation")

    assert "app.services.batch_generation" in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_PACKAGE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.batch_generation"],
        action="freeze",
    )




async def test_chapters_test_support_should_not_import_legacy_single_generation_shell():
    sys.modules.pop("tests.test_api.chapters_test_support", None)
    _clear_legacy_single_generation_modules()
    _clear_legacy_batch_generation_modules()

    importlib.import_module("tests.test_api.chapters_test_support")

    for module_name in LEGACY_SINGLE_GENERATION_MODULES:
        assert module_name not in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_MODULES:
        assert module_name not in sys.modules


async def test_chapters_api_should_not_import_legacy_batch_generation_shell_by_default():
    _clear_legacy_batch_generation_modules()

    importlib.import_module("app.api.chapters")

    for module_name in LEGACY_BATCH_GENERATION_MODULES:
        assert module_name not in sys.modules


async def test_should_register_legacy_single_generation_python_routes_when_explicitly_enabled(
    monkeypatch,
):
    _clear_legacy_single_generation_modules()
    monkeypatch.setattr(
        config_settings,
        "legacy_single_generation_python_routes_enabled",
        True,
    )
    app = create_app()
    route_paths = _route_paths(app)

    assert "/api/chapters/{chapter_id}/generate-stream" in route_paths
    assert "/api/chapters/{chapter_id}/generate-background" in route_paths
    assert "app.api.chapter_generation_routes" in sys.modules
    _assert_frozen_source_map(
        sys.modules["app.api.chapter_generation_routes"],
        rust_owner="backend-rs/src/api/chapter_generation_routes.rs",
        rollback_flag="legacy_single_generation_python_routes_enabled",
    )
    for module_name in LEGACY_SINGLE_GENERATION_ROUTE_REGISTRATION_MODULES:
        assert module_name not in sys.modules


async def test_should_not_register_batch_b_python_source_map_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_batch_b_python_source_maps_should_be_repointed_after_approval():
    for module_name, rust_owner, rollback_flag in BATCH_B_FROZEN_SOURCE_MAPS:
        module = importlib.import_module(module_name)
        _assert_frozen_source_map(
            module,
            rust_owner=rust_owner,
            rollback_flag=rollback_flag,
        )
        _assert_source_map_closeout_action(module, action="repoint")


async def test_batch_c_aggregate_source_maps_should_be_repointed_after_approval():
    for module_name, rust_owner, rollback_flag, action in BATCH_C_FROZEN_SOURCE_MAPS:
        module = importlib.import_module(module_name)
        _assert_frozen_source_map(
            module,
            rust_owner=rust_owner,
            rollback_flag=rollback_flag,
        )
        _assert_source_map_closeout_action(module, action=action)


async def test_batch_generation_python_service_source_maps_should_be_frozen_after_approval():
    for module_name, rust_owner, rollback_flag, action in BATCH_BATCH_GENERATION_FROZEN_SOURCE_MAPS:
        module = importlib.import_module(module_name)
        _assert_frozen_source_map(
            module,
            rust_owner=rust_owner,
            rollback_flag=rollback_flag,
        )
        _assert_source_map_closeout_action(module, action=action)


async def test_should_disable_legacy_chapter_draft_python_routes_when_explicitly_disabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        True,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_should_register_legacy_chapter_draft_python_routes_when_explicitly_enabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        False,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_present(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_should_disable_legacy_chapter_analysis_python_routes_when_explicitly_disabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        True,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_present(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_should_register_legacy_chapter_analysis_python_routes_when_explicitly_enabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        False,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_should_disable_legacy_chapter_regeneration_python_routes_when_explicitly_disabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        True,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        False,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_present(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_should_register_legacy_chapter_regeneration_python_routes_when_explicitly_enabled(
    monkeypatch,
):
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_draft_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_analysis_python_routes_enabled",
        False,
    )
    monkeypatch.setattr(
        config_settings,
        "legacy_chapter_regeneration_python_routes_enabled",
        True,
    )
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_present(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_legacy_single_generation_route_module_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.api.chapter_generation_routes",
            *LEGACY_SINGLE_GENERATION_ROUTE_MODULE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.api.chapter_generation_routes")

    assert "app.api.chapter_generation_routes" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_ROUTE_MODULE_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_single_generation_route_wiring_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.route_wiring_service",
            *LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.route_wiring_service")

    assert "app.services.chapter_generation.route_wiring_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.route_wiring_service"],
        action="freeze",
    )


async def test_legacy_single_generation_compat_shell_should_import_without_route_wiring():
    _clear_module_prefixes(
        (
            "app.services.compat.chapter_generation_route_compat_service",
            *LEGACY_SINGLE_GENERATION_COMPAT_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.compat.chapter_generation_route_compat_service")

    assert "app.services.compat.chapter_generation_route_compat_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_COMPAT_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_single_generation_stream_entry_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.entry_service",
            *LEGACY_SINGLE_GENERATION_STREAM_ENTRY_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.entry_service")

    assert "app.services.chapter_generation.stream.entry_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_ENTRY_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.entry_service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_wiring_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.wiring_service",
            *LEGACY_SINGLE_GENERATION_STREAM_WIRING_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.wiring_service")

    assert "app.services.chapter_generation.stream.wiring_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_WIRING_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.wiring_service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_finalize_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.finalize_service",
            *LEGACY_SINGLE_GENERATION_STREAM_FINALIZE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.finalize_service")

    assert "app.services.chapter_generation.stream.finalize_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_FINALIZE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.finalize_service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_candidate_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.candidate_service",
            *LEGACY_SINGLE_GENERATION_STREAM_CANDIDATE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.candidate_service")

    assert "app.services.chapter_generation.stream.candidate_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_CANDIDATE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.candidate_service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_service_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.service",
            *LEGACY_SINGLE_GENERATION_STREAM_SERVICE_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.service")

    assert "app.services.chapter_generation.stream.service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_execution_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.execution_service",
            *LEGACY_SINGLE_GENERATION_STREAM_EXECUTION_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.execution_service")

    assert "app.services.chapter_generation.stream.execution_service" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_EXECUTION_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.execution_service"],
        action="freeze",
    )


async def test_legacy_single_generation_stream_models_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.stream.models",
            *LEGACY_SINGLE_GENERATION_STREAM_MODELS_IMPORT_GUARDS,
        )
    )

    importlib.import_module("app.services.chapter_generation.stream.models")

    assert "app.services.chapter_generation.stream.models" in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_STREAM_MODELS_IMPORT_GUARDS:
        assert module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules["app.services.chapter_generation.stream.models"],
        action="freeze",
    )


async def test_should_register_legacy_batch_generation_python_routes_when_explicitly_enabled(
    monkeypatch,
):
    _clear_legacy_batch_generation_modules()
    monkeypatch.setattr(
        config_settings,
        "legacy_batch_generation_python_routes_enabled",
        True,
    )
    app = create_app()
    route_paths = {route.path for route in app.routes}

    assert "/api/chapters/project/{project_id}/batch-generate" in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/status" in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/stream" in route_paths
    batch_generation_routes = importlib.import_module(
        "app.api.chapter_batch_generation_routes"
    )
    _assert_frozen_source_map(
        batch_generation_routes,
        rust_owner="backend-rs/src/api/chapter_batch_generation.rs",
        rollback_flag="legacy_batch_generation_python_routes_enabled",
    )
    _assert_source_map_closeout_action(
        batch_generation_routes,
        action="freeze",
    )


def test_main_app_should_serve_json_health_routes_before_spa_fallback():
    client = TestClient(main_app)

    assert client.get('/health').json() == {'status': 'ok'}
    assert client.get('/livez').json() == {'status': 'ok'}
    assert client.get('/health/db-sessions').json()['status'] == 'ok'


def test_main_app_should_skip_auth_lookup_for_health_routes(monkeypatch):
    async def fake_get_user(user_id: str):
        raise AssertionError('health routes should not query user manager')

    monkeypatch.setattr(user_manager, 'get_user', fake_get_user)
    client = TestClient(main_app)

    assert client.get('/health', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/livez', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/health/db-sessions', cookies={'user_id': 'user-1'}).json()['status'] == 'ok'
