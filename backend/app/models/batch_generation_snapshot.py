"""批量生成快照模型。"""
from sqlalchemy import Column, String, DateTime, ForeignKey, JSON
from sqlalchemy.sql import func
from app.database import Base
import uuid


class BatchGenerationSnapshot(Base):
    """记录批量生成任务的质量指标与运行时状态快照。"""
    __tablename__ = "batch_generation_snapshots"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    batch_task_id = Column(
        String(36),
        ForeignKey("batch_generation_tasks.id", ondelete="CASCADE"),
        nullable=False,
        unique=True,
        comment="批量任务ID",
    )
    latest_quality_metrics = Column(JSON, nullable=True, comment="最新质量指标")
    quality_metrics_history = Column(JSON, nullable=True, comment="质量指标历史")
    quality_metrics_summary = Column(JSON, nullable=True, comment="质量指标汇总")
    workflow_runtime_state = Column(JSON, nullable=True, comment="Persisted workflow runtime state")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

    def __repr__(self):
        return f"<BatchGenerationSnapshot(batch_task_id={self.batch_task_id})>"
