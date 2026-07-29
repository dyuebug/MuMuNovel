"""Durable novel autopilot step ORM model for Alembic metadata."""

from __future__ import annotations

from sqlalchemy import (
    BigInteger,
    Column,
    DateTime,
    ForeignKey,
    Index,
    Integer,
    String,
    UniqueConstraint,
)

from migrator_app.models import Base


class NovelAutopilotStepRun(Base):
    """Idempotent durable execution-attempt record for one run step."""

    __tablename__ = "novel_autopilot_step_runs"
    __table_args__ = (
        UniqueConstraint(
            "run_id",
            "step_key",
            "attempt",
            name="uq_novel_autopilot_step_runs_run_step_attempt",
        ),
        Index(
            "ix_novel_autopilot_step_runs_run_status_created_at",
            "run_id",
            "status",
            "created_at",
        ),
    )

    id = Column(String(36), primary_key=True)
    run_id = Column(
        String(36),
        ForeignKey("novel_autopilot_runs.id", ondelete="CASCADE"),
        nullable=False,
    )
    step_key = Column(String(160), nullable=False)
    step_type = Column(String(64), nullable=False)
    phase = Column(String(64), nullable=False)
    chapter_id = Column(
        String(36),
        ForeignKey("chapters.id", ondelete="SET NULL"),
        nullable=True,
    )
    chapter_number = Column(Integer, nullable=True)
    attempt = Column(Integer, nullable=False)
    run_epoch = Column(BigInteger, nullable=False)
    status = Column(String(32), nullable=False)
    background_task_id = Column(String(36), nullable=True)
    input_digest = Column(String(80), nullable=False)
    result_digest = Column(String(80), nullable=True)
    quality_decision = Column(String(32), nullable=True)
    error_code = Column(String(128), nullable=True)
    started_at = Column(DateTime, nullable=True)
    completed_at = Column(DateTime, nullable=True)
    created_at = Column(DateTime, nullable=False)
    updated_at = Column(DateTime, nullable=False)
