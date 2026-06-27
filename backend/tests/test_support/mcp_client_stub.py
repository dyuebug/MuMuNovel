"""Historical MCP client stub for retired Python MCP production package."""
from __future__ import annotations

from typing import Any


class MCPClientStub:
    async def ensure_registered(
        self,
        *,
        user_id: str,
        plugin_name: str,
        url: str,
        plugin_type: str,
        headers: dict[str, Any] | None = None,
    ) -> bool:
        _ = (user_id, plugin_name, url, plugin_type, headers)
        return True

    async def get_tools(self, user_id: str, plugin_name: str) -> list[dict[str, Any]]:
        _ = (user_id, plugin_name)
        return []

    def format_tools_for_openai(
        self,
        tools: list[dict[str, Any]],
        plugin_name: str,
    ) -> list[dict[str, Any]]:
        formatted = []
        for tool in tools:
            name = str(tool.get("name") or tool.get("function", {}).get("name") or "")
            if not name:
                continue
            formatted.append(
                {
                    "type": "function",
                    "function": {
                        "name": f"{plugin_name}_{name}",
                        "description": str(tool.get("description") or ""),
                        "parameters": tool.get("inputSchema") or tool.get("parameters") or {},
                    },
                }
            )
        return formatted

    async def batch_call_tools(
        self,
        *,
        user_id: str,
        tool_calls: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        _ = user_id
        return [
            {
                "tool_call_id": tool_call.get("id"),
                "name": (tool_call.get("function") or {}).get("name"),
                "success": True,
                "content": "",
            }
            for tool_call in tool_calls
        ]

    def build_tool_context(
        self,
        tool_results: list[dict[str, Any]],
        format: str = "markdown",
    ) -> str:
        _ = format
        if not tool_results:
            return "【MCP工具结果】\n- 无工具结果"
        lines = ["【MCP工具结果】"]
        for result in tool_results:
            name = result.get("name") or "unknown_tool"
            content = result.get("content") or ""
            lines.append(f"- {name}: {content}")
        return "\n".join(lines)


mcp_client = MCPClientStub()
