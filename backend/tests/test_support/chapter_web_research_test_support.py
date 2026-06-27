"""生成前网络检索服务。"""

from __future__ import annotations

import asyncio
import ast
import json
import re
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any, Dict, List, Mapping, Optional
from urllib.parse import urlsplit, urlunsplit

import httpx

from tests.test_support.retired_runtime_test_support import PROJECT_ROOT, settings
from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.ai_gateway.ai_clients.openai_client import OpenAIClient
from tests.test_support.memory_service_test_support import memory_service

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.chapter import Chapter
    from migrator_app.models.outline import Outline
    from migrator_app.models.project import Project

logger = get_logger(__name__)

WEB_RESEARCH_PREF_KEY = "web_research"
DEFAULT_EXA_BASE_URL = "https://api.exa.ai"
DEFAULT_GROK_MODEL = "grok-4.1-fast"

SEARCH_PROMPT = """
# Core Instruction

1. User needs may be vague. Think divergently, infer intent from multiple angles, and leverage full conversation context to progressively clarify their true needs.
2. **Breadth-First Search**—Approach problems from multiple dimensions. Brainstorm 5+ perspectives and execute parallel searches for each. Consult as many high-quality sources as possible before responding.
3. **Depth-First Search**—After broad exploration, select ≥2 most relevant perspectives for deep investigation into specialized knowledge.
4. **Evidence-Based Reasoning & Traceable Sources**—Every claim must be followed by a citation (`citation_card` format). More credible sources strengthen arguments. If no references exist, remain silent.
5. Before responding, ensure full execution of Steps 1–4.

---

# Search Instruction

1. Think carefully before responding—anticipate the user’s true intent to ensure precision.
2. Verify every claim rigorously to avoid misinformation.
3. Follow problem logic—dig deeper until clues are exhaustively clear. If a question seems simple, still infer broader intent and search accordingly. Use multiple parallel tool calls per query and ensure answers are well-sourced.
4. Search in English first (prioritizing English resources for volume/quality), but switch to Chinese if context demands.
5. Prioritize authoritative sources: Wikipedia, academic databases, books, reputable media/journalism.
6. Favor sharing in-depth, specialized knowledge over generic or common-sense content.

---

# Output Style

0. **Be direct—no unnecessary follow-ups**.
1. Lead with the **most probable solution** before detailed analysis.
2. **Define every technical term** in plain language (annotate post-paragraph).
3. Explain expertise **simply yet profoundly**.
4. **Respect facts and search results—use statistical rigor to discern truth**.
5. **Every sentence must cite sources** (`citation_card`). More references = stronger credibility. Silence if uncited.
6. Expand on key concepts—after proposing solutions, **use real-world analogies** to demystify technical terms.
7. **Strictly format outputs in polished Markdown** (LaTeX for formulas, code blocks for scripts, etc.).
""".strip()

_URL_PATTERN = re.compile(r"https?://[^\s<>\"'`，。、；：！？》）】\)]+")
_MD_LINK_PATTERN = re.compile(r"\[([^\]]+)\]\((https?://[^)]+)\)")
_SOURCES_HEADING_PATTERN = re.compile(
    r"(?im)^"
    r"(?:#{1,6}\s*)?"
    r"(?:\*\*|__)?\s*"
    r"(sources?|references?|citations?|信源|参考资料|参考|引用|来源列表|来源)"
    r"\s*(?:\*\*|__)?"
    r"(?:\s*[（(][^)\n]*[)）])?"
    r"\s*[:：]?\s*$"
)
_SOURCES_FUNCTION_PATTERN = re.compile(
    r"(?im)(^|\n)\s*(sources|source|citations|citation|references|reference|citation_card|source_cards|source_card)\s*\("
)


def extract_unique_urls(text: str) -> list[str]:
    seen: set[str] = set()
    urls: list[str] = []
    for match in _URL_PATTERN.finditer(text or ""):
        url = match.group().rstrip(".,;:!?")
        if url not in seen:
            seen.add(url)
            urls.append(url)
    return urls


def split_answer_and_sources(text: str) -> tuple[str, list[dict[str, Any]]]:
    raw = (text or "").strip()
    if not raw:
        return "", []

    split = _split_function_call_sources(raw)
    if split:
        return split

    split = _split_heading_sources(raw)
    if split:
        return split

    split = _split_details_block_sources(raw)
    if split:
        return split

    split = _split_tail_link_block(raw)
    if split:
        return split

    return raw, []


def _split_function_call_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    matches = list(_SOURCES_FUNCTION_PATTERN.finditer(text))
    if not matches:
        return None
    for match in reversed(matches):
        extracted = _extract_balanced_call_at_end(text, match.end() - 1)
        if not extracted:
            continue
        _, args_text = extracted
        sources = _parse_sources_payload(args_text)
        if sources:
            return text[: match.start()].rstrip(), sources
    return None


def _extract_balanced_call_at_end(text: str, open_paren_idx: int) -> tuple[int, str] | None:
    if open_paren_idx < 0 or open_paren_idx >= len(text) or text[open_paren_idx] != "(":
        return None

    depth = 1
    in_string: str | None = None
    escape = False
    for idx in range(open_paren_idx + 1, len(text)):
        char = text[idx]
        if in_string:
            if escape:
                escape = False
                continue
            if char == "\\":
                escape = True
                continue
            if char == in_string:
                in_string = None
            continue

        if char in ("'", '"'):
            in_string = char
            continue
        if char == "(":
            depth += 1
            continue
        if char == ")":
            depth -= 1
            if depth == 0:
                if text[idx + 1 :].strip():
                    return None
                return idx, text[open_paren_idx + 1 : idx]
    return None


def _split_heading_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    matches = list(_SOURCES_HEADING_PATTERN.finditer(text))
    if not matches:
        return None
    for match in reversed(matches):
        start = match.start()
        sources = _extract_sources_from_text(text[start:])
        if sources:
            return text[:start].rstrip(), sources
    return None


def _split_tail_link_block(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    lines = text.splitlines()
    if not lines:
        return None

    idx = len(lines) - 1
    while idx >= 0 and not lines[idx].strip():
        idx -= 1
    if idx < 0:
        return None

    tail_end = idx
    link_like_count = 0
    while idx >= 0:
        line = lines[idx].strip()
        if not line:
            idx -= 1
            continue
        if not _is_link_only_line(line):
            break
        link_like_count += 1
        idx -= 1

    tail_start = idx + 1
    if link_like_count < 2:
        return None
    block_text = "\n".join(lines[tail_start : tail_end + 1])
    sources = _extract_sources_from_text(block_text)
    if not sources:
        return None
    return "\n".join(lines[:tail_start]).rstrip(), sources


def _split_details_block_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    lower = text.lower()
    close_idx = lower.rfind("</details>")
    if close_idx == -1 or text[close_idx + len("</details>") :].strip():
        return None
    open_idx = lower.rfind("<details", 0, close_idx)
    if open_idx == -1:
        return None

    sources = _extract_sources_from_text(text[open_idx : close_idx + len("</details>")])
    if len(sources) < 2:
        return None
    return text[:open_idx].rstrip(), sources


def _is_link_only_line(line: str) -> bool:
    stripped = re.sub(r"^\s*(?:[-*]|\d+\.)\s*", "", line).strip()
    return bool(stripped) and (stripped.startswith(("http://", "https://")) or bool(_MD_LINK_PATTERN.search(stripped)))


def _parse_sources_payload(payload: str) -> list[dict[str, Any]]:
    normalized_payload = (payload or "").strip().rstrip(";")
    if not normalized_payload:
        return []

    data: Any = None
    try:
        data = json.loads(normalized_payload)
    except Exception:
        try:
            data = ast.literal_eval(normalized_payload)
        except Exception:
            data = None

    if data is None:
        return _extract_sources_from_text(normalized_payload)
    if isinstance(data, dict):
        for key in ("sources", "citations", "references", "urls"):
            if key in data:
                return _normalize_embedded_sources(data[key])
    return _normalize_embedded_sources(data)


def _normalize_embedded_sources(data: Any) -> list[dict[str, Any]]:
    if isinstance(data, (list, tuple)):
        items = list(data)
    elif isinstance(data, dict):
        items = [data]
    else:
        items = [data]

    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in items:
        if isinstance(item, str):
            for url in extract_unique_urls(item):
                if url not in seen:
                    seen.add(url)
                    normalized.append({"url": url})
            continue
        if isinstance(item, (list, tuple)) and len(item) >= 2:
            title, url = item[0], item[1]
            if isinstance(url, str) and url.startswith(("http://", "https://")) and url not in seen:
                seen.add(url)
                entry: dict[str, Any] = {"url": url}
                if isinstance(title, str) and title.strip():
                    entry["title"] = title.strip()
                normalized.append(entry)
            continue
        if isinstance(item, dict):
            url = item.get("url") or item.get("href") or item.get("link")
            if not isinstance(url, str) or not url.startswith(("http://", "https://")) or url in seen:
                continue
            seen.add(url)
            entry = {"url": url}
            title = item.get("title") or item.get("name") or item.get("label")
            if isinstance(title, str) and title.strip():
                entry["title"] = title.strip()
            desc = item.get("description") or item.get("snippet") or item.get("content")
            if isinstance(desc, str) and desc.strip():
                entry["description"] = desc.strip()
            normalized.append(entry)
    return normalized


def _extract_sources_from_text(text: str) -> list[dict[str, Any]]:
    sources: list[dict[str, Any]] = []
    seen: set[str] = set()
    for title, url in _MD_LINK_PATTERN.findall(text or ""):
        normalized_url = (url or "").strip()
        if not normalized_url or normalized_url in seen:
            continue
        seen.add(normalized_url)
        normalized_title = (title or "").strip()
        sources.append({"title": normalized_title, "url": normalized_url} if normalized_title else {"url": normalized_url})

    for url in extract_unique_urls(text or ""):
        if url not in seen:
            seen.add(url)
            sources.append({"url": url})
    return sources


@dataclass(frozen=True)
class WebResearchRuntimeConfig:
    enabled: Optional[bool] = None
    exa_enabled: bool = True
    grok_enabled: bool = True
    exa_api_key: str = ""
    exa_base_url: str = ""
    grok_api_key: str = ""
    grok_base_url: str = ""
    grok_model: str = DEFAULT_GROK_MODEL
    grok_search_enabled: bool = False
    timeout_seconds: int = 90
    max_assets: int = 4


@dataclass(frozen=True)
class GrokSearchAdapterResult:
    content: str
    sources: list[dict[str, str]]
    mode: str = "grok_search_embedded"


class GrokSearchAdapterError(RuntimeError):
    """GrokSearch 适配器异常。"""


class GrokSearchAdapter:
    @staticmethod
    def normalize_api_base_url(base_url: str) -> str:
        normalized = str(base_url or "").strip()
        if not normalized:
            return ""
        parts = urlsplit(normalized)
        path = (parts.path or "").rstrip("/")
        if not path:
            path = "/v1"
        return urlunsplit((parts.scheme, parts.netloc, path, parts.query, parts.fragment)).rstrip("/")

    @classmethod
    def build_chat_completions_url(cls, base_url: str) -> str:
        normalized = cls.normalize_api_base_url(base_url)
        if not normalized:
            raise GrokSearchAdapterError("Grok Base URL 未配置")
        return f"{normalized}/chat/completions"

    async def search(
        self,
        *,
        query: str,
        api_key: str,
        api_base_url: str,
        model: str,
        platform: str = "",
    ) -> GrokSearchAdapterResult:
        if not str(query or "").strip():
            raise GrokSearchAdapterError("搜索 query 不能为空")
        if not str(api_key or "").strip():
            raise GrokSearchAdapterError("Grok API Key 未配置")

        endpoint = self.build_chat_completions_url(api_base_url)
        platform_prompt = ""
        if platform:
            platform_prompt = f"\n\n请优先关注平台或来源范围：{platform}"

        payload = {
            "model": model,
            "messages": [
                {"role": "system", "content": SEARCH_PROMPT},
                {"role": "user", "content": f"{query}{platform_prompt}"},
            ],
            "stream": True,
        }
        headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        }
        timeout = httpx.Timeout(connect=6.0, read=120.0, write=10.0, pool=None)

        try:
            async with httpx.AsyncClient(timeout=timeout, follow_redirects=True) as client:
                async with client.stream("POST", endpoint, headers=headers, json=payload) as response:
                    response.raise_for_status()
                    raw_content = await self._parse_streaming_response(response)
        except httpx.HTTPStatusError as exc:
            detail = exc.response.text.strip() if exc.response is not None else str(exc)
            status_code = exc.response.status_code if exc.response is not None else "?"
            raise GrokSearchAdapterError(f"GrokSearch HTTP 错误: {status_code} {detail}") from exc
        except httpx.HTTPError as exc:
            raise GrokSearchAdapterError(f"GrokSearch 请求失败: {exc}") from exc

        raw_content = str(raw_content or "").strip()
        if not raw_content:
            raise GrokSearchAdapterError("GrokSearch 返回内容为空")

        answer, raw_sources = split_answer_and_sources(raw_content)
        normalized_sources = self._normalize_sources(raw_sources)
        return GrokSearchAdapterResult(
            content=(answer or raw_content).strip(),
            sources=normalized_sources,
        )

    @staticmethod
    async def _parse_streaming_response(response: httpx.Response) -> str:
        content_parts: list[str] = []
        full_body_buffer: list[str] = []

        async for raw_line in response.aiter_lines():
            line = raw_line.strip()
            if not line:
                continue
            full_body_buffer.append(line)
            if not line.startswith("data:"):
                continue
            if line in ("data: [DONE]", "data:[DONE]"):
                continue
            try:
                data = json.loads(line[5:].lstrip())
            except json.JSONDecodeError:
                continue

            choices = data.get("choices", [])
            if not choices:
                continue
            delta = choices[0].get("delta", {})
            if "content" in delta:
                content_parts.append(str(delta["content"]))

        if not content_parts and full_body_buffer:
            merged = "".join(full_body_buffer)
            try:
                data = json.loads(merged)
            except json.JSONDecodeError:
                data = None
            if isinstance(data, dict) and data.get("choices"):
                message = data["choices"][0].get("message", {})
                message_content = str(message.get("content") or "").strip()
                if message_content:
                    content_parts.append(message_content)

        return "".join(content_parts).strip()

    @staticmethod
    def _normalize_sources(value: Any) -> list[dict[str, str]]:
        sources: list[dict[str, str]] = []
        if not isinstance(value, list):
            return sources

        for item in value:
            if not isinstance(item, dict):
                continue
            title = str(item.get("title") or item.get("url") or "未命名来源").strip()
            url = str(item.get("url") or "").strip()
            snippet = str(
                item.get("snippet")
                or item.get("summary")
                or item.get("description")
                or item.get("title")
                or ""
            ).strip()
            if not url and not snippet and title == "未命名来源":
                continue
            sources.append(
                {
                    "title": title or url or "未命名来源",
                    "url": url,
                    "snippet": snippet,
                }
            )
        return sources


class ChapterWebResearchService:
    MEMORY_TYPE = "research_reference"
    WORLD_MEMORY_TYPE = "research_world_building"
    CAREERS_MEMORY_TYPE = "research_careers"
    CHARACTERS_MEMORY_TYPE = "research_characters"
    OUTLINE_MEMORY_TYPE = "research_outline"
    MAX_SUMMARY_CHARS = 360
    MAX_RAW_CHARS = 4000

    def _default_runtime_config(self) -> WebResearchRuntimeConfig:
        return WebResearchRuntimeConfig(
            enabled=bool(settings.pre_generation_web_research_enabled),
            exa_enabled=bool(settings.pre_generation_web_research_exa_enabled),
            grok_enabled=bool(settings.pre_generation_web_research_grok_enabled),
            grok_model=DEFAULT_GROK_MODEL,
            grok_search_enabled=bool(settings.pre_generation_web_research_grok_search_enabled),
            timeout_seconds=max(15, int(settings.pre_generation_web_research_timeout_seconds)),
            max_assets=max(1, int(settings.pre_generation_web_research_max_assets)),
        )

    def build_runtime_config(
        self,
        *,
        preferences: Optional[Mapping[str, Any]] = None,
        overrides: Optional[Mapping[str, Any]] = None,
    ) -> WebResearchRuntimeConfig:
        default = self._default_runtime_config()
        pref_payload = {}
        if isinstance(preferences, Mapping):
            value = preferences.get(WEB_RESEARCH_PREF_KEY)
            if isinstance(value, Mapping):
                pref_payload = dict(value)
        payload = {
            "enabled": pref_payload.get("enabled", pref_payload.get("web_research_enabled", default.enabled)),
            "exa_enabled": pref_payload.get("exa_enabled", pref_payload.get("web_research_exa_enabled", default.exa_enabled)),
            "grok_enabled": pref_payload.get("grok_enabled", pref_payload.get("web_research_grok_enabled", default.grok_enabled)),
            "exa_api_key": str(pref_payload.get("exa_api_key") or pref_payload.get("web_research_exa_api_key") or "").strip(),
            "exa_base_url": str(pref_payload.get("exa_base_url") or pref_payload.get("web_research_exa_base_url") or "").strip(),
            "grok_api_key": str(pref_payload.get("grok_api_key") or pref_payload.get("web_research_grok_api_key") or "").strip(),
            "grok_base_url": str(pref_payload.get("grok_base_url") or pref_payload.get("web_research_grok_base_url") or "").strip(),
            "grok_model": str(pref_payload.get("grok_model") or pref_payload.get("web_research_grok_model") or DEFAULT_GROK_MODEL).strip() or DEFAULT_GROK_MODEL,
            "grok_search_enabled": pref_payload.get("grok_search_enabled", pref_payload.get("web_research_grok_search_enabled", default.grok_search_enabled)),
            "timeout_seconds": pref_payload.get("timeout_seconds", default.timeout_seconds),
            "max_assets": pref_payload.get("max_assets", default.max_assets),
        }
        if overrides:
            for key, value in overrides.items():
                if key in payload and value is not None:
                    payload[key] = value
        return WebResearchRuntimeConfig(
            enabled=None if payload["enabled"] is None else bool(payload["enabled"]),
            exa_enabled=bool(payload["exa_enabled"]),
            grok_enabled=bool(payload["grok_enabled"]),
            exa_api_key=str(payload["exa_api_key"] or "").strip(),
            exa_base_url=str(payload["exa_base_url"] or "").strip(),
            grok_api_key=str(payload["grok_api_key"] or "").strip(),
            grok_base_url=str(payload["grok_base_url"] or "").strip(),
            grok_model=str(payload["grok_model"] or DEFAULT_GROK_MODEL).strip() or DEFAULT_GROK_MODEL,
            grok_search_enabled=bool(payload["grok_search_enabled"]),
            timeout_seconds=max(15, int(float(payload["timeout_seconds"] or default.timeout_seconds))),
            max_assets=max(1, int(payload["max_assets"] or default.max_assets)),
        )

    async def get_runtime_config(self, *, user_id: Optional[str], db_session: Optional[AsyncSession]) -> WebResearchRuntimeConfig:
        from sqlalchemy import select

        from migrator_app.models import Settings

        if not user_id or db_session is None:
            return self._default_runtime_config()
        result = await db_session.execute(select(Settings).where(Settings.user_id == user_id))
        user_settings = result.scalar_one_or_none()
        if not user_settings:
            return self._default_runtime_config()
        try:
            preferences = json.loads(user_settings.preferences or "{}")
            if not isinstance(preferences, dict):
                preferences = {}
        except json.JSONDecodeError:
            preferences = {}
        return self.build_runtime_config(preferences=preferences)

    def is_enabled(self, requested: Optional[bool], runtime_config: Optional[WebResearchRuntimeConfig] = None) -> bool:
        if requested is not None:
            return bool(requested)
        if runtime_config and runtime_config.enabled is not None:
            return bool(runtime_config.enabled)
        return bool(settings.pre_generation_web_research_enabled)

    def skills_root(self) -> Path:
        skill_root = Path(settings.pre_generation_web_research_skill_repo_path).expanduser()
        if not skill_root.is_absolute():
            skill_root = (PROJECT_ROOT / skill_root).resolve()
        return skill_root

    @staticmethod
    def _clean_text(value: Optional[str]) -> str:
        if not value:
            return ""
        return " ".join(str(value).replace("\r", " ").replace("\n", " ").split()).strip()

    @classmethod
    def _clip_text(cls, value: Optional[str], limit: int) -> str:
        text = cls._clean_text(value)
        return text if len(text) <= limit else text[: limit - 3].rstrip() + "..."

    def _chapter_exa_query(self, project: Project, chapter: Chapter, outline: Optional[Outline], story_creation_brief: Optional[str], query_override: Optional[str]) -> str:
        if query_override:
            return self._clip_text(query_override, 320)
        parts = [project.genre, project.theme, chapter.title, getattr(outline, 'title', None), getattr(outline, 'summary', None), getattr(outline, 'content', None), story_creation_brief]
        cleaned = [self._clip_text(item, 140) for item in parts if self._clean_text(item)]
        return self._clip_text(" | ".join(cleaned[:4]), 320) if cleaned else ""

    def _chapter_grok_query(self, project: Project, chapter: Chapter, outline: Optional[Outline], story_creation_brief: Optional[str], query_override: Optional[str]) -> str:
        if query_override:
            return f"请围绕以下小说创作主题进行实时网络研究，并给出来源：{self._clip_text(query_override, 260)}"
        context = "；".join(part for part in [
            f"项目类型：{self._clip_text(project.genre, 40)}" if self._clean_text(project.genre) else "",
            f"主题：{self._clip_text(project.theme, 50)}" if self._clean_text(project.theme) else "",
            f"章节标题：{self._clip_text(chapter.title, 60)}" if self._clean_text(chapter.title) else "",
            f"章节大纲：{self._clip_text(getattr(outline, 'content', None) or getattr(outline, 'summary', None), 180)}" if self._clean_text(getattr(outline, 'content', None) or getattr(outline, 'summary', None)) else "",
            f"创作总控摘要：{self._clip_text(story_creation_brief, 120)}" if self._clean_text(story_creation_brief) else "",
        ] if part)
        return f"请为小说章节创作做实时网络研究，优先提炼事实、职业细节、社会情绪与可借鉴表达，并保留来源。背景：{context}" if context else ""
    async def _run_skill_script(self, *, skill_dir_name: str, script_name: str, args: List[str], timeout_seconds: int) -> Dict[str, Any]:
        skill_root = self.skills_root() / skill_dir_name
        script_path = skill_root / "scripts" / script_name
        if not script_path.exists():
            return {"error": "script_not_found", "detail": f"脚本不存在: {script_path}"}

        process = await asyncio.create_subprocess_exec(
            sys.executable,
            str(script_path),
            *args,
            cwd=str(skill_root),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        try:
            stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=timeout_seconds)
        except asyncio.TimeoutError:
            process.kill()
            await process.communicate()
            return {"error": "timeout", "detail": f"{skill_dir_name} 调用超时"}

        stdout_text = (stdout or b"").decode("utf-8", errors="replace").strip()
        stderr_text = (stderr or b"").decode("utf-8", errors="replace").strip()
        if not stdout_text:
            return {"error": "empty_stdout", "detail": stderr_text or f"{skill_dir_name} 未返回结果"}
        try:
            payload = json.loads(stdout_text)
        except json.JSONDecodeError:
            payload = {"error": "invalid_json", "detail": self._clip_text(stdout_text, 1200)}
        if process.returncode != 0 and not payload.get("error"):
            payload["error"] = f"exit_code_{process.returncode}"
            payload["detail"] = stderr_text or payload.get("detail") or "技能脚本执行失败"
        elif stderr_text and not payload.get("detail"):
            payload["detail"] = self._clip_text(stderr_text, 600)
        return payload

    @staticmethod
    def _resolve_exa_search_url(base_url: Optional[str]) -> str:
        normalized = str(base_url or DEFAULT_EXA_BASE_URL).strip() or DEFAULT_EXA_BASE_URL
        normalized = normalized.rstrip("/")
        if normalized.endswith("/search"):
            return normalized
        return f"{normalized}/search"

    @staticmethod
    def _resolve_openai_compatible_base_url(base_url: Optional[str]) -> str:
        normalized = str(base_url or "").strip()
        if not normalized:
            return ""

        parts = urlsplit(normalized)
        path = (parts.path or "").rstrip("/")
        if not path:
            path = "/v1"

        return urlunsplit((parts.scheme, parts.netloc, path, parts.query, parts.fragment)).rstrip("/")

    @staticmethod
    def _extract_json_object(text: str) -> Optional[Dict[str, Any]]:
        if not text:
            return None
        candidate = text.strip()
        if candidate.startswith("```"):
            parts = candidate.split("```")
            if len(parts) >= 3:
                candidate = parts[1]
                if "\n" in candidate:
                    candidate = candidate.split("\n", 1)[1]
        start = candidate.find("{")
        end = candidate.rfind("}")
        if start >= 0 and end > start:
            candidate = candidate[start : end + 1]
        try:
            parsed = json.loads(candidate)
        except json.JSONDecodeError:
            return None
        return parsed if isinstance(parsed, dict) else None

    @staticmethod
    def _normalize_sources(value: Any) -> List[Dict[str, str]]:
        sources: List[Dict[str, str]] = []
        if not isinstance(value, list):
            return sources
        for item in value:
            if not isinstance(item, Mapping):
                continue
            title = str(item.get("title") or item.get("url") or "").strip()
            url = str(item.get("url") or "").strip()
            snippet = str(item.get("snippet") or item.get("summary") or item.get("title") or "").strip()
            if not title and not url and not snippet:
                continue
            sources.append({
                "title": title or url or "来源",
                "url": url,
                "snippet": snippet,
            })
        return sources

    @staticmethod
    def _should_retry_as_stream(exc: Exception) -> bool:
        message = str(exc or "")
        return "非 JSON 内容" in message or "chat.completion.chunk" in message or "data:" in message

    async def _collect_stream_completion(
        self,
        *,
        client: OpenAIClient,
        messages: List[Dict[str, str]],
        model: str,
        temperature: float,
        max_tokens: int,
    ) -> Dict[str, Any]:
        parts: List[str] = []
        tool_calls: Optional[List[Dict[str, Any]]] = None

        async for chunk in client.chat_completion_stream(
            messages=messages,
            model=model,
            temperature=temperature,
            max_tokens=max_tokens,
        ):
            content = chunk.get("content")
            if isinstance(content, str) and content:
                parts.append(content)
            if chunk.get("tool_calls"):
                tool_calls = chunk.get("tool_calls")

        return {"content": "".join(parts).strip(), "tool_calls": tool_calls}

    @staticmethod
    def _can_run_direct_exa_search(runtime_config: WebResearchRuntimeConfig) -> bool:
        return bool(runtime_config.exa_api_key)

    @staticmethod
    def _can_run_direct_grok_search(runtime_config: WebResearchRuntimeConfig) -> bool:
        return bool(runtime_config.grok_api_key and runtime_config.grok_base_url)

    async def _run_exa_direct_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if not runtime_config.exa_api_key:
            return {"error": "missing_exa_credentials", "detail": "Exa API Key 为空"}

        request_url = self._resolve_exa_search_url(runtime_config.exa_base_url)
        timeout = httpx.Timeout(10.0, read=max(15.0, float(runtime_config.timeout_seconds)))
        try:
            async with httpx.AsyncClient(timeout=timeout) as client:
                response = await client.post(
                    request_url,
                    headers={
                        "Authorization": f"Bearer {runtime_config.exa_api_key}",
                        "Content-Type": "application/json",
                    },
                    json={"query": query, "numResults": 3},
                )
                response.raise_for_status()
                payload = response.json()
        except httpx.HTTPStatusError as exc:
            detail = exc.response.text.strip() if exc.response is not None else str(exc)
            return {
                "error": "direct_exa_http_error",
                "detail": self._clip_text(f"HTTP {exc.response.status_code}: {detail}", 600),
            }
        except (httpx.HTTPError, ValueError) as exc:
            return {"error": "direct_exa_request_failed", "detail": self._clip_text(str(exc), 600)}

        if not isinstance(payload, dict):
            return {"error": "invalid_exa_response", "detail": "Exa 返回格式不是 JSON 对象"}

        payload.setdefault("results", [])
        payload["mode"] = "direct_search_api"
        payload["request_url"] = request_url
        return payload

    async def _run_exa_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if runtime_config.exa_base_url:
            return await self._run_exa_direct_search(query, runtime_config)

        args = ["search", "--query", query, "--num", "3", "--text"]
        if runtime_config.exa_api_key:
            args.extend(["--api-key", runtime_config.exa_api_key])
        payload = await self._run_skill_script(
            skill_dir_name="exa-search",
            script_name="exa_search.py",
            args=args,
            timeout_seconds=runtime_config.timeout_seconds,
        )
        if payload.get("error") == "script_not_found" and self._can_run_direct_exa_search(runtime_config):
            logger.warning("⚠️ Exa skill script missing, fallback to direct Exa API search: %s", payload.get("detail"))
            return await self._run_exa_direct_search(query, runtime_config)
        return payload

    async def _run_grok_search_via_adapter(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if not runtime_config.grok_search_enabled:
            return {"error": "grok_search_disabled", "detail": "GrokSearch disabled"}

        adapter = GrokSearchAdapter()

        try:
            result = await adapter.search(
                query=query,
                api_key=runtime_config.grok_api_key,
                api_base_url=runtime_config.grok_base_url,
                model=runtime_config.grok_model or DEFAULT_GROK_MODEL,
            )
        except GrokSearchAdapterError as exc:
            return {"error": "grok_search_adapter_failed", "detail": self._clip_text(str(exc), 600)}

        return {
            "content": self._clip_text(result.content, self.MAX_RAW_CHARS),
            "sources": self._normalize_sources(result.sources),
            "mode": result.mode,
        }

    async def _run_grok_direct_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if not self._can_run_direct_grok_search(runtime_config):
            return {"error": "missing_grok_credentials", "detail": "Grok API Key 或 Base URL 为空"}

        resolved_base_url = self._resolve_openai_compatible_base_url(runtime_config.grok_base_url)
        messages = [
            {
                "role": "system",
                "content": (
                    "You are a web research assistant. Return JSON only with keys content and sources. "
                    "sources must be an array of objects with title, url, snippet. "
                    "If you do not have reliable source URLs, return an empty array."
                ),
            },
            {
                "role": "user",
                "content": f"Research this topic and keep it concise: {query}",
            },
        ]

        client = OpenAIClient(
            api_key=runtime_config.grok_api_key,
            base_url=resolved_base_url,
            compat_profile="openai",
        )
        try:
            response = await client.chat_completion(
                messages=messages,
                model=runtime_config.grok_model or DEFAULT_GROK_MODEL,
                temperature=0.2,
                max_tokens=512,
            )
        except RuntimeError as exc:
            if not self._should_retry_as_stream(exc):
                return {"error": "direct_grok_search_failed", "detail": self._clip_text(str(exc), 600)}
            try:
                response = await self._collect_stream_completion(
                    client=client,
                    messages=messages,
                    model=runtime_config.grok_model or DEFAULT_GROK_MODEL,
                    temperature=0.2,
                    max_tokens=512,
                )
            except Exception as stream_exc:
                return {"error": "direct_grok_search_failed", "detail": self._clip_text(str(stream_exc), 600)}
        except Exception as exc:
            return {"error": "direct_grok_search_failed", "detail": self._clip_text(str(exc), 600)}

        raw_content = str(response.get("content") or "").strip()
        if not raw_content:
            return {"error": "empty_response", "detail": "Grok 兼容接口已连接，但检索返回内容为空"}

        structured = self._extract_json_object(raw_content)
        if structured:
            content = self._clip_text(str(structured.get("content") or structured.get("summary") or raw_content), self.MAX_RAW_CHARS)
            return {
                "content": content,
                "sources": self._normalize_sources(structured.get("sources")),
                "mode": "direct_chat_search",
            }

        return {
            "content": self._clip_text(raw_content, self.MAX_RAW_CHARS),
            "sources": [],
            "mode": "direct_chat_search",
        }


    def _build_source_backfill_candidates(self, payload: Dict[str, Any]) -> List[Dict[str, str]]:
        candidates: List[Dict[str, str]] = []
        for item in (payload.get("results") or [])[:2]:
            url = self._clip_text(item.get("url") or "", 300)
            title = self._clip_text(item.get("title") or url or "Exa Source", 120)
            snippet = self._clip_text(item.get("text") or " ".join(str(text) for text in (item.get("highlights") or [])[:3]) or title, 220)
            if not url and not snippet:
                continue
            candidates.append({
                "title": title,
                "url": url,
                "snippet": snippet,
            })
        return self._normalize_sources(candidates)

    async def _maybe_backfill_grok_sources(
        self,
        *,
        query: str,
        payload: Dict[str, Any],
        runtime_config: WebResearchRuntimeConfig,
    ) -> Dict[str, Any]:
        if payload.get("error") or not str(payload.get("content") or "").strip():
            return payload
        if payload.get("sources"):
            return payload
        if not runtime_config.exa_enabled:
            return payload

        exa_payload = await self._run_exa_search(query, runtime_config)
        backfilled_sources = self._build_source_backfill_candidates(exa_payload)
        if not backfilled_sources:
            return payload

        merged_payload = dict(payload)
        merged_payload["sources"] = backfilled_sources
        merged_payload["sources_backfilled"] = True
        merged_payload["sources_backfill_provider"] = "exa"
        base_mode = str(merged_payload.get("mode") or "grok_search")
        merged_payload["mode"] = f"{base_mode}+exa_backfill"
        return merged_payload
    async def _run_grok_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if runtime_config.grok_search_enabled:
            adapter_payload = await self._run_grok_search_via_adapter(query, runtime_config)
            if not adapter_payload.get("error"):
                return await self._maybe_backfill_grok_sources(
                    query=query,
                    payload=adapter_payload,
                    runtime_config=runtime_config,
                )
            logger.warning("GrokSearch adapter failed, fallback to existing grok path: %s", adapter_payload.get("detail") or adapter_payload.get("error"))

        args = ["--mode", "research", "--query", query]
        if runtime_config.grok_api_key:
            args.extend(["--api-key", runtime_config.grok_api_key])
        if runtime_config.grok_base_url:
            args.extend(["--base-url", runtime_config.grok_base_url])
        if runtime_config.grok_model:
            args.extend(["--model", runtime_config.grok_model])
        payload = await self._run_skill_script(
            skill_dir_name="grok-search",
            script_name="grok_search.py",
            args=args,
            timeout_seconds=runtime_config.timeout_seconds,
        )
        if payload.get("error") == "script_not_found" and self._can_run_direct_grok_search(runtime_config):
            logger.warning("Grok skill script missing, fallback to OpenAI-compatible direct search: %s", payload.get("detail"))
            payload = await self._run_grok_direct_search(query, runtime_config)
        return await self._maybe_backfill_grok_sources(
            query=query,
            payload=payload,
            runtime_config=runtime_config,
        )

    async def _test_grok_direct_connection(self, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        if not runtime_config.grok_api_key or not runtime_config.grok_base_url:
            return {"error": "missing_grok_credentials", "detail": "Grok API Key 或 Base URL 为空"}

        resolved_base_url = self._resolve_openai_compatible_base_url(runtime_config.grok_base_url)
        messages = [
            {"role": "system", "content": "You are a connection test assistant."},
            {"role": "user", "content": "Reply with OK and one short sentence."},
        ]

        client = OpenAIClient(
            api_key=runtime_config.grok_api_key,
            base_url=resolved_base_url,
            compat_profile="openai",
        )
        try:
            response = await client.chat_completion(
                messages=messages,
                model=runtime_config.grok_model or DEFAULT_GROK_MODEL,
                temperature=0.0,
                max_tokens=48,
            )
        except RuntimeError as exc:
            if not self._should_retry_as_stream(exc):
                return {"error": "direct_connection_failed", "detail": self._clip_text(str(exc), 600)}
            try:
                response = await self._collect_stream_completion(
                    client=client,
                    messages=messages,
                    model=runtime_config.grok_model or DEFAULT_GROK_MODEL,
                    temperature=0.0,
                    max_tokens=48,
                )
            except Exception as stream_exc:
                return {"error": "direct_connection_failed", "detail": self._clip_text(str(stream_exc), 600)}
        except Exception as exc:
            return {"error": "direct_connection_failed", "detail": self._clip_text(str(exc), 600)}

        content = self._clip_text(response.get("content"), self.MAX_RAW_CHARS)
        if not content:
            return {"error": "empty_response", "detail": "Grok 兼容接口已连接，但返回内容为空"}

        return {"content": content, "sources": [], "mode": "direct_chat_test"}

    def _build_exa_assets(self, payload: Dict[str, Any]) -> List[Dict[str, str]]:
        if not isinstance(payload, dict) or payload.get("error"):
            return []
        assets: List[Dict[str, str]] = []
        for item in (payload.get("results") or [])[:2]:
            title = self._clip_text(item.get("title") or item.get("url") or "Exa 参考资料", 120)
            source = self._clip_text(item.get("url") or "exa-search", 300)
            highlights = item.get("highlights") or []
            summary = self._clip_text(" ".join(str(text) for text in highlights[:3]) or item.get("text") or title, self.MAX_SUMMARY_CHARS)
            if not summary:
                continue
            raw_content = self._clip_text(item.get("text") or "\n".join(str(text) for text in highlights[:5]), self.MAX_RAW_CHARS)
            assets.append({
                "title": title,
                "source": source,
                "summary": summary,
                "usage_hint": "用于补强真实设定、职业/地点/历史细节，吸收信息结构，不要直接照抄原文。",
                "asset_type": "exa_search_result",
                "raw_content": raw_content,
            })
        return assets

    def _build_grok_assets(self, payload: Dict[str, Any]) -> List[Dict[str, str]]:
        if not isinstance(payload, dict) or payload.get("error"):
            return []
        assets: List[Dict[str, str]] = []
        content = self._clip_text(payload.get("content"), self.MAX_SUMMARY_CHARS)
        raw_content = self._clip_text(payload.get("content"), self.MAX_RAW_CHARS)
        sources = payload.get("sources") or []
        primary_source = self._clip_text((sources[0] or {}).get("url") if sources else "grok-search", 300)
        if content:
            assets.append({
                "title": "Grok 实时综述",
                "source": primary_source or "grok-search",
                "summary": content,
                "usage_hint": "用于提炼当下语感、讨论热点和社会氛围，避免把观点原样写成正文。",
                "asset_type": "grok_search_summary",
                "raw_content": raw_content,
            })
        for item in sources[:2]:
            title = self._clip_text(item.get("title") or item.get("url") or "Grok 来源", 120)
            source = self._clip_text(item.get("url") or "grok-search", 300)
            summary = self._clip_text(item.get("snippet") or item.get("title") or source, 220)
            if not summary:
                continue
            assets.append({
                "title": title,
                "source": source,
                "summary": summary,
                "usage_hint": "作为外部讨论样本参考，用来优化用词、氛围与现实感。",
                "asset_type": "grok_search_source",
                "raw_content": summary,
            })
        return assets

    def _write_archive(self, *, archive_scope: str, archive_id: str, bundle: Dict[str, Any]) -> str:
        archive_dir = PROJECT_ROOT / "data" / "web_research" / (archive_scope or "misc")
        archive_dir.mkdir(parents=True, exist_ok=True)
        output_path = archive_dir / f"{archive_id}.json"
        with open(output_path, "w", encoding="utf-8") as file:
            json.dump(bundle, file, ensure_ascii=False, indent=2)
        return str(output_path)

    async def collect_assets(
        self,
        *,
        user_id: Optional[str],
        db_session: Optional[AsyncSession],
        exa_query: str,
        grok_query: str,
        enable_web_research: Optional[bool],
        archive_scope: str,
        archive_id: str,
        metadata: Optional[Dict[str, Any]] = None,
        runtime_config: Optional[WebResearchRuntimeConfig] = None,
    ) -> Dict[str, Any]:
        resolved_config = runtime_config or await self.get_runtime_config(user_id=user_id, db_session=db_session)
        if not self.is_enabled(enable_web_research, resolved_config):
            return {"enabled": False, "assets": [], "query": "", "archive_path": ""}
        skills_root_exists = self.skills_root().exists()
        direct_search_available = (
            (resolved_config.exa_enabled and bool(exa_query) and self._can_run_direct_exa_search(resolved_config))
            or (resolved_config.grok_enabled and bool(grok_query) and self._can_run_direct_grok_search(resolved_config))
        )
        if not skills_root_exists and not direct_search_available:
            logger.warning("⚠️ 外部检索技能目录不存在，跳过预生成检索: %s", self.skills_root())
            return {"enabled": True, "assets": [], "query": "", "archive_path": "", "skip_reason": "skills_root_missing"}
        if not skills_root_exists:
            logger.warning("⚠️ 外部检索技能目录不存在，尝试使用 API 直连回退: %s", self.skills_root())
        if not exa_query and not grok_query:
            return {"enabled": True, "assets": [], "query": "", "archive_path": "", "skip_reason": "empty_query"}

        exa_payload: Dict[str, Any] = {}
        grok_payload: Dict[str, Any] = {}
        if resolved_config.exa_enabled and exa_query:
            exa_payload = await self._run_exa_search(exa_query, resolved_config)
        if resolved_config.grok_enabled and grok_query:
            grok_payload = await self._run_grok_search(grok_query, resolved_config)

        assets = (self._build_exa_assets(exa_payload) + self._build_grok_assets(grok_payload))[: resolved_config.max_assets]
        bundle = {
            "generated_at": datetime.now().isoformat(),
            "query": {"exa": exa_query, "grok": grok_query},
            "assets": assets,
            "exa": exa_payload,
            "grok": grok_payload,
        }
        if metadata:
            bundle.update(metadata)
        archive_path = self._write_archive(archive_scope=archive_scope, archive_id=archive_id, bundle=bundle)
        return {
            "enabled": True,
            "assets": assets,
            "query": exa_query or grok_query,
            "archive_path": archive_path,
            "exa": exa_payload,
            "grok": grok_payload,
        }
    async def collect_for_chapter(
        self,
        *,
        user_id: str,
        db_session: AsyncSession,
        project: Project,
        chapter: Chapter,
        outline: Optional[Outline],
        story_creation_brief: Optional[str],
        enable_web_research: Optional[bool],
        web_research_query: Optional[str],
    ) -> Dict[str, Any]:
        return await self.collect_assets(
            user_id=user_id,
            db_session=db_session,
            exa_query=self._chapter_exa_query(project, chapter, outline, story_creation_brief, web_research_query),
            grok_query=self._chapter_grok_query(project, chapter, outline, story_creation_brief, web_research_query),
            enable_web_research=enable_web_research,
            archive_scope=project.id,
            archive_id=chapter.id,
            metadata={"project_id": project.id, "chapter_id": chapter.id, "chapter_number": chapter.chapter_number},
        )

    async def replace_memories(
        self,
        *,
        db_session: AsyncSession,
        user_id: str,
        project_id: str,
        query: str,
        archive_path: str,
        assets: List[Dict[str, str]],
        memory_type: str,
        title_prefix: str,
        story_timeline: int,
        chapter_id: Optional[str] = None,
    ) -> List[str]:
        from sqlalchemy import select
        from migrator_app.models import StoryMemory

        if not assets:
            return []
        where_conditions = [StoryMemory.project_id == project_id, StoryMemory.memory_type == memory_type]
        where_conditions.append(StoryMemory.chapter_id == chapter_id if chapter_id else StoryMemory.chapter_id.is_(None))
        existing_result = await db_session.execute(select(StoryMemory).where(*where_conditions))
        for item in list(existing_result.scalars().all()):
            await db_session.delete(item)
        await db_session.flush()
        await memory_service.delete_memories_by_types(
            user_id=user_id,
            project_id=project_id,
            chapter_id=chapter_id,
            memory_types=[memory_type],
        )

        saved_ids: List[str] = []
        for index, asset in enumerate(assets, start=1):
            memory_id = str(uuid.uuid4())
            title = self._clip_text(f"{title_prefix} {index}: {asset.get('title') or '未命名资料'}", 180)
            summary = self._clip_text(asset.get("summary"), 500)
            memory_content = self._clip_text(f"{title} 来源：{asset.get('source') or '未知来源'} 摘要：{summary}", 600)
            full_context = json.dumps({"query": query, "archive_path": archive_path, "asset": asset}, ensure_ascii=False)
            db_session.add(StoryMemory(
                id=memory_id,
                project_id=project_id,
                chapter_id=chapter_id,
                memory_type=memory_type,
                title=title,
                content=summary or memory_content,
                full_context=full_context,
                tags=["web_research", asset.get("asset_type") or "external_asset"],
                importance_score=0.62,
                story_timeline=story_timeline,
                chapter_position=0,
                text_length=len(summary or memory_content),
                vector_id=memory_id,
            ))
            await memory_service.add_memory(
                user_id=user_id,
                project_id=project_id,
                memory_id=memory_id,
                content=memory_content,
                memory_type=memory_type,
                metadata={
                    "chapter_id": chapter_id or "",
                    "chapter_number": story_timeline,
                    "importance_score": 0.62,
                    "tags": ["web_research", asset.get("asset_type") or "external_asset"],
                    "title": title,
                },
            )
            saved_ids.append(memory_id)
        await db_session.commit()
        return saved_ids

    async def replace_chapter_memories(self, *, db_session: AsyncSession, user_id: str, project: Project, chapter: Chapter, query: str, archive_path: str, assets: List[Dict[str, str]]) -> List[str]:
        return await self.replace_memories(
            db_session=db_session,
            user_id=user_id,
            project_id=project.id,
            query=query,
            archive_path=archive_path,
            assets=assets,
            memory_type=self.MEMORY_TYPE,
            title_prefix="外部资料",
            story_timeline=chapter.chapter_number,
            chapter_id=chapter.id,
        )

    async def test_provider_connection(self, *, provider: str, overrides: Mapping[str, Any], query: Optional[str] = None) -> Dict[str, Any]:
        runtime_config = self.build_runtime_config(overrides=overrides)
        provider_name = (provider or "").strip().lower()
        if provider_name == "exa":
            exa_query = query or "historical fiction writing details with reliable sources"
            payload = (
                await self._run_exa_direct_search(exa_query, runtime_config)
                if self._can_run_direct_exa_search(runtime_config)
                else await self._run_exa_search(exa_query, runtime_config)
            )
            results = payload.get("results") or []
            success = not payload.get("error") and bool(results)
            error_type = None
            if payload.get("error"):
                error_type = "DirectApiError" if payload.get("mode") == "direct_search_api" or self._can_run_direct_exa_search(runtime_config) else "SkillError"
            return {
                "success": success,
                "provider": "exa",
                "message": "Exa 连接测试成功" if success else "Exa 连接测试失败",
                "response_preview": self._clip_text(((results[0] or {}).get("text") if results else "") or ((results[0] or {}).get("title") if results else ""), 180),
                "result_count": len(results),
                "search_status": "success_with_sources" if success else "failed",
                "status_note": None,
                "error": payload.get("detail") or payload.get("error"),
                "error_type": error_type,
                "suggestions": [] if success else ["检查 Exa API Key 是否正确", "确认 Exa Base URL 可访问；未填写时会使用默认地址"],
            }
        payload = await self._run_grok_search(query or "Summarize current discussion around fiction writing trends with sources", runtime_config)
        if payload.get("error") == "script_not_found" and self._can_run_direct_grok_search(runtime_config):
            logger.warning("Grok skill script missing, fallback to OpenAI-compatible direct test: %s", payload.get("detail"))
            payload = await self._test_grok_direct_connection(runtime_config)
        sources = payload.get("sources") or []
        content = self._clip_text(payload.get("content"), 180)
        success = not payload.get("error") and bool(content)
        search_status = "failed"
        status_note = None
        message = "Grok 连接测试失败"
        if success and sources:
            search_status = "success_with_sources"
            message = "Grok 连接测试成功"
            if payload.get("sources_backfilled"):
                status_note = "已联网检索，来源已由 Exa 自动补全。"
                message = "Grok 连接测试成功（来源已自动补全）"
        elif success:
            search_status = "success_without_sources"
            status_note = "已联网检索并返回摘要，但本次未返回可展示来源。"
            message = "Grok 连接测试成功（未返回结构化来源）"
        return {
            "success": success,
            "provider": "grok",
            "message": message,
            "response_preview": content,
            "source_count": len(sources),
            "search_status": search_status,
            "status_note": status_note,
            "sources_backfilled": bool(payload.get("sources_backfilled")),
            "error": payload.get("detail") or payload.get("error"),
            "error_type": "SkillError" if payload.get("error") else None,
            "suggestions": [] if success else ["检查 Grok API Key 是否正确", "确认 Grok Base URL 可访问且兼容 OpenAI 格式"],
        }

chapter_web_research_service = ChapterWebResearchService()


