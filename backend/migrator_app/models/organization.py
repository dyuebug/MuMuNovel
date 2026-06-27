"""Organization ORM models."""

from __future__ import annotations

import uuid

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Text
from sqlalchemy.sql import func

from migrator_app.models import Base


class Organization(Base):
    """Persisted organization rows owned by the shared model layer."""

    __tablename__ = "organizations"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()), comment="组织ID")
    character_id = Column(
        String(36),
        ForeignKey("characters.id", ondelete="CASCADE"),
        nullable=False,
        unique=True,
        comment="关联的角色ID",
    )
    project_id = Column(
        String(36),
        ForeignKey("projects.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="项目ID",
    )
    parent_org_id = Column(
        String(36),
        ForeignKey("organizations.id", ondelete="SET NULL"),
        comment="父组织ID",
    )
    level = Column(Integer, default=0, comment="组织层级")
    power_level = Column(Integer, default=50, comment="势力等级：0-100")
    member_count = Column(Integer, default=0, comment="成员数量")
    location = Column(Text, comment="所在地")
    motto = Column(String(200), comment="宗旨/口号")
    color = Column(String(100), comment="代表颜色")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")


class OrganizationMember(Base):
    """Persisted organization-member rows owned by the shared model layer."""

    __tablename__ = "organization_members"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()), comment="成员关系ID")
    organization_id = Column(
        String(36),
        ForeignKey("organizations.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="组织ID",
    )
    character_id = Column(
        String(36),
        ForeignKey("characters.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
        comment="角色ID",
    )
    position = Column(String(100), nullable=False, comment="职位名称")
    rank = Column(Integer, default=0, comment="职位等级")
    status = Column(String(20), default="active", comment="状态：active/retired/expelled/deceased")
    joined_at = Column(String(100), comment="加入时间（故事时间）")
    left_at = Column(String(100), comment="离开时间（故事时间）")
    loyalty = Column(Integer, default=50, comment="忠诚度：0-100")
    contribution = Column(Integer, default=0, comment="贡献度：0-100")
    source = Column(String(20), default="ai", comment="来源：ai/manual")
    notes = Column(Text, comment="备注")
    created_at = Column(DateTime, server_default=func.now(), comment="创建时间")
    updated_at = Column(DateTime, server_default=func.now(), onupdate=func.now(), comment="更新时间")

