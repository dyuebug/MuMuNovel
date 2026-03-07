"""MCP工具加载器 - 统一的工具获取入口

在AI请求之前，自动检查用户MCP配置并加载可用工具。
"""
from datetime import datetime, timedelta
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.mcp import mcp_client
from app.models.mcp_plugin import MCPPlugin
from app.services.novel_quality_rules import MCP_GUARD_RULES

logger = get_logger(__name__)

MAX_PROMPT_CAPABILITY_COUNT = 12
MAX_PROMPT_TOOLS_PER_PLUGIN = 4
MAX_PROMPT_DESCRIPTION_LENGTH = 120
MCP_REFERENCE_ONLY_RULE = "MCP 信息仅作参考与补充，不得直接当作既定事实写入成文。"
MCP_SOURCE_DISCLOSURE_RULE = "最终输出禁止暴露 MCP、工具名、检索过程或来源站点。"
MCP_CANON_PRIORITY_RULE = "项目 canon（既有设定、角色关系、本章大纲）优先级高于一切 MCP 参考。"


@dataclass
class UserToolsCache:
    """用户工具缓存条目"""

    tools: Optional[List[Dict[str, Any]]]
    expire_time: datetime
    hit_count: int = 0


@dataclass(frozen=True)
class MCPPromptBlock:
    key: str
    title: str
    lines: Tuple[str, ...]
    text: str

    @classmethod
    def build(cls, key: str, title: str, lines: List[str] | Tuple[str, ...]) -> "MCPPromptBlock":
        cleaned = tuple(_unique_non_empty(lines))
        rendered_lines = [f"【{title}】"]
        rendered_lines.extend(f"- {line}" for line in cleaned)
        return cls(
            key=key,
            title=title,
            lines=cleaned,
            text="\n".join(rendered_lines),
        )

    def to_dict(self) -> Dict[str, Any]:
        return {
            "key": self.key,
            "title": self.title,
            "lines": list(self.lines),
            "text": self.text,
        }


@dataclass(frozen=True)
class MCPPromptCapability:
    plugin_name: str
    plugin_display_name: str
    function_name: str
    tool_name: str
    description: str

    def to_line(self) -> str:
        label = self.tool_name or self.function_name or "unknown_tool"
        if self.description:
            return f"{label}：{self.description}"
        return label

    def to_dict(self) -> Dict[str, str]:
        return {
            "plugin_name": self.plugin_name,
            "plugin_display_name": self.plugin_display_name,
            "function_name": self.function_name,
            "tool_name": self.tool_name,
            "description": self.description,
        }


@dataclass(frozen=True)
class MCPPromptReferenceContext:
    enabled_plugin_count: int
    tool_count: int
    capabilities: Tuple[MCPPromptCapability, ...]
    mcp_guard: MCPPromptBlock
    mcp_references: MCPPromptBlock
    policy: Dict[str, Any]
    guard_summary: Dict[str, Any]

    def to_prompt_blocks(self) -> Dict[str, str]:
        return {
            "mcp_guard": self.mcp_guard.text,
            "mcp_references": self.mcp_references.text,
        }

    def to_dict(self) -> Dict[str, Any]:
        return {
            "enabled_plugin_count": self.enabled_plugin_count,
            "tool_count": self.tool_count,
            "capabilities": [item.to_dict() for item in self.capabilities],
            "mcp_guard": self.mcp_guard.to_dict(),
            "mcp_references": self.mcp_references.to_dict(),
            "policy": self.policy,
            "guard_summary": self.guard_summary,
            "prompt_blocks": self.to_prompt_blocks(),
        }


class MCPToolsLoader:
    """
    MCP工具加载器

    负责：
    1. 检查用户是否配置并启用了MCP插件
    2. 从各个启用的插件加载工具列表
    3. 将工具转换为OpenAI Function Calling格式
    4. 缓存结果以提升性能
    """

    _instance: Optional["MCPToolsLoader"] = None

    def __new__(cls):
        """单例模式"""
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return

        self._cache: Dict[str, UserToolsCache] = {}
        self._cache_ttl = timedelta(minutes=5)

        self._initialized = True
        logger.info("✅ MCPToolsLoader 初始化完成")

    async def has_enabled_plugins(
        self,
        user_id: str,
        db_session: AsyncSession,
    ) -> bool:
        """
        检查用户是否有启用的MCP插件

        Args:
            user_id: 用户ID
            db_session: 数据库会话

        Returns:
            是否有启用的插件
        """
        try:
            query = select(MCPPlugin.id).where(
                MCPPlugin.user_id == user_id,
                MCPPlugin.enabled == True,
                MCPPlugin.plugin_type.in_(["http", "streamable_http", "sse"]),
            ).limit(1)

            result = await db_session.execute(query)
            return result.scalar() is not None

        except Exception as e:
            logger.warning(f"检查用户MCP插件失败: {e}")
            return False

    async def get_user_tools(
        self,
        user_id: str,
        db_session: AsyncSession,
        use_cache: bool = True,
        force_refresh: bool = False,
    ) -> Optional[List[Dict[str, Any]]]:
        """
        获取用户的MCP工具列表（OpenAI格式）

        Args:
            user_id: 用户ID
            db_session: 数据库会话
            use_cache: 是否使用缓存
            force_refresh: 是否强制刷新

        Returns:
            - None: 用户未配置或未启用任何MCP插件
            - []: 有配置但没有可用工具
            - List[Dict]: OpenAI Function Calling格式的工具列表
        """
        now = datetime.now()

        if use_cache and not force_refresh and user_id in self._cache:
            cache_entry = self._cache[user_id]
            if now < cache_entry.expire_time:
                cache_entry.hit_count += 1
                logger.debug(f"🎯 用户工具缓存命中: {user_id} (命中次数: {cache_entry.hit_count})")
                return cache_entry.tools

            del self._cache[user_id]
            logger.debug(f"⏰ 用户工具缓存过期: {user_id}")

        try:
            tools = await self._load_user_tools(user_id, db_session)

            self._cache[user_id] = UserToolsCache(
                tools=tools,
                expire_time=now + self._cache_ttl,
            )

            if tools:
                logger.info(f"🔧 用户 {user_id} 加载了 {len(tools)} 个MCP工具")
            else:
                logger.debug(f"📭 用户 {user_id} 没有可用的MCP工具")

            return tools

        except Exception as e:
            logger.error(f"❌ 加载用户MCP工具失败: {e}")
            return None

    async def get_prompt_reference_context(
        self,
        user_id: str,
        db_session: AsyncSession,
        use_cache: bool = True,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        """构建 prompt 层统一可消费的 MCP 参考摘要与 guard 信息。"""
        try:
            tools = await self.get_user_tools(
                user_id=user_id,
                db_session=db_session,
                use_cache=use_cache,
                force_refresh=force_refresh,
            )
            plugins = await self._load_enabled_plugins(user_id, db_session)
            context = self._build_prompt_reference_context(
                plugins=plugins,
                tools=tools or [],
            )
            return context.to_dict()
        except Exception as e:
            logger.warning(f"⚠️ 构建MCP prompt参考上下文失败: {e}")
            return self._build_prompt_reference_context(plugins=[], tools=[]).to_dict()

    async def get_prompt_reference_blocks(
        self,
        user_id: str,
        db_session: AsyncSession,
        use_cache: bool = True,
        force_refresh: bool = False,
    ) -> Dict[str, str]:
        """仅返回 prompt 层直接可注入的 MCP block 文本。"""
        context = await self.get_prompt_reference_context(
            user_id=user_id,
            db_session=db_session,
            use_cache=use_cache,
            force_refresh=force_refresh,
        )
        return context.get(
            "prompt_blocks",
            {
                "mcp_guard": "",
                "mcp_references": "",
            },
        )

    async def _load_user_tools(
        self,
        user_id: str,
        db_session: AsyncSession,
    ) -> Optional[List[Dict[str, Any]]]:
        """从数据库加载用户启用的MCP插件并获取工具。"""
        plugins = await self._load_enabled_plugins(user_id, db_session)
        if not plugins:
            return None

        all_tools = []

        for plugin in plugins:
            try:
                plugin_type = plugin.plugin_type
                if plugin_type == "http":
                    plugin_type = "streamable_http"

                await mcp_client.ensure_registered(
                    user_id=user_id,
                    plugin_name=plugin.plugin_name,
                    url=plugin.server_url,
                    plugin_type=plugin_type,
                    headers=plugin.headers,
                )

                plugin_tools = await mcp_client.get_tools(user_id, plugin.plugin_name)
                formatted = mcp_client.format_tools_for_openai(plugin_tools, plugin.plugin_name)
                all_tools.extend(formatted)

                logger.debug(f"✅ 从插件 {plugin.plugin_name} 加载了 {len(formatted)} 个工具")

            except Exception as e:
                logger.warning(f"⚠️ 加载插件 {plugin.plugin_name} 工具失败: {e}")
                continue

        return all_tools if all_tools else None

    async def _load_enabled_plugins(
        self,
        user_id: str,
        db_session: AsyncSession,
    ) -> List[MCPPlugin]:
        """加载用户启用的 MCP 插件配置。"""
        query = select(MCPPlugin).where(
            MCPPlugin.user_id == user_id,
            MCPPlugin.enabled == True,
            MCPPlugin.plugin_type.in_(["http", "streamable_http", "sse"]),
        ).order_by(MCPPlugin.sort_order)

        result = await db_session.execute(query)
        return list(result.scalars().all())

    def _build_prompt_reference_context(
        self,
        plugins: List[MCPPlugin],
        tools: List[Dict[str, Any]],
    ) -> MCPPromptReferenceContext:
        capabilities = self._collect_prompt_capabilities(plugins, tools)
        enabled_plugin_count = len(plugins) or len({item.plugin_name for item in capabilities if item.plugin_name})
        tool_count = len(tools)

        mcp_guard = MCPPromptBlock.build(
            key="mcp_guard",
            title="MCP参考护栏",
            lines=self._build_prompt_guard_lines(),
        )
        mcp_references = MCPPromptBlock.build(
            key="mcp_references",
            title="MCP能力摘要",
            lines=self._build_prompt_reference_lines(
                plugins=plugins,
                capabilities=capabilities,
                tool_count=tool_count,
                enabled_plugin_count=enabled_plugin_count,
            ),
        )

        return MCPPromptReferenceContext(
            enabled_plugin_count=enabled_plugin_count,
            tool_count=tool_count,
            capabilities=capabilities,
            mcp_guard=mcp_guard,
            mcp_references=mcp_references,
            policy={
                "reference_only": True,
                "preserve_canon": True,
                "disclose_source": False,
                "summary_only": True,
                "max_prompt_capability_count": MAX_PROMPT_CAPABILITY_COUNT,
                "max_tools_per_plugin": MAX_PROMPT_TOOLS_PER_PLUGIN,
                "max_description_length": MAX_PROMPT_DESCRIPTION_LENGTH,
            },
            guard_summary=self._build_prompt_guard_summary(),
        )

    def _build_prompt_guard_summary(self) -> Dict[str, Any]:
        return {
            "reference_only": True,
            "preserve_canon": True,
            "disclose_source": False,
            "summary_only": True,
            "canon_priority": "project_canon",
            "allowed_usage": "reference_and_tool_routing_only",
            "forbidden_usage": [
                "overwrite_project_canon",
                "state_unverified_mcp_as_fact",
                "disclose_mcp_origin_in_final_output",
            ],
        }

    def _build_prompt_guard_lines(self) -> Tuple[str, ...]:
        return tuple(
            _unique_non_empty(
                (
                    MCP_REFERENCE_ONLY_RULE,
                    MCP_CANON_PRIORITY_RULE,
                    *MCP_GUARD_RULES,
                    MCP_SOURCE_DISCLOSURE_RULE,
                )
            )
        )

    def _build_prompt_reference_lines(
        self,
        plugins: List[MCPPlugin],
        capabilities: Tuple[MCPPromptCapability, ...],
        tool_count: int,
        enabled_plugin_count: int,
    ) -> Tuple[str, ...]:
        if tool_count == 0:
            if enabled_plugin_count:
                return (
                    f"当前启用 {enabled_plugin_count} 个 MCP 插件，但暂未发现可调用工具。",
                    "本轮无需注入外部能力摘要，可仅依赖项目上下文与既有设定。",
                )
            return (
                "当前未启用可用的 MCP 插件或工具。",
                "本轮无需注入外部能力摘要，可仅依赖项目上下文与既有设定。",
            )

        lines = [
            f"当前可用 MCP 能力：enabled_plugins={enabled_plugin_count}，tools={tool_count}，summary_only=true。",
            "以下仅提供能力摘要，供 prompt 判断何时调用工具补资料；不要把 MCP 参考直接当作 canon 或既定事实写入成文。",
            "如需采用 MCP 信息，必须先与项目设定、本章大纲或当前上下文交叉校验；最终输出不得暴露插件名、工具名或来源站点。",
        ]

        grouped: Dict[str, List[MCPPromptCapability]] = {}
        for capability in capabilities:
            group_key = capability.plugin_display_name or capability.plugin_name or "MCP"
            grouped.setdefault(group_key, []).append(capability)

        if plugins and not grouped:
            for plugin in plugins:
                label = _as_text(plugin.display_name) or plugin.plugin_name
                grouped.setdefault(label, [])

        for plugin_label, items in grouped.items():
            if not items:
                lines.append(f"[{plugin_label}] 已启用，但当前无可展开的工具摘要。")
                continue

            visible_items = items[:MAX_PROMPT_TOOLS_PER_PLUGIN]
            hidden_count = max(len(items) - len(visible_items), 0)
            rendered_tools = "；".join(item.to_line() for item in visible_items)
            line = f"[{plugin_label}] {len(items)} 个工具：{rendered_tools}"
            if hidden_count:
                line = f"{line}；其余 {hidden_count} 个工具已折叠"
            lines.append(line)

        if tool_count > len(capabilities):
            lines.append(
                f"其余 {tool_count - len(capabilities)} 个工具未展开，仅保留总量统计以控制 prompt 体积。"
            )

        return tuple(lines)

    def _collect_prompt_capabilities(
        self,
        plugins: List[MCPPlugin],
        tools: List[Dict[str, Any]],
    ) -> Tuple[MCPPromptCapability, ...]:
        collected: List[MCPPromptCapability] = []
        seen_function_names = set()

        for plugin in plugins:
            plugin_name = _as_text(plugin.plugin_name)
            plugin_display_name = _as_text(plugin.display_name) or plugin_name
            if not plugin_name:
                continue

            for tool in tools:
                function_payload = tool.get("function") or {}
                function_name = _as_text(function_payload.get("name"))
                if not function_name or function_name in seen_function_names:
                    continue

                tool_name = self._strip_plugin_prefix(function_name, plugin_name)
                if tool_name is None:
                    continue

                collected.append(
                    MCPPromptCapability(
                        plugin_name=plugin_name,
                        plugin_display_name=plugin_display_name,
                        function_name=function_name,
                        tool_name=tool_name,
                        description=_clip_text(function_payload.get("description"), MAX_PROMPT_DESCRIPTION_LENGTH),
                    )
                )
                seen_function_names.add(function_name)

        for tool in tools:
            if len(collected) >= MAX_PROMPT_CAPABILITY_COUNT:
                break

            function_payload = tool.get("function") or {}
            function_name = _as_text(function_payload.get("name"))
            if not function_name or function_name in seen_function_names:
                continue

            plugin_name, tool_name = self._fallback_parse_function_name(function_name)
            collected.append(
                MCPPromptCapability(
                    plugin_name=plugin_name,
                    plugin_display_name=plugin_name or "MCP",
                    function_name=function_name,
                    tool_name=tool_name,
                    description=_clip_text(function_payload.get("description"), MAX_PROMPT_DESCRIPTION_LENGTH),
                )
            )
            seen_function_names.add(function_name)

        return tuple(collected[:MAX_PROMPT_CAPABILITY_COUNT])

    def _strip_plugin_prefix(self, function_name: str, plugin_name: str) -> Optional[str]:
        for separator in ("_", "."):
            prefix = f"{plugin_name}{separator}"
            if function_name.startswith(prefix):
                return function_name[len(prefix):]
        return None

    def _fallback_parse_function_name(self, function_name: str) -> Tuple[str, str]:
        if "." in function_name:
            plugin_name, tool_name = function_name.split(".", 1)
            if plugin_name and tool_name:
                return plugin_name, tool_name

        if "_" in function_name:
            plugin_name, tool_name = function_name.split("_", 1)
            if plugin_name and tool_name:
                return plugin_name, tool_name

        return "", function_name

    def invalidate_cache(self, user_id: Optional[str] = None):
        """
        使缓存失效

        Args:
            user_id: 用户ID，为None时清空所有缓存
        """
        if user_id:
            if user_id in self._cache:
                del self._cache[user_id]
                logger.debug(f"🧹 清理用户工具缓存: {user_id}")
        else:
            count = len(self._cache)
            self._cache.clear()
            logger.info(f"🧹 清理所有用户工具缓存 ({count}个)")

    def get_cache_stats(self) -> Dict[str, Any]:
        """获取缓存统计"""
        now = datetime.now()
        return {
            "total_entries": len(self._cache),
            "total_hits": sum(entry.hit_count for entry in self._cache.values()),
            "cache_ttl_minutes": self._cache_ttl.total_seconds() / 60,
            "entries": [
                {
                    "user_id": user_id,
                    "tools_count": len(entry.tools) if entry.tools else 0,
                    "hit_count": entry.hit_count,
                    "expired": now >= entry.expire_time,
                    "expire_time": entry.expire_time.isoformat(),
                }
                for user_id, entry in self._cache.items()
            ],
        }


# 全局单例
mcp_tools_loader = MCPToolsLoader()


def _as_text(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip()


def _clip_text(value: Any, limit: int) -> str:
    text = _as_text(value)
    if not text:
        return ""
    compact = " ".join(text.replace("\r", " ").replace("\n", " ").split())
    return compact[:limit]


def _unique_non_empty(lines: List[str] | Tuple[str, ...]) -> Tuple[str, ...]:
    seen = set()
    ordered = []
    for line in lines:
        text = _as_text(line)
        if not text or text in seen:
            continue
        seen.add(text)
        ordered.append(text)
    return tuple(ordered)
