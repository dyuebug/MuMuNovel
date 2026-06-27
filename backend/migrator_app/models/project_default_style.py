"""Project default style ORM model."""

from __future__ import annotations

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, UniqueConstraint
from sqlalchemy.sql import func

from migrator_app.models import Base


class ProjectDefaultStyle(Base):
    """Persisted project-default-style rows owned by the shared model layer."""

    __tablename__ = "project_default_styles"

    id = Column(Integer, primary_key=True, autoincrement=True)
    project_id = Column(
        String(36),
        ForeignKey("projects.id", ondelete="CASCADE"),
        nullable=False,
        comment="项目ID",
    )
    style_id = Column(
        Integer,
        ForeignKey("writing_styles.id", ondelete="CASCADE"),
        nullable=False,
        comment="风格ID",
    )
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

    __table_args__ = (UniqueConstraint("project_id", name="uix_project_default_style"),)

