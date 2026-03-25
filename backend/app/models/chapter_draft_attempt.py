"""???????????"""
from sqlalchemy import Column, String, Text, Integer, DateTime, ForeignKey, JSON
from sqlalchemy.sql import func
from app.database import Base
import uuid


class ChapterDraftAttempt(Base):
    """??????????????????????"""
    __tablename__ = "chapter_draft_attempts"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    project_id = Column(String(36), ForeignKey("projects.id", ondelete="CASCADE"), nullable=False, comment="??ID")
    chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"), nullable=True, comment="??ID")
    batch_task_id = Column(String(36), ForeignKey("batch_generation_tasks.id", ondelete="SET NULL"), nullable=True, comment="????ID")
    source = Column(String(40), nullable=False, default="chapter", comment="???chapter/batch/regenerate")
    attempt_state = Column(String(40), nullable=False, default="candidate", comment="????")
    quality_gate_action = Column(String(40), nullable=True, comment="??????")
    quality_gate_decision = Column(String(40), nullable=True, comment="??????")
    word_count = Column(Integer, nullable=False, default=0, comment="?????")
    summary_preview = Column(Text, nullable=True, comment="???????")
    content_preview = Column(Text, nullable=True, comment="???????")
    quality_metrics = Column(JSON, nullable=True, comment="??????")
    repair_payload = Column(JSON, nullable=True, comment="??????")
    created_at = Column(DateTime, server_default=func.now(), comment="????")

    def __repr__(self):
        return (
            f"<ChapterDraftAttempt(id={self.id}, chapter_id={self.chapter_id}, "
            f"attempt_state={self.attempt_state})>"
        )
