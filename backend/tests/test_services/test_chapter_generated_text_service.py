from app.services.chapter_generated_text_service import trim_text_to_sentence_boundary


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
