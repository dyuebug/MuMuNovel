import json
from typing import Any

import pytest
from fastapi import FastAPI, Request
from httpx import ASGITransport, AsyncClient
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import inspiration as inspiration_api

pytestmark = pytest.mark.asyncio


class FakeAIService:
    """用于测试的 AIService 替身，支持按调用顺序返回流式文本。"""

    def __init__(self, responses: list[str]):
        self.responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(kwargs)
        content = self.responses.pop(0) if self.responses else ""
        chunk_size = 16
        for index in range(0, len(content), chunk_size):
            yield content[index : index + chunk_size]

    def _clean_json_response(self, content: str) -> str:
        return content


def _build_inspiration_app(test_db: AsyncSession, ai_service: FakeAIService) -> FastAPI:
    app = FastAPI()
    app.include_router(inspiration_api.router, prefix="/api")

    async def override_get_db():
        yield test_db

    async def override_get_user_ai_service():
        return ai_service

    app.dependency_overrides[inspiration_api.get_db] = override_get_db
    app.dependency_overrides[inspiration_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = request.headers.get("x-test-user-id")
        return await call_next(request)

    return app


def _build_client(test_db: AsyncSession, ai_service: FakeAIService) -> AsyncClient:
    app = _build_inspiration_app(test_db, ai_service)
    return AsyncClient(transport=ASGITransport(app=app), base_url="http://testserver")


def _auth_headers(user_id: str) -> dict[str, str]:
    return {"x-test-user-id": user_id}


async def test_should_generate_options_when_request_is_valid(test_db, mock_user, mocker):
    ai_payload = {
        "prompt": "请选择一个你最喜欢的标题",
        "options": ["风起云涌", "群星回响", "旧城新梦"],
    }
    ai_service = FakeAIService([json.dumps(ai_payload, ensure_ascii=False)])

    async def fake_get_template(template_key: str, user_id: str, db: AsyncSession) -> str:
        templates = {
            "INSPIRATION_TITLE_SYSTEM": "系统模板：{initial_idea}",
            "INSPIRATION_TITLE_USER": "用户模板：{initial_idea}",
        }
        return templates[template_key]

    get_template_mock = mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(side_effect=fake_get_template),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/generate-options",
            headers=_auth_headers(mock_user.user_id),
            json={"step": "title", "context": {"initial_idea": "赛博朋克与成长"}},
        )

    assert response.status_code == 200
    body = response.json()
    assert body["options"] == ai_payload["options"]
    assert body["prompt"] == ai_payload["prompt"]
    assert get_template_mock.await_count == 2
    assert ai_service.calls[0]["temperature"] == inspiration_api.TEMPERATURE_SETTINGS["title"]
    assert "风格与可读性要求" in ai_service.calls[0]["system_prompt"]
    assert "书名专项" in ai_service.calls[0]["system_prompt"]


async def test_should_return_error_when_generate_options_step_is_unsupported(
    test_db,
    mock_user,
    mocker,
):
    ai_service = FakeAIService(['{"options": ["A", "B", "C"]}'])
    get_template_mock = mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="无效步骤不应走到模板查询"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/generate-options",
            headers=_auth_headers(mock_user.user_id),
            json={"step": "not-supported", "context": {}},
        )

    assert response.status_code == 200
    body = response.json()
    assert "error" in body
    assert body["options"] == []
    assert ai_service.calls == []
    get_template_mock.assert_not_awaited()


async def test_should_retry_and_return_fallback_when_generate_options_json_invalid(
    test_db,
    mock_user,
    mocker,
):
    ai_service = FakeAIService(["not-json-1", "not-json-2", "not-json-3"])

    mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="模板：{initial_idea}{title}{description}{theme}"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/generate-options",
            headers=_auth_headers(mock_user.user_id),
            json={"step": "theme", "context": {"title": "失落之城"}},
        )

    assert response.status_code == 200
    body = response.json()
    assert "error" in body
    assert len(body["options"]) == 2
    assert len(ai_service.calls) == 3


async def test_should_refine_options_when_feedback_is_provided(test_db, mock_user, mocker):
    ai_payload = {
        "options": [
            "她越想救回家人，命运越逼她亲手选一个人失去，连最珍惜的那段亲情都开始变成交换筹码。",
            "真相每靠近一步，她都得先割舍一段最舍不得的关系，到最后连活下去都像是在替别人还债。",
            "这不是谁被拯救的问题，而是谁先承认自己也在伤人，谁又愿意为迟来的清醒付出真正代价。",
        ],
        "prompt": "选择更贴合反馈的主题",
    }
    ai_service = FakeAIService([json.dumps(ai_payload, ensure_ascii=False)])

    async def fake_get_template(template_key: str, user_id: str, db: AsyncSession) -> str:
        templates = {
            "INSPIRATION_THEME_SYSTEM": "系统：{title}-{description}-{theme}",
            "INSPIRATION_THEME_USER": "用户：{initial_idea}",
        }
        return templates[template_key]

    mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(side_effect=fake_get_template),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/refine-options",
            headers=_auth_headers(mock_user.user_id),
            json={
                "step": "theme",
                "context": {
                    "initial_idea": "悬疑与家庭矛盾",
                    "title": "暗夜回廊",
                    "description": "主角回到旧城调查失踪案",
                    "theme": "救赎",
                },
                "feedback": "想要更有悲剧张力",
                "previous_options": ["旧选项A", "旧选项B"],
            },
        )

    assert response.status_code == 200
    body = response.json()
    assert body["options"] == ai_payload["options"]

    call_kwargs = ai_service.calls[0]
    assert call_kwargs["temperature"] == pytest.approx(
        min(inspiration_api.TEMPERATURE_SETTINGS["theme"] + 0.1, 0.9)
    )
    assert "想要更有悲剧张力" in call_kwargs["system_prompt"]
    assert "旧选项A" in call_kwargs["system_prompt"]


async def test_should_quick_generate_and_keep_user_fields_priority(test_db, mock_user, mocker):
    ai_payload = {
        "title": "AI标题",
        "description": "AI补全简介",
        "theme": "AI补全主题",
        "genre": ["悬疑", "都市"],
        "narrative_perspective": "第一人称",
    }
    ai_service = FakeAIService([json.dumps(ai_payload, ensure_ascii=False)])

    mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="请补全以下内容：{existing}"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/quick-generate",
            headers=_auth_headers(mock_user.user_id),
            json={"title": "用户已输入标题"},
        )

    assert response.status_code == 200
    body = response.json()
    assert body["title"] == "用户已输入标题"
    assert body["description"] == "AI补全简介"
    assert body["theme"] == "AI补全主题"
    assert body["genre"] == ["悬疑", "都市"]
    assert body["narrative_perspective"] == "第一人称"
    assert ai_service.calls[0]["temperature"] == pytest.approx(0.78)
    assert "用户已输入标题" in ai_service.calls[0]["system_prompt"]


async def test_should_normalize_quick_generate_genre_and_keep_user_perspective(
    test_db,
    mock_user,
    mocker,
):
    ai_payload = {
        "title": "雨夜追凶",
        "description": "她本想查一桩旧案，却被新的命案拖进更深的局。",
        "theme": "真相从来不是最先该付出的代价。",
        "genre": "悬疑,都市,女性成长",
        "narrative_perspective": "omniscient",
    }
    ai_service = FakeAIService([json.dumps(ai_payload, ensure_ascii=False)])

    mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="请补全以下内容：{existing}"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/quick-generate",
            headers=_auth_headers(mock_user.user_id),
            json={
                "description": "她回到旧城查妹妹失踪案。",
                "genre": "悬疑, 都市 / 女性成长",
                "narrative_perspective": "第三人称",
            },
        )

    assert response.status_code == 200
    body = response.json()
    assert body["genre"] == ["悬疑", "都市", "女性成长"]
    assert body["narrative_perspective"] == "第三人称"


async def test_should_return_error_when_quick_generate_response_is_not_json(
    test_db,
    mock_user,
    mocker,
):
    ai_service = FakeAIService(["non-json-content"])

    mocker.patch.object(
        inspiration_api.PromptService,
        "get_template",
        new=mocker.AsyncMock(return_value="请补全：{existing}"),
    )

    async with _build_client(test_db, ai_service) as client:
        response = await client.post(
            "/api/inspiration/quick-generate",
            headers=_auth_headers(mock_user.user_id),
            json={"description": "已有简介"},
        )

    assert response.status_code == 200
    body = response.json()
    assert "error" in body
    assert isinstance(body["error"], str)
    assert body["error"]


@pytest.mark.parametrize(
    ("result", "step"),
    [
        ({}, "title"),
        ({"options": "not-list"}, "title"),
        ({"options": ["只给一个"]}, "title"),
        ({"options": ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"]}, "title"),
        ({"options": ["ok", "", "ok2"]}, "title"),
        ({"options": ["ok", 123, "ok2"]}, "title"),
        ({"options": ["ok", "b" * 501, "ok2"]}, "title"),
        ({"options": ["超长类型标签超过十个字", "悬疑", "都市"]}, "genre"),
    ],
)
def test_should_return_invalid_when_validate_options_response_input_illegal(
    result,
    step,
):
    is_valid, error_msg = inspiration_api.validate_options_response(result, step)
    assert is_valid is False
    assert error_msg


def test_should_return_valid_when_validate_options_response_input_is_valid():
    result = {"options": ["暗潮", "迷雾", "黎明"]}

    is_valid, error_msg = inspiration_api.validate_options_response(result, "title")

    assert is_valid is True
    assert error_msg == ""


def test_should_return_invalid_when_validate_options_response_has_duplicate_options():
    result = {"options": ["暗潮", "暗潮", "黎明"]}

    is_valid, error_msg = inspiration_api.validate_options_response(result, "title")

    assert is_valid is False
    assert "重复" in error_msg


def test_should_return_invalid_when_description_has_unexplained_jargon():
    result = {
        "options": [
            "主角要找到失踪妹妹，却被门影、回灌和锚点困住，城市规则不断重置，所有线索都在倒着追人，连每次求助都像触发另一套陌生协议。",
            "他在旧城翻案，镜像协议和阵列回响牵出更大阴谋，却没人愿意用人话解释这些术语到底意味着什么，所有人都默认你懂这些黑话。",
            "她决定破局，直面家族谎言，却发现位面裂隙和法则回潮正在把整座城拖进失控边缘，甚至连最普通的街道都像被重写过一次。"
        ]
    }

    is_valid, error_msg = inspiration_api.validate_options_response(result, "description")

    assert is_valid is False
    assert "术语密度过高" in error_msg
