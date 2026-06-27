import pytest
import sys
import importlib
from pathlib import Path
from fastapi.testclient import TestClient

from tests.test_support.app_runtime.app_factory import create_app
from tests.test_support.app_runtime.main import app as main_app
from tests.test_support.retired_runtime_test_support import settings as config_settings


pytestmark = pytest.mark.asyncio


LEGACY_SINGLE_GENERATION_MODULES = (
    "app.services.chapter_generation.route_wiring_service",
)

LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS = (
    "app.api",
    "app.api.outlines",
    "app.api.chapter_route_helpers",
    "app.database",
    "app.models",
    "tests.test_support.ai_gateway.ai_service",
    "tests.test_support.chapter_context_test_support",
    "sqlalchemy",
)

LEGACY_SINGLE_GENERATION_STREAM_FINALIZE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
)

LEGACY_SINGLE_GENERATION_STREAM_CANDIDATE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "tests.test_support.batch_generation_single_chapter_wiring_test_adapter",
)

LEGACY_SINGLE_GENERATION_STREAM_EXECUTION_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "sqlalchemy",
)

LEGACY_BATCH_GENERATION_SOURCE_MAP_MODULES = (
    "app.services.batch_generation",
)

RETIRED_BATCH_GENERATION_DEFAULT_IMPORT_BLOCKLIST = ()

LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS = (
    "app.api.common",
    "app.api.settings",
    "app.database",
    "app.models",
    "sqlalchemy",
)

LEGACY_CHAPTER_ANALYSIS_PREP_SERVICE_IMPORT_GUARDS = (
    "app.models",
    "sqlalchemy",
)

LEGACY_CHAPTER_ANALYSIS_SUPPORT_SERVICE_IMPORT_GUARDS = (
    "tests.test_support.ai_gateway.ai_service",
    "app.services.chapter_generated_text_service",
    "sqlalchemy",
)

LEGACY_CHAPTER_ANALYSIS_EXECUTION_SERVICE_IMPORT_GUARDS = (
    "app.database",
    "app.models",
    "tests.test_support.ai_gateway.ai_service",
    "tests.test_support.foreshadow_test_support",
    "tests.test_support.plot_analyzer_test_support",
    "sqlalchemy",
)

LEGACY_CHAPTER_ANALYSIS_ROUTE_IMPORT_GUARDS = (
    "app.api.chapter_route_helpers",
    "app.models",
    "sqlalchemy",
)

LEGACY_CHAPTER_ANALYSIS_TASK_ROUTE_IMPORT_GUARDS = (
    "app.api.chapter_route_helpers",
    "app.api.common",
    "tests.test_support.ai_gateway.ai_service",
    "app.services.manual_chapter_analysis_execution_service",
    "sqlalchemy",
)

LEGACY_CHAPTER_REGENERATION_ROUTE_IMPORT_GUARDS = (
    "app.api.chapter_route_helpers",
    "app.api.settings",
    "app.database",
    "tests.test_support.ai_gateway.ai_service",
    "sqlalchemy",
)

LAZY_BOOTSTRAP_ROUTE_MODULES = (
    "app.api.settings",
    "app.api.outlines",
    "app.api.characters",
    "app.api.careers",
    "app.api.chapter_crud_routes",
    "app.api.chapter_annotation_routes",
    "app.api.chapter_expansion_plan_routes",
    "app.api.chapter_quality_routes",
    "app.api.organizations",
    "app.api.background_tasks",
)

RETIRED_BATCH_GENERATION_FROZEN_SOURCE_MAPS = ()

RETIRED_CHAPTER_ANALYSIS_FROZEN_SOURCE_MAPS = ()

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

RETIRED_BATCH_B_FROZEN_SOURCE_MAPS = ()

RETIRED_BATCH_B_PROMOTED_FROZEN_SOURCE_MAPS = ()

INSPIRATION_ROUTE_PATHS = (
    "/api/inspiration/generate-options",
    "/api/inspiration/refine-options",
    "/api/inspiration/quick-generate",
)

MCP_PLUGINS_ROUTE_PATHS = (
    "/api/mcp/plugins",
    "/api/mcp/plugins/simple",
    "/api/mcp/plugins/metrics",
    "/api/mcp/plugins/cache/stats",
    "/api/mcp/plugins/sessions/stats",
    "/api/mcp/plugins/{plugin_id}",
    "/api/mcp/plugins/{plugin_id}/status",
)

SETTINGS_ROUTE_PATHS = (
    "/api/settings",
    "/api/settings/api-key",
    "/api/settings/presets",
    "/api/settings/presets/{preset_id}",
    "/api/settings/presets/{preset_id}/activate",
    "/api/settings/presets/{preset_id}/test",
    "/api/settings/presets/from-current",
    "/api/settings/models",
    "/api/settings/fetch-models",
    "/api/settings/test",
    "/api/settings/check-function-calling",
    "/api/settings/test-web-research",
)

AUTH_ROUTE_PATHS = (
    "/api/auth/config",
    "/api/auth/local/login",
    "/api/auth/linuxdo/url",
    "/api/auth/linuxdo/callback",
    "/api/auth/callback",
    "/api/auth/refresh",
    "/api/auth/logout",
    "/api/auth/user",
    "/api/auth/password/status",
    "/api/auth/password/set",
    "/api/auth/password/initialize",
    "/api/auth/bind/login",
)

USERS_ROUTE_PATHS = (
    "/api/users/current",
    "/api/users",
    "/api/users/set-admin",
    "/api/users/reset-password",
    "/api/users/{user_id}",
)

ADMIN_ROUTE_PATHS = (
    "/api/admin/users",
    "/api/admin/users/{user_id}",
    "/api/admin/users/{user_id}/toggle-status",
    "/api/admin/users/{user_id}/reset-password",
)

PROMPT_TEMPLATES_ROUTE_PATHS = (
    "/api/prompt-templates",
    "/api/prompt-templates/categories",
    "/api/prompt-templates/system-defaults",
    "/api/prompt-templates/sync-status",
    "/api/prompt-templates/{template_key}/sync-to-default",
    "/api/prompt-templates/{template_key}",
    "/api/prompt-templates/{template_key}/reset",
    "/api/prompt-templates/export",
    "/api/prompt-templates/import",
    "/api/prompt-templates/{template_key}/preview",
)

PROMPT_WORKSHOP_ROUTE_PATHS = (
    "/api/prompt-workshop/status",
    "/api/prompt-workshop/items",
    "/api/prompt-workshop/items/{item_id}",
    "/api/prompt-workshop/items/{item_id}/import",
    "/api/prompt-workshop/items/{item_id}/like",
    "/api/prompt-workshop/items/{item_id}/download",
    "/api/prompt-workshop/submit",
    "/api/prompt-workshop/my-submissions",
    "/api/prompt-workshop/submissions/{submission_id}",
    "/api/prompt-workshop/admin/submissions",
    "/api/prompt-workshop/admin/submissions/{submission_id}/review",
    "/api/prompt-workshop/admin/items",
    "/api/prompt-workshop/admin/items/{item_id}",
    "/api/prompt-workshop/admin/stats",
)

CHANGELOG_ROUTE_PATHS = (
    "/api/changelog",
    "/api/changelog/refresh",
)

BOOK_IMPORT_ROUTE_PATHS = (
    "/api/book-import/tasks",
    "/api/book-import/tasks/{task_id}",
    "/api/book-import/tasks/{task_id}/preview",
    "/api/book-import/tasks/{task_id}/apply",
    "/api/book-import/tasks/{task_id}/apply-stream",
    "/api/book-import/tasks/{task_id}/retry-stream",
)

FORESHADOWS_ROUTE_PATHS = (
    "/api/foreshadows/projects/{project_id}",
    "/api/foreshadows/projects/{project_id}/stats",
    "/api/foreshadows/projects/{project_id}/context/{chapter_number}",
    "/api/foreshadows/projects/{project_id}/pending-resolve",
    "/api/foreshadows/{foreshadow_id}",
    "/api/foreshadows",
    "/api/foreshadows/{foreshadow_id}/plant",
    "/api/foreshadows/{foreshadow_id}/resolve",
    "/api/foreshadows/{foreshadow_id}/abandon",
    "/api/foreshadows/projects/{project_id}/sync-from-analysis",
)

RELATIONSHIPS_ROUTE_PATHS = (
    "/api/relationships/types",
    "/api/relationships/project/{project_id}",
    "/api/relationships/graph/{project_id}",
    "/api/relationships/",
    "/api/relationships/{relationship_id}",
)

WRITING_STYLES_ROUTE_PATHS = (
    "/api/writing-styles/presets/list",
    "/api/writing-styles",
    "/api/writing-styles/user",
    "/api/writing-styles/project/{project_id}",
    "/api/writing-styles/{style_id}",
    "/api/writing-styles/{style_id}/set-default",
    "/api/writing-styles/project/{project_id}/init-defaults",
)

POLISH_ROUTE_PATHS = (
    "/api/polish",
    "/api/polish/batch",
)

BACKGROUND_TASKS_ROUTE_PATHS = (
    "/api/background-tasks",
    "/api/background-tasks/{task_id}",
    "/api/background-tasks/{task_id}/cancel",
    "/api/background-tasks/{task_id}/workflow-state",
)

PROJECTS_ROUTE_PATHS = (
    "/api/projects",
    "/api/projects/{project_id}",
    "/api/projects/{project_id}/export",
    "/api/projects/{project_id}/check-consistency",
    "/api/projects/{project_id}/fix-organizations",
    "/api/projects/{project_id}/fix-member-counts",
    "/api/projects/{project_id}/export-data",
    "/api/projects/validate-import",
    "/api/projects/import",
)

WIZARD_STREAM_ROUTE_PATHS = (
    "/api/wizard-stream/world-building",
    "/api/wizard-stream/career-system",
    "/api/wizard-stream/characters",
    "/api/wizard-stream/outline",
    "/api/wizard-stream/world-building/{project_id}/regenerate",
)

OUTLINES_ROUTE_PATHS = (
    "/api/outlines",
    "/api/outlines/project/{project_id}",
    "/api/outlines/{outline_id}",
    "/api/outlines/generate-stream",
    "/api/outlines/{outline_id}/create-single-chapter",
    "/api/outlines/{outline_id}/expand-stream",
    "/api/outlines/{outline_id}/chapters",
    "/api/outlines/batch-expand-stream",
    "/api/outlines/{outline_id}/create-chapters-from-plans",
)

CAREERS_ROUTE_PATHS = (
    "/api/careers",
    "/api/careers/generate-system",
    "/api/careers/{career_id}",
    "/api/careers/character/{character_id}/careers",
    "/api/careers/character/{character_id}/careers/main",
    "/api/careers/character/{character_id}/careers/sub",
    "/api/careers/character/{character_id}/careers/{career_id}/stage",
    "/api/careers/character/{character_id}/careers/{career_id}",
)

CHARACTERS_ROUTE_PATHS = (
    "/api/characters",
    "/api/characters/project/{project_id}",
    "/api/characters/{character_id}",
    "/api/characters/generate-stream",
    "/api/characters/export",
    "/api/characters/import",
    "/api/characters/validate-import",
)

ORGANIZATIONS_ROUTE_PATHS = (
    "/api/organizations",
    "/api/organizations/project/{project_id}",
    "/api/organizations/{org_id}",
    "/api/organizations/{org_id}/members",
    "/api/organizations/members/{member_id}",
    "/api/organizations/generate-stream",
)

CHAPTER_CRUD_ROUTE_PATHS = (
    "/api/chapters",
    "/api/chapters/project/{project_id}",
    "/api/chapters/{chapter_id}",
    "/api/chapters/{chapter_id}/navigation",
    "/api/chapters/{chapter_id}/annotations",
    "/api/chapters/{chapter_id}/expansion-plan",
    "/api/chapters/project/{project_id}/quality-trend",
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
    expanded_prefixes = set(module_prefixes)
    if "app.database" in expanded_prefixes or "app.models" in expanded_prefixes:
        expanded_prefixes.add("app.model_base")
    if "app.models" in expanded_prefixes:
        expanded_prefixes.add("migrator_app.models")

    for module_name in list(sys.modules):
        if any(
            module_name == module_prefix or module_name.startswith(f"{module_prefix}.")
            for module_prefix in expanded_prefixes
        ):
            sys.modules.pop(module_name, None)


def _clear_legacy_batch_generation_modules():
    for module_name in LEGACY_BATCH_GENERATION_SOURCE_MAP_MODULES:
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


def _assert_thin_shell_import(module_name, import_guards, *, action):
    _clear_module_prefixes((module_name, *import_guards))

    importlib.import_module(module_name)

    assert module_name in sys.modules
    for guarded_module_name in import_guards:
        assert guarded_module_name not in sys.modules
    _assert_source_map_closeout_action(
        sys.modules[module_name],
        action=action,
    )


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
    assert not RETIRED_BATCH_GENERATION_DEFAULT_IMPORT_BLOCKLIST


async def test_should_not_register_inspiration_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, INSPIRATION_ROUTE_PATHS)


async def test_should_not_register_mcp_plugins_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, MCP_PLUGINS_ROUTE_PATHS)


async def test_should_not_register_settings_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, SETTINGS_ROUTE_PATHS)


async def test_should_not_register_auth_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, AUTH_ROUTE_PATHS)


async def test_should_not_register_users_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, USERS_ROUTE_PATHS)


async def test_should_not_register_admin_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, ADMIN_ROUTE_PATHS)


async def test_legacy_identity_python_route_shells_should_be_deleted():
    for module_name in ("app.api.auth", "app.api.users", "app.api.admin"):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_should_not_register_prompt_templates_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, PROMPT_TEMPLATES_ROUTE_PATHS)


async def test_should_not_register_prompt_workshop_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, PROMPT_WORKSHOP_ROUTE_PATHS)


async def test_should_not_register_changelog_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, CHANGELOG_ROUTE_PATHS)


async def test_should_not_register_book_import_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BOOK_IMPORT_ROUTE_PATHS)


async def test_should_not_register_foreshadows_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, FORESHADOWS_ROUTE_PATHS)


async def test_should_not_register_relationships_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, RELATIONSHIPS_ROUTE_PATHS)


async def test_should_not_register_writing_styles_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, WRITING_STYLES_ROUTE_PATHS)


async def test_should_not_register_polish_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, POLISH_ROUTE_PATHS)


async def test_should_not_register_background_tasks_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BACKGROUND_TASKS_ROUTE_PATHS)


async def test_should_not_register_projects_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, PROJECTS_ROUTE_PATHS)


async def test_should_not_register_wizard_stream_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, WIZARD_STREAM_ROUTE_PATHS)


async def test_should_not_register_outlines_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, OUTLINES_ROUTE_PATHS)


async def test_should_not_register_careers_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, CAREERS_ROUTE_PATHS)


async def test_should_not_register_characters_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, CHARACTERS_ROUTE_PATHS)


async def test_should_not_register_organizations_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, ORGANIZATIONS_ROUTE_PATHS)


async def test_should_not_register_chapter_crud_python_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, CHAPTER_CRUD_ROUTE_PATHS)


async def test_should_not_import_legacy_bootstrap_route_modules_by_default():
    _clear_module_prefixes(LAZY_BOOTSTRAP_ROUTE_MODULES)

    create_app()

    for module_name in LAZY_BOOTSTRAP_ROUTE_MODULES:
        assert module_name not in sys.modules


async def test_legacy_batch_generation_route_wiring_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation",
            *LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS,
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services.batch_generation")

    assert "app.services.batch_generation" not in sys.modules
    for module_name in LEGACY_BATCH_GENERATION_ROUTE_WIRING_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_manual_chapter_analysis_execution_service_should_be_deleted():
    _clear_module_prefixes(
        (
            "app.services.manual_chapter_analysis_execution_service",
            *LEGACY_CHAPTER_ANALYSIS_EXECUTION_SERVICE_IMPORT_GUARDS,
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services.manual_chapter_analysis_execution_service")

    assert "app.services.manual_chapter_analysis_execution_service" not in sys.modules
    for module_name in LEGACY_CHAPTER_ANALYSIS_EXECUTION_SERVICE_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_chapter_analysis_route_shell_should_be_deleted():
    _clear_module_prefixes(
        (
            "app.api.chapter_analysis_routes",
            *LEGACY_CHAPTER_ANALYSIS_ROUTE_IMPORT_GUARDS,
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.chapter_analysis_routes")

    assert "app.api.chapter_analysis_routes" not in sys.modules
    for module_name in LEGACY_CHAPTER_ANALYSIS_ROUTE_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_chapter_regeneration_route_shell_should_be_deleted():
    _clear_module_prefixes(
        (
            "app.api.chapter_regeneration_routes",
            *LEGACY_CHAPTER_REGENERATION_ROUTE_IMPORT_GUARDS,
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.chapter_regeneration_routes")

    assert "app.api.chapter_regeneration_routes" not in sys.modules
    for module_name in LEGACY_CHAPTER_REGENERATION_ROUTE_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_legacy_batch_generation_package_root_should_import_as_thin_shell():
    _clear_module_prefixes(
        (
            "app.services.batch_generation",
            "app.database",
            "app.models",
            "sqlalchemy",
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services.batch_generation")

    assert "app.services.batch_generation" not in sys.modules
    assert not (Path(__file__).parents[2] / "app" / "database.py").exists()
    for module_name in ("app.models", "sqlalchemy"):
        assert module_name not in sys.modules




async def test_chapters_test_support_should_not_import_legacy_single_generation_shell():
    sys.modules.pop("tests.test_api.chapters_test_support", None)
    sys.modules.pop("tests.test_support.foreshadow_test_support", None)
    from tests.test_support.database_test_support import Base

    removed_table = Base.metadata.tables.get("foreshadows")
    if "foreshadows" in Base.metadata.tables:
        Base.metadata.remove(removed_table)
    try:
        _clear_legacy_single_generation_modules()
        _clear_legacy_batch_generation_modules()

        importlib.import_module("tests.test_api.chapters_test_support")

        for module_name in LEGACY_SINGLE_GENERATION_MODULES:
            assert module_name not in sys.modules
        assert not RETIRED_BATCH_GENERATION_DEFAULT_IMPORT_BLOCKLIST
    finally:
        if removed_table is not None and "foreshadows" not in Base.metadata.tables:
            Base.metadata._add_table("foreshadows", None, removed_table)


async def test_chapters_api_should_not_import_legacy_batch_generation_shell_by_default():
    _clear_legacy_batch_generation_modules()

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api")
    assert "app.api" not in sys.modules

    assert not RETIRED_BATCH_GENERATION_DEFAULT_IMPORT_BLOCKLIST


async def test_should_not_register_batch_b_python_source_map_routes_by_default():
    app = create_app()
    route_paths = _route_paths(app)

    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)


async def test_batch_b_python_source_maps_should_match_per_lane_closeout_stage():
    assert not RETIRED_BATCH_B_FROZEN_SOURCE_MAPS


async def test_batch_b_promoted_python_source_maps_should_be_frozen_after_closeout_promotion():
    assert not RETIRED_BATCH_B_PROMOTED_FROZEN_SOURCE_MAPS


async def test_batch_generation_python_service_source_maps_should_be_frozen_after_approval():
    assert not RETIRED_BATCH_GENERATION_FROZEN_SOURCE_MAPS


async def test_chapter_analysis_python_service_source_maps_should_be_frozen_after_approval():
    assert not RETIRED_CHAPTER_ANALYSIS_FROZEN_SOURCE_MAPS


async def test_should_keep_chapter_regeneration_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_chapter_regeneration_python_routes_enabled")
    _assert_paths_absent(route_paths, BATCH_B_DRAFT_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_ANALYSIS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, BATCH_B_REGENERATION_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.chapter_regeneration_routes",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.chapter_regeneration_routes")
    assert "app.api.chapter_regeneration_routes" not in sys.modules


async def test_should_keep_background_tasks_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_background_tasks_python_routes_enabled")
    _assert_paths_absent(route_paths, BACKGROUND_TASKS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.background_tasks",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.background_tasks")
    assert "app.api.background_tasks" not in sys.modules


async def test_should_keep_projects_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_projects_python_routes_enabled")
    _assert_paths_absent(route_paths, PROJECTS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.projects",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.projects")
    assert "app.api.projects" not in sys.modules


async def test_legacy_settings_route_shell_should_be_deleted():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_settings_python_routes_enabled")
    _assert_paths_absent(route_paths, SETTINGS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.settings",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.settings")
    assert "app.api.settings" not in sys.modules


async def test_legacy_chapter_route_helpers_should_be_deleted():
    _clear_module_prefixes(("app.api.chapter_route_helpers",))

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.chapter_route_helpers")

    assert "app.api.chapter_route_helpers" not in sys.modules


async def test_legacy_api_common_helpers_should_be_deleted():
    for module_name in ("app.api.common", "app.api.ai_dependencies"):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_prompt_template_helper_services_should_be_deleted():
    for module_name in (
        "app.services.prompt_template_access_service",
        "app.services.prompt_template_catalog_service",
        "app.services.prompt_template_render_service",
        "app.services.prompt_template_sync_service",
        "app.services.story_prompt_template_support_service",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_story_style_helper_services_should_be_deleted():
    sys.modules.pop("app.services", None)

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services")
    assert "app.services" not in sys.modules

    for module_name in (
        "app.services.story_style_profile_service",
        "app.services.story_writing_style_service",
        "app.services.story_repair_payload_service",
        "app.services.background_task_manager",
        "app.services.memory_service",
        "app.services.ai_gateway",
        "app.services.task_system",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_python_utils_should_be_deleted():
    for module_name in (
        "app.utils.data_consistency",
        "app.utils.exception_message",
        "app.utils.sse_response",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_python_mcp_package_should_be_deleted():
    for module_name in (
        "app.mcp",
        "app.mcp.config",
        "app.mcp.facade",
        "app.mcp.status_sync",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_python_schemas_package_should_be_deleted():
    for module_name in (
        "app.schemas",
        "app.schemas.generation_payload",
        "app.schemas.generation_preferences",
        "app.schemas.import_export",
        "app.schemas.mcp_plugin",
        "app.schemas.novel_quality_profile_service",
        "app.schemas.novel_quality_rules",
        "app.schemas.quality",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_legacy_python_fastapi_runtime_shell_should_be_deleted():
    for module_name in (
        "app.main",
        "app.bootstrap",
        "app.bootstrap.app_factory",
        "app.bootstrap.lifespan",
        "app.bootstrap.static_assets",
    ):
        _clear_module_prefixes((module_name,))

        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)

        assert module_name not in sys.modules


async def test_should_keep_auth_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_auth_python_routes_enabled")
    _assert_paths_absent(route_paths, AUTH_ROUTE_PATHS)


async def test_should_keep_user_admin_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_user_admin_python_routes_enabled")
    _assert_paths_absent(route_paths, USERS_ROUTE_PATHS)
    _assert_paths_absent(route_paths, ADMIN_ROUTE_PATHS)


async def test_should_keep_wizard_stream_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_wizard_stream_python_routes_enabled")
    _assert_paths_absent(route_paths, WIZARD_STREAM_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.wizard_stream",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.wizard_stream")
    assert "app.api.wizard_stream" not in sys.modules


async def test_should_keep_outlines_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_outlines_python_routes_enabled")
    _assert_paths_absent(route_paths, OUTLINES_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.outlines",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.outlines")
    assert "app.api.outlines" not in sys.modules


async def test_should_keep_careers_python_routes_unregistered_after_route_shell_closeout():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_careers_python_routes_enabled")
    _assert_paths_absent(route_paths, CAREERS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.careers",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.careers")
    assert "app.api.careers" not in sys.modules


async def test_legacy_characters_route_shell_should_be_deleted():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_characters_python_routes_enabled")
    _assert_paths_absent(route_paths, CHARACTERS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.characters",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.characters")
    assert "app.api.characters" not in sys.modules


async def test_legacy_organizations_route_shell_should_be_deleted():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_organizations_python_routes_enabled")
    _assert_paths_absent(route_paths, ORGANIZATIONS_ROUTE_PATHS)

    _clear_module_prefixes(("app.api.organizations",))
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.api.organizations")
    assert "app.api.organizations" not in sys.modules


async def test_legacy_chapter_crud_route_shells_should_be_deleted():
    app = create_app()
    route_paths = _route_paths(app)

    assert not hasattr(config_settings, "legacy_chapter_crud_python_routes_enabled")
    _assert_paths_absent(route_paths, CHAPTER_CRUD_ROUTE_PATHS)

    for module_name in (
        "app.api.chapter_crud_routes",
        "app.api.chapter_annotation_routes",
        "app.api.chapter_expansion_plan_routes",
        "app.api.chapter_quality_routes",
    ):
        _clear_module_prefixes((module_name,))
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(module_name)
        assert module_name not in sys.modules


async def test_legacy_single_generation_route_wiring_shim_should_be_deleted():
    _clear_module_prefixes(
        (
            "app.services.chapter_generation.route_wiring_service",
            *LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS,
        )
    )

    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services.chapter_generation.route_wiring_service")

    assert "app.services.chapter_generation.route_wiring_service" not in sys.modules
    for module_name in LEGACY_SINGLE_GENERATION_ROUTE_WIRING_IMPORT_GUARDS:
        assert module_name not in sys.modules


async def test_should_keep_legacy_batch_generation_python_routes_unregistered_after_route_shell_closeout():
    _clear_legacy_batch_generation_modules()
    app = create_app()
    route_paths = {route.path for route in app.routes}

    assert not hasattr(config_settings, "legacy_batch_generation_python_routes_enabled")
    assert "/api/chapters/project/{project_id}/batch-generate" not in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/status" not in route_paths
    assert "/api/chapters/batch-generate/{batch_id}/stream" not in route_paths
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("app.services.batch_generation")


def test_main_app_should_serve_json_health_routes_before_spa_fallback():
    client = TestClient(main_app)

    assert client.get('/health').json() == {'status': 'ok'}
    assert client.get('/livez').json() == {'status': 'ok'}
    assert client.get('/health/db-sessions').json()['status'] == 'ok'


def test_main_app_should_serve_health_routes_with_login_cookies_after_auth_closeout():
    client = TestClient(main_app)

    assert client.get('/health', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/livez', cookies={'user_id': 'user-1'}).json() == {'status': 'ok'}
    assert client.get('/health/db-sessions', cookies={'user_id': 'user-1'}).json()['status'] == 'ok'


def test_test_support_runtime_should_keep_static_fallback_path_on_backend_static():
    from tests.test_support.app_runtime import static_assets

    static_path = static_assets.Path(static_assets.__file__).parents[3] / "static"

    assert static_path.name == "static"
    assert static_path.parent.name == "backend"
