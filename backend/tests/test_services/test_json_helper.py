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


def test_should_clean_analysis_json_with_quality_blocks_and_trailing_text():
    raw_response = """剧情分析如下，请直接入库：
```json
{
  "hooks": [
    {
      "type": "悬念",
      "content": "主角发现玉佩裂痕扩散",
      "strength": 8,
      "position": "开头",
      "keyword": "玉佩裂痕一路蔓延"
    }
  ],
  "serial_rhythm": {
    "opening_hook": {
      "present": true,
      "strength": 8,
      "type": "异常变化",
      "keyword": "玉佩裂痕一路蔓延",
      "assessment": "前段直接进入异常"
    },
    "payoff_chain": {
      "present": true,
      "strength": 7,
      "stages": ["铺垫", "爆发", "反馈"],
      "keyword": "他一掌震碎石门",
      "assessment": "小爽点闭环完整"
    },
    "ending_cliffhanger": {
      "present": false,
      "strength": 3,
      "type": "无",
      "keyword": null,
      "assessment": "结尾牵引偏弱"
    }
  },
  "foreshadows": [],
  "conflict": {
    "types": ["人与人"],
    "parties": ["林玄-守秘", "执法堂-追查"],
    "level": 7,
    "description": "追查逼近主角底牌",
    "resolution_progress": 0.2
  },
  "emotional_arc": {
    "primary_emotion": "紧张",
    "intensity": 8,
    "curve": "压抑→爆发→警觉",
    "secondary_emotions": ["警惕"]
  },
  "character_states": [],
  "plot_points": [
    {
      "content": "林玄确认内门有人泄密",
      "type": "revelation",
      "importance": 0.9,
      "impact": "把调查升级为宗门内斗",
      "keyword": "内门里有人泄了口风"
    }
  ],
  "scenes": [],
  "organization_states": [],
  "pacing": "varied",
  "dialogue_ratio": 0.35,
  "description_ratio": 0.25,
  "scores": {
    "pacing": 7.2,
    "engagement": 7.8,
    "coherence": 7.4,
    "overall": 7.5,
    "score_justification": "节奏和钩子有效，但章尾牵引仍有提升空间"
  },
  "plot_stage": "发展",
  "suggestions": [
    "【章尾牵引】建议让执法堂在最后一段直接堵门，强化追读冲动"
  ]
}
```
以上是最终答案。"""

    parsed = parse_json(raw_response)

    assert parsed["plot_stage"] == "发展"
    assert parsed["scores"]["overall"] == 7.5
    assert parsed["serial_rhythm"]["opening_hook"]["present"] is True
    assert parsed["suggestions"][0].startswith("【章尾牵引】")


def test_should_parse_checker_json_with_unescaped_newlines_in_evidence_and_suggestion():
    raw_response = """```json
{
  "overall_assessment": "一般",
  "severity_counts": {
    "critical": 1,
    "major": 1,
    "minor": 0
  },
  "issues": [
    {
      "severity": "critical",
      "category": "逻辑连贯",
      "location": "第3段到第4段",
      "evidence": "上一段还写他丹田受损
下一段却直接御剑冲阵",
      "impact": "角色能力边界失真，读者会出戏",
      "suggestion": "补一小段说明他是借了外力
或把御剑改成勉强奔袭"
    },
    {
      "severity": "major",
      "category": "章尾牵引",
      "location": "结尾段",
      "evidence": "风停了，众人各自散去",
      "impact": "章尾缺少追读动力",
      "suggestion": "在最后补一个危险逼近或信息缺口"
    }
  ],
  "priority_actions": [
    "先修正丹田受损后的行动逻辑",
    "强化章尾卡点"
  ],
  "revision_suggestions": [
    "让冲阵依赖阵盘或同伴托举",
    "结尾增加执法堂追兵现身"
  ],
  "serial_rhythm_assessment": {
    "opening_hook_ok": true,
    "payoff_chain_ok": true,
    "ending_cliffhanger_ok": false
  }
}
```"""

    parsed = parse_json(raw_response)

    assert parsed["severity_counts"] == {"critical": 1, "major": 1, "minor": 0}
    assert parsed["issues"][0]["evidence"] == "上一段还写他丹田受损\n下一段却直接御剑冲阵"
    assert parsed["issues"][0]["suggestion"] == "补一小段说明他是借了外力\n或把御剑改成勉强奔袭"
    assert parsed["serial_rhythm_assessment"]["ending_cliffhanger_ok"] is False


def test_should_parse_reviser_json_with_full_text_and_fusion_tag_lines():
    raw_response = """以下为自动修订草稿：
{
  "revised_text": "林玄把裂开的玉佩按回胸口。\n夜风从断墙间灌进来，像有人贴着耳骨吹气。\n他没有再退，反手扣紧阵盘，逼自己沿着血迹一步步走向石门。\n门后忽然传来脚步声，他听见执法堂弟子低声报数，才知道今晚真正的围杀刚刚开始。\n模板追踪标签：rule_v3_fusion_20260303",
  "applied_issues": [
    "补上丹田受损后仍行动的外力依托",
    "把章尾改成追兵堵门"
  ],
  "unresolved_issues": [
    "师门关系线索仍偏少，需要下章补强"
  ],
  "change_summary": "优先修复行动逻辑并强化章尾追读牵引"
}
附：无需解释。"""

    cleaned = clean_json_response(raw_response)
    parsed = json.loads(cleaned)

    assert parsed["applied_issues"] == ["补上丹田受损后仍行动的外力依托", "把章尾改成追兵堵门"]
    assert parsed["unresolved_issues"] == ["师门关系线索仍偏少，需要下章补强"]
    assert "模板追踪标签：rule_v3_fusion_20260303" in parsed["revised_text"]
    assert parsed["change_summary"] == "优先修复行动逻辑并强化章尾追读牵引"


def test_should_skip_preface_json_and_parse_analysis_payload():
    raw_response = """返回格式说明：
{"schema": "chapter_analysis", "version": "v2"}

最终结果如下：
```json
{
  "plot_stage": "高潮前夜",
  "scores": {
    "overall": 8.1,
    "score_justification": "追兵压境与内应暴露形成双重拉力"
  },
  "serial_rhythm": {
    "opening_hook": {
      "present": true,
      "strength": 8,
      "type": "危机逼近",
      "keyword": "执法堂已封锁山门",
      "assessment": "开场立即抬高风险"
    },
    "payoff_chain": {
      "present": true,
      "strength": 8,
      "stages": ["预警", "碰撞", "反噬"],
      "keyword": "阵盘反震",
      "assessment": "冲突递进明确"
    },
    "ending_cliffhanger": {
      "present": true,
      "strength": 9,
      "type": "身份暴露",
      "keyword": "内应的名字被当众喊出",
      "assessment": "章尾追读牵引明显"
    }
  },
  "suggestions": [
    "保留章尾点名内应的瞬间，下一章直接承接围杀"
  ]
}
```
"""

    parsed = parse_json(raw_response)

    assert parsed["plot_stage"] == "高潮前夜"
    assert parsed["scores"]["overall"] == 8.1
    assert parsed["serial_rhythm"]["ending_cliffhanger"]["present"] is True
    assert parsed["suggestions"] == ["保留章尾点名内应的瞬间，下一章直接承接围杀"]


def test_should_skip_preface_array_and_parse_checker_payload():
    raw_response = """问题分类枚举：
["critical", "major", "minor"]

请以此为准，以下才是检查结果：
{
  "overall_assessment": "需优先修正逻辑",
  "severity_counts": {
    "critical": 1,
    "major": 0,
    "minor": 1
  },
  "issues": [
    {
      "severity": "critical",
      "category": "逻辑连贯",
      "location": "第5段",
      "evidence": "他刚说灵力枯竭\n转眼又连续催动三次法诀",
      "impact": "能力边界失真",
      "suggestion": "补足外力来源或降低动作强度"
    },
    {
      "severity": "minor",
      "category": "节奏",
      "location": "中段",
      "evidence": "解释段略长",
      "impact": "局部拖慢推进",
      "suggestion": "压缩背景说明"
    }
  ],
  "priority_actions": [
    "先修复灵力耗尽后的行动依据"
  ],
  "revision_suggestions": [
    "让阵盘短暂代偿主角消耗"
  ]
}
注：无需补充说明。"""

    parsed = parse_json(raw_response)

    assert parsed["overall_assessment"] == "需优先修正逻辑"
    assert parsed["severity_counts"] == {"critical": 1, "major": 0, "minor": 1}
    assert parsed["issues"][0]["evidence"] == "他刚说灵力枯竭\n转眼又连续催动三次法诀"
    assert parsed["priority_actions"] == ["先修复灵力耗尽后的行动依据"]


def test_should_skip_preface_json_and_parse_reviser_payload_with_multiline_text():
    raw_response = """草稿元信息：
{"mode": "auto_revision", "quality_profile": "longform_serial_v1"}

{
  "revised_text": "林玄抬手压住翻涌的气血。
他借阵盘残余灵光硬撑着迈出石阶，脚下每一步都像踩在碎刃上。
直到山门外传来执法堂整齐的喝令，他才意识到真正的退路已经被人提前封死。",
  "applied_issues": [
    "补出外力支撑来源",
    "把章尾改成山门被封"
  ],
  "unresolved_issues": [],
  "change_summary": "先补逻辑，再抬高章尾压迫感"
}
"""

    parsed = parse_json(raw_response)

    assert parsed["applied_issues"] == ["补出外力支撑来源", "把章尾改成山门被封"]
    assert parsed["unresolved_issues"] == []
    assert parsed["revised_text"].startswith("林玄抬手压住翻涌的气血。\n他借阵盘残余灵光硬撑着迈出石阶")
    assert parsed["change_summary"] == "先补逻辑，再抬高章尾压迫感"
