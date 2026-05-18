"""批量生成任务数据模型"""
from sqlalchemy import Column, String, Integer, DateTime, Boolean, JSON, text
from sqlalchemy.sql import func
from app.database import Base
import uuid


class BatchGenerationTask(Base):
    """批量生成任务表"""
    __tablename__ = "batch_generation_tasks"
    
    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    project_id = Column(String(36), nullable=False, comment="项目ID")
    user_id = Column(String(100), nullable=False, comment="用户ID")
    
    # 任务配置
    start_chapter_number = Column(Integer, nullable=False, comment="起始章节序号")
    chapter_count = Column(Integer, nullable=False, comment="生成章节数量")
    chapter_ids = Column(JSON, nullable=False, comment="待生成的章节ID列表")
    style_id = Column(Integer, comment="使用的写作风格ID")
    target_word_count = Column(
        Integer,
        nullable=False,
        default=3000,
        server_default=text("3000"),
        comment="目标字数",
    )
    enable_analysis = Column(
        Boolean,
        nullable=False,
        default=False,
        server_default=text("false"),
        comment="是否启用同步分析",
    )
    
    # 任务状态
    status = Column(
        String(20),
        nullable=False,
        default="pending",
        server_default=text("'pending'"),
        comment="任务状态: pending/running/completed/failed/cancelled",
    )
    total_chapters = Column(
        Integer,
        nullable=False,
        default=0,
        server_default=text("0"),
        comment="总章节数",
    )
    completed_chapters = Column(
        Integer,
        nullable=False,
        default=0,
        server_default=text("0"),
        comment="已完成章节数",
    )
    failed_chapters = Column(
        JSON,
        nullable=False,
        default=list,
        server_default=text("'[]'"),
        comment="失败的章节信息列表",
    )
    current_chapter_id = Column(String(36), comment="当前正在生成的章节ID")
    current_chapter_number = Column(Integer, comment="当前正在生成的章节序号")
    current_retry_count = Column(
        Integer,
        nullable=False,
        default=0,
        server_default=text("0"),
        comment="当前章节重试次数",
    )
    max_retries = Column(
        Integer,
        nullable=False,
        default=3,
        server_default=text("3"),
        comment="最大重试次数",
    )
    
    # 时间记录
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    started_at = Column(DateTime, comment="开始时间")
    completed_at = Column(DateTime, comment="完成时间")
    
    # 错误信息
    error_message = Column(String(500), comment="错误信息")
    
    def __repr__(self):
        return f"<BatchGenerationTask(id={self.id}, status={self.status}, completed={self.completed_chapters}/{self.total_chapters})>"
