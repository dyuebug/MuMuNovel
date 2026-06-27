"""Generation history ORM model."""

from __future__ import annotations

import uuid

from sqlalchemy import Column, DateTime, Float, ForeignKey, Integer, String, Text
from sqlalchemy.sql import func

from migrator_app.models import Base


class GenerationHistory(Base):
    """Persisted generation history rows owned by the shared model layer."""

    __tablename__ = "generation_history"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    project_id = Column(String(36), ForeignKey("projects.id", ondelete="CASCADE"), nullable=False)
    chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"), nullable=True)
    prompt = Column(Text, comment="使用的提示词")
    generated_content = Column(Text, comment="生成的内容")
    model = Column(String(50), comment="使用的模型")
    tokens_used = Column(Integer, comment="消耗的token数")
    generation_time = Column(Float, comment="生成耗时(秒)")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")

    def __repr__(self):
        return f"<GenerationHistory(id={self.id}, model={self.model})>"

