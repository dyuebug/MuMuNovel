import pytest

from tests.test_support import character_organization_stream_entry_test_support as stream_entry


pytestmark = pytest.mark.asyncio


async def test_should_resolve_character_organization_template_from_module_level_owner_surface(
    monkeypatch,
):
    captured = {}

    async def fake_get_template(template_key, user_id, db):
        captured["args"] = (template_key, user_id, db)
        return "FAKE_TEMPLATE"

    monkeypatch.setattr(stream_entry, "get_template", fake_get_template)

    result = await stream_entry._get_character_organization_template(
        "SINGLE_CHARACTER_GENERATION",
        "user-1",
        object(),
    )

    assert result == "FAKE_TEMPLATE"
    assert captured["args"][0] == "SINGLE_CHARACTER_GENERATION"
    assert captured["args"][1] == "user-1"


async def test_should_resolve_character_organization_format_prompt_from_module_level_owner_surface(
    monkeypatch,
):
    captured = {}

    def fake_format_prompt(template, **kwargs):
        captured["template"] = template
        captured["kwargs"] = kwargs
        return "FORMATTED_PROMPT"

    monkeypatch.setattr(stream_entry, "format_prompt", fake_format_prompt)

    result = stream_entry._format_character_organization_prompt(
        "RAW_TEMPLATE",
        name="seed name",
        role_type="supporting",
    )

    assert result == "FORMATTED_PROMPT"
    assert captured["template"] == "RAW_TEMPLATE"
    assert captured["kwargs"]["name"] == "seed name"
    assert captured["kwargs"]["role_type"] == "supporting"
