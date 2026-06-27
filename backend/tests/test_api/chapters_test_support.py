import json
import importlib
import sys
from types import SimpleNamespace
from typing import Any

import pytest
import pytest_asyncio
from fastapi import APIRouter, BackgroundTasks, Depends, FastAPI, HTTPException, Query, Request
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from tests.test_support.chapter_route_helpers_test_support import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from tests.test_support.ai_dependencies_test_support import (
    get_user_ai_service as shared_get_user_ai_service,
)
from tests.test_support.api_common_test_support import verify_project_access
from tests.test_support.database_test_support import Base, get_db as app_get_db
from migrator_app.models.chapter import Chapter
from migrator_app.models import ChapterDraftAttempt, GenerationHistory
from migrator_app.models.outline import Outline
from migrator_app.models.project import Project
from tests.test_support.chapter_schema_test_support import (
    BatchGenerateRequest,
    BatchGenerateResponse,
    BatchGenerateStatusResponse,
    ChapterGenerateRequest,
)
from tests.test_support.foreshadow_test_support import foreshadow_service
from tests.test_support.memory_service_test_support import memory_service
from tests.test_support import (
    chapter_regeneration_route_test_adapter as chapter_regeneration_routes_api,
)
from tests.test_support import chapter_prompt_quality_test_support as chapter_prompt_quality_service_module
from tests.test_support.chapter_generated_text_test_support import (
    contains_chapter_workflow_meta_text,
)
from tests.test_support.task_system import workflow_runtime_state_store
from tests.test_support.task_quality_snapshot_test_support import (
    task_quality_metrics_cache,
)
from tests.test_support.chapter_generation_history_test_support import (
    _build_candidate_draft_payload,
    _extract_candidate_draft_full_content,
    _load_latest_candidate_draft_attempt,
    build_auto_revision_draft_payload,
    build_generation_history_payload,
    build_reviser_apply_history_payload,
    is_reviser_draft_stale,
    load_latest_reviser_history,
    require_candidate_draft_full_content,
)
from tests.test_support import (
    manual_chapter_analysis_execution_test_support as manual_chapter_analysis_execution_service,
)
from tests.test_support import (
    chapter_annotation_route_test_adapter as chapter_annotation_routes_api,
)
from tests.test_support import (
    chapter_crud_route_test_adapter as chapter_crud_routes_api,
)
from tests.test_support import (
    chapter_expansion_plan_route_test_adapter as chapter_expansion_plan_routes_api,
)
from tests.test_support import (
    chapter_quality_route_test_adapter as chapter_quality_routes_api,
)
from tests.test_support.project_quality_trend_test_support import (
    project_quality_trend_cache,
)
from tests.test_support import chapter_analysis_route_test_adapter as chapter_analysis_routes_api
from tests.test_support.batch_generation_run_wiring_test_adapter import (
    execute_batch_generation_in_order_with_default_wiring as real_execute_batch_generation_in_order_with_default_wiring,
)


_single_generation_stream_entry_module = None
_single_generation_background_entry_module = None
_batch_generation_route_test_adapter_module = None
test_support_services_namespace = SimpleNamespace()


def _resolve_loaded_test_module_attr(module_names: tuple[str, ...], attr_name: str):
    for module_name in module_names:
        module = sys.modules.get(module_name)
        if module is not None and hasattr(module, attr_name):
            return getattr(module, attr_name)
    return None


def _restore_shared_test_module_identities():
    """Keep test monkeypatch targets aligned with runtime imports after sys.modules surgery."""
    sys.modules["tests.test_support.chapter_prompt_quality_test_support"] = (
        chapter_prompt_quality_service_module
    )
    test_support_services_namespace.chapter_prompt_quality_test_support = (
        chapter_prompt_quality_service_module
    )

    batch_generation_run_wiring_service = _resolve_loaded_test_module_attr(
        (
            "test_chapters_batch_generation",
            "tests.test_api.test_chapters_batch_generation",
            "test_chapters_batch_status_resume",
            "tests.test_api.test_chapters_batch_status_resume",
        ),
        "batch_generation_run_wiring_service",
    )
    if batch_generation_run_wiring_service is None:
        batch_generation_run_wiring_service = getattr(
            test_support_services_namespace,
            "batch_generation_run_wiring_service",
            None,
        )
    if batch_generation_run_wiring_service is not None:
        test_support_services_namespace.batch_generation_run_wiring_service = (
            batch_generation_run_wiring_service
        )
        sys.modules["tests.test_support.batch_generation_run_wiring_test_adapter"] = (
            batch_generation_run_wiring_service
        )

    batch_generation_retry_service = _resolve_loaded_test_module_attr(
        (
            "test_chapters_batch_generation",
            "tests.test_api.test_chapters_batch_generation",
            "test_chapters_batch_status_resume",
            "tests.test_api.test_chapters_batch_status_resume",
        ),
        "batch_generation_retry_service",
    )
    if batch_generation_retry_service is None:
        batch_generation_retry_service = getattr(
            test_support_services_namespace,
            "batch_generation_retry_service",
            None,
        )
    if batch_generation_retry_service is not None:
        test_support_services_namespace.batch_generation_retry_service = (
            batch_generation_retry_service
        )
        sys.modules["tests.test_support.batch_generation_retry_test_adapter"] = (
            batch_generation_retry_service
        )

    batch_generation_single_chapter_wiring_service = _resolve_loaded_test_module_attr(
        (
            "test_chapters_batch_generation",
            "tests.test_api.test_chapters_batch_generation",
            "test_chapters",
            "tests.test_api.test_chapters",
            "test_chapter_quality_metrics",
            "tests.test_api.test_chapter_quality_metrics",
            "test_chapters_candidate_rerank",
            "tests.test_api.test_chapters_candidate_rerank",
        ),
        "batch_generation_single_chapter_entry_service",
    )
    if batch_generation_single_chapter_wiring_service is None:
        batch_generation_single_chapter_wiring_service = getattr(
            test_support_services_namespace,
            "batch_generation_single_chapter_wiring_test_adapter",
            None,
        )
    if batch_generation_single_chapter_wiring_service is not None:
        test_support_services_namespace.batch_generation_single_chapter_wiring_test_adapter = (
            batch_generation_single_chapter_wiring_service
        )
        sys.modules["tests.test_support.batch_generation_single_chapter_wiring_test_adapter"] = (
            batch_generation_single_chapter_wiring_service
        )

async def REAL_EXECUTE_BATCH_GENERATION_IN_ORDER(*args, **kwargs):
    _restore_shared_test_module_identities()
    return await real_execute_batch_generation_in_order_with_default_wiring(
        *args, **kwargs
    )


def load_single_generation_stream_entry_module():
    """Load the active single-generation stream owner for HTTP behavior tests."""
    global _single_generation_stream_entry_module

    _restore_shared_test_module_identities()
    module = sys.modules.get(
        "tests.test_support.single_generation_stream_entry_test_adapter"
    )
    if module is None and _single_generation_stream_entry_module is None:
        _single_generation_stream_entry_module = importlib.import_module(
            "tests.test_support.single_generation_stream_entry_test_adapter"
        )
        module = _single_generation_stream_entry_module
    elif module is None:
        module = _single_generation_stream_entry_module

    _single_generation_stream_entry_module = module

    return module


def load_single_generation_background_entry_module():
    """Load the active single-generation background owner for HTTP behavior tests."""
    global _single_generation_background_entry_module

    _restore_shared_test_module_identities()
    module = sys.modules.get(
        "tests.test_support.single_generation_background_entry_test_adapter"
    )
    if module is None and _single_generation_background_entry_module is None:
        _single_generation_background_entry_module = importlib.import_module(
            "tests.test_support.single_generation_background_entry_test_adapter"
        )
        module = _single_generation_background_entry_module
    elif module is None:
        module = _single_generation_background_entry_module

    _single_generation_background_entry_module = module

    return module


def build_single_generation_test_router():
    """Build a test-only router around the active single-generation owner modules."""
    stream_entry_service = load_single_generation_stream_entry_module()
    background_entry_service = load_single_generation_background_entry_module()
    router = APIRouter(prefix="/chapters", tags=["chapters"])

    async def get_db(request: Request):
        async for session in app_get_db(request):
            yield session

    async def get_user_ai_service(request: Request, db=Depends(get_db)):
        from tests.test_support.ai_dependencies_test_support import (
            get_user_ai_service as app_get_user_ai_service,
            require_login,
        )

        return await app_get_user_ai_service(user=require_login(request), db=db)

    @router.post("/{chapter_id}/generate-stream", summary="AI stream chapter generation")
    async def generate_chapter_content_stream(
        chapter_id: str,
        request: Request,
        background_tasks: BackgroundTasks,
        generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
        user_ai_service=Depends(get_user_ai_service),
    ):
        return await stream_entry_service.generate_chapter_content_stream_with_default_wiring(
            chapter_id=chapter_id,
            request=request,
            background_tasks=background_tasks,
            generate_request=generate_request,
            user_ai_service=user_ai_service,
        )

    @router.post("/{chapter_id}/generate-background", summary="AI background chapter generation")
    async def generate_chapter_content_background(
        chapter_id: str,
        request: Request,
        background_tasks: BackgroundTasks,
        generate_request: ChapterGenerateRequest = ChapterGenerateRequest(),
        db=Depends(get_db),
        user_ai_service=Depends(get_user_ai_service),
    ):
        return await background_entry_service.generate_chapter_content_background_with_default_wiring(
            chapter_id=chapter_id,
            request=request,
            background_tasks=background_tasks,
            generate_request=generate_request,
            db_session=db,
            user_ai_service=user_ai_service,
        )

    return router, get_db, get_user_ai_service, stream_entry_service


def build_chapter_draft_test_router():
    """Build a test-only router around the surviving draft owner contracts."""
    router = APIRouter(prefix="/chapters", tags=["chapters"])

    async def get_db(request: Request):
        async for session in app_get_db(request):
            yield session

    def _resolve_allow_stale(payload: dict[str, Any] | None) -> bool:
        if not payload:
            return False
        value = payload.get("allow_stale", False)
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            return value.strip().lower() in {"1", "true", "yes", "on"}
        return bool(value)

    @router.get("/{chapter_id}/analysis/auto-revision-draft")
    async def get_auto_revision_draft(
        chapter_id: str,
        request: Request,
        history_id: str | None = Query(None),
        db=Depends(get_db),
    ):
        user_id = require_authenticated_user_id(request)
        chapter = await load_accessible_chapter_or_404(
            db=db,
            chapter_id=chapter_id,
            user_id=user_id,
        )
        reviser = await load_latest_reviser_history(db, chapter_id, history_id=history_id)
        if reviser is None:
            return {"chapter_id": chapter_id, "auto_revision_draft": None}
        history, reviser_result = reviser
        return {
            "chapter_id": chapter_id,
            "auto_revision_draft": build_auto_revision_draft_payload(
                reviser_result=reviser_result,
                history_id=history.id,
                created_at=history.created_at,
                chapter_updated_at=chapter.updated_at,
                include_full_text=True,
            ),
        }

    @router.post("/{chapter_id}/analysis/auto-revision-draft/apply")
    async def apply_auto_revision_draft(
        chapter_id: str,
        request: Request,
        apply_request: dict[str, Any] | None = None,
        db=Depends(get_db),
    ):
        user_id = require_authenticated_user_id(request)
        chapter = await load_accessible_chapter_or_404(
            db=db,
            chapter_id=chapter_id,
            user_id=user_id,
        )
        history_id_raw = (apply_request or {}).get("history_id")
        history_id = str(history_id_raw).strip() if history_id_raw is not None else ""
        allow_stale = _resolve_allow_stale(apply_request)
        reviser = await load_latest_reviser_history(
            db,
            chapter_id,
            history_id=history_id or None,
        )
        if reviser is None:
            raise HTTPException(status_code=404, detail="该章节暂无自动修订草稿")
        history, reviser_result = reviser
        revised_text = str(reviser_result.get("revised_text") or "")
        if not revised_text.strip():
            raise HTTPException(status_code=409, detail="自动修订草稿内容为空，无法应用")
        if contains_chapter_workflow_meta_text(revised_text):
            raise HTTPException(status_code=409, detail="自动修订草稿包含流程化元文本，无法应用")

        stale_applied = is_reviser_draft_stale(chapter.updated_at, history.created_at)
        if stale_applied and not allow_stale:
            raise HTTPException(
                status_code=409,
                detail="自动修订草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
            )

        old_word_count = int(chapter.word_count or len(chapter.content or ""))
        chapter.content = revised_text
        chapter.word_count = len(revised_text)
        db.add(
            GenerationHistory(
                project_id=chapter.project_id,
                chapter_id=chapter.id,
                prompt="auto_revision_draft_apply",
                generated_content=build_reviser_apply_history_payload(
                    source_history_id=history.id,
                    source_created_at=history.created_at,
                    critical_count=int(reviser_result.get("critical_count") or 0),
                    major_count=int(reviser_result.get("major_count") or 0),
                    priority_issue_count=int(
                        reviser_result.get("priority_issue_count")
                        or (
                            int(reviser_result.get("critical_count") or 0)
                            + int(reviser_result.get("major_count") or 0)
                        )
                    ),
                    applied_critical_count=int(reviser_result.get("applied_critical_count") or 0),
                    applied_issue_count=int(
                        reviser_result.get("applied_issue_count")
                        or reviser_result.get("applied_critical_count")
                        or 0
                    ),
                    old_word_count=old_word_count,
                    new_word_count=len(revised_text),
                    stale_applied=stale_applied,
                    allow_stale=allow_stale,
                ),
                model="chapter_text_reviser_apply_v1",
            )
        )
        await db.commit()
        await db.refresh(chapter)
        return {
            "success": True,
            "chapter_id": chapter.id,
            "word_count": len(revised_text),
            "old_word_count": old_word_count,
            "draft_history_id": history.id,
            "draft_created_at": history.created_at.isoformat() if history.created_at else None,
            "stale_applied": stale_applied,
            "message": "自动修订草稿已应用到章节正文",
        }

    @router.get("/{chapter_id}/analysis/candidate-draft")
    async def get_candidate_draft(
        chapter_id: str,
        request: Request,
        attempt_id: str | None = Query(None),
        db=Depends(get_db),
    ):
        user_id = require_authenticated_user_id(request)
        chapter = await load_accessible_chapter_or_404(
            db=db,
            chapter_id=chapter_id,
            user_id=user_id,
        )
        draft_attempt = await _load_latest_candidate_draft_attempt(
            db,
            chapter_id,
            attempt_id=attempt_id,
        )
        if draft_attempt is None:
            return {"chapter_id": chapter_id, "candidate_draft": None}
        return {
            "chapter_id": chapter_id,
            "candidate_draft": _build_candidate_draft_payload(
                draft_attempt=draft_attempt,
                chapter_updated_at=chapter.updated_at,
                include_full_text=True,
            ),
        }

    @router.post("/{chapter_id}/analysis/candidate-draft/apply")
    async def apply_candidate_draft(
        chapter_id: str,
        request: Request,
        apply_request: dict[str, Any] | None = None,
        db=Depends(get_db),
    ):
        user_id = require_authenticated_user_id(request)
        chapter = await load_accessible_chapter_or_404(
            db=db,
            chapter_id=chapter_id,
            user_id=user_id,
        )
        attempt_id_raw = (apply_request or {}).get("attempt_id")
        attempt_id = str(attempt_id_raw).strip() if attempt_id_raw is not None else ""
        allow_stale = _resolve_allow_stale(apply_request)
        draft_attempt = await _load_latest_candidate_draft_attempt(
            db,
            chapter_id,
            attempt_id=attempt_id or None,
        )
        if draft_attempt is None:
            raise HTTPException(status_code=404, detail="该章节暂无候选草稿")

        candidate_content_raw, has_full_content = _extract_candidate_draft_full_content(
            draft_attempt
        )
        if not has_full_content or not candidate_content_raw.strip():
            raise HTTPException(status_code=409, detail="该候选草稿仅保存了预览，无法直接恢复正文")
        candidate_content = require_candidate_draft_full_content(draft_attempt)
        if contains_chapter_workflow_meta_text(candidate_content):
            raise HTTPException(status_code=409, detail="候选草稿包含流程化元文本，无法应用")

        stale_applied = is_reviser_draft_stale(chapter.updated_at, draft_attempt.created_at)
        if stale_applied and not allow_stale:
            raise HTTPException(
                status_code=409,
                detail="候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
            )

        old_word_count = int(chapter.word_count or len(chapter.content or ""))
        chapter.content = candidate_content
        chapter.word_count = len(candidate_content)
        quality_metrics = (
            dict(draft_attempt.quality_metrics or {})
            if isinstance(draft_attempt.quality_metrics, dict)
            else None
        )
        db.add(
            GenerationHistory(
                project_id=chapter.project_id,
                chapter_id=chapter.id,
                prompt="candidate_draft_apply",
                generated_content=build_generation_history_payload(
                    candidate_content,
                    quality_metrics,
                    content_applied=True,
                    attempt_state=str(draft_attempt.attempt_state or "").strip() or None,
                ),
                model="chapter_candidate_apply_v1",
            )
        )
        await db.commit()
        await db.refresh(chapter)
        return {
            "success": True,
            "chapter_id": chapter.id,
            "word_count": len(candidate_content),
            "old_word_count": old_word_count,
            "draft_attempt_id": draft_attempt.id,
            "draft_created_at": draft_attempt.created_at.isoformat()
            if draft_attempt.created_at
            else None,
            "stale_applied": stale_applied,
            "message": "候选草稿已恢复到章节正文",
        }

    return router, get_db


def build_batch_generation_test_router():
    """Build a test-only router around the active batch-generation owner modules."""
    batch_generation_route_wiring_service = load_batch_generation_test_adapter_module()
    return (
        batch_generation_route_wiring_service.router,
        batch_generation_route_wiring_service.get_db,
        batch_generation_route_wiring_service.get_user_ai_service,
        batch_generation_route_wiring_service,
    )


def load_batch_generation_test_adapter_module():
    """Load the test-only batch-generation route adapter."""
    global _batch_generation_route_test_adapter_module

    _restore_shared_test_module_identities()
    batch_generation_run_wiring_service = getattr(
        test_support_services_namespace,
        "batch_generation_run_wiring_service",
        None,
    )
    if batch_generation_run_wiring_service is not None:
        sys.modules["tests.test_support.batch_generation_run_wiring_test_adapter"] = (
            batch_generation_run_wiring_service
        )

    route_wiring_module = sys.modules.get(
        "tests.test_support.batch_generation_route_test_adapter"
    )
    if route_wiring_module is None and _batch_generation_route_test_adapter_module is None:
        _batch_generation_route_test_adapter_module = importlib.import_module(
            "tests.test_support.batch_generation_route_test_adapter"
        )
        route_wiring_module = _batch_generation_route_test_adapter_module
    elif route_wiring_module is None:
        route_wiring_module = _batch_generation_route_test_adapter_module

    _batch_generation_route_test_adapter_module = route_wiring_module

    return route_wiring_module


class FakeAIService:
    def __init__(self):
        self.chunks = ["濞翠礁绱￠悧鍥唽A", "濞翠礁绱￠悧鍥唽B"]
        self.calls: list[dict[str, Any]] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(kwargs)
        for chunk in self.chunks:
            yield chunk

@pytest.fixture
def fake_ai_service():
    return FakeAIService()


@pytest.fixture(autouse=True)
def restore_test_module_identities():
    _restore_shared_test_module_identities()
    yield
    _restore_shared_test_module_identities()

@pytest.fixture(autouse=True)
def mock_side_effect_services(monkeypatch):
    async def fake_delete_chapter_memories(*args, **kwargs):
        return None

    async def fake_delete_chapter_foreshadows(*args, **kwargs):
        return {"deleted_count": 0}

    async def fake_auto_plant_pending_foreshadows(*args, **kwargs):
        return {"planted_count": 0}

    async def fake_analyze_chapter_background(*args, **kwargs):
        return True

    async def fake_execute_batch_generation(*args, **kwargs):
        return None

    from tests.test_support import (
        batch_generation_run_wiring_test_adapter as batch_generation_run_wiring_service,
    )

    monkeypatch.setattr(
        memory_service,
        "delete_chapter_memories",
        fake_delete_chapter_memories,
    )
    monkeypatch.setattr(
        foreshadow_service,
        "delete_chapter_foreshadows",
        fake_delete_chapter_foreshadows,
    )
    monkeypatch.setattr(
        chapter_crud_routes_api.memory_service,
        "delete_chapter_memories",
        fake_delete_chapter_memories,
    )
    monkeypatch.setattr(
        chapter_crud_routes_api.foreshadow_service,
        "delete_chapter_foreshadows",
        fake_delete_chapter_foreshadows,
    )
    monkeypatch.setattr(
        foreshadow_service,
        "auto_plant_pending_foreshadows",
        fake_auto_plant_pending_foreshadows,
    )
    monkeypatch.setattr(
        manual_chapter_analysis_execution_service,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        chapter_analysis_routes_api,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_default_wiring",
        fake_execute_batch_generation,
    )

def _build_quality_history_payload(metrics: dict[str, Any]) -> str:
    return json.dumps(
        {
            "log_type": "chapter_generation_quality_v1",
            "quality_metrics": metrics,
        },
        ensure_ascii=False,
    )

@pytest.fixture(autouse=True)
def reset_chapters_runtime_caches():
    task_quality_metrics_cache.clear()
    workflow_runtime_state_store.cache.clear()
    project_quality_trend_cache.clear()
    yield
    task_quality_metrics_cache.clear()
    workflow_runtime_state_store.cache.clear()
    project_quality_trend_cache.clear()

@pytest_asyncio.fixture
async def chapters_session_factory():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    try:
        yield async_sessionmaker(engine, expire_on_commit=False)
    finally:
        await engine.dispose()

@pytest_asyncio.fixture
async def chapters_client(chapters_session_factory, fake_ai_service, mock_user, monkeypatch):
    (
        chapter_generation_test_router,
        chapter_generation_get_db,
        chapter_generation_get_user_ai_service,
        chapter_generation_route_wiring_service,
    ) = build_single_generation_test_router()
    (
        chapter_batch_generation_test_router,
        chapter_batch_generation_get_db,
        chapter_batch_generation_get_user_ai_service,
        batch_generation_route_wiring_service,
    ) = build_batch_generation_test_router()
    chapter_draft_test_router, chapter_draft_get_db = build_chapter_draft_test_router()
    app = FastAPI()
    app.include_router(chapter_crud_routes_api.router, prefix="/api")
    app.include_router(chapter_analysis_routes_api.router, prefix="/api")
    app.include_router(chapter_annotation_routes_api.router, prefix="/api")
    app.include_router(chapter_batch_generation_test_router, prefix="/api")
    app.include_router(chapter_draft_test_router, prefix="/api")
    app.include_router(chapter_generation_test_router, prefix="/api")
    app.include_router(chapter_quality_routes_api.router, prefix="/api")
    app.include_router(chapter_expansion_plan_routes_api.router, prefix="/api")
    app.include_router(chapter_regeneration_routes_api.router, prefix="/api")

    async def override_get_db(_request=None):
        async with chapters_session_factory() as session:
            try:
                yield session
            finally:
                # Allow upstream services to manage transactions, but ensure we don't
                # return a session to the pool with a pending/failed transaction.
                try:
                    if session.in_transaction():
                        await session.rollback()
                except Exception:
                    pass

    async def override_get_user_ai_service():
        return fake_ai_service

    @app.middleware("http")
    async def inject_user_state(request, call_next):
        header_user_id = request.headers.get("x-test-user-id", mock_user.user_id)
        if header_user_id == "__none__":
            request.state.user_id = None
            request.state.user = None
        else:
            request.state.user_id = header_user_id
            request.state.user = (
                mock_user
                if header_user_id == mock_user.user_id
                else SimpleNamespace(user_id=header_user_id)
            )
        return await call_next(request)

    app.dependency_overrides[app_get_db] = override_get_db
    app.dependency_overrides[shared_get_user_ai_service] = override_get_user_ai_service
    app.dependency_overrides[chapter_analysis_routes_api.get_db] = override_get_db
    app.dependency_overrides[chapter_analysis_routes_api.get_user_ai_service] = (
        override_get_user_ai_service
    )
    app.dependency_overrides[chapter_draft_get_db] = override_get_db
    app.dependency_overrides[chapter_generation_get_db] = override_get_db
    app.dependency_overrides[chapter_generation_get_user_ai_service] = override_get_user_ai_service
    app.dependency_overrides[chapter_batch_generation_get_db] = override_get_db
    app.dependency_overrides[chapter_batch_generation_get_user_ai_service] = (
        override_get_user_ai_service
    )
    app.dependency_overrides[chapter_regeneration_routes_api.get_db] = override_get_db
    app.dependency_overrides[chapter_regeneration_routes_api.get_user_ai_service] = (
        override_get_user_ai_service
    )

    monkeypatch.setattr(chapter_regeneration_routes_api, "get_db", override_get_db)
    monkeypatch.setattr(chapter_generation_route_wiring_service, "get_db", override_get_db)
    monkeypatch.setattr(batch_generation_route_wiring_service, "verify_project_access", verify_project_access)
    monkeypatch.setattr(
        chapter_generation_route_wiring_service,
        "execute_chapter_analysis_background",
        manual_chapter_analysis_execution_service.execute_chapter_analysis_background,
    )

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client

def parse_sse_data(stream_text: str) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line in stream_text.splitlines():
        if line.startswith("data: "):
            events.append(json.loads(line.removeprefix("data: ")))
    return events

async def create_project(chapters_session_factory, user_id: str, **overrides) -> Project:
    async with chapters_session_factory() as session:
        project = Project(
            user_id=user_id,
            title=overrides.get("title", "test-project"),
            genre=overrides.get("genre", "fantasy"),
            theme=overrides.get("theme", "adventure"),
            outline_mode=overrides.get("outline_mode", "one-to-many"),
            current_words=overrides.get("current_words", 0),
            narrative_perspective=overrides.get("narrative_perspective", "third_person"),
            default_creative_mode=overrides.get("default_creative_mode"),
            default_story_focus=overrides.get("default_story_focus"),
            default_plot_stage=overrides.get("default_plot_stage"),
            default_story_creation_brief=overrides.get("default_story_creation_brief"),
            default_quality_preset=overrides.get("default_quality_preset"),
            default_quality_notes=overrides.get("default_quality_notes"),
        )
        session.add(project)
        await session.commit()
        await session.refresh(project)
        return project

async def create_outline(
    chapters_session_factory,
    project_id: str,
    order_index: int = 1,
    title: str = "outline-1",
    content: str = "outline content",
) -> Outline:
    async with chapters_session_factory() as session:
        outline = Outline(
            project_id=project_id,
            title=title,
            content=content,
            order_index=order_index,
        )
        session.add(outline)
        await session.commit()
        await session.refresh(outline)
        return outline

async def create_chapter(
    chapters_session_factory,
    project_id: str,
    chapter_number: int,
    title: str,
    content: str | None = None,
    outline_id: str | None = None,
    status: str = "draft",
    expansion_plan: str | None = None,
) -> Chapter:
    async with chapters_session_factory() as session:
        chapter = Chapter(
            project_id=project_id,
            chapter_number=chapter_number,
            title=title,
            content=content,
            word_count=len(content) if content else 0,
            status=status,
            outline_id=outline_id,
            expansion_plan=expansion_plan,
        )
        session.add(chapter)
        await session.commit()
        await session.refresh(chapter)
        return chapter
