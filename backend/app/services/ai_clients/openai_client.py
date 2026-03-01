"""OpenAI client with OpenAI-compatible and Responses API support."""
import json
from typing import Any, AsyncGenerator, Dict, List, Optional

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
        self._responses_profiles = {"sub2api"}

    def _use_responses_api(self) -> bool:
        """Whether this profile should use /responses wire API."""
        return self.compat_profile in self._responses_profiles

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
            # Deep copy to avoid mutating original tool definitions.
            current = json.loads(json.dumps(tool, ensure_ascii=False))
            if "function" in current and "parameters" in current["function"]:
                current["function"]["parameters"] = {
                    k: v
                    for k, v in current["function"]["parameters"].items()
                    if k != "$schema"
                }
            cleaned.append(current)
        return cleaned

    def _build_payload(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
        stream: bool = False,
    ) -> Dict[str, Any]:
        cleaned_tools = self._sanitize_tools(tools)

        if self._use_responses_api():
            payload: Dict[str, Any] = {
                "model": model,
                "input": messages,
                "temperature": temperature,
                "max_output_tokens": max_tokens,
            }
            if stream:
                payload["stream"] = True
            if cleaned_tools:
                payload["tools"] = cleaned_tools
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

    async def chat_completion(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = self._build_payload(messages, model, temperature, max_tokens, tools, tool_choice)

        logger.debug(f"OpenAI request payload: {json.dumps(payload, ensure_ascii=False, indent=2)}")

        endpoint = "/responses" if self._use_responses_api() else "/chat/completions"
        data = await self._request_with_retry("POST", endpoint, payload)

        logger.debug(f"OpenAI raw response: {json.dumps(data, ensure_ascii=False, indent=2)}")

        if self._use_responses_api():
            tool_calls = self._extract_response_tool_calls(data)
            status = data.get("status")
            if tool_calls:
                finish_reason = "tool_calls"
            elif status in {"completed", "succeeded", None}:
                finish_reason = "stop"
            elif status == "incomplete":
                incomplete_reason = (data.get("incomplete_details") or {}).get("reason")
                finish_reason = "length" if incomplete_reason == "max_output_tokens" else "incomplete"
            else:
                finish_reason = str(status)

            return {
                "content": self._extract_response_text(data),
                "tool_calls": tool_calls,
                "finish_reason": finish_reason,
            }

        choices = data.get("choices", [])
        if not choices:
            raise ValueError("API returned empty choices")

        choice = choices[0]
        message = choice.get("message", {})
        return {
            "content": message.get("content", ""),
            "tool_calls": message.get("tool_calls"),
            "finish_reason": choice.get("finish_reason"),
        }

    async def chat_completion_stream(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        """
        Stream completion output.

        Yields dictionaries:
        - {"content": "..."}
        - {"tool_calls": [...], "done": True}
        - {"done": True}
        """
        payload = self._build_payload(
            messages,
            model,
            temperature,
            max_tokens,
            tools,
            tool_choice,
            stream=True,
        )

        if self._use_responses_api():
            async for chunk in self._responses_stream(payload):
                yield chunk
            return

        async for chunk in self._chat_completions_stream(payload):
            yield chunk

    async def _chat_completions_stream(self, payload: Dict[str, Any]) -> AsyncGenerator[Dict[str, Any], None]:
        tool_calls_buffer: Dict[str, Dict[str, Any]] = {}
        done_emitted = False

        async with await self._request_with_retry("POST", "/chat/completions", payload, stream=True) as response:
            response.raise_for_status()
            async for line in response.aiter_lines():
                if not line.startswith("data: "):
                    continue

                data_str = line[6:].strip()
                if data_str == "[DONE]":
                    if tool_calls_buffer:
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
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

        if not done_emitted:
            if tool_calls_buffer:
                yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
            yield {"done": True}

    async def _responses_stream(self, payload: Dict[str, Any]) -> AsyncGenerator[Dict[str, Any], None]:
        tool_calls_buffer: Dict[str, Dict[str, Any]] = {}
        done_emitted = False

        async with await self._request_with_retry("POST", "/responses", payload, stream=True) as response:
            response.raise_for_status()
            async for line in response.aiter_lines():
                if not line.startswith("data: "):
                    continue

                data_str = line[6:].strip()
                if data_str == "[DONE]":
                    if tool_calls_buffer:
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
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
                    if tool_calls_buffer:
                        yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
                    yield {"done": True}
                    done_emitted = True
                    break

                if event_type == "response.error":
                    raise ValueError(f"Responses API stream error: {event}")

        if not done_emitted:
            if tool_calls_buffer:
                yield {"tool_calls": list(tool_calls_buffer.values()), "done": True}
            yield {"done": True}
