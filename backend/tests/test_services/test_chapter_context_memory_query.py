from unittest.mock import AsyncMock

import pytest

from app.services.chapter_context_service import (
    OneToManyContextBuilder,
    build_memory_query_text,
)


def test_should_build_memory_query_text_with_mixed_continuity_hints():
    query = build_memory_query_text(
        "苏离潜入码头调查暗影公会，并准备回收遗失的钥匙。",
        related_names=["苏离", "林澈"],
        character_arc_snapshot="【角色弧光快照】\n- 苏离：当前状态（第12章更新）因议会背叛而变得警惕克制",
        foreshadow_reminders="【伏笔提醒】\n- 遗失钥匙：必须在三章内回收",
        chapter_careers="【职业体系】\n- 苏离 / 策士：第3阶，擅长调度与反制",
        chapter_characters="苏离\n  关系网络: 与林澈：互信受损\n  组织归属: 暗影公会（外勤）",
    )

    assert "本章大纲：苏离潜入码头调查暗影公会" in query
    assert "角色焦点：苏离、林澈" in query
    assert "角色状态：苏离：当前状态" in query
    assert "伏笔线索：遗失钥匙：必须在三章内回收" in query
    assert "职业线索：苏离 / 策士：第3阶" in query
    assert "关系与组织：关系网络: 与林澈：互信受损；组织归属: 暗影公会（外勤）" in query


@pytest.mark.asyncio
async def test_should_query_memories_with_mixed_context_hint():
    memory_service = AsyncMock()
    memory_service.search_memories.return_value = [
        {
            "id": "memory-1",
            "content": "苏离曾在码头丢失钥匙，并因此暴露了暗线身份。",
            "similarity": 0.91,
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 11,
                "related_characters": ["苏离"],
                "importance_score": 0.88,
            },
        }
    ]
    service = OneToManyContextBuilder(memory_service=memory_service)

    result = await service._get_relevant_memories_enhanced(
        user_id="user-1",
        project_id="project-1",
        chapter_number=12,
        chapter_outline="苏离潜入码头调查暗影公会，并准备回收遗失的钥匙。",
        db=None,
        related_names=["苏离", "林澈"],
        character_arc_snapshot="【角色弧光快照】\n- 苏离：当前状态（第12章更新）因议会背叛而变得警惕克制",
        foreshadow_reminders="【伏笔提醒】\n- 遗失钥匙：必须在三章内回收",
        chapter_careers="【职业体系】\n- 苏离 / 策士：第3阶，擅长调度与反制",
        chapter_characters="苏离\n  关系网络: 与林澈：互信受损\n  组织归属: 暗影公会（外勤）",
    )

    assert result is not None
    assert "【相关记忆】" in result
    assert "苏离曾在码头丢失钥匙" in result

    called_query = memory_service.search_memories.await_args.kwargs["query"]
    assert "角色焦点：苏离、林澈" in called_query
    assert "伏笔线索：遗失钥匙：必须在三章内回收" in called_query
    assert "关系与组织：关系网络: 与林澈：互信受损" in called_query


@pytest.mark.asyncio
@pytest.mark.asyncio
async def test_should_balance_memory_type_coverage_and_deduplicate_dense_results():
    memory_service = AsyncMock()
    dense_plot = "苏离在码头丢失钥匙并暴露暗线身份，这让后续回收钥匙成为主线任务。"
    memory_service.search_memories.return_value = [
        {
            "id": "plot-1",
            "content": dense_plot,
            "similarity": 0.95,
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 11,
                "related_characters": ["苏离"],
                "importance_score": 0.91,
            },
        },
        {
            "id": "plot-duplicate",
            "content": dense_plot,
            "similarity": 0.94,
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 11,
                "related_characters": ["苏离"],
                "importance_score": 0.89,
            },
        },
        {
            "id": "character-1",
            "content": "林澈因为误判导致苏离受伤，两人的互信明显下降。",
            "similarity": 0.82,
            "metadata": {
                "memory_type": "character_event",
                "chapter_number": 10,
                "related_characters": ["林澈", "苏离"],
                "importance_score": 0.79,
            },
        },
        {
            "id": "hook-1",
            "content": "港口巡夜人曾提到第十二章会出现一把假钥匙，这是追读钩子。",
            "similarity": 0.78,
            "metadata": {
                "memory_type": "hook",
                "chapter_number": 9,
                "related_characters": ["苏离"],
                "importance_score": 0.73,
            },
        },
        {
            "id": "world-1",
            "content": "北码头实行双层盘查制度，外勤成员必须携带蓝铜徽章才能进入内仓。",
            "similarity": 0.76,
            "metadata": {
                "memory_type": "world_detail",
                "chapter_number": 8,
                "related_characters": ["苏离"],
                "importance_score": 0.70,
            },
        },
    ]
    service = OneToManyContextBuilder(memory_service=memory_service)

    result = await service._get_relevant_memories_enhanced(
        user_id="user-1",
        project_id="project-1",
        chapter_number=12,
        chapter_outline="苏离重返码头，准备找回钥匙并确认林澈是否仍然可信。",
        db=None,
        related_names=["苏离", "林澈"],
    )

    assert result is not None
    assert result.count(dense_plot[:18]) == 1
    assert "(情节事件 / 相关度:" in result
    assert "(人物事件 / 相关度:" in result
    assert "(钩子 / 相关度:" in result
    assert "(世界细节 / 相关度:" in result
    assert len(result) <= OneToManyContextBuilder.MEMORY_CHAR_BUDGET + 32


@pytest.mark.asyncio
async def test_should_budget_and_diversify_relevant_memories_for_prompt():
    memory_service = AsyncMock()
    memory_service.search_memories.return_value = [
        {
            "id": "memory-plot-primary",
            "content": "苏离在码头丢失钥匙并暴露暗线身份，这让回收钥匙成为主线任务。",
            "similarity": 0.94,
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 11,
                "importance_score": 0.92,
            },
        },
        {
            "id": "memory-plot-duplicate",
            "content": "苏离在码头丢失钥匙并暴露暗线身份，这让回收钥匙成为主线任务。",
            "similarity": 0.93,
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 10,
                "importance_score": 0.9,
            },
        },
        {
            "id": "memory-hook",
            "content": "港口巡夜人提到第十二章会出现一把假钥匙。",
            "similarity": 0.9,
            "metadata": {
                "memory_type": "hook",
                "chapter_number": 11,
                "importance_score": 0.83,
            },
        },
        {
            "id": "memory-dialogue",
            "content": "林澈在对话里故意试探苏离是否还愿意继续合作。",
            "similarity": 0.87,
            "metadata": {
                "memory_type": "dialogue",
                "chapter_number": 9,
                "importance_score": 0.8,
            },
        },
        {
            "id": "memory-world",
            "content": "北码头实行双层盘查制度，外勤成员必须携带蓝铜徽章。",
            "similarity": 0.84,
            "metadata": {
                "memory_type": "world_detail",
                "chapter_number": 8,
                "importance_score": 0.75,
            },
        },
        {
            "id": "memory-character",
            "content": "林澈因为误判导致苏离受伤，两人的互信明显下降。",
            "similarity": 0.82,
            "metadata": {
                "memory_type": "character_event",
                "chapter_number": 10,
                "importance_score": 0.78,
            },
        },
        {
            "id": "memory-low-similarity",
            "content": "街市里一段无关紧要的闲谈。",
            "similarity": 0.32,
            "metadata": {
                "memory_type": "scene",
                "chapter_number": 3,
                "importance_score": 0.2,
            },
        },
    ]
    service = OneToManyContextBuilder(memory_service=memory_service)
    service.MEMORY_COUNT = 4
    service.MEMORY_TOTAL_CHARS_BUDGET = 420
    service.MEMORY_PREVIEW_LENGTH = 60

    result = await service._get_relevant_memories_enhanced(
        user_id="user-1",
        project_id="project-1",
        chapter_number=12,
        chapter_outline="苏离重返码头，准备找回钥匙并确认林澈是否仍然可信。",
        db=None,
        related_names=["苏离", "林澈"],
    )

    assert result is not None
    assert result.count("苏离在码头丢失钥匙") == 1
    assert "林澈因为误判导致苏离受伤" in result
    assert "港口巡夜人提到第十二章会出现一把假钥匙" in result
    assert "街市里一段无关紧要的闲谈" not in result
