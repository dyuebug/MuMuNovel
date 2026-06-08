from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
    trim_text_to_sentence_boundary,
)


def test_should_trim_to_recent_sentence_boundary_within_lookback_window():
    text = "\u7b2c\u4e00\u53e5\u94fa\u57ab\u3002\u7b2c\u4e8c\u53e5\u8f6c\u6298\u3002\u7b2c\u4e09\u53e5\u5c55\u5f00\u3002"

    trimmed = trim_text_to_sentence_boundary(text, hard_limit=10)

    assert trimmed == "\u7b2c\u4e00\u53e5\u94fa\u57ab\u3002"


def test_should_append_sentence_boundary_when_no_boundary_is_found():
    text = "abcdefghijklmn"

    trimmed = trim_text_to_sentence_boundary(text, hard_limit=5)

    assert trimmed == "abcde\u3002"


def test_should_return_original_text_when_under_limit():
    text = "\u77ed\u53e5\u3002"

    trimmed = trim_text_to_sentence_boundary(text, hard_limit=20)

    assert trimmed == "\u77ed\u53e5\u3002"


def test_should_detect_workflow_meta_text():
    text = "step 1: draft conflict\nHe pushed the door open and stepped inside."
    assert contains_chapter_workflow_meta_text(text) is True


def test_should_sanitize_generated_narrative_text():
    raw_text = "\n".join([
        "step 1: describe conflict",
        "The wind outside kept rising, but he still lit the lamp.",
        "step 2: invoke agent",
        "She said nothing and folded the letter into a smaller square.",
    ])

    cleaned, removed_count = sanitize_generated_narrative_text(raw_text)

    assert removed_count == 2
    assert "step 1" not in cleaned
    assert "step 2" not in cleaned
    assert "The wind outside kept rising" in cleaned
    assert "She said nothing" in cleaned
