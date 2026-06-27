import pytest
from types import SimpleNamespace

from tests.test_support.outlines_route_test_adapter import (
    AutoCharacterService,
    AutoOrganizationService,
)
from tests.test_support import outlines_route_test_adapter as outlines_api


pytestmark = pytest.mark.asyncio


async def _fake_get_template(*args, **kwargs):
    return "template"


def _fake_format_prompt(*args, **kwargs):
    return "prompt"


async def test_should_forward_enable_mcp_to_character_json_retry(monkeypatch):
    captured = {}

    class FakeAIService:
        async def call_with_json_retry(self, **kwargs):
            captured.update(kwargs)
            return {"name": "Lin", "relationships": []}

    monkeypatch.setattr(outlines_api, "get_template", _fake_get_template)
    monkeypatch.setattr(outlines_api, "format_prompt", _fake_format_prompt)

    service = AutoCharacterService(FakeAIService())

    async def fake_build_careers_info(*args, **kwargs):
        return ""

    monkeypatch.setattr(service, "_build_careers_info", fake_build_careers_info)

    project = SimpleNamespace(
        id="project-1",
        title="Test Project",
        genre=None,
        theme=None,
        world_time_period=None,
        world_location=None,
        world_atmosphere=None,
        world_rules=None,
    )

    await service._generate_character_details(
        spec={"name": "Lin"},
        project=project,
        existing_characters=[],
        db=object(),
        user_id="user-1",
        enable_mcp=False,
    )

    assert captured["auto_mcp"] is False


async def test_should_forward_enable_mcp_to_organization_json_retry(monkeypatch):
    captured = {}

    class FakeAIService:
        async def call_with_json_retry(self, **kwargs):
            captured.update(kwargs)
            return {"name": "Guild"}

    monkeypatch.setattr(outlines_api, "get_template", _fake_get_template)
    monkeypatch.setattr(outlines_api, "format_prompt", _fake_format_prompt)

    service = AutoOrganizationService(FakeAIService())

    project = SimpleNamespace(
        id="project-1",
        title="Test Project",
        genre=None,
        theme=None,
        world_time_period=None,
        world_location=None,
        world_atmosphere=None,
        world_rules=None,
    )

    await service._generate_organization_details(
        spec={"name": "Guild"},
        project=project,
        existing_characters=[],
        existing_organizations=[],
        db=object(),
        user_id="user-1",
        enable_mcp=False,
    )

    assert captured["auto_mcp"] is False
