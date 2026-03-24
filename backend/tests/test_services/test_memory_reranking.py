from app.services.memory_ranking import rank_memories_for_generation


def test_should_prioritize_recent_character_memory_for_generation():
    memories = [
        {
            "id": "generic-world-detail",
            "content": "古城的阵法依然稳定运转，没有新的异常。",
            "metadata": {
                "memory_type": "world_detail",
                "chapter_number": 4,
                "importance": 0.35,
                "title": "古城阵法",
            },
            "similarity": 0.9,
        },
        {
            "id": "focused-character-event",
            "content": "苏离在议会背叛后不再轻信任何承诺，并开始主动试探盟友。",
            "metadata": {
                "memory_type": "character_event",
                "chapter_number": 19,
                "importance": 0.92,
                "title": "苏离的变化",
                "related_characters": ["苏离"],
            },
            "similarity": 0.74,
        },
    ]

    ranked = rank_memories_for_generation(
        memories=memories,
        current_chapter=20,
        preferred_types=["character_event", "plot_point"],
        related_names=["苏离"],
        limit=5,
    )

    assert [memory["id"] for memory in ranked][:2] == [
        "focused-character-event",
        "generic-world-detail",
    ]
    assert ranked[0]["ranking_score"] > ranked[1]["ranking_score"]


def test_should_dedupe_duplicate_memories_after_rerank():
    memories = [
        {
            "content": "旧城区的火灾实际上是诱敌计。",
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 8,
                "importance": 0.8,
                "title": "旧城区火灾",
            },
            "similarity": 0.82,
        },
        {
            "content": "旧城区的火灾实际上是诱敌计。",
            "metadata": {
                "memory_type": "plot_point",
                "chapter_number": 8,
                "importance": 0.8,
                "title": "旧城区火灾",
            },
            "similarity": 0.8,
        },
    ]

    ranked = rank_memories_for_generation(memories=memories, current_chapter=10, limit=5)

    assert len(ranked) == 1
    assert ranked[0]["metadata"]["title"] == "旧城区火灾"
