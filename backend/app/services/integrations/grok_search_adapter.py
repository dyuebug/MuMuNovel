"""内置版 GrokSearch 轻量适配器。"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any
from urllib.parse import urlsplit, urlunsplit

import httpx

from app.services.grok_search_embedded import SEARCH_PROMPT, split_answer_and_sources


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
            if not url and not snippet and not title:
                continue
            sources.append(
                {
                    "title": title or url or "未命名来源",
                    "url": url,
                    "snippet": snippet,
                }
            )
        return sources
