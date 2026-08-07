"""Durable novel autopilot run ORM model for Alembic metadata."""

from __future__ import annotations

from sqlalchemy import (
    BigInteger,
    Column,
    DateTime,
    Float,
    ForeignKey,
    Index,
    Integer,
    JSON,
    String,
    Text,
)

from migrator_app.models import Base


class NovelAutopilotRun(Base):
    """Durable orchestration cursor, budget, and lifecycle record."""

    __tablename__ = "novel_autopilot_runs"
    __table_args__ = (
        Index(
            "uq_novel_autopilot_runs_active_scope_key",
            "active_scope_key",
            unique=True,
        ),
        Index(
            "ix_novel_autopilot_runs_project_created_at",
            "project_id",
            "created_at",
        ),
        Index(
            "ix_novel_autopilot_runs_status_next_attempt_at",
            "status",
            "next_attempt_at",
        ),
    )

    id = Column(String(36), primary_key=True)
    project_id = Column(
        String(36),
        ForeignKey("projects.id", ondelete="CASCADE"),
        nullable=False,
    )
    user_id = Column(String(100), nullable=False)
    schema_version = Column(String(64), nullable=False)
    status = Column(String(32), nullable=False)
    current_phase = Column(String(64), nullable=False)
    current_step = Column(String(128), nullable=True)
    active_scope_key = Column(String(36), nullable=True)
    current_chapter_id = Column(
        String(36),
        ForeignKey("chapters.id", ondelete="SET NULL"),
        nullable=True,
    )
    current_chapter_number = Column(Integer, nullable=True)
    total_chapters = Column(Integer, nullable=False)
    completed_chapters = Column(Integer, nullable=False)
    failed_chapters = Column(JSON, nullable=False)
    pending_rewrites = Column(JSON, nullable=False)
    total_word_count = Column(BigInteger, nullable=False)
    execution_scope = Column(String(64), nullable=False)
    human_gate_mode = Column(String(64), nullable=False)
    gate_interval = Column(Integer, nullable=True)
    config_snapshot = Column(JSON, nullable=False)
    max_chapters = Column(Integer, nullable=True)
    max_tokens = Column(BigInteger, nullable=True)
    max_estimated_cost = Column(Float, nullable=True)
    max_runtime_seconds = Column(BigInteger, nullable=True)
    used_tokens = Column(BigInteger, nullable=False)
    estimated_cost = Column(Float, nullable=False)
    epoch = Column(BigInteger, nullable=False)
    version = Column(BigInteger, nullable=False)
    consecutive_provider_failures = Column(Integer, nullable=False)
    consecutive_quality_failures = Column(Integer, nullable=False)
    last_error_code = Column(String(128), nullable=True)
    next_attempt_at = Column(DateTime, nullable=True)
    guidance_digest = Column(String(80), nullable=True)
    active_background_task_id = Column(String(36), nullable=True)
    final_export_ref = Column(Text, nullable=True)
    created_at = Column(DateTime, nullable=False)
    updated_at = Column(DateTime, nullable=False)
    started_at = Column(DateTime, nullable=True)
    paused_at = Column(DateTime, nullable=True)
    completed_at = Column(DateTime, nullable=True)
