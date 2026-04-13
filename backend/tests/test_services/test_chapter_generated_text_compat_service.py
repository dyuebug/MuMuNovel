from app.services import chapter_generated_text_compat_service as compat_service


def test_should_detect_workflow_meta_text_via_compat_service():
    text = "step 1: draft conflict\nHe pushed the door open and stepped inside."
    assert compat_service.contains_chapter_workflow_meta_text(text) is True


def test_should_sanitize_generated_narrative_text_via_compat_service():
    raw_text = "\n".join([
        "step 1: describe conflict",
        "The wind outside kept rising, but he still lit the lamp.",
        "step 2: invoke agent",
        "She said nothing and folded the letter into a smaller square.",
    ])

    cleaned, removed_count = compat_service.sanitize_generated_narrative_text(raw_text)

    assert removed_count == 2
    assert "step 1" not in cleaned
    assert "step 2" not in cleaned
    assert "The wind outside kept rising" in cleaned
    assert "She said nothing" in cleaned
