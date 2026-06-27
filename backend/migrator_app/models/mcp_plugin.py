"""MCP plugin persistence model."""

from __future__ import annotations

import uuid

from sqlalchemy import Boolean, Column, DateTime, Index, Integer, JSON, String, Text
from sqlalchemy.sql import func

from migrator_app.models import Base


class MCPPlugin(Base):
    """Persisted MCP plugin configuration and runtime status."""

    __tablename__ = "mcp_plugins"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    user_id = Column(String(50), nullable=False, index=True, comment="用户ID")
    plugin_name = Column(String(100), nullable=False, comment="插件名称（唯一标识）")
    display_name = Column(String(200), nullable=False, comment="显示名称")
    description = Column(Text, comment="插件描述")
    plugin_type = Column(String(50), default="http", comment="插件类型：http/stdio")
    server_url = Column(String(500), comment="服务器URL（HTTP类型）")
    command = Column(String(500), comment="启动命令（stdio类型）")
    args = Column(JSON, comment="命令参数（stdio类型）")
    env = Column(JSON, comment="环境变量")
    headers = Column(JSON, comment="HTTP请求头")
    config = Column(JSON, comment="插件特定配置（JSON）")
    tools = Column(JSON, comment="提供的工具列表")
    enabled = Column(Boolean, default=True, comment="是否启用")
    status = Column(String(50), default="inactive", comment="状态：active/inactive/error")
    last_error = Column(Text, comment="最后错误信息")
    last_test_at = Column(DateTime, comment="最后测试时间")
    category = Column(String(100), default="general", comment="分类")
    sort_order = Column(Integer, default=0, comment="排序顺序")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

    __table_args__ = (
        Index("idx_user_plugin", "user_id", "plugin_name", unique=True),
        Index("idx_user_enabled", "user_id", "enabled"),
    )

