"""Autopilot invocation audit ORM model for Alembic metadata."""

from __future__ import annotations

from sqlalchemy import Boolean, Column, DateTime, ForeignKey, Index, String, Text, UniqueConstraint

from migrator_app.models import Base


class AutopilotInvocationAudit(Base):
    """Durable, privacy-safe record of one confirmed Autopilot tool invocation."""

    __tablename__ = "autopilot_invocation_audits"
    __table_args__ = (
        UniqueConstraint("task_id", name="uq_autopilot_invocation_audits_task_id"),
        Index(
            "ix_autopilot_invocation_audits_project_created_at",
            "project_id",
            "created_at",
        ),
    )

    id = Column(String(36), primary_key=True)
    task_id = Column(String(36), nullable=False)
    project_id = Column(
        String(36),
        ForeignKey("projects.id", ondelete="CASCADE"),
        nullable=False,
    )
    actor_user_id = Column(String(100), nullable=False)
    schema_version = Column(String(64), nullable=False)
    tool_name = Column(String(128), nullable=False)
    tool_schema_version = Column(String(64), nullable=False)
    confirmed_by_user = Column(Boolean, nullable=False)
    execution_mode = Column(String(64), nullable=False)
    provider_name = Column(Text, nullable=True)
    model_name = Column(Text, nullable=True)
    prompt_digest = Column(String(80), nullable=True)
    input_digest = Column(String(80), nullable=False)
    input_summary = Column(Text, nullable=False)
    status = Column(String(32), nullable=False)
    result_summary = Column(Text, nullable=True)
    error_code = Column(String(128), nullable=True)
    created_at = Column(DateTime, nullable=False)
    started_at = Column(DateTime, nullable=True)
    completed_at = Column(DateTime, nullable=True)
