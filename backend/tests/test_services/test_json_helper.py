import json

from app.services.json_helper import clean_json_response, parse_json


def test_should_escape_invalid_control_chars_inside_json_strings():
    raw_response = '{"content": "第一行\n第二行\t第三列\r第四行", "scores": {"overall": 9}}'
    raw_response = raw_response.replace('\\n', '\n').replace('\\t', '\t').replace('\\r', '\r')

    cleaned_response = clean_json_response(raw_response)
    parsed = json.loads(cleaned_response)

    assert parsed["content"] == "第一行\n第二行\t第三列\r第四行"
    assert parsed["scores"]["overall"] == 9


def test_should_parse_markdown_wrapped_json_with_control_chars():
    raw_response = """```json
{
  \"summary\": \"段落A
段落B\",
  \"hooks\": [],
  \"plot_points\": [],
  \"scores\": {\"overall\": 8.5}
}
```"""

    parsed = parse_json(raw_response)

    assert parsed["summary"] == "段落A\n段落B"
    assert parsed["scores"]["overall"] == 8.5
