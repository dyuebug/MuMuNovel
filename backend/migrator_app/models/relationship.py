"""Relationship ORM models."""

from __future__ import annotations

import uuid

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Text
from sqlalchemy.sql import func

from migrator_app.models import Base


class RelationshipType(Base):
    """Persisted relationship-type rows owned by the shared model layer."""

    __tablename__ = "relationship_types"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    name = Column(String(50), nullable=False, comment="关系名称")
    category = Column(String(20), nullable=False, comment="分类：family/social/hostile/professional")
    reverse_name = Column(String(50), comment="反向关系名称")
    intimacy_range = Column(String(20), comment="亲密度范围：high/medium/low")
    icon = Column(String(50), comment="图标标识")
    description = Column(Text, comment="关系描述")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")


class CharacterRelationship(Base):
    """Persisted character-relationship rows owned by the shared model layer."""

    __tablename__ = "character_relationships"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()), comment="关系ID")
    project_id = Column(
        String(36),
        ForeignKey("projects.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="项目ID",
    )
    character_from_id = Column(
        String(36),
        ForeignKey("characters.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="角色A的ID",
    )
    character_to_id = Column(
        String(36),
        ForeignKey("characters.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="角色B的ID",
    )
    relationship_type_id = Column(
        Integer,
        ForeignKey("relationship_types.id"),
        index=True,
        comment="关系类型ID",
    )
    relationship_name = Column(String(100), comment="自定义关系名称")
    intimacy_level = Column(Integer, default=50, comment="亲密度：-100到100")
    status = Column(String(20), default="active", comment="状态：active/broken/past/complicated")
    description = Column(Text, comment="关系详细描述")
    started_at = Column(String(100), comment="关系开始时间（故事时间）")
    ended_at = Column(String(100), comment="关系结束时间（故事时间）")
    source = Column(String(20), default="ai", comment="来源：ai/manual/imported")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

