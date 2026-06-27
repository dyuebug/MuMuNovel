"""Writing style ORM model."""

from __future__ import annotations

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Text
from sqlalchemy.sql import func

from migrator_app.models import Base


class WritingStyle(Base):
    """Persisted writing-style rows owned by the shared model layer."""

    __tablename__ = "writing_styles"

    id = Column(Integer, primary_key=True, autoincrement=True)
    user_id = Column(
        String(255),
        ForeignKey("users.user_id", ondelete="CASCADE"),
        nullable=True,
        comment="所属用户ID（NULL表示全局预设风格）",
    )
    name = Column(String(100), nullable=False, comment="风格名称")
    style_type = Column(String(50), nullable=False, comment="风格类型：preset/custom")
    preset_id = Column(String(50), comment="预设风格ID：natural/classical/modern等")
    description = Column(Text, comment="风格描述")
    prompt_content = Column(Text, nullable=False, comment="风格提示词内容")
    order_index = Column(Integer, default=0, comment="排序序号")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

