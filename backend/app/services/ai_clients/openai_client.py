"""OpenAI client with OpenAI-compatible and Responses API support."""
import asyncio
import json
import time
from datetime import datetime, timezone
from typing import Any, AsyncGenerator, Dict, List, Optional

import httpx

from app.logger import get_logger
from .base_client import BaseAIClient

logger = get_logger(__name__)


class OpenAIClient(BaseAIClient):
    """OpenAI API client."""

    def __init__(
        self,
        api_key: str,
        base_url: str,
        config=None,
        backup_urls: Optional[List[str]] = None,
        compat_profile: str = "openai",
    ):
        """
        Initialize OpenAI client.

        Args:
            api_key: API key
            base_url: API base URL
            config: client config
            backup_urls: fallback URL list
            compat_profile: compatibility profile (openai/newapi/azure/custom/sub2api)
        """
        super().__init__(api_key, base_url, config, backup_urls)
        self.compat_profile = compat_profile.lower()
        self._responses_profiles = {"sub2api", "openai_responses"}

    def _use_responses_api(self) -> bool:
        """Whether this profile should use /responses wire API."""
        return self.compat_profile in self._responses_profiles

    @staticmethod
    def _should_fallback_from_responses(
        error: httpx.HTTPStatusError,
        tools: Optional[list],
    ) -> bool:
        """Whether tool-calling requests should retry via /chat/completions."""
        if not tools:
            return False

        status_code = error.response.status_code if error.response is not None else None
        if status_code is None:
            return False

        return status_code >= 500 or status_code in {404, 405, 415, 422}

    @staticmethod
    def _should_retry_chat_completions_candidate(
        error: Exception,
        tools: Optional[list],
        *,
        allow_without_tools: bool = False,
    ) -> bool:
        """Whether another chat-completions base URL candidate should be tried."""
        if isinstance(error, httpx.HTTPStatusError):
            if allow_without_tools:
                status_code = error.response.status_code if error.response is not None else None
                return status_code is not None and (
                    status_code >= 500 or status_code in {404, 405, 415, 422}
                )
            return OpenAIClient._should_fallback_from_responses(error, tools)

        if isinstance(error, (httpx.ConnectError, httpx.TimeoutException)):
            return allow_without_tools or bool(tools)

        if not isinstance(error, RuntimeError):
            return False
        if not allow_without_tools and not tools:
            return False

        message = str(error)
        lowered = message.lower()
        return (
            'non json' in lowered
            or 'non-json' in lowered
            or 'non sse' in lowered
            or 'base url' in lowered
            or '/v1' in message
            or 'doctype html' in lowered
        )

    def _build_chat_completions_base_url_candidates(
        self,
        prefer_normalized_v1_candidate: bool = False,
    ) -> List[str]:
        """Build fallback base URLs for OpenAI-compatible chat completions."""
        primary = self.base_url.rstrip('/')
        normalized_v1 = primary if primary.endswith('/v1') else f'{primary}/v1'

        if self.compat_profile == 'sub2api':
            candidates = [normalized_v1]
        elif prefer_normalized_v1_candidate and not primary.endswith('/v1'):
            candidates = [normalized_v1, primary]
        else:
            candidates = [primary]
            if not primary.endswith('/v1'):
                candidates.append(normalized_v1)

        unique_candidates: List[str] = []
        for candidate in candidates:
            if candidate not in unique_candidates:
                unique_candidates.append(candidate)
        return unique_candidates

    def _start_transport_trace(self, operation: str, *, prefer_chat_completions: bool) -> None:
        profile_uses_responses = self._use_responses_api()
        initial_api_mode = "responses" if profile_uses_responses else "chat_completions"
        requested_api_mode = "chat_completions" if prefer_chat_completions and profile_uses_responses else initial_api_mode
        self.reset_transport_diagnostics(
            {
                "operation": operation,
                "compat_profile": self.compat_profile,
                "profile_uses_responses": profile_uses_responses,
                "requested_api_mode": requested_api_mode,
                "prefer_chat_completions": prefer_chat_completions,
                "original_base_url": self.base_url,
                "request_started_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
                "_request_started_perf_counter": time.perf_counter(),
            }
        )
        self._record_transport_event(
            "api_mode_selected",
            api_mode=requested_api_mode,
            reason="initial_request",
        )
        if requested_api_mode != initial_api_mode:
            self._record_transport_event(
                "api_mode_forced",
                from_api_mode=initial_api_mode,
                to_api_mode=requested_api_mode,
                reason="prefer_chat_completions",
            )

    def _record_api_mode_fallback(self, from_api_mode: str, to_api_mode: str, error: Exception) -> None:
        status_code = None
        if isinstance(error, httpx.HTTPStatusError) and error.response is not None:
            status_code = error.response.status_code
        self._record_transport_event(
            "api_mode_fallback",
            from_api_mode=from_api_mode,
            to_api_mode=to_api_mode,
            error_type=type(error).__name__,
            error_message=str(error),
            status_code=status_code,
            latency_ms=self._get_transport_elapsed_ms(),
        )

    def _get_transport_elapsed_ms(self) -> Optional[float]:
        diagnostics = self._ensure_transport_diagnostics()
        started_at = diagnostics.get("_request_started_perf_counter")
        if not isinstance(started_at, (int, float)):
            return None
        return round(max((time.perf_counter() - float(started_at)) * 1000, 0.0), 2)

    def _get_last_successful_transport_attempt(self) -> Optional[Dict[str, Any]]:
        diagnostics = self._ensure_transport_diagnostics()
        attempts = diagnostics.get("attempts") or []
        for attempt in reversed(attempts):
            if attempt.get("result") == "success":
                return attempt
        return None

    def _record_first_stream_chunk(
        self,
        *,
        api_mode: Optional[str],
        endpoint: str,
        base_url: Optional[str],
    ) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        if diagnostics.get("first_chunk_latency_ms") is not None:
            return
        latency_ms = self._get_transport_elapsed_ms()
        self._set_transport_diagnostic_values(
            first_chunk_latency_ms=latency_ms,
            first_chunk_api_mode=api_mode,
            first_chunk_endpoint_path=endpoint,
            first_chunk_base_url=base_url,
        )
        self._record_transport_event(
            "first_chunk_received",
            api_mode=api_mode,
            endpoint_path=endpoint,
            base_url=base_url,
            latency_ms=latency_ms,
        )

    def _record_stream_completion(
        self,
        *,
        api_mode: Optional[str],
        endpoint: str,
        base_url: Optional[str],
        chunk_count: int,
        done_emitted: bool,
    ) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        if diagnostics.get("stream_completed_latency_ms") is not None:
            return
        latency_ms = self._get_transport_elapsed_ms()
        self._set_transport_diagnostic_values(
            stream_completed_latency_ms=latency_ms,
            stream_completed_api_mode=api_mode,
            stream_completed_endpoint_path=endpoint,
            stream_completed_base_url=base_url,
            stream_chunk_count=chunk_count,
        )
        self._record_transport_event(
            "stream_completed",
            api_mode=api_mode,
            endpoint_path=endpoint,
            base_url=base_url,
            latency_ms=latency_ms,
            chunk_count=chunk_count,
            done_emitted=done_emitted,
        )

    def _record_request_completion(
        self,
        *,
        api_mode: Optional[str],
        endpoint: str,
        base_url: Optional[str],
        finish_reason: Optional[str],
    ) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        if diagnostics.get("request_completed_latency_ms") is not None:
            return
        latency_ms = self._get_transport_elapsed_ms()
        self._set_transport_diagnostic_values(
            request_completed_latency_ms=latency_ms,
            request_completed_api_mode=api_mode,
            request_completed_endpoint_path=endpoint,
            request_completed_base_url=base_url,
            request_finish_reason=finish_reason,
        )
        self._record_transport_event(
            "request_completed",
            api_mode=api_mode,
            endpoint_path=endpoint,
            base_url=base_url,
            latency_ms=latency_ms,
            finish_reason=finish_reason,
        )


    async def _request_chat_completions_fallback(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
        allow_without_tools: bool = False,
    ) -> Dict[str, Any]:
        """Retry a failed Responses tool request via chat-completions candidates."""
        payload = self._build_payload(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            use_responses_api=False,
        )
        logger.debug(
            f'OpenAI fallback payload: {json.dumps(payload, ensure_ascii=False, indent=2)}'
        )

        prefer_normalized_v1_candidate = bool(
            (request_options or {}).get("prefer_normalized_v1_candidate")
        )
        base_url_candidates = self._build_chat_completions_base_url_candidates(
            prefer_normalized_v1_candidate=prefer_normalized_v1_candidate,
        )
        original_base_url = self.base_url
        last_exception: Optional[Exception] = None

        for index, candidate_base_url in enumerate(base_url_candidates):
            self._record_transport_event(
                "chat_completions_candidate_selected",
                api_mode="chat_completions",
                candidate_base_url=candidate_base_url,
                original_base_url=original_base_url,
                candidate_index=index + 1,
                candidate_count=len(base_url_candidates),
                latency_ms=self._get_transport_elapsed_ms(),
            )
            if candidate_base_url != original_base_url:
                logger.warning(
                    'Retrying Chat Completions fallback with normalized base URL: '
                    f'{candidate_base_url}'
                )

            try:
                request_kwargs = {"base_url_override": candidate_base_url}
                if request_options is not None:
                    request_kwargs["request_options"] = request_options
                data = await self._request_with_retry('POST', '/chat/completions', payload, **request_kwargs)
                self._record_transport_event(
                    "chat_completions_candidate_succeeded",
                    api_mode="chat_completions",
                    candidate_base_url=candidate_base_url,
                    candidate_index=index + 1,
                    latency_ms=self._get_transport_elapsed_ms(),
                )
                return data
            except (httpx.HTTPStatusError, httpx.ConnectError, httpx.TimeoutException, RuntimeError) as exc:
                last_exception = exc
                has_next_candidate = index < len(base_url_candidates) - 1
                self._record_transport_event(
                    "chat_completions_candidate_failed",
                    api_mode="chat_completions",
                    candidate_base_url=candidate_base_url,
                    candidate_index=index + 1,
                    error_type=type(exc).__name__,
                    error_message=str(exc),
                    status_code=exc.response.status_code if isinstance(exc, httpx.HTTPStatusError) and exc.response is not None else None,
                    latency_ms=self._get_transport_elapsed_ms(),
                )
                if has_next_candidate and self._should_retry_chat_completions_candidate(exc, tools, allow_without_tools=allow_without_tools):
                    logger.warning(
                        'Chat Completions fallback failed for base URL '
                        f'{candidate_base_url}; trying next candidate'
                    )
                    continue
                raise

        if last_exception is not None:
            raise last_exception
        raise RuntimeError('Chat Completions fallback failed without a captured exception')

    def _build_headers(self) -> Dict[str, str]:
        """Build request headers based on provider profile."""
        if self.compat_profile == "azure":
            return {
                "api-key": self.api_key,
                "Content-Type": "application/json",
            }

        return {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }


    @staticmethod
    def _sanitize_tools(tools: Optional[list]) -> Optional[list]:
        """Remove unsupported $schema keys from function parameter schema."""
        if not tools:
            return None

        cleaned: List[dict] = []
        for tool in tools:
            current = json.loads(json.dumps(tool, ensure_ascii=False))
            if "function" in current and "parameters" in current["function"]:
                current["function"]["parameters"] = {
                    k: v
                    for k, v in current["function"]["parameters"].items()
                    if k != "$schema"
                }
            elif "parameters" in current:
                current["parameters"] = {
                    k: v
                    for k, v in current["parameters"].items()
                    if k != "$schema"
                }
            cleaned.append(current)
        return cleaned

    @staticmethod
    def _build_responses_input(messages: list) -> List[dict]:
        """Convert chat-completions messages to Responses API input blocks."""
        normalized_messages: List[dict] = []
        for message in messages:
            if not isinstance(message, dict):
                continue

            role = str(message.get("role") or "user")
            content = message.get("content")
            blocks: List[dict] = []

            if isinstance(content, list):
                for block in content:
                    if isinstance(block, dict):
                        block_type = str(block.get("type") or "").strip()
                        if block_type in {"input_text", "input_image", "input_audio"}:
                            blocks.append(block)
                            continue
                        if block_type in {"text", "output_text"}:
                            text_value = str(block.get("text") or "").strip()
                            if text_value:
                                blocks.append({"type": "input_text", "text": text_value})
                            continue
                    text_value = str(block or "").strip()
                    if text_value:
                        blocks.append({"type": "input_text", "text": text_value})
            else:
                text_value = str(content or "").strip()
                if text_value:
                    blocks.append({"type": "input_text", "text": text_value})

            if blocks:
                normalized_messages.append({"role": role, "content": blocks})
        return normalized_messages

    @staticmethod
    def _build_responses_tools(tools: Optional[list]) -> Optional[list]:
        """Convert chat-completions tool definitions to Responses API function schema."""
        if not tools:
            return None

        converted_tools: List[dict] = []
        for tool in tools:
            if not isinstance(tool, dict):
                continue

            current = json.loads(json.dumps(tool, ensure_ascii=False))
            if current.get("type") != "function":
                converted_tools.append(current)
                continue

            function_payload = current.get("function") if isinstance(current.get("function"), dict) else current
            converted_tool = {
                "type": "function",
                "name": function_payload.get("name"),
                "description": function_payload.get("description"),
                "parameters": function_payload.get("parameters"),
            }
            strict = function_payload.get("strict", current.get("strict"))
            if strict is not None:
                converted_tool["strict"] = strict
            converted_tools.append({k: v for k, v in converted_tool.items() if v is not None})
        return converted_tools or None

    def _build_payload(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        stream: bool = False,
        use_responses_api: Optional[bool] = None,
    ) -> Dict[str, Any]:
        cleaned_tools = self._sanitize_tools(tools)

        if use_responses_api is None:
            use_responses_api = self._use_responses_api()

        if use_responses_api:
            payload: Dict[str, Any] = {
                "model": model,
                "input": self._build_responses_input(messages),
                "temperature": temperature,
                "max_output_tokens": max_tokens,
            }
            if stream:
                payload["stream"] = True
            responses_tools = self._build_responses_tools(cleaned_tools)
            if responses_tools:
                payload["tools"] = responses_tools
                if tool_choice:
                    payload["tool_choice"] = tool_choice
            return payload

        payload = {
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        if stream:
            payload["stream"] = True
        if cleaned_tools:
            payload["tools"] = cleaned_tools
            if tool_choice:
                payload["tool_choice"] = tool_choice
        return payload

    @staticmethod
    def _extract_response_text(data: Dict[str, Any]) -> str:
        """Extract assistant text from Responses API response payload."""
        output_text = data.get("output_text")
        if isinstance(output_text, str) and output_text:
            return output_text

        text_chunks: List[str] = []
        output_items = data.get("output", [])
        if isinstance(output_items, list):
            for item in output_items:
                if not isinstance(item, dict):
                    continue
                if item.get("type") != "message":
                    continue
                for block in item.get("content", []):
                    if not isinstance(block, dict):
                        continue
                    if block.get("type") in {"output_text", "text"}:
                        text = block.get("text")
                        if isinstance(text, str) and text:
                            text_chunks.append(text)
        return "".join(text_chunks)

    @staticmethod
    def _parse_response_tool_call(item: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Convert a Responses API function_call item to OpenAI tool_calls format."""
        if item.get("type") not in {"function_call", "tool_call"}:
            return None

        name = item.get("name") or item.get("function", {}).get("name")
        raw_arguments = item.get("arguments")
        if raw_arguments is None:
            raw_arguments = item.get("function", {}).get("arguments", "")

        if isinstance(raw_arguments, dict):
            arguments = json.dumps(raw_arguments, ensure_ascii=False)
        elif isinstance(raw_arguments, str):
            arguments = raw_arguments
        else:
            arguments = ""

        call_id = item.get("call_id") or item.get("id") or f"call_{name or 'unknown'}"
        return {
            "id": call_id,
            "type": "function",
            "function": {
                "name": name or "unknown_function",
                "arguments": arguments,
            },
        }

    def _extract_response_tool_calls(self, data: Dict[str, Any]) -> Optional[List[Dict[str, Any]]]:
        """Extract all function/tool calls from Responses API response payload."""
        tool_calls: List[Dict[str, Any]] = []
        output_items = data.get("output", [])
        if isinstance(output_items, list):
            for item in output_items:
                if not isinstance(item, dict):
                    continue
                parsed = self._parse_response_tool_call(item)
                if parsed:
                    tool_calls.append(parsed)
        return tool_calls or None

    @staticmethod
    def _merge_tool_call_chunk(
        buffer: Dict[str, Dict[str, Any]],
        call_id: str,
        name: Optional[str] = None,
        arguments_delta: Optional[str] = None,
    ) -> None:
        """Merge streaming function_call argument chunks by call id."""
        if call_id not in buffer:
            buffer[call_id] = {
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name or "unknown_function",
                    "arguments": "",
                },
            }
        if name:
            buffer[call_id]["function"]["name"] = name
        if arguments_delta:
            buffer[call_id]["function"]["arguments"] += arguments_delta

    @staticmethod
    def _extract_message_content_value(value: Any) -> str:
        if isinstance(value, str):
            return value
        if isinstance(value, list):
            chunks: List[str] = []
            for item in value:
                if isinstance(item, str):
                    chunks.append(item)
                    continue
                if not isinstance(item, dict):
                    continue
                text = item.get("text")
                if isinstance(text, str) and text:
                    chunks.append(text)
            return "".join(chunks)
        return ""

    def _parse_chat_completions_sse_text(self, raw_sse_text: str) -> Dict[str, Any]:
        content_parts: List[str] = []
        tool_calls_buffer: Dict[str, Dict[str, Any]] = {}
        finish_reason: Optional[str] = None

        for raw_line in raw_sse_text.splitlines():
            line = raw_line.strip()
            if not line.startswith("data: "):
                continue

            data_str = line[6:].strip()
            if not data_str or data_str == "[DONE]":
                continue

            try:
                data = json.loads(data_str)
            except json.JSONDecodeError:
                continue

            choices = data.get("choices", [])
            if not choices:
                continue

            choice = choices[0]
            finish_reason = choice.get("finish_reason") or finish_reason

            delta = choice.get("delta") or {}
            delta_content = self._extract_message_content_value(delta.get("content"))
            if delta_content:
                content_parts.append(delta_content)

            message = choice.get("message") or {}
            message_content = self._extract_message_content_value(message.get("content"))
            if message_content:
                content_parts.append(message_content)

            text_content = choice.get("text")
            if isinstance(text_content, str) and text_content:
                content_parts.append(text_content)

            tc_list = delta.get("tool_calls") or message.get("tool_calls")
            if tc_list:
                for tc in tc_list:
                    index = str(tc.get("index", len(tool_calls_buffer)))
                    if index not in tool_calls_buffer:
                        tool_calls_buffer[index] = tc
                    else:
                        existing = tool_calls_buffer[index]
                        if "function" in tc and "function" in existing:
                            arguments_delta = tc["function"].get("arguments")
                            if arguments_delta:
                                existing["function"]["arguments"] = (
                                    existing["function"].get("arguments", "") + arguments_delta
                                )

        return {
            "content": "".join(content_parts),
            "tool_calls": list(tool_calls_buffer.values()) or None,
            "finish_reason": finish_reason or "stop",
        }

    def _parse_chat_completions_response(self, data: Dict[str, Any]) -> Dict[str, Any]:
        choices = data.get("choices", [])
        if not choices:
            return {
                "content": "",
                "tool_calls": None,
                "finish_reason": data.get("finish_reason") or "stop",
            }

        choice = choices[0]
        message = choice.get("message") or {}
        return {
            "content": self._extract_message_content_value(message.get("content")),
            "tool_calls": message.get("tool_calls"),
            "finish_reason": choice.get("finish_reason") or "stop",
        }

    async def chat_completion(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        prefer_chat_completions = bool((request_options or {}).get("prefer_chat_completions"))
        prefer_normalized_v1_candidate = bool(
            (request_options or {}).get("prefer_normalized_v1_candidate")
        )
        self._start_transport_trace(
            "chat_completion",
            prefer_chat_completions=prefer_chat_completions,
        )
        use_responses_api = self._use_responses_api() and not prefer_chat_completions
        payload = self._build_payload(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            use_responses_api=use_responses_api,
        )

        logger.debug(f"OpenAI request payload: {json.dumps(payload, ensure_ascii=False, indent=2)}")

        endpoint = "/responses" if use_responses_api else "/chat/completions"
        if prefer_chat_completions and self._use_responses_api():
            logger.info('Preferring Chat Completions fallback for this request')
            fallback_kwargs = {"allow_without_tools": True}
            if request_options is not None:
                fallback_kwargs["request_options"] = request_options
            data = await self._request_chat_completions_fallback(
                messages,
                model,
                temperature,
                max_tokens,
                tools,
                tool_choice,
                **fallback_kwargs,
            )
            use_responses_api = False
            endpoint = "/chat/completions"
        elif prefer_normalized_v1_candidate and not use_responses_api:
            logger.info('Preferring normalized Chat Completions base URL candidates for this request')
            fallback_kwargs = {"allow_without_tools": True}
            if request_options is not None:
                fallback_kwargs["request_options"] = request_options
            data = await self._request_chat_completions_fallback(
                messages,
                model,
                temperature,
                max_tokens,
                tools,
                tool_choice,
                **fallback_kwargs,
            )
            endpoint = "/chat/completions"
        else:
            try:
                request_kwargs = {}
                if request_options is not None:
                    request_kwargs["request_options"] = request_options
                data = await self._request_with_retry("POST", endpoint, payload, **request_kwargs)
            except (httpx.HTTPStatusError, httpx.ConnectError, httpx.TimeoutException, RuntimeError) as exc:
                should_fallback = False
                if use_responses_api:
                    if isinstance(exc, httpx.HTTPStatusError):
                        should_fallback = self._should_fallback_from_responses(exc, tools)
                    else:
                        should_fallback = self._should_retry_chat_completions_candidate(exc, tools)

                if should_fallback:
                    logger.warning(
                        'Responses API tool request failed '
                        f'({type(exc).__name__}); retrying via Chat Completions fallback'
                    )
                    self._record_api_mode_fallback("responses", "chat_completions", exc)
                    use_responses_api = False
                    fallback_kwargs = {}
                    if request_options is not None:
                        fallback_kwargs["request_options"] = request_options
                    data = await self._request_chat_completions_fallback(
                        messages,
                        model,
                        temperature,
                        max_tokens,
                        tools,
                        tool_choice,
                        **fallback_kwargs,
                    )
                    endpoint = "/chat/completions"
                else:
                    raise

        logger.debug(f"OpenAI raw response: {json.dumps(data, ensure_ascii=False, indent=2)}")

        successful_attempt = self._get_last_successful_transport_attempt()
        final_base_url = successful_attempt.get("base_url") if successful_attempt else self.base_url
        final_api_mode = "responses" if use_responses_api else "chat_completions"

        if not use_responses_api and isinstance(data, dict) and data.get("_raw_sse_text"):
            result = self._parse_chat_completions_sse_text(str(data.get("_raw_sse_text") or ""))
            self._record_request_completion(
                api_mode=final_api_mode,
                endpoint=endpoint,
                base_url=final_base_url,
                finish_reason=result.get("finish_reason"),
            )
            return result

        if use_responses_api:
            tool_calls = self._extract_response_tool_calls(data)
            status = data.get("status")
            if tool_calls:
                finish_reason = "tool_calls"
            elif status in {"completed", "succeeded", None}:
                finish_reason = "stop"
            elif status == "incomplete":
                finish_reason = "length"
            else:
                finish_reason = status

            result = {
                "content": self._extract_response_text(data),
                "tool_calls": tool_calls or None,
                "finish_reason": finish_reason,
                "raw_response": data,
            }
            self._record_request_completion(
                api_mode=final_api_mode,
                endpoint=endpoint,
                base_url=final_base_url,
                finish_reason=finish_reason,
            )
            return result

        result = self._parse_chat_completions_response(data)
        self._record_request_completion(
            api_mode=final_api_mode,
            endpoint=endpoint,
            base_url=final_base_url,
            finish_reason=result.get("finish_reason"),
        )
        return result

    async def chat_completion_stream(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        prefer_chat_completions = bool((request_options or {}).get("prefer_chat_completions"))
        prefer_normalized_v1_candidate = bool(
            (request_options or {}).get("prefer_normalized_v1_candidate")
        )
        self._start_transport_trace(
            "chat_completion_stream",
            prefer_chat_completions=prefer_chat_completions,
        )
        use_responses_api = self._use_responses_api() and not prefer_chat_completions
        payload = self._build_payload(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            stream=True,
            use_responses_api=use_responses_api,
        )

        if prefer_chat_completions and self._use_responses_api():
            logger.info('Preferring Chat Completions stream fallback for this request')
            async for chunk in self._chat_completions_stream_with_fallback(
                messages,
                model,
                temperature,
                max_tokens,
                tools,
                tool_choice,
                request_options=request_options,
            ):
                yield chunk
            return

        if prefer_normalized_v1_candidate and not use_responses_api:
            logger.info('Preferring normalized Chat Completions stream base URL candidates for this request')
            async for chunk in self._chat_completions_stream_with_fallback(
                messages,
                model,
                temperature,
                max_tokens,
                tools,
                tool_choice,
                request_options=request_options,
            ):
                yield chunk
            return

        if use_responses_api:
            try:
                async for chunk in self._responses_stream(payload, request_options=request_options):
                    yield chunk
                return
            except (httpx.HTTPStatusError, httpx.ConnectError, httpx.TimeoutException, RuntimeError) as exc:
                if self._should_retry_chat_completions_candidate(
                    exc,
                    tools,
                    allow_without_tools=True,
                ):
                    logger.warning(
                        'Responses API stream request failed '
                        f'({type(exc).__name__}); retrying via Chat Completions stream fallback'
                    )
                    self._record_api_mode_fallback("responses", "chat_completions", exc)
                    async for chunk in self._chat_completions_stream_with_fallback(
                        messages,
                        model,
                        temperature,
                        max_tokens,
                        tools,
                        tool_choice,
                        request_options=request_options,
                    ):
                        yield chunk
                    return
                raise

        async for chunk in self._chat_completions_stream(payload, request_options=request_options):
            yield chunk

    async def _chat_completions_stream_with_fallback(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        request_options: Optional[Dict[str, Any]] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        """Retry a failed Responses stream request via chat-completions candidates."""
        payload = self._build_payload(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            stream=True,
            use_responses_api=False,
        )

        prefer_normalized_v1_candidate = bool(
            (request_options or {}).get("prefer_normalized_v1_candidate")
        )
        base_url_candidates = self._build_chat_completions_base_url_candidates(
            prefer_normalized_v1_candidate=prefer_normalized_v1_candidate,
        )
        original_base_url = self.base_url
        last_exception: Optional[Exception] = None

        for index, candidate_base_url in enumerate(base_url_candidates):
            self._record_transport_event(
                "chat_completions_candidate_selected",
                api_mode="chat_completions",
                candidate_base_url=candidate_base_url,
                original_base_url=original_base_url,
                candidate_index=index + 1,
                candidate_count=len(base_url_candidates),
                latency_ms=self._get_transport_elapsed_ms(),
            )
            if candidate_base_url != original_base_url:
                logger.warning(
                    'Retrying Chat Completions stream fallback with normalized base URL: '
                    f'{candidate_base_url}'
                )

            try:
                async for chunk in self._chat_completions_stream(
                    payload,
                    request_options=request_options,
                    base_url_override=candidate_base_url,
                ):
                    yield chunk
                self._record_transport_event(
                    "chat_completions_candidate_succeeded",
                    api_mode="chat_completions",
                    candidate_base_url=candidate_base_url,
                    candidate_index=index + 1,
                    latency_ms=self._get_transport_elapsed_ms(),
                )
                return
            except (httpx.HTTPStatusError, httpx.ConnectError, httpx.TimeoutException, RuntimeError) as exc:
                last_exception = exc
                has_next_candidate = index < len(base_url_candidates) - 1
                self._record_transport_event(
                    "chat_completions_candidate_failed",
                    api_mode="chat_completions",
                    candidate_base_url=candidate_base_url,
                    candidate_index=index + 1,
                    error_type=type(exc).__name__,
                    error_message=str(exc),
                    status_code=exc.response.status_code if isinstance(exc, httpx.HTTPStatusError) and exc.response is not None else None,
                    latency_ms=self._get_transport_elapsed_ms(),
                )
                if has_next_candidate and self._should_retry_chat_completions_candidate(
                    exc,
                    tools,
                    allow_without_tools=True,
                ):
                    logger.warning(
                        'Chat Completions stream fallback failed for base URL '
                        f'{candidate_base_url}; trying next candidate'
                    )
                    continue
                raise

        if last_exception is not None:
            raise last_exception
        raise RuntimeError('Chat Completions stream fallback failed without a captured exception')

    async def _iter_stream_lines_with_first_data_timeout(
        self,
        response: httpx.Response,
        endpoint: str,
        request_options: Optional[Dict[str, Any]] = None,
        request_base_url: Optional[str] = None,
    ) -> AsyncGenerator[str, None]:
        first_chunk_timeout = None
        if request_options is not None:
            configured_timeout = request_options.get("first_chunk_timeout")
            if configured_timeout is not None:
                first_chunk_timeout = float(configured_timeout)

        line_iterator = response.aiter_lines()
        api_mode = "responses" if endpoint == "/responses" else "chat_completions" if endpoint == "/chat/completions" else None
        effective_base_url = str(request_base_url or self.base_url).rstrip("/")
        if first_chunk_timeout is None or first_chunk_timeout <= 0:
            saw_first_data_line = False
            async for line in line_iterator:
                if line.startswith("data: ") and not saw_first_data_line:
                    self._record_first_stream_chunk(
                        api_mode=api_mode,
                        endpoint=endpoint,
                        base_url=effective_base_url,
                    )
                    saw_first_data_line = True
                yield line
            return

        loop = asyncio.get_running_loop()
        deadline = loop.time() + first_chunk_timeout
        saw_first_data_line = False
        request = getattr(response, "request", None)
        if request is None:
            fallback_base_url = str(request_base_url or self.base_url).rstrip("/")
            request = httpx.Request("POST", f"{fallback_base_url}{endpoint}")

        while True:
            try:
                if saw_first_data_line:
                    line = await line_iterator.__anext__()
                else:
                    remaining = deadline - loop.time()
                    if remaining <= 0:
                        raise asyncio.TimeoutError
                    line = await asyncio.wait_for(line_iterator.__anext__(), timeout=remaining)
            except StopAsyncIteration:
                break
            except asyncio.TimeoutError as exc:
                raise httpx.ReadTimeout(
                    f"Timed out waiting for first SSE data line from {endpoint}",
                    request=request,
                ) from exc

            if line.startswith("data: "):
                if not saw_first_data_line:
                    self._record_first_stream_chunk(
                        api_mode=api_mode,
                        endpoint=endpoint,
                        base_url=effective_base_url,
                    )
                saw_first_data_line = True
            yield line

    async def _chat_completions_stream(
        self,
        payload: Dict[str, Any],
        request_options: Optional[Dict[str, Any]] = None,
        base_url_override: Optional[str] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        tool_calls_buffer: Dict[str, Dict[str, Any]] = {}
        done_emitted = False
        saw_data_line = False
        emitted_chunk_count = 0
        request_base_url = str(base_url_override or self.base_url).rstrip("/")

        request_kwargs = {"stream": True}
        if request_options is not None:
            request_kwargs["request_options"] = request_options
        if base_url_override is not None:
            request_kwargs["base_url_override"] = base_url_override

        async with await self._request_with_retry(
            "POST",
            "/chat/completions",
            payload,
            **request_kwargs,
        ) as response:
            response.raise_for_status()
            async for line in self._iter_stream_lines_with_first_data_timeout(
                response,
                "/chat/completions",
                request_options=request_options,
                request_base_url=base_url_override,
            ):
                if not line.startswith("data: "):
                    continue

                saw_data_line = True

                data_str = line[6:].strip()
                if data_str == "[DONE]":
                    final_chunk_count = emitted_chunk_count + (1 if tool_calls_buffer else 0) + 1
                    self._record_stream_completion(
                        api_mode="chat_completions",
                        endpoint="/chat/completions",
                        base_url=request_base_url,
                        chunk_count=final_chunk_count,
                        done_emitted=True,
                    )
                    if tool_calls_buffer:
                        emitted_chunk_count += 1
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
                    emitted_chunk_count += 1
                    yield {"done": True}
                    done_emitted = True
                    break

                try:
                    data = json.loads(data_str)
                except json.JSONDecodeError:
                    continue

                choices = data.get("choices", [])
                if not choices:
                    continue

                delta = choices[0].get("delta", {})
                content = delta.get("content", "")
                if isinstance(content, str) and content:
                    emitted_chunk_count += 1
                    yield {"content": content}

                tc_list = delta.get("tool_calls")
                if tc_list:
                    for tc in tc_list:
                        index = str(tc.get("index", 0))
                        if index not in tool_calls_buffer:
                            tool_calls_buffer[index] = tc
                        else:
                            existing = tool_calls_buffer[index]
                            if "function" in tc and "function" in existing:
                                arguments_delta = tc["function"].get("arguments")
                                if arguments_delta:
                                    existing["function"]["arguments"] = (
                                        existing["function"].get("arguments", "") + arguments_delta
                                    )

        if not saw_data_line:
            raise RuntimeError(
                'Stream endpoint returned non-SSE content; Base URL path may be incorrect '
                '(missing /v1).'
            )

        if not done_emitted:
            final_chunk_count = emitted_chunk_count + (1 if tool_calls_buffer else 0) + 1
            self._record_stream_completion(
                api_mode="chat_completions",
                endpoint="/chat/completions",
                base_url=request_base_url,
                chunk_count=final_chunk_count,
                done_emitted=False,
            )
            if tool_calls_buffer:
                emitted_chunk_count += 1
                yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
            emitted_chunk_count += 1
            yield {"done": True}

    async def _responses_stream(
        self,
        payload: Dict[str, Any],
        request_options: Optional[Dict[str, Any]] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        tool_calls_buffer: Dict[str, Dict[str, Any]] = {}
        done_emitted = False
        saw_data_line = False
        emitted_chunk_count = 0

        request_kwargs = {"stream": True}
        if request_options is not None:
            request_kwargs["request_options"] = request_options

        async with await self._request_with_retry(
            "POST",
            "/responses",
            payload,
            **request_kwargs,
        ) as response:
            response.raise_for_status()
            async for line in self._iter_stream_lines_with_first_data_timeout(
                response,
                "/responses",
                request_options=request_options,
            ):
                if not line.startswith("data: "):
                    continue

                saw_data_line = True

                data_str = line[6:].strip()
                if data_str == "[DONE]":
                    final_chunk_count = emitted_chunk_count + (1 if tool_calls_buffer else 0) + 1
                    self._record_stream_completion(
                        api_mode="responses",
                        endpoint="/responses",
                        base_url=self.base_url,
                        chunk_count=final_chunk_count,
                        done_emitted=True,
                    )
                    if tool_calls_buffer:
                        emitted_chunk_count += 1
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
                    emitted_chunk_count += 1
                    yield {"done": True}
                    done_emitted = True
                    break

                try:
                    event = json.loads(data_str)
                except json.JSONDecodeError:
                    continue

                event_type = event.get("type", "")

                if event_type == "response.output_text.delta":
                    delta = event.get("delta")
                    if isinstance(delta, str) and delta:
                        emitted_chunk_count += 1
                        yield {"content": delta}
                    continue

                if event_type in {"response.output_item.added", "response.output_item.done"}:
                    item = event.get("item", {})
                    if isinstance(item, dict):
                        parsed = self._parse_response_tool_call(item)
                        if parsed:
                            tool_calls_buffer[parsed["id"]] = parsed
                    continue

                if event_type == "response.function_call_arguments.delta":
                    call_id = event.get("item_id") or event.get("call_id")
                    if isinstance(call_id, str) and call_id:
                        self._merge_tool_call_chunk(
                            tool_calls_buffer,
                            call_id=call_id,
                            name=event.get("name"),
                            arguments_delta=event.get("delta", ""),
                        )
                    continue

                if event_type == "response.function_call_arguments.done":
                    call_id = event.get("item_id") or event.get("call_id")
                    if isinstance(call_id, str) and call_id:
                        self._merge_tool_call_chunk(
                            tool_calls_buffer,
                            call_id=call_id,
                            name=event.get("name"),
                        )
                        final_arguments = event.get("arguments")
                        if isinstance(final_arguments, str):
                            tool_calls_buffer[call_id]["function"]["arguments"] = final_arguments
                    continue

                if event_type == "response.completed":
                    response_data = event.get("response", {})
                    if isinstance(response_data, dict):
                        final_calls = self._extract_response_tool_calls(response_data) or []
                        for call in final_calls:
                            tool_calls_buffer[call["id"]] = call
                    final_chunk_count = emitted_chunk_count + (1 if tool_calls_buffer else 0) + 1
                    self._record_stream_completion(
                        api_mode="responses",
                        endpoint="/responses",
                        base_url=self.base_url,
                        chunk_count=final_chunk_count,
                        done_emitted=True,
                    )
                    if tool_calls_buffer:
                        emitted_chunk_count += 1
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
                    emitted_chunk_count += 1
                    yield {"done": True}
                    done_emitted = True
                    break

                if event_type == "response.error":
                    raise ValueError(f"Responses API stream error: {event}")

        if not saw_data_line:
            raise RuntimeError(
                'Stream endpoint returned non-SSE content; Base URL path may be incorrect '
                '(missing /v1).'
            )

        if not done_emitted:
            final_chunk_count = emitted_chunk_count + (1 if tool_calls_buffer else 0) + 1
            self._record_stream_completion(
                api_mode="responses",
                endpoint="/responses",
                base_url=self.base_url,
                chunk_count=final_chunk_count,
                done_emitted=False,
            )
            if tool_calls_buffer:
                emitted_chunk_count += 1
                yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
            emitted_chunk_count += 1
            yield {"done": True}
