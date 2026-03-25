"""??????????"""
from sqlalchemy import Column, String, DateTime, ForeignKey, JSON
from sqlalchemy.sql import func
from app.database import Base
import uuid


class BatchGenerationSnapshot(Base):
    """?????????????????????????"""
    __tablename__ = "batch_generation_snapshots"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    batch_task_id = Column(
        String(36),
        ForeignKey("batch_generation_tasks.id", ondelete="CASCADE"),
        nullable=False,
        unique=True,
        comment="????ID",
    )
    latest_quality_metrics = Column(JSON, nullable=True, comment="????????")
    quality_metrics_history = Column(JSON, nullable=True, comment="?????????")
    quality_metrics_summary = Column(JSON, nullable=True, comment="?????????")
    created_at = Column(DateTime, server_default=func.now(), comment="????")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="????")

    def __repr__(self):
        return f"<BatchGenerationSnapshot(batch_task_id={self.batch_task_id})>"
