from typing import Any

import pytest
from fastapi import FastAPI, Request
from httpx import ASGITransport, AsyncClient
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import polish as polish_api

pytestmark = pytest.mark.asyncio


class FakeAIService:
    def __init__(self, response: str):
        self.response = response
        self.calls: list[dict[str, Any]] = []

    async def generate_text(self, **kwargs):
        self.calls.append(kwargs)
        return self.response


def _build_polish_app(test_db: AsyncSession, ai_service: FakeAIService) -> FastAPI:
    app = FastAPI()
    app.include_router(polish_api.router, prefix="/api")

    async def override_get_db():
        yield test_db

    async def override_get_user_ai_service():
        return ai_service

    app.dependency_overrides[polish_api.get_db] = override_get_db
    app.dependency_overrides[polish_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = request.headers.get("x-test-user-id")
        return await call_next(request)

    return app


def _build_client(test_db: AsyncSession, ai_service: FakeAIService) -> AsyncClient:
    app = _build_polish_app(test_db, ai_service)
    return AsyncClient(transport=ASGITransport(app=app), base_url="http://testserver")


def _auth_headers(user_id: str) -> dict[str, str]:
    return {"x-test-user-id": user_id}


async def test_should_accept_legacy_text_field_and_build_runtime_controls(
    test_db,
    mock_user,
    mocker,
):
    ai_service = FakeAIService("润色后的文本")

    mocker.patch.object(
        polish_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(
            return_value=(
                "请润色以下文本：\n{original_text}\n{focus_instruction}\n"
                "{structure_instruction}\n{style_hint_block}"
            )
        ),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/polish",
            headers=_auth_headers(mock_user.user_id),
            json={
                "text": "原始文本",
                "style": "压低解释味，保留人物火气",
                "focus_mode": "dialogue",
                "preserve_paragraphs": True,
                "retain_hooks": True,
            },
        )

    assert response.status_code == 200
    body = response.json()
    assert body["original_text"] == "原始文本"
    assert body["polished_text"] == "润色后的文本"
    assert body["word_count_before"] == len("原始文本")
    assert ai_service.calls[0]["temperature"] == pytest.approx(0.8)
    assert "优先处理人物对白" in ai_service.calls[0]["prompt"]
    assert "压低解释味，保留人物火气" in ai_service.calls[0]["prompt"]


async def test_should_accept_object_body_for_polish_batch(test_db, mock_user, mocker):
    ai_service = FakeAIService("批量润色结果")

    mocker.patch.object(
        polish_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="{original_text}\n{focus_instruction}\n{structure_instruction}"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/polish/batch",
            headers=_auth_headers(mock_user.user_id),
            json={
                "texts": ["第一段", "第二段"],
                "focus_mode": "hook",
                "retain_hooks": False,
            },
        )

    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 2
    assert len(body["results"]) == 2
    assert ai_service.calls[0]["temperature"] == pytest.approx(0.8)
    assert "优先处理开场与结尾牵引" in ai_service.calls[0]["prompt"]
