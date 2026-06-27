"""伏笔测试支持模块 - 保留测试和回滚场景需要的伏笔管理逻辑。"""

from __future__ import annotations

import hashlib
import uuid
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Dict, List, Optional

from sqlalchemy import and_, delete, desc, func, or_, select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.database_test_support import Base
from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models.chapter import Chapter
from migrator_app.models import PlotAnalysis
from sqlalchemy import Boolean, Column, DateTime, Float, ForeignKey, Integer, JSON, String, Text
from sqlalchemy.sql import func as sql_func

logger = get_logger(__name__)


@dataclass
class ForeshadowCreate:
    project_id: str
    title: str
    content: str
    hint_text: Optional[str] = None
    resolution_text: Optional[str] = None
    plant_chapter_number: Optional[int] = None
    target_resolve_chapter_number: Optional[int] = None
    is_long_term: bool = False
    importance: float = 0.5
    strength: int = 5
    subtlety: int = 5
    related_characters: Optional[List[str]] = None
    tags: Optional[List[str]] = None
    category: Optional[str] = None
    notes: Optional[str] = None
    resolution_notes: Optional[str] = None
    auto_remind: bool = True
    remind_before_chapters: int = 5
    include_in_context: bool = True


@dataclass
class ForeshadowUpdate:
    title: Optional[str] = None
    content: Optional[str] = None
    hint_text: Optional[str] = None
    resolution_text: Optional[str] = None
    plant_chapter_number: Optional[int] = None
    target_resolve_chapter_number: Optional[int] = None
    status: Optional[str] = None
    is_long_term: Optional[bool] = None
    importance: Optional[float] = None
    strength: Optional[int] = None
    subtlety: Optional[int] = None
    urgency: Optional[int] = None
    related_characters: Optional[List[str]] = None
    related_foreshadow_ids: Optional[List[str]] = None
    tags: Optional[List[str]] = None
    category: Optional[str] = None
    notes: Optional[str] = None
    resolution_notes: Optional[str] = None
    auto_remind: Optional[bool] = None
    remind_before_chapters: Optional[int] = None
    include_in_context: Optional[bool] = None

    def model_dump(self, exclude_unset: bool = False) -> Dict[str, Any]:
        data = {
            "title": self.title,
            "content": self.content,
            "hint_text": self.hint_text,
            "resolution_text": self.resolution_text,
            "plant_chapter_number": self.plant_chapter_number,
            "target_resolve_chapter_number": self.target_resolve_chapter_number,
            "status": self.status,
            "is_long_term": self.is_long_term,
            "importance": self.importance,
            "strength": self.strength,
            "subtlety": self.subtlety,
            "urgency": self.urgency,
            "related_characters": self.related_characters,
            "related_foreshadow_ids": self.related_foreshadow_ids,
            "tags": self.tags,
            "category": self.category,
            "notes": self.notes,
            "resolution_notes": self.resolution_notes,
            "auto_remind": self.auto_remind,
            "remind_before_chapters": self.remind_before_chapters,
            "include_in_context": self.include_in_context,
        }
        if exclude_unset:
            return {key: value for key, value in data.items() if value is not None}
        return data


@dataclass
class PlantForeshadowRequest:
    chapter_id: str
    chapter_number: int
    hint_text: Optional[str] = None


@dataclass
class ResolveForeshadowRequest:
    chapter_id: str
    chapter_number: int
    resolution_text: Optional[str] = None
    is_partial: bool = False


@dataclass
class SyncFromAnalysisRequest:
    chapter_ids: Optional[List[str]] = None
    overwrite_existing: bool = False
    auto_set_planted: bool = True


class Foreshadow(Base):
    __tablename__ = "foreshadows"

    id = Column(String(36), primary_key=True, default=lambda: str(uuid.uuid4()))
    project_id = Column(String(36), ForeignKey("projects.id", ondelete="CASCADE"), nullable=False, index=True)
    title = Column(String(200), nullable=False)
    content = Column(Text, nullable=False)
    hint_text = Column(Text)
    resolution_text = Column(Text)
    source_type = Column(String(20), default="manual")
    source_memory_id = Column(String(100))
    source_analysis_id = Column(String(36))
    plant_chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"))
    plant_chapter_number = Column(Integer)
    target_resolve_chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"))
    target_resolve_chapter_number = Column(Integer)
    actual_resolve_chapter_id = Column(String(36), ForeignKey("chapters.id", ondelete="SET NULL"))
    actual_resolve_chapter_number = Column(Integer)
    status = Column(String(20), default="pending", index=True)
    is_long_term = Column(Boolean, default=False)
    importance = Column(Float, default=0.5)
    strength = Column(Integer, default=5)
    subtlety = Column(Integer, default=5)
    urgency = Column(Integer, default=0)
    related_characters = Column(JSON)
    related_foreshadow_ids = Column(JSON)
    tags = Column(JSON)
    category = Column(String(50))
    notes = Column(Text)
    resolution_notes = Column(Text)
    auto_remind = Column(Boolean, default=True)
    remind_before_chapters = Column(Integer, default=5)
    include_in_context = Column(Boolean, default=True)
    created_at = Column(DateTime, server_default=sql_func.now())
    updated_at = Column(DateTime, server_default=sql_func.now(), onupdate=sql_func.now())
    planted_at = Column(DateTime)
    resolved_at = Column(DateTime)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "project_id": self.project_id,
            "title": self.title,
            "content": self.content,
            "hint_text": self.hint_text,
            "resolution_text": self.resolution_text,
            "source_type": self.source_type,
            "source_memory_id": self.source_memory_id,
            "plant_chapter_id": self.plant_chapter_id,
            "plant_chapter_number": self.plant_chapter_number,
            "target_resolve_chapter_id": self.target_resolve_chapter_id,
            "target_resolve_chapter_number": self.target_resolve_chapter_number,
            "actual_resolve_chapter_id": self.actual_resolve_chapter_id,
            "actual_resolve_chapter_number": self.actual_resolve_chapter_number,
            "status": self.status,
            "is_long_term": self.is_long_term,
            "importance": self.importance,
            "strength": self.strength,
            "subtlety": self.subtlety,
            "urgency": self.urgency,
            "related_characters": self.related_characters or [],
            "related_foreshadow_ids": self.related_foreshadow_ids or [],
            "tags": self.tags or [],
            "category": self.category,
            "notes": self.notes,
            "resolution_notes": self.resolution_notes,
            "auto_remind": self.auto_remind,
            "remind_before_chapters": self.remind_before_chapters,
            "include_in_context": self.include_in_context,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
            "planted_at": self.planted_at.isoformat() if self.planted_at else None,
            "resolved_at": self.resolved_at.isoformat() if self.resolved_at else None,
        }


def generate_stable_foreshadow_id(
    chapter_id: str,
    content: str,
    foreshadow_type: str = "planted",
) -> str:
    """生成稳定的伏笔唯一标识符。"""
    content_normalized = content.strip().lower()
    content_hash = hashlib.md5(content_normalized.encode("utf-8")).hexdigest()[:12]
    chapter_hash = hashlib.md5(chapter_id.encode("utf-8")).hexdigest()[:8]
    return f"{foreshadow_type}_{chapter_hash}_{content_hash}"


class ForeshadowService:
    """伏笔管理测试支持 owner。"""

    async def get_project_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        status: Optional[str] = None,
        category: Optional[str] = None,
        source_type: Optional[str] = None,
        is_long_term: Optional[bool] = None,
        page: int = 1,
        limit: int = 50,
    ) -> Dict[str, Any]:
        conditions = [Foreshadow.project_id == project_id]

        if status:
            conditions.append(Foreshadow.status == status)
        if category:
            conditions.append(Foreshadow.category == category)
        if source_type:
            conditions.append(Foreshadow.source_type == source_type)
        if is_long_term is not None:
            conditions.append(Foreshadow.is_long_term == is_long_term)

        count_query = select(func.count(Foreshadow.id)).where(and_(*conditions))
        total_result = await db.execute(count_query)
        total = total_result.scalar() or 0

        query = (
            select(Foreshadow)
            .where(and_(*conditions))
            .order_by(
                Foreshadow.plant_chapter_number.asc().nulls_last(),
                desc(Foreshadow.importance),
                desc(Foreshadow.created_at),
            )
            .offset((page - 1) * limit)
            .limit(limit)
        )

        result = await db.execute(query)
        foreshadows = result.scalars().all()
        stats = await self.get_stats(db, project_id)

        return {
            "total": total,
            "items": [item.to_dict() for item in foreshadows],
            "stats": stats,
        }

    async def get_foreshadow(
        self,
        db: AsyncSession,
        foreshadow_id: str,
    ) -> Optional[Foreshadow]:
        result = await db.execute(select(Foreshadow).where(Foreshadow.id == foreshadow_id))
        return result.scalar_one_or_none()

    async def create_foreshadow(
        self,
        db: AsyncSession,
        data: ForeshadowCreate,
    ) -> Foreshadow:
        foreshadow = Foreshadow(
            id=str(uuid.uuid4()),
            project_id=data.project_id,
            title=data.title,
            content=data.content,
            hint_text=data.hint_text,
            resolution_text=data.resolution_text,
            source_type="manual",
            plant_chapter_number=data.plant_chapter_number,
            target_resolve_chapter_number=data.target_resolve_chapter_number,
            status="pending",
            is_long_term=data.is_long_term,
            importance=data.importance,
            strength=data.strength,
            subtlety=data.subtlety,
            urgency=0,
            related_characters=data.related_characters,
            tags=data.tags,
            category=data.category,
            notes=data.notes,
            resolution_notes=data.resolution_notes,
            auto_remind=data.auto_remind,
            remind_before_chapters=data.remind_before_chapters,
            include_in_context=data.include_in_context,
        )

        db.add(foreshadow)
        await db.commit()
        await db.refresh(foreshadow)
        return foreshadow

    async def update_foreshadow(
        self,
        db: AsyncSession,
        foreshadow_id: str,
        data: ForeshadowUpdate,
    ) -> Optional[Foreshadow]:
        foreshadow = await self.get_foreshadow(db, foreshadow_id)
        if not foreshadow:
            return None

        update_data = data.model_dump(exclude_unset=True)
        for key, value in update_data.items():
            if hasattr(foreshadow, key):
                setattr(foreshadow, key, value)

        await db.commit()
        await db.refresh(foreshadow)
        return foreshadow

    async def delete_foreshadow(
        self,
        db: AsyncSession,
        foreshadow_id: str,
    ) -> bool:
        foreshadow = await self.get_foreshadow(db, foreshadow_id)
        if not foreshadow:
            return False

        await db.delete(foreshadow)
        await db.commit()
        return True

    async def mark_as_planted(
        self,
        db: AsyncSession,
        foreshadow_id: str,
        data: PlantForeshadowRequest,
    ) -> Optional[Foreshadow]:
        foreshadow = await self.get_foreshadow(db, foreshadow_id)
        if not foreshadow:
            return None

        foreshadow.status = "planted"
        foreshadow.plant_chapter_id = data.chapter_id
        foreshadow.plant_chapter_number = data.chapter_number
        foreshadow.planted_at = datetime.now()
        if data.hint_text:
            foreshadow.hint_text = data.hint_text

        await db.commit()
        await db.refresh(foreshadow)
        return foreshadow

    async def mark_as_resolved(
        self,
        db: AsyncSession,
        foreshadow_id: str,
        data: ResolveForeshadowRequest,
    ) -> Optional[Foreshadow]:
        foreshadow = await self.get_foreshadow(db, foreshadow_id)
        if not foreshadow:
            return None

        foreshadow.status = "partially_resolved" if data.is_partial else "resolved"
        foreshadow.actual_resolve_chapter_id = data.chapter_id
        foreshadow.actual_resolve_chapter_number = data.chapter_number
        foreshadow.resolved_at = datetime.now()
        if data.resolution_text:
            foreshadow.resolution_text = data.resolution_text

        await db.commit()
        await db.refresh(foreshadow)
        return foreshadow

    async def mark_as_abandoned(
        self,
        db: AsyncSession,
        foreshadow_id: str,
        reason: Optional[str] = None,
    ) -> Optional[Foreshadow]:
        foreshadow = await self.get_foreshadow(db, foreshadow_id)
        if not foreshadow:
            return None

        foreshadow.status = "abandoned"
        if reason:
            foreshadow.notes = f"{foreshadow.notes or ''}\n[废弃原因] {reason}".strip()

        await db.commit()
        await db.refresh(foreshadow)
        return foreshadow

    async def sync_from_analysis(
        self,
        db: AsyncSession,
        project_id: str,
        data: SyncFromAnalysisRequest,
    ) -> Dict[str, Any]:
        total_stats = {
            "synced_count": 0,
            "skipped_count": 0,
            "resolved_count": 0,
            "new_foreshadows": [],
            "skipped_reasons": [],
        }

        query = select(PlotAnalysis).where(PlotAnalysis.project_id == project_id)
        if data.chapter_ids:
            query = query.where(PlotAnalysis.chapter_id.in_(data.chapter_ids))

        result = await db.execute(query)
        analyses = result.scalars().all()

        for analysis in analyses:
            if not analysis.foreshadows:
                continue

            chapter_result = await db.execute(
                select(Chapter).where(Chapter.id == analysis.chapter_id)
            )
            chapter = chapter_result.scalar_one_or_none()
            if not chapter:
                continue

            chapter_stats = await self.auto_update_from_analysis(
                db=db,
                project_id=project_id,
                chapter_id=chapter.id,
                chapter_number=chapter.chapter_number,
                analysis_foreshadows=analysis.foreshadows,
            )
            total_stats["synced_count"] += (
                chapter_stats.get("planted_count", 0)
                + chapter_stats.get("resolved_count", 0)
            )
            total_stats["resolved_count"] += chapter_stats.get("resolved_count", 0)
            total_stats["skipped_count"] += chapter_stats.get("skipped_resolve_count", 0)

        return total_stats

    async def get_pending_resolve_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        current_chapter: int,
        lookahead: int = 5,
    ) -> List[Foreshadow]:
        query = (
            select(Foreshadow)
            .where(
                and_(
                    Foreshadow.project_id == project_id,
                    Foreshadow.status == "planted",
                    Foreshadow.target_resolve_chapter_number != None,
                    Foreshadow.target_resolve_chapter_number <= current_chapter + lookahead,
                    Foreshadow.auto_remind == True,
                )
            )
            .order_by(Foreshadow.target_resolve_chapter_number)
        )
        result = await db.execute(query)
        return list(result.scalars().all())

    async def get_overdue_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        current_chapter: int,
    ) -> List[Foreshadow]:
        query = (
            select(Foreshadow)
            .where(
                and_(
                    Foreshadow.project_id == project_id,
                    Foreshadow.status == "planted",
                    Foreshadow.target_resolve_chapter_number != None,
                    Foreshadow.target_resolve_chapter_number < current_chapter,
                )
            )
            .order_by(Foreshadow.target_resolve_chapter_number)
        )
        result = await db.execute(query)
        return list(result.scalars().all())

    async def get_must_resolve_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_number: int,
    ) -> List[Foreshadow]:
        query = (
            select(Foreshadow)
            .where(
                and_(
                    Foreshadow.project_id == project_id,
                    Foreshadow.status == "planted",
                    Foreshadow.target_resolve_chapter_number == chapter_number,
                )
            )
            .order_by(desc(Foreshadow.importance))
        )
        result = await db.execute(query)
        return list(result.scalars().all())

    async def get_foreshadows_to_plant(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_number: int,
    ) -> List[Foreshadow]:
        query = (
            select(Foreshadow)
            .where(
                and_(
                    Foreshadow.project_id == project_id,
                    Foreshadow.status == "pending",
                    Foreshadow.plant_chapter_number == chapter_number,
                )
            )
            .order_by(desc(Foreshadow.importance))
        )
        result = await db.execute(query)
        return list(result.scalars().all())

    async def get_planted_foreshadows_for_analysis(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_number: int,
    ) -> List[Dict[str, Any]]:
        query = (
            select(Foreshadow)
            .where(
                and_(
                    Foreshadow.project_id == project_id,
                    Foreshadow.status == "planted",
                    Foreshadow.plant_chapter_number < chapter_number,
                )
            )
            .order_by(desc(Foreshadow.importance), Foreshadow.plant_chapter_number)
        )
        result = await db.execute(query)
        return [item.to_dict() for item in result.scalars().all()]

    async def clean_chapter_analysis_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_id: str,
    ) -> Dict[str, Any]:
        result = await db.execute(
            delete(Foreshadow)
            .where(Foreshadow.project_id == project_id)
            .where(Foreshadow.source_type == "analysis")
            .where(Foreshadow.plant_chapter_id == chapter_id)
        )
        await db.commit()
        return {"success": True, "cleaned_count": int(result.rowcount or 0)}

    async def delete_chapter_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_id: str,
        only_analysis_source: bool = False,
    ) -> Dict[str, Any]:
        query = delete(Foreshadow).where(
            and_(
                Foreshadow.project_id == project_id,
                or_(
                    Foreshadow.plant_chapter_id == chapter_id,
                    Foreshadow.actual_resolve_chapter_id == chapter_id,
                ),
            )
        )
        if only_analysis_source:
            query = query.where(Foreshadow.source_type == "analysis")

        result = await db.execute(query)
        await db.commit()
        return {"success": True, "deleted_count": int(result.rowcount or 0)}

    async def delete_chapter_analysis_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_id: str,
    ) -> Dict[str, Any]:
        return await self.delete_chapter_foreshadows(
            db=db,
            project_id=project_id,
            chapter_id=chapter_id,
            only_analysis_source=True,
        )

    async def clear_project_foreshadows_for_reset(
        self,
        db: AsyncSession,
        project_id: str,
    ) -> Dict[str, Any]:
        result = await db.execute(
            delete(Foreshadow).where(Foreshadow.project_id == project_id)
        )
        await db.commit()
        return {"success": True, "deleted_count": int(result.rowcount or 0)}

    async def get_stats(
        self,
        db: AsyncSession,
        project_id: str,
        current_chapter: Optional[int] = None,
    ) -> Dict[str, Any]:
        result = await db.execute(select(Foreshadow).where(Foreshadow.project_id == project_id))
        foreshadows = list(result.scalars().all())

        planted = [item for item in foreshadows if item.status == "planted"]
        resolved = [item for item in foreshadows if item.status == "resolved"]
        partially_resolved = [item for item in foreshadows if item.status == "partially_resolved"]
        pending = [item for item in foreshadows if item.status == "pending"]
        abandoned = [item for item in foreshadows if item.status == "abandoned"]

        overdue_count = 0
        if current_chapter is not None:
            overdue_count = sum(
                1
                for item in planted
                if item.target_resolve_chapter_number
                and item.target_resolve_chapter_number < current_chapter
            )

        return {
            "total": len(foreshadows),
            "pending": len(pending),
            "planted": len(planted),
            "resolved": len(resolved),
            "partially_resolved": len(partially_resolved),
            "abandoned": len(abandoned),
            "overdue": overdue_count,
        }

    async def get_context(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_number: int,
        include_pending: bool = True,
        include_overdue: bool = True,
        lookahead: int = 5,
    ) -> Dict[str, Any]:
        must_resolve = await self.get_must_resolve_foreshadows(db, project_id, chapter_number)
        overdue = (
            await self.get_overdue_foreshadows(db, project_id, chapter_number)
            if include_overdue
            else []
        )
        pending_resolve = await self.get_pending_resolve_foreshadows(
            db,
            project_id,
            chapter_number,
            lookahead=lookahead,
        )
        pending = (
            await self.get_foreshadows_to_plant(db, project_id, chapter_number)
            if include_pending
            else []
        )

        return {
            "must_resolve": [item.to_dict() for item in must_resolve],
            "overdue": [item.to_dict() for item in overdue],
            "pending_resolve": [item.to_dict() for item in pending_resolve],
            "pending": [item.to_dict() for item in pending],
        }

    async def list_pending_resolve(
        self,
        db: AsyncSession,
        project_id: str,
        current_chapter: int,
        lookahead: int = 5,
    ) -> Dict[str, Any]:
        items = await self.get_pending_resolve_foreshadows(
            db,
            project_id,
            current_chapter,
            lookahead=lookahead,
        )
        return {"items": [item.to_dict() for item in items], "total": len(items)}

    async def auto_update_from_analysis(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_id: str,
        chapter_number: int,
        analysis_foreshadows: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        stats = {
            "planted_count": 0,
            "resolved_count": 0,
            "created_count": 0,
            "skipped_resolve_count": 0,
            "matched_by_content": 0,
            "created_ids": [],
            "updated_ids": [],
            "errors": [],
        }

        planted_foreshadows = await self.get_planted_foreshadows_for_analysis(
            db,
            project_id,
            chapter_number,
        )

        for fs_data in analysis_foreshadows or []:
            try:
                fs_type = fs_data.get("type")
                matched_by_content = False

                if fs_type == "resolved":
                    reference_id = fs_data.get("reference_foreshadow_id")
                    existing = None

                    if reference_id:
                        existing = await self.get_foreshadow(db, reference_id)
                        if not (existing and existing.project_id == project_id):
                            existing = None

                    if not existing and planted_foreshadows:
                        matched = self._match_foreshadow_by_content(
                            fs_data,
                            planted_foreshadows,
                        )
                        if matched:
                            matched_by_content = True
                            existing = await self.get_foreshadow(db, matched.get("id"))

                    if existing and existing.status == "planted":
                        existing.status = "resolved"
                        existing.actual_resolve_chapter_id = chapter_id
                        existing.actual_resolve_chapter_number = chapter_number
                        existing.resolved_at = datetime.now()
                        if fs_data.get("content"):
                            existing.resolution_text = fs_data.get("content")

                        await db.flush()
                        await db.refresh(existing)

                        stats["resolved_count"] += 1
                        stats["updated_ids"].append(existing.id)
                        if matched_by_content:
                            stats["matched_by_content"] += 1

                        planted_foreshadows = [
                            item
                            for item in planted_foreshadows
                            if item["id"] != existing.id
                        ]
                    elif not existing:
                        stats["skipped_resolve_count"] += 1
                        continue

                elif fs_type == "planted":
                    fs_content = fs_data.get("content", "")
                    if not fs_content:
                        continue

                    fs_title = fs_data.get("title", "")
                    if not fs_title:
                        fs_title = fs_content[:50] + ("..." if len(fs_content) > 50 else "")

                    source_memory_id = generate_stable_foreshadow_id(
                        chapter_id,
                        fs_content,
                        fs_type,
                    )

                    existing_check = await db.execute(
                        select(Foreshadow).where(
                            and_(
                                Foreshadow.project_id == project_id,
                                or_(
                                    Foreshadow.source_memory_id == source_memory_id,
                                    and_(
                                        Foreshadow.title == fs_title,
                                        Foreshadow.plant_chapter_id == chapter_id,
                                        Foreshadow.source_type == "analysis",
                                    ),
                                ),
                            )
                        )
                    )
                    existing_fs = existing_check.scalar_one_or_none()

                    if existing_fs:
                        existing_fs.title = fs_title
                        existing_fs.content = fs_content
                        existing_fs.strength = fs_data.get("strength", existing_fs.strength)
                        existing_fs.subtlety = fs_data.get("subtlety", existing_fs.subtlety)
                        existing_fs.hint_text = fs_data.get("keyword", existing_fs.hint_text)
                        existing_fs.category = fs_data.get("category", existing_fs.category)
                        existing_fs.is_long_term = fs_data.get(
                            "is_long_term",
                            existing_fs.is_long_term,
                        )
                        existing_fs.related_characters = fs_data.get(
                            "related_characters",
                            existing_fs.related_characters,
                        )
                        if fs_data.get("estimated_resolve_chapter"):
                            existing_fs.target_resolve_chapter_number = fs_data.get(
                                "estimated_resolve_chapter"
                            )
                        existing_fs.source_memory_id = source_memory_id
                        await db.flush()
                        stats["updated_ids"].append(existing_fs.id)
                    else:
                        estimated_resolve = fs_data.get("estimated_resolve_chapter")
                        new_foreshadow = Foreshadow(
                            id=str(uuid.uuid4()),
                            project_id=project_id,
                            title=fs_title,
                            content=fs_content,
                            hint_text=fs_data.get("keyword"),
                            source_type="analysis",
                            source_memory_id=source_memory_id,
                            plant_chapter_id=chapter_id,
                            plant_chapter_number=chapter_number,
                            planted_at=datetime.now(),
                            target_resolve_chapter_number=estimated_resolve,
                            status="planted",
                            is_long_term=fs_data.get("is_long_term", False),
                            importance=min(fs_data.get("strength", 5) / 10.0, 1.0),
                            strength=fs_data.get("strength", 5),
                            subtlety=fs_data.get("subtlety", 5),
                            category=fs_data.get("category"),
                            related_characters=fs_data.get("related_characters"),
                            auto_remind=True,
                            remind_before_chapters=5,
                            include_in_context=True,
                        )
                        db.add(new_foreshadow)
                        await db.flush()

                        stats["planted_count"] += 1
                        stats["created_count"] += 1
                        stats["created_ids"].append(new_foreshadow.id)
            except Exception as item_error:
                error_msg = f"处理伏笔时出错: {str(item_error)}"
                stats["errors"].append(error_msg)
                logger.error(f"❌ {error_msg}")

        await db.commit()
        return stats

    async def auto_plant_pending_foreshadows(
        self,
        db: AsyncSession,
        project_id: str,
        chapter_id: str,
        chapter_number: int,
        chapter_content: str,
    ) -> Dict[str, Any]:
        stats = {"checked_count": 0, "planted_count": 0, "planted_ids": []}
        pending_foreshadows = await self.get_foreshadows_to_plant(
            db,
            project_id,
            chapter_number,
        )
        stats["checked_count"] = len(pending_foreshadows)

        for foreshadow in pending_foreshadows:
            foreshadow.status = "planted"
            foreshadow.plant_chapter_id = chapter_id
            foreshadow.planted_at = datetime.now()
            await db.flush()
            stats["planted_count"] += 1
            stats["planted_ids"].append(foreshadow.id)

        await db.commit()
        return stats

    def _match_foreshadow_by_content(
        self,
        resolved_fs_data: Dict[str, Any],
        planted_foreshadows: List[Dict[str, Any]],
        min_similarity: float = 0.5,
    ) -> Optional[Dict[str, Any]]:
        if not planted_foreshadows:
            return None

        resolved_title = resolved_fs_data.get("title", "").strip()
        resolved_content = resolved_fs_data.get("content", "").strip()
        resolved_keyword = resolved_fs_data.get("keyword", "").strip()
        resolved_category = resolved_fs_data.get("category")
        resolved_characters = set(resolved_fs_data.get("related_characters", []))
        reference_chapter = resolved_fs_data.get("reference_chapter")

        resolved_title_clean = resolved_title
        for suffix in ["回收", "揭示", "解答", "兑现"]:
            if resolved_title.endswith(suffix):
                resolved_title_clean = resolved_title[: -len(suffix)]
                break

        best_match = None
        best_score = 0.0

        for item in planted_foreshadows:
            score = 0.0
            fs_title = item.get("title", "").strip()
            fs_content = item.get("content", "").strip()
            fs_category = item.get("category")
            fs_characters = set(item.get("related_characters", []))
            fs_plant_chapter = item.get("plant_chapter_number")

            if resolved_title and fs_title:
                if resolved_title == fs_title:
                    score = 1.0
                elif resolved_title_clean and resolved_title_clean == fs_title:
                    score = 0.95
                elif resolved_title in fs_title or fs_title in resolved_title:
                    score = max(score, 0.8)
                elif resolved_title_clean and (
                    resolved_title_clean in fs_title or fs_title in resolved_title_clean
                ):
                    score = max(score, 0.75)
                else:
                    title_overlap = self._calculate_word_overlap(resolved_title, fs_title)
                    score = max(score, title_overlap * 0.7)

            if resolved_keyword and fs_content and resolved_keyword in fs_content:
                score = max(score, 0.75)

            if resolved_content and fs_content:
                content_overlap = self._calculate_word_overlap(resolved_content, fs_content)
                score = max(score, content_overlap * 0.6)

            if reference_chapter and fs_plant_chapter and reference_chapter == fs_plant_chapter:
                score += 0.15

            if resolved_category and fs_category and resolved_category == fs_category:
                score += 0.1

            if resolved_characters and fs_characters:
                overlap = len(resolved_characters & fs_characters) / max(
                    len(resolved_characters | fs_characters),
                    1,
                )
                score += overlap * 0.1

            if score > best_score and score >= min_similarity:
                best_score = score
                best_match = item

        return best_match

    def _calculate_word_overlap(self, text1: str, text2: str) -> float:
        if not text1 or not text2:
            return 0.0

        def get_ngrams(text: str, n: int) -> set[str]:
            normalized = text.lower().replace(" ", "").replace("\n", "")
            if len(normalized) < n:
                return {normalized}
            return {
                normalized[index:index + n]
                for index in range(len(normalized) - n + 1)
            }

        ngrams1_2 = get_ngrams(text1, 2)
        ngrams2_2 = get_ngrams(text2, 2)
        overlap_2 = len(ngrams1_2 & ngrams2_2) / max(len(ngrams1_2 | ngrams2_2), 1)

        ngrams1_3 = get_ngrams(text1, 3)
        ngrams2_3 = get_ngrams(text2, 3)
        overlap_3 = len(ngrams1_3 & ngrams2_3) / max(len(ngrams1_3 | ngrams2_3), 1)
        return overlap_2 * 0.4 + overlap_3 * 0.6


foreshadow_service = ForeshadowService()




