# -*- coding: utf-8 -*-
"""Durable Novel Autopilot end-to-end smoke over the real HTTP gateway.

The smoke uses a deterministic local OpenAI-compatible provider. It verifies
pause fencing, restart persistence, resume, quality closure and real TXT export
without sending prompts, credentials, guidance or provider reasoning to output.
"""

from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
import re
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping, Sequence

from run_strangler_gateway_smoke import (
    SmokeFailure,
    body_preview,
    bootstrap_local_login_session,
    request_probe,
    resolve_local_auth_credentials,
)

DEFAULT_BASE_URL = "http://127.0.0.1:8005"
DEFAULT_HTTP_TIMEOUT = 10.0
DEFAULT_WAIT_TIMEOUT = 240.0
DEFAULT_PROVIDER_HOST = "host.docker.internal"
MOCK_MODEL_ID = "novel-autopilot-smoke-model"
EXPECTED_MODEL_MARKERS = {
    "INSPIRATION_QUICK_COMPLETE",
    "WORLD_BUILDING",
    "CAREER_SYSTEM_GENERATION",
    "CHARACTERS_BATCH_GENERATION",
    "SINGLE_ORGANIZATION_GENERATION",
    "OUTLINE_CREATE",
    "OUTLINE_EXPAND_SINGLE",
    "CHAPTER_GENERATION",
    "PLOT_ANALYSIS",
    "AI_DENOISING",
}
EXPECTED_STEP_TYPES = {
    "foundation",
    "world_building",
    "career_design",
    "character_design",
    "organization_design",
    "outline",
    "outline_expand",
    "chapter_generate",
    "chapter_analyze",
    "chapter_repair",
    "book_review",
    "book_polish",
    "export",
}
SETTINGS_FIELDS = (
    "api_provider",
    "api_base_url",
    "api_backup_urls",
    "provider_type",
    "fallback_strategy",
    "azure_api_version",
    "llm_model",
    "temperature",
    "max_tokens",
    "system_prompt",
    "preferences",
    "web_research_enabled",
    "web_research_exa_enabled",
    "web_research_grok_enabled",
    "web_research_exa_api_key",
    "web_research_exa_base_url",
    "web_research_grok_api_key",
    "web_research_grok_base_url",
    "web_research_grok_model",
    "web_research_grok_search_enabled",
)


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_output_path() -> Path:
    return repo_root() / "tmp" / "smoke" / "tmp_novel_autopilot_smoke_latest.json"


def extract_chapter_number(prompt: str, total_chapters: int) -> int:
    task_patterns = (
        r"SMOKE_(?:GENERATED|REPAIRED|POLISHED)_CHAPTER_(\d+)",
        r"全面分析第\s*(\d+)\s*章",
        r"撰写第\s*(\d+)\s*章",
        r"章节[：:]\s*第?\s*(\d+)\s*章?",
        r"(?:当前|目标)章节[^0-9]{0,20}第?\s*(\d+)\s*章?",
        r"章节(?:编号|序号)[^0-9]{0,20}(\d+)",
    )
    for pattern in task_patterns:
        match = re.search(pattern, prompt, flags=re.IGNORECASE)
        if match:
            value = int(match.group(1))
            if 1 <= value <= total_chapters:
                return value

    structured_patterns = (
        r'"chapter_number"\s*:\s*(\d+)',
        r"'chapter_number'\s*:\s*(\d+)",
        r"\bchapter_number\b[^0-9]{0,20}(\d+)",
    )
    for pattern in structured_patterns:
        match = re.search(pattern, prompt, flags=re.IGNORECASE)
        if match:
            value = int(match.group(1))
            if 1 <= value <= total_chapters:
                return value

    fallback = re.search(r"第\s*(\d+)\s*章", prompt)
    if fallback:
        value = int(fallback.group(1))
        if 1 <= value <= total_chapters:
            return value

    return 1


def build_smoke_project_payload(total_chapters: int) -> dict[str, Any]:
    return {
        "title": f"Durable Autopilot Smoke {uuid.uuid4().hex[:8]}",
        "target_words": total_chapters * 1200,
        "default_creative_mode": "hook",
        "default_story_focus": "advance_plot",
        "default_plot_stage": "development",
        "default_story_creation_brief": "每章推进线索、冲突和代价",
        "default_quality_preset": "plot_drive",
        "default_quality_notes": "目标清楚、冲突升级、结尾保留钩子",
        "outline_mode": "one-to-many",
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate durable one-click novel generation over real HTTP"
    )
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--total-chapters", type=int, default=3)
    parser.add_argument("--provider-host", default=DEFAULT_PROVIDER_HOST)
    parser.add_argument("--http-timeout", type=float, default=DEFAULT_HTTP_TIMEOUT)
    parser.add_argument("--wait-timeout", type=float, default=DEFAULT_WAIT_TIMEOUT)
    parser.add_argument("--poll-interval", type=float, default=0.5)
    parser.add_argument("--username")
    parser.add_argument("--password")
    parser.add_argument("--env-file", type=Path)
    parser.add_argument("--output", type=Path, default=default_output_path())
    parser.add_argument(
        "--restart-command",
        default='docker compose -f "docker-compose.strangler.yml" restart rust-backend',
        help="PowerShell command used to restart only the Rust backend",
    )
    parser.add_argument("--skip-restart", action="store_true")
    return parser


def assert_status(response: Mapping[str, Any], expected: Sequence[int], label: str) -> Any:
    status = int(response.get("status_code") or 0)
    if status not in expected:
        raise SmokeFailure(
            f"{label} returned HTTP {status}; body={body_preview(response.get('body'))}"
        )
    return response.get("body")


def request_json(
    opener: Any,
    *,
    base_url: str,
    method: str,
    path: str,
    timeout: float,
    payload: Any | None = None,
    expected: Sequence[int] = (200,),
    label: str,
) -> Any:
    response = request_probe(
        base_url=base_url,
        method=method,
        path=path,
        timeout=timeout,
        json_body=payload,
        opener=opener,
    )
    body = assert_status(response, expected, label)
    if not isinstance(body, (dict, list)):
        raise SmokeFailure(f"{label} did not return JSON; body={body_preview(body)}")
    return body


def request_bytes(
    opener: Any,
    *,
    base_url: str,
    path: str,
    timeout: float,
) -> tuple[bytes, Mapping[str, Any]]:
    response = request_probe(
        base_url=base_url,
        method="GET",
        path=path,
        timeout=timeout,
        opener=opener,
    )
    assert_status(response, (200,), "download project export")
    body = response.get("body")
    if isinstance(body, bytes):
        return body, response
    if isinstance(body, str):
        return body.encode("utf-8"), response
    raise SmokeFailure("download project export returned an unsupported body type")


def split_chunks(text: str, size: int = 96) -> tuple[str, ...]:
    return tuple(text[index : index + size] for index in range(0, len(text), size)) or ("",)


def long_chapter(marker: str, chapter_number: int, extra: str) -> str:
    """Build a deterministic near-budget chapter that passes production quality gates.

    The smoke must exercise the real chapter generation service instead of relaxing its
    thresholds. Keep the fixture rich in observable story signals: opening hook,
    obstacle-choice-cost chains, world-rule causality, outline anchors, short dialogue,
    payoff feedback and an ending cliffhanger.
    """
    opening = [
        (
            f"{marker}。雾港的第{chapter_number}次钟声忽然压过潮水，旧塔警报同时亮起红字："
            "失踪名单将在今晚重新点名。林澈刚踏过封锁线就被巡夜人拦住，她必须立刻确认"
            "母亲是否已被锁定，否则下一次钟声会暴露被点名者的身份。"
        ),
        (
            "顾岚把港务档案室的残页拍在桌上，质问是谁改写了巡夜路线。窗外传来追兵的"
            "脚步和尖叫，危险逼近，林澈只能在报警与潜入旧塔之间作出选择。"
        ),
        "“别出声！钟声一响，登记过的名字就会暴露。”顾岚按住她的手。",
        "“那就马上查完。谁在名单背后？”林澈反问。",
        (
            f"林澈在雾港推进第{chapter_number}阶段调查，目标受到守钟规则阻碍；"
            "她选择留下可验证证据，并立即承担暴露行踪的后果。"
        ),
        (
            f"她先核对第{chapter_number}组记录，再触发第{chapter_number}次钟声规则，"
            f"让第{chapter_number}阶段证据链和升级后的代价都变成现场可见的行动结果。"
        ),
    ]
    scene_locations = ["港务档案室", "潮汐仓库", "旧塔机房", "巡夜岗亭", "地下水道", "封港闸门"]
    evidence_items = ["撕碎的点名册", "改写的巡夜路线", "守钟人的铜牌", "带血的登记簿", "失效的校对章", "母亲留下的录音"]
    scenes: list[str] = []
    for index, (location, evidence) in enumerate(zip(scene_locations, evidence_items), start=1):
        scenes.extend([
            (
                f"第{index}条线索把林澈和顾岚带到{location}。入口已经封锁，守门人逼迫她"
                f"交出{evidence}，调查当场受阻；她咬牙决定反锁侧门继续复核，但这个选择"
                "会让身份暴露，也可能失去保护母亲的最后机会，代价清清楚楚。"
            ),
            (
                "雾港规则限制所有人：钟声会暴露被点名者的身份。一旦登记簿写下真名，"
                "警报就会触发追索，所以他们不得不在下一次钟声前完成校对；否则封锁线"
                "将锁死退路，并导致更多失踪者被带走。"
            ),
            f"“快，把{evidence}给我！”林澈说。",
            "“不行！追兵到了……你还要继续？”顾岚打断她。",
            "“必须继续。现在撤，所有证据都会消失！”",
            (
                f"她原本认定{evidence}只是诱饵，谁知夹层突然弹开，竟然露出旧塔的真实"
                "点名顺序。林澈当场找到缺失编号，顾岚愣住，守门人的脸色也变了；这一"
                "次反馈证明失踪名单、巡夜路线和异常钟声属于同一条证据链。"
            ),
            (
                f"{extra} 两人核对雾钟线索、港务档案室记录和旧塔钟声间隔，把结果交给"
                "可信的巡夜同伴。新的证据迫使幕后组织提前行动，冲突没有消失，反而从"
                "隐秘追查升级成公开对峙。"
            ),
        ])
    ending = [
        (
            "封港闸门终于开启一线，林澈以为已经翻盘，没想到母亲的录音突然自动播放。"
            "听见那句约定，顾岚呼吸停住，远处人群也一阵哗然：名单上的最后编号并不属于"
            "受害者，而属于负责敲钟的人。"
        ),
        (
            "纸页背面浮出新的红字——下一次点名只剩十分钟，而守钟人的姓名正是林澈。"
            "旧塔随即响起第一声预告钟，追兵的脚步声逼近，下一秒就会从两侧封住退路。"
            "她握紧证据抬头：真相不是谁会失踪，而是谁已经替她敲响了钟？"
        ),
    ]
    content = "\n\n".join([*opening, *scenes, *ending])
    if not 2_700 <= len(content) <= 3_600:
        raise SmokeFailure(f"deterministic chapter fixture length is out of range: {len(content)}")
    return content


FOUNDATION_RESPONSE = {
    "title": "雾钟封港",
    "description": "边境雾港被异常钟声封锁，调查员林澈必须在亲人被点名前查明失踪名单背后的交易。",
    "theme": "真相与代价",
    "genre": ["悬疑", "奇幻"],
    "narrative_perspective": "第三人称",
}
WORLD_RESPONSE = {
    "time_period": "近未来",
    "location": "雾港",
    "atmosphere": "紧张而压抑",
    "rules": "钟声会暴露被点名者的身份",
}
CAREER_RESPONSE = {
    "main_careers": [
        {
            "name": "调查员",
            "description": "追查异常事件",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "见习调查员", "description": "掌握基础调查能力"}],
        },
        {
            "name": "守钟人",
            "description": "维护旧塔并识别异常钟声",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "听钟者", "description": "辨识基础钟声信号"}],
        },
        {
            "name": "港务官",
            "description": "管理港区通行与档案",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "巡查员", "description": "执行港区基础巡查"}],
        },
    ],
    "sub_careers": [
        {
            "name": "情报商",
            "description": "经营灰色情报",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "线人", "description": "建立基础情报网络"}],
        },
        {
            "name": "档案师",
            "description": "整理旧港历史记录",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "抄录员", "description": "校验基础档案"}],
        },
    ],
}


def character_payload() -> list[dict[str, Any]]:
    names = ["林澈", "顾岚", "沈砚", "白棠", "钟叔"]
    roles = ["protagonist", "supporting", "supporting", "supporting", "antagonist"]
    result = []
    for index, (name, role) in enumerate(zip(names, roles), start=1):
        result.append({
            "name": name,
            "is_organization": False,
            "age": 22 + index,
            "gender": "女" if index in (1, 2, 4) else "男",
            "role_type": role,
            "personality": "谨慎、坚定、重视证据",
            "background": "长期生活在雾港并与旧塔事件存在联系",
            "appearance": "常穿便于行动的深色外套",
            "traits": ["敏锐", "克制"],
            "relationships_array": [],
            "organization_memberships": [],
            "career_assignment": {
                "main_career": "调查员",
                "main_stage": 1,
                "sub_careers": [{"career": "情报商", "stage": 1}],
            },
        })
    return result


def outline_payload(total_chapters: int) -> dict[str, Any]:
    return {
        "chapters": [{
            "chapter_number": 1,
            "title": "雾钟名单",
            "summary": (
                f"林澈围绕异常点名册完成一条覆盖{total_chapters}章的主线："
                "确认规则、追查证据、承担代价并揭开旧塔交易。"
            ),
        }]
    }


def outline_expansion_payload(target_chapter_count: int) -> list[dict[str, Any]]:
    return [
        {
            "sub_index": number,
            "title": f"第{number}章 雾钟线索",
            "plot_summary": (
                f"林澈在雾港推进第{number}阶段调查，目标受到守钟规则阻碍；"
                "她选择留下可验证证据，并立即承担暴露行踪的后果。"
            ),
            "key_events": [f"核对第{number}组记录", f"触发第{number}次钟声规则"],
            "character_focus": ["林澈", "顾岚"],
            "emotional_tone": "紧张",
            "narrative_goal": f"推进第{number}阶段证据链并升级代价",
            "conflict_type": "调查目标与封锁规则冲突",
            "estimated_words": 3000,
            "scenes": [{
                "location": "雾港旧塔",
                "characters": ["林澈", "顾岚"],
                "purpose": f"完成第{number}阶段调查交锋",
            }],
        }
        for number in range(1, target_chapter_count + 1)
    ]

@dataclass
class MockState:
    total_chapters: int
    world_request_started: threading.Event = field(default_factory=threading.Event)
    release_first_world_request: threading.Event = field(default_factory=threading.Event)
    lock: threading.Lock = field(default_factory=threading.Lock)
    request_count: int = 0
    marker_counts: dict[str, int] = field(default_factory=dict)
    chapter_analysis_counts: dict[int, int] = field(default_factory=dict)
    guidance_seen: bool = False

    def record(self, marker: str, prompt: str) -> int:
        with self.lock:
            self.request_count += 1
            self.marker_counts[marker] = self.marker_counts.get(marker, 0) + 1
            if "SMOKE_GUIDANCE_" in prompt:
                self.guidance_seen = True
            return self.marker_counts[marker]

    def next_analysis_attempt(self, chapter_number: int) -> int:
        with self.lock:
            next_value = self.chapter_analysis_counts.get(chapter_number, 0) + 1
            self.chapter_analysis_counts[chapter_number] = next_value
            return next_value

    def public_summary(self) -> dict[str, Any]:
        with self.lock:
            return {
                "request_count": self.request_count,
                "marker_counts": dict(sorted(self.marker_counts.items())),
                "chapter_analysis_counts": dict(sorted(self.chapter_analysis_counts.items())),
                "guidance_seen": self.guidance_seen,
            }


class AutopilotMockServer:
    def __init__(self, total_chapters: int) -> None:
        self.state = MockState(total_chapters=total_chapters)
        handler = self._build_handler()
        self._server = http.server.ThreadingHTTPServer(("0.0.0.0", 0), handler)
        self._thread = threading.Thread(
            target=self._server.serve_forever,
            name="novel-autopilot-smoke-provider",
            daemon=True,
        )

    @property
    def port(self) -> int:
        return int(self._server.server_address[1])

    def provider_base_url(self, provider_host: str) -> str:
        return f"http://{provider_host}:{self.port}/v1"

    def __enter__(self) -> "AutopilotMockServer":
        self._thread.start()
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=3.0)

    def _build_handler(self) -> type[http.server.BaseHTTPRequestHandler]:
        state = self.state

        class Handler(http.server.BaseHTTPRequestHandler):
            server_version = "NovelAutopilotSmokeProvider/1.0"

            def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
                return

            def do_GET(self) -> None:  # noqa: N802
                if self.path.rstrip("/") == "/v1/models":
                    self._write_json({
                        "object": "list",
                        "data": [{
                            "id": MOCK_MODEL_ID,
                            "object": "model",
                            "owned_by": "mumu-smoke",
                        }],
                    })
                    return
                self.send_error(404)

            def do_POST(self) -> None:  # noqa: N802
                normalized_path = self.path.rstrip("/")
                if normalized_path in ("/v1/embeddings", "/embeddings"):
                    self._read_payload()
                    self._write_json({
                        "object": "list",
                        "data": [{"object": "embedding", "index": 0, "embedding": [0.1] * 16}],
                        "model": MOCK_MODEL_ID,
                        "usage": {"prompt_tokens": 1, "total_tokens": 1},
                    })
                    return
                if normalized_path not in ("/v1/chat/completions", "/chat/completions"):
                    self.send_error(404)
                    return

                payload = self._read_payload()
                prompt = self._joined_prompt(payload)
                marker, text = self._dispatch(prompt)
                state.record(marker, prompt)
                if marker == "UNKNOWN":
                    self._write_error(422, "unclassified_prompt_template")
                    return

                if marker == "WORLD_BUILDING" and state.marker_counts.get(marker) == 1:
                    state.world_request_started.set()
                    state.release_first_world_request.wait(timeout=60.0)

                if bool(payload.get("stream", True)):
                    self._write_sse(text)
                else:
                    self._write_json({
                        "id": f"chatcmpl-{uuid.uuid4().hex[:12]}",
                        "object": "chat.completion",
                        "model": MOCK_MODEL_ID,
                        "choices": [{
                            "index": 0,
                            "message": {"role": "assistant", "content": text},
                            "finish_reason": "stop",
                        }],
                        "usage": {"prompt_tokens": 100, "completion_tokens": 100, "total_tokens": 200},
                    })

            def _read_payload(self) -> dict[str, Any]:
                length = int(self.headers.get("Content-Length") or "0")
                raw = self.rfile.read(length) if length > 0 else b"{}"
                try:
                    value = json.loads(raw.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    value = {}
                return value if isinstance(value, dict) else {}

            @staticmethod
            def _joined_prompt(payload: Mapping[str, Any]) -> str:
                messages = payload.get("messages")
                if not isinstance(messages, list):
                    return ""
                parts: list[str] = []
                for item in messages:
                    if not isinstance(item, dict):
                        continue
                    content = item.get("content", "")
                    if isinstance(content, list):
                        for part in content:
                            if isinstance(part, dict):
                                parts.append(str(part.get("text", "")))
                    else:
                        parts.append(str(content))
                return "\n".join(parts)

            def _dispatch(self, prompt: str) -> tuple[str, str]:
                if "PLOT_ANALYSIS" in prompt:
                    chapter_number = self._chapter_number(prompt)
                    attempt = state.next_analysis_attempt(chapter_number)
                    if chapter_number == 1 and attempt == 1:
                        return "PLOT_ANALYSIS", json.dumps({
                            "scores": {
                                "overall": 7.2,
                                "pacing": 7.1,
                                "engagement": 7.2,
                                "coherence": 7.3,
                                "score_justification": "烟测返修",
                            },
                            "suggestions": ["补强人物动机并增加可见代价"],
                        }, ensure_ascii=False)
                    if chapter_number == 2 and attempt == 1:
                        return "PLOT_ANALYSIS", json.dumps({
                            "scores": {
                                "overall": 8.8,
                                "pacing": 8.7,
                                "engagement": 8.9,
                                "coherence": 8.8,
                                "score_justification": "通过但仍需全书润色",
                            },
                            "suggestions": ["压缩中段重复描述"],
                        }, ensure_ascii=False)
                    return "PLOT_ANALYSIS", json.dumps({
                        "scores": {
                            "overall": 8.8,
                            "pacing": 8.7,
                            "engagement": 8.8,
                            "coherence": 8.9,
                            "score_justification": "质量门通过",
                        },
                        "suggestions": [],
                    }, ensure_ascii=False)

                if "AI_DENOISING" in prompt:
                    chapter_number = self._chapter_number(prompt)
                    return "AI_DENOISING", long_chapter(
                        f"SMOKE_POLISHED_CHAPTER_{chapter_number}",
                        chapter_number,
                        "润色后中段更紧凑，证据链也更清楚。",
                    )

                if any(key in prompt for key in (
                    "CHAPTER_GENERATION_ONE_TO_MANY",
                    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
                    "CHAPTER_GENERATION_ONE_TO_ONE",
                    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
                )):
                    chapter_number = self._chapter_number(prompt)
                    if "【修复诊断】" in prompt or "【修复目标】" in prompt or "补强人物动机并增加可见代价" in prompt:
                        return "CHAPTER_GENERATION", long_chapter(
                            f"SMOKE_REPAIRED_CHAPTER_{chapter_number}",
                            chapter_number,
                            "她主动说明隐瞒动机，并以失去安全屋作为可见代价。",
                        )
                    return "CHAPTER_GENERATION", long_chapter(
                        f"SMOKE_GENERATED_CHAPTER_{chapter_number}",
                        chapter_number,
                        "新的证人让调查方向发生转折。",
                    )

                if "INSPIRATION_QUICK_COMPLETE" in prompt:
                    return "INSPIRATION_QUICK_COMPLETE", json.dumps(FOUNDATION_RESPONSE, ensure_ascii=False)
                if "WORLD_BUILDING" in prompt:
                    return "WORLD_BUILDING", json.dumps(WORLD_RESPONSE, ensure_ascii=False)
                if "CAREER_SYSTEM_GENERATION" in prompt:
                    return "CAREER_SYSTEM_GENERATION", json.dumps(CAREER_RESPONSE, ensure_ascii=False)
                if "CHARACTERS_BATCH_GENERATION" in prompt:
                    return "CHARACTERS_BATCH_GENERATION", json.dumps(character_payload(), ensure_ascii=False)
                if "SINGLE_ORGANIZATION_GENERATION" in prompt:
                    return "SINGLE_ORGANIZATION_GENERATION", json.dumps({
                        "name": "雾港守钟会",
                        "is_organization": True,
                        "organization_type": "秘密调查组织",
                        "description": "由旧港调查员建立，长期记录异常钟声。",
                        "purpose": "保护点名册并追查钟声来源",
                        "power_level": 70,
                        "location": "雾港旧塔",
                        "member_count": 2,
                        "initial_members": [],
                        "organization_relationships": [],
                    }, ensure_ascii=False)
                if "OUTLINE_EXPAND_SINGLE" in prompt:
                    target_chapter_count = self._target_chapter_count(prompt)
                    return "OUTLINE_EXPAND_SINGLE", json.dumps(
                        outline_expansion_payload(target_chapter_count),
                        ensure_ascii=False,
                    )
                if "OUTLINE_CREATE" in prompt:
                    return "OUTLINE_CREATE", json.dumps(outline_payload(state.total_chapters), ensure_ascii=False)

                return "UNKNOWN", ""

            @staticmethod
            def _chapter_number(prompt: str) -> int:
                return extract_chapter_number(prompt, state.total_chapters)

            @staticmethod
            def _target_chapter_count(prompt: str) -> int:
                import re

                patterns = (
                    r"target_chapter_count[^0-9]{0,20}(\d+)",
                    r"展开为\s*(\d+)\s*个章节",
                    r"返回\s*(\d+)\s*个章节规划",
                )
                for pattern in patterns:
                    match = re.search(pattern, prompt, flags=re.IGNORECASE)
                    if match:
                        value = int(match.group(1))
                        if 1 <= value <= state.total_chapters:
                            return value
                raise SmokeFailure("outline expansion prompt is missing target_chapter_count")

            def _write_sse(self, text: str) -> None:
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream; charset=utf-8")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()
                for chunk in split_chunks(text):
                    payload = {
                        "id": f"chatcmpl-{uuid.uuid4().hex[:12]}",
                        "object": "chat.completion.chunk",
                        "model": MOCK_MODEL_ID,
                        "choices": [{"index": 0, "delta": {"content": chunk}, "finish_reason": None}],
                    }
                    self.wfile.write(f"data: {json.dumps(payload, ensure_ascii=False)}\n\n".encode("utf-8"))
                    self.wfile.flush()
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()

            def _write_error(self, status: int, code: str) -> None:
                raw = json.dumps({"error": {"code": code}}, ensure_ascii=False).encode("utf-8")
                self.send_response(status)
                self.send_header("Content-Type", "application/json; charset=utf-8")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def _write_json(self, payload: Any) -> None:
                raw = json.dumps(payload, ensure_ascii=False).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json; charset=utf-8")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

        return Handler

def get_run(
    opener: Any,
    *,
    base_url: str,
    project_id: str,
    run_id: str,
    timeout: float,
) -> dict[str, Any]:
    body = request_json(
        opener,
        base_url=base_url,
        method="GET",
        path=f"/api/projects/{project_id}/novel-autopilot-runs/{run_id}",
        timeout=timeout,
        label="get autopilot run",
    )
    run = body.get("run") if isinstance(body, dict) else None
    if not isinstance(run, dict):
        raise SmokeFailure("get autopilot run response is missing run")
    return run


def get_background_task(
    opener: Any,
    *,
    base_url: str,
    task_id: str,
    timeout: float,
) -> dict[str, Any]:
    body = request_json(
        opener,
        base_url=base_url,
        method="GET",
        path=f"/api/background-tasks/{task_id}",
        timeout=timeout,
        label="get autopilot background task",
    )
    if not isinstance(body, dict) or not isinstance(body.get("status"), str):
        raise SmokeFailure("get autopilot background task response is missing status")
    return body


def background_task_failure_summary(task: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "task_id": task.get("task_id"),
        "task_type": task.get("task_type"),
        "status": task.get("status"),
        "stage_code": task.get("stage_code"),
        "updated_at": task.get("updated_at"),
    }


def list_steps(
    opener: Any,
    *,
    base_url: str,
    project_id: str,
    run_id: str,
    timeout: float,
) -> list[dict[str, Any]]:
    body = request_json(
        opener,
        base_url=base_url,
        method="GET",
        path=f"/api/projects/{project_id}/novel-autopilot-runs/{run_id}/steps",
        timeout=timeout,
        label="list autopilot steps",
    )
    items = body.get("items") if isinstance(body, dict) else None
    if not isinstance(items, list):
        raise SmokeFailure("list autopilot steps response is missing items")
    return [item for item in items if isinstance(item, dict)]


def run_failure_summary(run: Mapping[str, Any], steps: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    latest_step = max(
        steps,
        key=lambda item: (
            str(item.get("created_at") or ""),
            int(item.get("attempt") or 0),
        ),
        default={},
    )
    return {
        "status": run.get("status"),
        "phase": run.get("current_phase"),
        "step": run.get("current_step") or latest_step.get("step_key"),
        "version": run.get("version"),
        "last_error_code": run.get("last_error_code"),
        "step_status": latest_step.get("status"),
        "step_error_code": latest_step.get("error_code"),
        "attempt": latest_step.get("attempt"),
        "quality_decision": latest_step.get("quality_decision"),
    }


def wait_for_run(
    opener: Any,
    *,
    base_url: str,
    project_id: str,
    run_id: str,
    timeout: float,
    poll_interval: float,
    predicate: Any,
    label: str,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    last_run: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        last_run = get_run(
            opener,
            base_url=base_url,
            project_id=project_id,
            run_id=run_id,
            timeout=min(10.0, timeout),
        )
        if predicate(last_run):
            return last_run
        status = last_run.get("status")
        if status in ("failed", "cancelled", "waiting_human"):
            steps = list_steps(
                opener,
                base_url=base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=min(10.0, timeout),
            )
            raise SmokeFailure(f"{label} stopped before completion: {run_failure_summary(last_run, steps)}")
        active_task_id = str(last_run.get("active_background_task_id") or "").strip()
        if active_task_id:
            background_task = get_background_task(
                opener,
                base_url=base_url,
                task_id=active_task_id,
                timeout=min(10.0, timeout),
            )
            task_status = background_task.get("status")
            task_type = background_task.get("task_type")
            if task_type != "unknown" and task_status in ("failed", "cancelled"):
                steps = list_steps(
                    opener,
                    base_url=base_url,
                    project_id=project_id,
                    run_id=run_id,
                    timeout=min(10.0, timeout),
                )
                failure = run_failure_summary(last_run, steps)
                failure["background_task"] = background_task_failure_summary(background_task)
                raise SmokeFailure(
                    f"{label} background task stopped before run convergence: {failure}"
                )
        time.sleep(poll_interval)
    summary = run_failure_summary(last_run or {}, [])
    raise SmokeFailure(f"timed out waiting for {label}: {summary}")


def mutate_run(
    opener: Any,
    *,
    base_url: str,
    project_id: str,
    run_id: str,
    action: str,
    timeout: float,
    extra: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    current = get_run(
        opener,
        base_url=base_url,
        project_id=project_id,
        run_id=run_id,
        timeout=timeout,
    )
    payload = {"expected_version": current["version"]}
    if extra:
        payload.update(extra)
    body = request_json(
        opener,
        base_url=base_url,
        method="POST",
        path=f"/api/projects/{project_id}/novel-autopilot-runs/{run_id}/{action}",
        timeout=timeout,
        payload=payload,
        label=f"{action} autopilot run",
    )
    run = body.get("run") if isinstance(body, dict) else None
    if not isinstance(run, dict):
        raise SmokeFailure(f"{action} autopilot response is missing run")
    return run


def snapshot_settings(
    opener: Any,
    *,
    base_url: str,
    timeout: float,
) -> dict[str, Any]:
    settings = request_json(
        opener,
        base_url=base_url,
        method="GET",
        path="/api/settings",
        timeout=timeout,
        label="get settings snapshot",
    )
    key_body = request_json(
        opener,
        base_url=base_url,
        method="GET",
        path="/api/settings/api-key",
        timeout=timeout,
        label="get stored API key snapshot",
    )
    if not isinstance(settings, dict) or not isinstance(key_body, dict):
        raise SmokeFailure("settings snapshot response is invalid")
    return {
        "settings": settings,
        "api_key": str(key_body.get("api_key") or ""),
        "has_api_key": bool(key_body.get("has_api_key")),
    }


def settings_restore_payload(snapshot: Mapping[str, Any]) -> dict[str, Any]:
    settings = snapshot.get("settings")
    if not isinstance(settings, dict):
        raise SmokeFailure("settings snapshot cannot be restored")
    payload = {field: settings[field] for field in SETTINGS_FIELDS if field in settings}
    if snapshot.get("has_api_key"):
        payload["api_key"] = snapshot.get("api_key", "")
        payload["clear_api_key"] = False
    else:
        payload["clear_api_key"] = True
    return payload


def update_settings(
    opener: Any,
    *,
    base_url: str,
    timeout: float,
    payload: Mapping[str, Any],
    label: str,
) -> None:
    request_json(
        opener,
        base_url=base_url,
        method="POST",
        path="/api/settings",
        timeout=timeout,
        payload=dict(payload),
        expected=(200, 201),
        label=label,
    )


def restart_backend(command: str, cwd: Path, timeout: float) -> dict[str, Any]:
    started = time.perf_counter()
    completed = subprocess.run(
        ["powershell", "-NoProfile", "-Command", command],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        check=False,
    )
    if completed.returncode != 0:
        stderr_tail = "\n".join(completed.stderr.splitlines()[-12:])
        raise SmokeFailure(
            f"backend restart failed with exit code {completed.returncode}; stderr_tail={stderr_tail}"
        )
    return {
        "executed": True,
        "exit_code": completed.returncode,
        "elapsed_ms": round((time.perf_counter() - started) * 1000, 2),
    }


def wait_for_health(base_url: str, timeout: float, poll_interval: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            response = request_probe(
                base_url=base_url,
                method="GET",
                path="/health",
                timeout=min(5.0, timeout),
            )
            if int(response.get("status_code") or 0) == 200:
                return
        except SmokeFailure:
            pass
        time.sleep(poll_interval)
    raise SmokeFailure("backend health did not recover after restart")


def parse_export_descriptor(value: Any) -> dict[str, Any]:
    if not isinstance(value, str) or not value.strip():
        raise SmokeFailure("completed run is missing final_export_ref")
    try:
        descriptor = json.loads(value)
    except json.JSONDecodeError as exc:
        raise SmokeFailure("final_export_ref is not valid JSON") from exc
    if not isinstance(descriptor, dict):
        raise SmokeFailure("final_export_ref is not a JSON object")
    return descriptor


def safe_run_summary(run: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "id": run.get("id"),
        "status": run.get("status"),
        "current_phase": run.get("current_phase"),
        "execution_scope": run.get("execution_scope"),
        "total_chapters": run.get("total_chapters"),
        "completed_chapters": run.get("completed_chapters"),
        "failed_chapter_count": run.get("failed_chapter_count"),
        "pending_rewrite_count": run.get("pending_rewrite_count"),
        "total_word_count": run.get("total_word_count"),
        "used_tokens": run.get("used_tokens"),
        "epoch": run.get("epoch"),
        "version": run.get("version"),
        "has_guidance": run.get("has_guidance"),
        "last_error_code": run.get("last_error_code"),
    }


def assert_completed_run(run: Mapping[str, Any], total_chapters: int) -> None:
    expected = {
        "status": "completed",
        "execution_scope": "complete_book",
        "completed_chapters": total_chapters,
        "failed_chapter_count": 0,
        "pending_rewrite_count": 0,
    }
    for key, value in expected.items():
        if run.get(key) != value:
            raise SmokeFailure(f"completed run assertion failed: {key}={run.get(key)!r}, expected={value!r}")
    if int(run.get("total_chapters") or 0) != total_chapters:
        raise SmokeFailure("completed run total_chapters does not match requested chapter count")
    if int(run.get("total_word_count") or 0) <= 0:
        raise SmokeFailure("completed run total_word_count must be positive")
    if not bool(run.get("has_guidance")):
        raise SmokeFailure("guidance flag was not preserved through restart/resume")


def assert_steps(steps: Sequence[Mapping[str, Any]], total_chapters: int) -> dict[str, Any]:
    completed_types = {
        str(step.get("step_type"))
        for step in steps
        if step.get("status") == "completed"
    }
    missing = sorted(EXPECTED_STEP_TYPES - completed_types)
    if missing:
        raise SmokeFailure(f"completed step types are missing: {missing}")
    stale_count = sum(1 for step in steps if step.get("status") == "stale")
    if stale_count < 1:
        raise SmokeFailure("pause fencing did not produce a stale in-flight step")
    type_counts: dict[str, int] = {}
    for step in steps:
        step_type = str(step.get("step_type") or "")
        if step.get("status") == "completed":
            type_counts[step_type] = type_counts.get(step_type, 0) + 1
    if type_counts.get("outline_expand", 0) != 1:
        raise SmokeFailure("one-to-many smoke must complete exactly one outline_expand step")
    if type_counts.get("chapter_generate", 0) < total_chapters:
        raise SmokeFailure("chapter_generate did not cover all chapters")
    if type_counts.get("chapter_analyze", 0) < total_chapters + 2:
        raise SmokeFailure("chapter_analyze did not cover repair/polish re-analysis")
    return {
        "total": len(steps),
        "stale_count": stale_count,
        "completed_type_counts": dict(sorted(type_counts.items())),
    }

def wait_for_stale_step(
    opener: Any,
    *,
    base_url: str,
    project_id: str,
    run_id: str,
    timeout: float,
    poll_interval: float,
) -> list[dict[str, Any]]:
    deadline = time.monotonic() + timeout
    last_steps: list[dict[str, Any]] = []
    while time.monotonic() < deadline:
        last_steps = list_steps(
            opener,
            base_url=base_url,
            project_id=project_id,
            run_id=run_id,
            timeout=min(10.0, timeout),
        )
        if any(step.get("status") == "stale" for step in last_steps):
            return last_steps
        time.sleep(poll_interval)
    statuses = [step.get("status") for step in last_steps]
    raise SmokeFailure(f"timed out waiting for stale in-flight step; statuses={statuses}")


def run_smoke(args: argparse.Namespace) -> dict[str, Any]:
    if args.total_chapters < 3:
        raise SmokeFailure("--total-chapters must be at least 3 to verify repair and polish")

    username, password, used_env_file = resolve_local_auth_credentials(
        username=args.username,
        password=args.password,
        env_file=args.env_file,
    )
    opener, login_summary = bootstrap_local_login_session(
        base_url=args.base_url,
        timeout=args.http_timeout,
        username=username,
        password=password,
        login_path="/api/auth/local/login",
        require_token_cookie=True,
    )
    settings_snapshot = snapshot_settings(
        opener,
        base_url=args.base_url,
        timeout=args.http_timeout,
    )
    guidance = f"SMOKE_GUIDANCE_{uuid.uuid4().hex}: 保持证据链清晰并让每章结尾留下可验证钩子。"
    summary: dict[str, Any] = {
        "schema_version": "novel-autopilot-smoke/v1",
        "ok": False,
        "base_url": args.base_url,
        "total_chapters": args.total_chapters,
        "auth": {
            "status_code": login_summary.get("status_code"),
            "user_id": login_summary.get("user_id"),
            "credential_source": str(used_env_file) if used_env_file else "cli_or_environment",
        },
        "settings": {
            "snapshot_taken": True,
            "had_api_key": bool(settings_snapshot.get("has_api_key")),
            "restored": False,
        },
        "restart": {"executed": False},
    }
    primary_error: BaseException | None = None

    try:
        with AutopilotMockServer(args.total_chapters) as mock:
            update_settings(
                opener,
                base_url=args.base_url,
                timeout=args.http_timeout,
                label="point settings to deterministic provider",
                payload={
                    "api_provider": "openai",
                    "provider_type": "openai",
                    "api_key": "smoke-local-key",
                    "clear_api_key": False,
                    "api_base_url": mock.provider_base_url(args.provider_host),
                    "api_backup_urls": [],
                    "fallback_strategy": "none",
                    "llm_model": MOCK_MODEL_ID,
                    "temperature": 0.0,
                    "max_tokens": 8192,
                    "web_research_enabled": False,
                    "web_research_exa_enabled": False,
                    "web_research_grok_enabled": False,
                },
            )

            project_body = request_json(
                opener,
                base_url=args.base_url,
                method="POST",
                path="/api/projects",
                timeout=args.http_timeout,
                expected=(201,),
                label="create smoke project",
                payload=build_smoke_project_payload(args.total_chapters),
            )
            project_id = str(project_body.get("id") or "") if isinstance(project_body, dict) else ""
            if not project_id:
                raise SmokeFailure("create project response is missing id")
            summary["project_id"] = project_id

            create_body = request_json(
                opener,
                base_url=args.base_url,
                method="POST",
                path=f"/api/projects/{project_id}/novel-autopilot-runs",
                timeout=args.http_timeout,
                expected=(200, 201),
                label="create complete-book autopilot run",
                payload={
                    "total_chapters": args.total_chapters,
                    "config": {
                        "execution_scope": "complete_book",
                        "human_gate_mode": "fully_automatic",
                        "gate_interval": 1,
                        "next_chapter_count": None,
                        "max_chapters": args.total_chapters,
                        "max_tokens": 500000,
                        "max_estimated_cost": None,
                        "max_runtime_seconds": int(args.wait_timeout * 3),
                        "max_step_attempts": 4,
                        "max_consecutive_provider_failures": 4,
                        "max_consecutive_quality_failures": 4,
                        "regenerate_existing": False,
                        "run_book_review": True,
                        "run_book_polish": True,
                        "export_format": "txt",
                    },
                },
            )
            created_run = create_body.get("run") if isinstance(create_body, dict) else None
            if not isinstance(created_run, dict):
                raise SmokeFailure("create run response is missing run")
            run_id = str(created_run.get("id") or "")
            if not run_id:
                raise SmokeFailure("create run response is missing run id")
            summary["run_id"] = run_id
            summary["created"] = bool(create_body.get("created"))

            if not mock.state.world_request_started.wait(timeout=args.wait_timeout):
                raise SmokeFailure("deterministic provider did not receive the world-building request")

            paused_run = mutate_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                action="pause",
                timeout=args.http_timeout,
            )
            if paused_run.get("status") != "paused":
                raise SmokeFailure("pause did not move run to paused")
            paused_epoch = paused_run.get("epoch")
            paused_version = paused_run.get("version")

            guided_run = mutate_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                action="guidance",
                timeout=args.http_timeout,
                extra={"guidance": guidance},
            )
            if guided_run.get("status") != "paused" or not guided_run.get("has_guidance"):
                raise SmokeFailure("guidance was not persisted while the run was paused")
            guided_epoch = guided_run.get("epoch")
            guided_version = guided_run.get("version")
            if not isinstance(paused_epoch, int) or not isinstance(guided_epoch, int):
                raise SmokeFailure("pause or guidance response is missing an integer epoch")
            if not isinstance(paused_version, int) or not isinstance(guided_version, int):
                raise SmokeFailure("pause or guidance response is missing an integer version")
            if guided_epoch <= paused_epoch:
                raise SmokeFailure("guidance did not advance the paused run epoch fence")
            if guided_version <= paused_version:
                raise SmokeFailure("guidance did not advance the paused run version fence")

            mock.state.release_first_world_request.set()
            stale_steps = wait_for_stale_step(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=args.wait_timeout,
                poll_interval=args.poll_interval,
            )
            before_restart = get_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=args.http_timeout,
            )
            if before_restart.get("status") != "paused":
                raise SmokeFailure("late provider result changed the paused run status")
            if before_restart.get("epoch") != guided_epoch:
                raise SmokeFailure("late provider result changed the paused run epoch")

            if args.skip_restart:
                summary["restart"] = {"executed": False, "skipped": True}
            else:
                summary["restart"] = restart_backend(
                    args.restart_command,
                    repo_root(),
                    timeout=max(60.0, args.wait_timeout),
                )
                wait_for_health(args.base_url, args.wait_timeout, args.poll_interval)

            after_restart = get_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=args.http_timeout,
            )
            if after_restart.get("status") != "paused":
                raise SmokeFailure("run did not remain paused after backend restart")
            if after_restart.get("epoch") != guided_epoch:
                raise SmokeFailure("paused run epoch changed across backend restart")
            if not after_restart.get("has_guidance"):
                raise SmokeFailure("guidance flag was lost across backend restart")

            resumed_run = mutate_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                action="resume",
                timeout=args.http_timeout,
            )
            if resumed_run.get("status") not in ("queued", "running"):
                raise SmokeFailure("resume did not requeue the paused run")

            completed_run = wait_for_run(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=args.wait_timeout,
                poll_interval=args.poll_interval,
                predicate=lambda run: run.get("status") == "completed",
                label="complete-book run completion",
            )
            assert_completed_run(completed_run, args.total_chapters)
            final_steps = list_steps(
                opener,
                base_url=args.base_url,
                project_id=project_id,
                run_id=run_id,
                timeout=args.http_timeout,
            )
            step_summary = assert_steps(final_steps, args.total_chapters)

            descriptor = parse_export_descriptor(completed_run.get("final_export_ref"))
            export_bytes, export_response = request_bytes(
                opener,
                base_url=args.base_url,
                path=f"/api/projects/{project_id}/export",
                timeout=args.http_timeout,
            )
            actual_digest = "sha256:" + hashlib.sha256(export_bytes).hexdigest()
            if descriptor.get("content_digest") != actual_digest:
                raise SmokeFailure("real export digest does not match final_export_ref")
            export_text = export_bytes.decode("utf-8-sig")
            for marker in ("SMOKE_REPAIRED_CHAPTER_1", "SMOKE_POLISHED_CHAPTER_2"):
                if marker not in export_text:
                    raise SmokeFailure(f"real export is missing expected marker: {marker}")

            provider_summary = mock.state.public_summary()
            missing_markers = sorted(EXPECTED_MODEL_MARKERS - set(provider_summary["marker_counts"]))
            if missing_markers:
                raise SmokeFailure(f"deterministic provider did not observe model stages: {missing_markers}")
            if provider_summary["marker_counts"].get("UNKNOWN"):
                raise SmokeFailure("deterministic provider observed an unclassified model request")
            if not provider_summary["guidance_seen"]:
                raise SmokeFailure("resumed provider prompt did not observe persisted guidance")

            summary.update({
                "ok": True,
                "pause_fence": {
                    "paused_epoch": paused_epoch,
                    "paused_version": paused_version,
                    "guided_version": guided_version,
                    "stale_step_count_before_restart": sum(
                        1 for step in stale_steps if step.get("status") == "stale"
                    ),
                },
                "run": safe_run_summary(completed_run),
                "steps": step_summary,
                "provider": provider_summary,
                "export": {
                    "schema_version": descriptor.get("schema_version"),
                    "format": descriptor.get("format"),
                    "filename": descriptor.get("filename"),
                    "content_type": export_response.get("content_type"),
                    "content_digest": actual_digest,
                    "chapter_count": descriptor.get("chapter_count"),
                    "total_word_count": descriptor.get("total_word_count"),
                    "byte_count": len(export_bytes),
                    "repair_marker_present": True,
                    "polish_marker_present": True,
                },
            })
    except BaseException as exc:
        primary_error = exc
    finally:
        restore_error: BaseException | None = None
        try:
            update_settings(
                opener,
                base_url=args.base_url,
                timeout=args.http_timeout,
                payload=settings_restore_payload(settings_snapshot),
                label="restore original settings",
            )
            summary["settings"]["restored"] = True
        except BaseException as exc:
            restore_error = exc
            summary["settings"]["restored"] = False
        if restore_error is not None:
            if primary_error is not None:
                raise SmokeFailure(
                    f"smoke failed and settings restoration also failed: "
                    f"primary={type(primary_error).__name__}; restore={type(restore_error).__name__}"
                ) from restore_error
            raise SmokeFailure("settings restoration failed") from restore_error
        if primary_error is not None:
            raise primary_error

    return summary


def write_summary(path: Path, summary: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        summary = run_smoke(args)
        write_summary(args.output, summary)
        print(json.dumps(summary, ensure_ascii=False, indent=2))
        return 0
    except BaseException as exc:
        failure = {
            "schema_version": "novel-autopilot-smoke/v1",
            "ok": False,
            "error_type": type(exc).__name__,
            "error": str(exc),
        }
        write_summary(args.output, failure)
        print(json.dumps(failure, ensure_ascii=False, indent=2), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
