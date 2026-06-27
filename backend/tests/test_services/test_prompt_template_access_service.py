import pytest
from sqlalchemy import text

from migrator_app.models import PromptTemplate
from tests.test_support.prompt_service_test_support import MCP_TOOL_TEST
from tests.test_support.prompt_template_facade_test_support import get_system_template_info
from tests.test_support.prompt_template_access_test_support import (
    get_template,
    get_template_with_fallback,
)
from tests.test_support.prompt_template_render_test_support import prepare_template_content


pytestmark = pytest.mark.asyncio


async def _prepare_prompt_template_table(test_db):
    await test_db.execute(
        text(
            """
            CREATE TABLE prompt_templates (
                id VARCHAR(36) PRIMARY KEY,
                user_id VARCHAR(50) NOT NULL,
                template_key VARCHAR(100) NOT NULL,
                template_name VARCHAR(200) NOT NULL,
                template_content TEXT NOT NULL,
                description TEXT,
                category VARCHAR(50),
                parameters TEXT,
                is_active BOOLEAN,
                is_system_default BOOLEAN,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
    )
    await test_db.commit()


async def test_should_return_system_template_when_user_or_db_missing():
    result = await get_template_with_fallback(
        template_key="MCP_TOOL_TEST",
        user_id=None,
        db=None,
        template_lookup=lambda key: globals().get(key),
        template_prepare=prepare_template_content,
        get_system_template_info=lambda key: get_system_template_info(
            template_key=key,
            template_lookup=lambda template_name: globals().get(template_name),
        ),
    )

    assert result == MCP_TOOL_TEST


async def test_should_prefer_active_custom_template(test_db):
    await _prepare_prompt_template_table(test_db)
    test_db.add(
        PromptTemplate(
            id="template-1",
            user_id="user-1",
            template_key="MCP_TOOL_TEST",
            template_name="Custom MCP",
            template_content="自定义模板正文",
            is_active=True,
            is_system_default=False,
        )
    )
    await test_db.commit()

    result = await get_template(
        template_key="MCP_TOOL_TEST",
        user_id="user-1",
        db=test_db,
        template_lookup=lambda key: globals().get(key),
        template_prepare=prepare_template_content,
        get_system_template_info=lambda key: get_system_template_info(
            template_key=key,
            template_lookup=lambda template_name: globals().get(template_name),
        ),
    )

    assert result.startswith('<prompt_template_key value="MCP_TOOL_TEST" />')
    assert result.endswith("自定义模板正文")


async def test_should_fallback_to_prepared_system_template_when_custom_missing(test_db):
    await _prepare_prompt_template_table(test_db)

    result = await get_template(
        template_key="MCP_TOOL_TEST",
        user_id="user-2",
        db=test_db,
        template_lookup=lambda key: globals().get(key),
        template_prepare=prepare_template_content,
        get_system_template_info=lambda key: get_system_template_info(
            template_key=key,
            template_lookup=lambda template_name: globals().get(template_name),
        ),
    )

    assert result.startswith('<prompt_template_key value="MCP_TOOL_TEST" />')
    assert "你是MCP插件测试助手" in result
