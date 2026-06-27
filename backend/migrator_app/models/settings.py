"""Settings persistence model."""

from __future__ import annotations

import uuid

from sqlalchemy import Column, DateTime, Float, Index, Integer, String, Text, text
from sqlalchemy.sql import func

from migrator_app.models import Base


class Settings(Base):
    """Persisted user AI settings."""

    __tablename__ = "settings"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    user_id = Column(String(50), nullable=False, unique=True, index=True, comment="用户ID")
    api_provider = Column(
        String(50),
        nullable=False,
        default="openai",
        server_default=text("'openai'"),
        comment="API提供商",
    )
    api_key = Column(String(500), comment="API密钥")
    api_base_url = Column(String(500), comment="自定义API地址")
    api_backup_urls = Column(Text, comment="备用API地址列表(JSON)")
    provider_type = Column(
        String(50),
        nullable=False,
        default="openai",
        server_default=text("'openai'"),
        comment="Provider类型(openai/azure/newapi/custom)",
    )
    fallback_strategy = Column(
        String(20),
        nullable=False,
        default="auto",
        server_default=text("'auto'"),
        comment="端点切换策略(auto/manual)",
    )
    azure_api_version = Column(String(50), comment="Azure API版本")
    llm_model = Column(
        String(100),
        nullable=False,
        default="gpt-4",
        server_default=text("'gpt-4'"),
        comment="模型名称",
    )
    temperature = Column(
        Float,
        nullable=False,
        default=0.7,
        server_default=text("0.7"),
        comment="温度参数",
    )
    max_tokens = Column(
        Integer,
        nullable=False,
        default=2000,
        server_default=text("2000"),
        comment="最大token数",
    )
    system_prompt = Column(Text, comment="系统级别提示词，每次AI调用都会使用")
    preferences = Column(Text, comment="其他偏好设置(JSON)")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

    __table_args__ = (
        Index("idx_user_id", "user_id"),
    )

