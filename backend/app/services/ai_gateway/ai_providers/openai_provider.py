"""OpenAI Provider"""
import httpx
from typing import Any, AsyncGenerator, Dict, List, Optional

from app.logger import get_logger
from app.services.ai_gateway.ai_clients.openai_client import OpenAIClient
from .base_provider import BaseAIProvider

logger = get_logger(__name__)


class OpenAIProvider(BaseAIProvider):
    """OpenAI 提供商"""

    def __init__(self, client: OpenAIClient):
        self.client = client

    async def generate(
        self,
        prompt: str,
        model: str,
        temperature: float,
        max_tokens: int,
        system_prompt: Optional[str] = None,
        tools: Optional[List[Dict]] = None,
        tool_choice: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        messages.append({"role": "user", "content": prompt})

        client_kwargs = {
            "messages": messages,
            "model": model,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "tools": tools,
            "tool_choice": tool_choice,
        }
        if request_options is not None:
            client_kwargs["request_options"] = request_options
        return await self.client.chat_completion(**client_kwargs)

    async def generate_stream(
        self,
        prompt: str,
        model: str,
        temperature: float,
        max_tokens: int,
        system_prompt: Optional[str] = None,
        tools: Optional[List[Dict]] = None,
        tool_choice: Optional[str] = None,
        user_id: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> AsyncGenerator[str, None]:
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        messages.append({"role": "user", "content": prompt})

        if tools:
            logger.debug(f"OpenAIProvider: 有 {len(tools)} 个工具，使用流式处理")
            actual_tool_choice = tool_choice if tool_choice else "auto"
            tool_calls_buffer = []

            async for chunk in self.client.chat_completion_stream(
                messages=messages,
                model=model,
                temperature=temperature,
                max_tokens=max_tokens,
                tools=tools,
                tool_choice=actual_tool_choice,
                request_options=request_options,
            ):
                if chunk.get("tool_calls"):
                    tool_calls_buffer.extend(chunk["tool_calls"])
                    logger.debug(f"收到工具调用: {len(chunk['tool_calls'])} 个")

                if chunk.get("done"):
                    if tool_calls_buffer:
                        logger.info(f"流式结束，处理 {len(tool_calls_buffer)} 个工具调用")
                        from app.mcp import mcp_client

                        actual_user_id = user_id or ""
                        tool_results = await mcp_client.batch_call_tools(
                            user_id=actual_user_id,
                            tool_calls=tool_calls_buffer,
                        )
                        tool_context = mcp_client.build_tool_context(tool_results, format="markdown")

                        final_prompt = (
                            f"{prompt}\n\n"
                            f"{tool_context}\n\n"
                            "请基于以上工具查询结果，给出完整详细的回答。"
                        )
                        final_messages = messages.copy()
                        final_messages.append({"role": "user", "content": final_prompt})

                        async for final_chunk in self._generate_with_tools(
                            final_messages,
                            model,
                            temperature,
                            max_tokens,
                            tools,
                            user_id,
                            request_options,
                        ):
                            yield final_chunk
                    break

                if chunk.get("content"):
                    yield chunk["content"]
            return

        resolved_request_options = dict(request_options or {})
        allow_non_stream_fallback = bool(resolved_request_options.get("allow_non_stream_fallback", True))

        async def _fetch_non_stream_fallback_content() -> str:
            fallback_response = await self.client.chat_completion(
                messages=messages,
                model=model,
                temperature=temperature,
                max_tokens=max_tokens,
                request_options=resolved_request_options,
            )
            fallback_content = fallback_response.get("content", "")
            return fallback_content if isinstance(fallback_content, str) else ""

        emitted_content = False
        try:
            async for chunk in self.client.chat_completion_stream(
                messages=messages,
                model=model,
                temperature=temperature,
                max_tokens=max_tokens,
                request_options=resolved_request_options,
            ):
                if isinstance(chunk, dict):
                    if chunk.get("content"):
                        emitted_content = True
                        yield chunk["content"]
                else:
                    emitted_content = True
                    yield chunk
        except (httpx.HTTPStatusError, httpx.ConnectError, httpx.TimeoutException, RuntimeError) as exc:
            if emitted_content:
                raise

            if not allow_non_stream_fallback:
                logger.warning(
                    "OpenAIProvider stream failed before content and non-stream fallback is disabled: "
                    f"{exc}"
                )
                raise

            logger.warning(
                "OpenAIProvider stream failed before content; retrying once via non-stream completion: "
                f"{exc}"
            )
            fallback_content = await _fetch_non_stream_fallback_content()
            if fallback_content:
                yield fallback_content
            return

        if not emitted_content:
            if not allow_non_stream_fallback:
                logger.warning(
                    "OpenAIProvider stream completed without narrative content and non-stream fallback is disabled"
                )
                raise RuntimeError(
                    "OpenAIProvider stream completed without narrative content and non-stream fallback is disabled"
                )
            logger.warning(
                "OpenAIProvider stream completed without narrative content; "
                "retrying once via non-stream completion"
            )
            fallback_content = await _fetch_non_stream_fallback_content()
            if fallback_content:
                yield fallback_content

    async def _generate_with_tools(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: list,
        user_id: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> AsyncGenerator[str, None]:
        """辅助方法：带工具的流式生成（无 tool_choice，AI 自由决定）"""
        async for chunk in self.client.chat_completion_stream(
            messages=messages,
            model=model,
            temperature=temperature,
            max_tokens=max_tokens,
            tools=tools,
            tool_choice="auto",
            request_options=request_options,
        ):
            if chunk.get("tool_calls"):
                from app.mcp import mcp_client

                actual_user_id = user_id or ""
                tool_results = await mcp_client.batch_call_tools(
                    user_id=actual_user_id,
                    tool_calls=chunk["tool_calls"],
                )
                tool_context = mcp_client.build_tool_context(tool_results, format="markdown")

                messages.append(
                    {
                        "role": "user",
                        "content": (
                            f"{tool_context}\n\n"
                            "请基于以上工具查询结果，给出完整详细的回答。"
                        ),
                    }
                )

                async for final_chunk in self._generate_with_tools(
                    messages,
                    model,
                    temperature,
                    max_tokens,
                    tools,
                    user_id,
                    request_options,
                ):
                    yield final_chunk
                break

            if chunk.get("done"):
                break

            if chunk.get("content"):
                yield chunk["content"]
