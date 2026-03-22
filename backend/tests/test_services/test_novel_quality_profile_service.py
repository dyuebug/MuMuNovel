from app.services.novel_quality_profile_service import novel_quality_profile_service


def test_should_include_new_generation_dimensions_in_quality_profile():
    profile = novel_quality_profile_service.build_profile(
        payload={"genre": "悬疑", "style_name": "默认"}
    )

    assert "viewpoint_discipline" in profile.quality_dimensions
    assert "scene_anchoring" in profile.quality_dimensions
    assert "information_release" in profile.quality_dimensions
    assert "emotion_landing" in profile.quality_dimensions
    assert "action_rendering" in profile.quality_dimensions
    assert "summary_tone_control" in profile.quality_dimensions
    assert "repetition_control" in profile.quality_dimensions
    assert "voice_separation" in profile.quality_dimensions
    assert "paragraph_rhythm" in profile.quality_dimensions
    assert "[视角纪律]" in profile.prompt_blocks.generation
    assert "[场景锚定]" in profile.prompt_blocks.generation
    assert "[信息投放]" in profile.prompt_blocks.generation
    assert "[情绪落点]" in profile.prompt_blocks.generation
    assert "[动作显影]" in profile.prompt_blocks.generation
    assert "[总结腔抑制]" in profile.prompt_blocks.generation
    assert "[重复压缩]" in profile.prompt_blocks.generation
    assert "[口吻分离]" in profile.prompt_blocks.generation
    assert "[段落节奏]" in profile.prompt_blocks.generation


def test_should_expose_new_checker_categories_in_quality_profile():
    profile = novel_quality_profile_service.build_profile()

    assert "视角纪律" in profile.prompt_blocks.checker
    assert "场景锚定" in profile.prompt_blocks.checker
    assert "信息投放" in profile.prompt_blocks.checker
    assert "情绪落点" in profile.prompt_blocks.checker
    assert "动作显影" in profile.prompt_blocks.checker
    assert "总结腔抑制" in profile.prompt_blocks.checker
    assert "重复压缩" in profile.prompt_blocks.checker
    assert "口吻分离" in profile.prompt_blocks.checker
    assert "段落节奏" in profile.prompt_blocks.checker
