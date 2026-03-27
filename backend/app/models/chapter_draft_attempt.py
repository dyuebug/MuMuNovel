"""章节草稿尝试模型。"""
from sqlalchemy import Column, String, Text, Integer, DateTime, ForeignKey, JSON
from sqlalchemy.sql import func
from app.database import Base
import uuid


class ChapterDraftAttempt(Base):
    """记录章节生成、批量生成与重写过程中的草稿尝试。"""
    __tablename__ = "chapter_draft_attempts"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    project_id = Column(String(36), ForeignKey("projects.id", ondelete="CASCADE"), nullable=False, comment="项目ID")
    chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"), nullable=True, comment="章节ID")
    batch_task_id = Column(String(36), ForeignKey("batch_generation_tasks.id", ondelete="SET NULL"), nullable=True, comment="批量任务ID")
    source = Column(String(40), nullable=False, default="chapter", comment="来源：chapter/batch/regenerate")
    attempt_state = Column(String(40), nullable=False, default="candidate", comment="尝试状态")
    quality_gate_action = Column(String(40), nullable=True, comment="质量门动作")
    quality_gate_decision = Column(String(40), nullable=True, comment="质量门决策")
    word_count = Column(Integer, nullable=False, default=0, comment="草稿字数")
    summary_preview = Column(Text, nullable=True, comment="摘要预览")
    content_preview = Column(Text, nullable=True, comment="内容预览")
    quality_metrics = Column(JSON, nullable=True, comment="质量指标")
    repair_payload = Column(JSON, nullable=True, comment="修复载荷")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")

    def __repr__(self):
        return (
            f"<ChapterDraftAttempt(id={self.id}, chapter_id={self.chapter_id}, "
            f"attempt_state={self.attempt_state})>"
        )
