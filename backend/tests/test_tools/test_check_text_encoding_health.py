from tools.check_text_encoding_health import (
    DOCUMENTATION_ROOT,
    REPO_ROOT,
    detect_reasons,
    resolve_roots,
    strip_safe_qmark_contexts,
)


def test_should_flag_plain_qmark_runs_in_normal_text():
    suspicious_qmarks = "?" * 14
    reasons = detect_reasons(suspicious_qmarks, strict_qmark=False)

    assert "qmark" in reasons


def test_should_not_flag_qmark_runs_inside_regex_literals():
    line = 'pattern = re.compile(r"(^|[\u3002\uff01])([?"\'\u201c\u201d(]*)")'

    reasons = detect_reasons(line, strict_qmark=False)

    assert "qmark" not in reasons


def test_should_not_flag_question_marks_in_url_query_string():
    line = 'url = f"https://example.com/search?q=test&lang=zh"'

    reasons = detect_reasons(line, strict_qmark=True)

    assert "qmark" not in reasons


def test_should_strip_regex_and_url_before_qmark_detection():
    regex_qmarks = "?" * 4
    line = f'url = "https://example.com?a=1"; pattern = re.split(r"[{regex_qmarks}]+", text)'

    sanitized = strip_safe_qmark_contexts(line)

    assert "https://example.com" not in sanitized
    assert regex_qmarks not in sanitized


def test_should_not_flag_nullish_coalescing_operator():
    line = 'const title = value ?? fallback'

    reasons = detect_reasons(line, strict_qmark=True)

    assert "qmark" not in reasons


def test_should_use_only_explicit_roots_when_root_is_provided():
    roots = resolve_roots(["backend/app/main.py"])

    assert roots == [(REPO_ROOT / "backend/app/main.py").resolve()]


def test_should_exclude_docs_from_default_roots():
    roots = resolve_roots([])

    assert (REPO_ROOT / DOCUMENTATION_ROOT).resolve() not in roots


def test_should_include_docs_when_requested():
    roots = resolve_roots([], include_docs=True)

    assert (REPO_ROOT / DOCUMENTATION_ROOT).resolve() in roots
