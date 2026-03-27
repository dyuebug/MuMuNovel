from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
import json
import re
from typing import Any, Mapping, Optional

from sqlalchemy import event, or_, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import Session

from app.models.chapter import Chapter
from app.models.character import Character
from app.models.career import Career, CharacterCareer
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.relationship import CharacterRelationship, Organization

ProjectContinuityLedgerItem = str | dict[str, Any]

_MAX_RECENT_ANALYSES = 12
_EMPTY_TIME = datetime.min


_SESSION_CACHE_KEY = "project_continuity_ledger_cache"


def _resolve_session_cache_store(db_session: AsyncSession) -> dict[Any, ProjectContinuityLedger]:
    sync_session = getattr(db_session, "sync_session", None)
    info = getattr(sync_session, "info", None)
    if not isinstance(info, dict):
        return {}
    cache_store = info.get(_SESSION_CACHE_KEY)
    if isinstance(cache_store, dict):
        return cache_store
    cache_store = {}
    info[_SESSION_CACHE_KEY] = cache_store
    return cache_store


def _build_session_cache_key(
    db_session: AsyncSession,
    project_id: str,
    limit: int,
) -> tuple[str, int]:
    return str(project_id), int(limit)


@event.listens_for(Session, "after_commit")
@event.listens_for(Session, "after_rollback")
def _clear_project_continuity_ledger_cache(sync_session: Session) -> None:
    info = getattr(sync_session, "info", None)
    if isinstance(info, dict):
        info.pop(_SESSION_CACHE_KEY, None)


@dataclass(frozen=True)
class ProjectContinuityLedger:
    """项目级 continuity ledger 聚合结果。"""

    character_state_ledger: tuple[ProjectContinuityLedgerItem, ...] = ()
    relationship_state_ledger: tuple[ProjectContinuityLedgerItem, ...] = ()
    foreshadow_state_ledger: tuple[ProjectContinuityLedgerItem, ...] = ()
    organization_state_ledger: tuple[ProjectContinuityLedgerItem, ...] = ()
    career_state_ledger: tuple[ProjectContinuityLedgerItem, ...] = ()

    def has_any_entries(self) -> bool:
        return bool(
            self.character_state_ledger
            or self.relationship_state_ledger
            or self.foreshadow_state_ledger
            or self.organization_state_ledger
            or self.career_state_ledger
        )


def _compact_text(value: Any, *, limit: int = 72) -> str:
    text = re.sub(r"\s+", " ", str(value or "")).strip()
    if not text:
        return ""
    if len(text) <= limit:
        return text
    return f"{text[:limit - 3].rstrip()}..."


def _append_unique(items: list[str], seen_keys: set[Any], dedupe_key: Any, value: Optional[str], *, limit: int) -> None:
    if len(items) >= limit:
        return
    text = _compact_text(value)
    if not text or dedupe_key in seen_keys:
        return
    seen_keys.add(dedupe_key)
    items.append(text)


def _append_unique_entry(
    items: list[ProjectContinuityLedgerItem],
    seen_keys: set[Any],
    dedupe_key: Any,
    *,
    label: Optional[str] = None,
    summary: Optional[str] = None,
    status: Optional[str] = None,
    target_chapter: Optional[int] = None,
    limit: int,
) -> None:
    if len(items) >= limit or dedupe_key in seen_keys:
        return

    normalized_label = _compact_text(label, limit=36)
    normalized_summary = _compact_text(summary, limit=72)
    normalized_status = _normalize_status_label(status)

    entry: dict[str, Any] = {}
    if normalized_label:
        entry["label"] = normalized_label
    if normalized_summary:
        entry["summary"] = normalized_summary
    if normalized_status:
        entry["status"] = normalized_status
    if isinstance(target_chapter, int) and target_chapter > 0:
        entry["target_chapter"] = target_chapter

    if not entry:
        return

    seen_keys.add(dedupe_key)
    items.append(entry)


def _safe_int(value: Any) -> Optional[int]:
    try:
        if value in (None, ""):
            return None
        return int(value)
    except (TypeError, ValueError):
        return None


def _safe_json_list(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if isinstance(value, str):
        try:
            loaded = json.loads(value)
        except Exception:
            return []
        return loaded if isinstance(loaded, list) else []
    return []


def _normalize_status_label(value: Any) -> str:
    normalized = _compact_text(value, limit=24).lower()
    if normalized in {"", "active", "alive", "normal"}:
        return ""
    return normalized


def _normalize_mapping_list(value: Any) -> list[Mapping[str, Any]]:
    if isinstance(value, list):
        return [item for item in value if isinstance(item, Mapping)]
    if isinstance(value, Mapping):
        return [value]
    return []


def _sort_time(value: Any) -> datetime:
    return value if isinstance(value, datetime) else _EMPTY_TIME


def _relationship_pair_key(name_a: str, name_b: str) -> tuple[str, str]:
    return tuple(sorted((name_a.lower(), name_b.lower())))


def _build_character_state_items(
    characters: list[Character],
    analyses: list[PlotAnalysis],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_names: set[str] = set()
    ranked_characters = sorted(
        characters,
        key=lambda character: (
            _safe_int(getattr(character, "state_updated_chapter", None)) or -1,
            _safe_int(getattr(character, "status_changed_chapter", None)) or -1,
            _sort_time(getattr(character, "updated_at", None)),
            _sort_time(getattr(character, "created_at", None)),
        ),
        reverse=True,
    )
    for character in ranked_characters:
        if getattr(character, "is_organization", False):
            continue
        name = _compact_text(getattr(character, "name", None), limit=32)
        if not name:
            continue
        fragments: list[str] = []
        current_state = _compact_text(getattr(character, "current_state", None), limit=72)
        if current_state:
            fragments.append(current_state)
        status = _normalize_status_label(getattr(character, "status", None))
        if not fragments and not status:
            continue
        _append_unique_entry(
            items,
            seen_names,
            name.lower(),
            label=name,
            summary='; '.join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    for analysis in analyses:
        for state in reversed(_normalize_mapping_list(getattr(analysis, "character_states", None))):
            name = _compact_text(state.get("character_name") or state.get("name"), limit=32)
            if not name or name.lower() in seen_names:
                continue
            state_text = _compact_text(
                state.get("state_after") or state.get("psychological_change") or state.get("current_state") or state.get("state"),
                limit=72,
            )
            if not state_text:
                continue
            _append_unique_entry(
                items,
                seen_names,
                name.lower(),
                label=name,
                summary=state_text,
                limit=limit,
            )
            if len(items) >= limit:
                return tuple(items)
    return tuple(items)


def _build_relationship_state_items(
    relationships: list[CharacterRelationship],
    character_name_map: Mapping[str, str],
    analyses: list[PlotAnalysis],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_pairs: set[tuple[str, str]] = set()
    ranked_relationships = sorted(
        relationships,
        key=lambda relationship: (
            _sort_time(getattr(relationship, "updated_at", None)),
            _sort_time(getattr(relationship, "created_at", None)),
            abs(_safe_int(getattr(relationship, "intimacy_level", None)) or 0),
        ),
        reverse=True,
    )
    for relationship in ranked_relationships:
        from_name = _compact_text(character_name_map.get(getattr(relationship, "character_from_id", "")), limit=24)
        to_name = _compact_text(character_name_map.get(getattr(relationship, "character_to_id", "")), limit=24)
        if not from_name or not to_name or from_name == to_name:
            continue
        pair_key = _relationship_pair_key(from_name, to_name)
        if pair_key in seen_pairs:
            continue
        fragments: list[str] = []
        relationship_name = _compact_text(getattr(relationship, "relationship_name", None), limit=40)
        description = _compact_text(getattr(relationship, "description", None), limit=72)
        if relationship_name:
            fragments.append(relationship_name)
        if description and description.lower() != relationship_name.lower():
            fragments.append(description)
        intimacy_level = _safe_int(getattr(relationship, "intimacy_level", None))
        if not fragments and intimacy_level is not None:
            fragments.append(f"intimacy={intimacy_level}")
        status = _normalize_status_label(getattr(relationship, "status", None))
        if not fragments and not status:
            continue
        _append_unique_entry(
            items,
            seen_pairs,
            pair_key,
            label=f"{from_name}/{to_name}",
            summary='; '.join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    for analysis in analyses:
        for state in reversed(_normalize_mapping_list(getattr(analysis, "character_states", None))):
            base_name = _compact_text(state.get("character_name") or state.get("name"), limit=24)
            relationship_changes = state.get("relationship_changes")
            if not base_name or not isinstance(relationship_changes, Mapping):
                continue
            for other_name_raw, change_raw in relationship_changes.items():
                other_name = _compact_text(other_name_raw, limit=24)
                change_text = _compact_text(change_raw, limit=72)
                if not other_name or not change_text:
                    continue
                pair_key = _relationship_pair_key(base_name, other_name)
                if pair_key in seen_pairs:
                    continue
                _append_unique_entry(
                    items,
                    seen_pairs,
                    pair_key,
                    label=f"{base_name}/{other_name}",
                    summary=change_text,
                    limit=limit,
                )
                if len(items) >= limit:
                    return tuple(items)
    return tuple(items)


def _build_foreshadow_state_items(
    foreshadow_memories: list[StoryMemory],
    analyses: list[PlotAnalysis],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_heads: set[str] = set()
    ranked_memories = sorted(
        foreshadow_memories,
        key=lambda memory: (
            getattr(memory, "importance_score", 0.0) or 0.0,
            getattr(memory, "foreshadow_strength", 0.0) or 0.0,
            _safe_int(getattr(memory, "story_timeline", None)) or -1,
            _sort_time(getattr(memory, "updated_at", None)),
            _sort_time(getattr(memory, "created_at", None)),
        ),
        reverse=True,
    )
    for memory in ranked_memories:
        head = _compact_text(getattr(memory, "title", None), limit=36) or _compact_text(getattr(memory, "content", None), limit=36)
        if not head:
            continue
        detail = _compact_text(getattr(memory, "content", None), limit=72)
        _append_unique_entry(
            items,
            seen_heads,
            head.lower(),
            label=head,
            summary=(detail if detail and detail.lower() != head.lower() else None),
            status="planted",
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    for analysis in analyses:
        for foreshadow in reversed(_normalize_mapping_list(getattr(analysis, "foreshadows", None))):
            foreshadow_type = _compact_text(foreshadow.get("type"), limit=16).lower()
            if foreshadow_type == "resolved":
                continue
            head = _compact_text(foreshadow.get("content") or foreshadow.get("title"), limit=36)
            if not head:
                continue
            _append_unique_entry(
                items,
                seen_heads,
                head.lower(),
                label=head,
                status=foreshadow_type if foreshadow_type else None,
                limit=limit,
            )
            if len(items) >= limit:
                return tuple(items)
    return tuple(items)


def _build_organization_state_items(
    organizations: list[tuple[Character, Optional[Organization]]],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_names: set[str] = set()
    ranked_orgs = sorted(
        organizations,
        key=lambda pair: (
            _safe_int(getattr(pair[0], "state_updated_chapter", None)) or -1,
            _safe_int(getattr(pair[0], "status_changed_chapter", None)) or -1,
            _sort_time(getattr(pair[0], "updated_at", None)),
            _sort_time(getattr(pair[1], "updated_at", None) if pair[1] else None),
        ),
        reverse=True,
    )
    for org_char, organization in ranked_orgs:
        name = _compact_text(getattr(org_char, "name", None), limit=36)
        if not name:
            continue
        fragments: list[str] = []
        current_state = _compact_text(getattr(org_char, "current_state", None), limit=72)
        if current_state:
            fragments.append(current_state)
        status = _normalize_status_label(getattr(org_char, "status", None))
        if organization is not None:
            power_level = _safe_int(getattr(organization, "power_level", None))
            if power_level is not None:
                fragments.append(f"power={power_level}")
            location = _compact_text(getattr(organization, "location", None), limit=36)
            if location:
                fragments.append(f"location={location}")
        if not fragments and not status:
            continue
        _append_unique_entry(
            items,
            seen_names,
            name.lower(),
            label=name,
            summary='; '.join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    return tuple(items)


def _build_career_state_items(
    career_rows: list[tuple[CharacterCareer, Character, Career]],
    characters: list[Character],
    career_map: Mapping[str, Career],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_keys: set[tuple[str, str]] = set()
    ranked_rows = sorted(
        career_rows,
        key=lambda row: (
            1 if getattr(row[0], "career_type", "") == "main" else 0,
            _sort_time(getattr(row[0], "updated_at", None)),
            _safe_int(getattr(row[0], "current_stage", None)) or 0,
        ),
        reverse=True,
    )
    for character_career, character, career in ranked_rows:
        if getattr(character, "is_organization", False):
            continue
        char_name = _compact_text(getattr(character, "name", None), limit=24)
        career_name = _compact_text(getattr(career, "name", None), limit=24)
        if not char_name or not career_name:
            continue
        dedupe_key = (char_name.lower(), career_name.lower())
        fragments = [f"stage {max(_safe_int(getattr(character_career, 'current_stage', None)) or 1, 1)}"]
        progress = _safe_int(getattr(character_career, "stage_progress", None))
        if progress:
            fragments.append(f"progress {progress}%")
        notes = _compact_text(getattr(character_career, "notes", None), limit=48)
        if notes:
            fragments.append(notes)
        _append_unique_entry(
            items,
            seen_keys,
            dedupe_key,
            label=f"{char_name}/{career_name}",
            summary='; '.join(list(dict.fromkeys(fragments))[:2]),
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    if items:
        return tuple(items)
    for character in characters:
        if getattr(character, "is_organization", False):
            continue
        char_name = _compact_text(getattr(character, "name", None), limit=24)
        if not char_name:
            continue
        main_career = career_map.get(getattr(character, "main_career_id", None) or "")
        if main_career is not None:
            career_name = _compact_text(getattr(main_career, "name", None), limit=24)
            stage = max(_safe_int(getattr(character, "main_career_stage", None)) or 1, 1)
            _append_unique_entry(
                items,
                seen_keys,
                (char_name.lower(), career_name.lower()),
                label=f"{char_name}/{career_name}",
                summary=f"stage {stage}",
                limit=limit,
            )
            if len(items) >= limit:
                return tuple(items)
        for sub_data in _safe_json_list(getattr(character, "sub_careers", None)):
            if not isinstance(sub_data, Mapping):
                continue
            career = career_map.get(str(sub_data.get("career_id") or ""))
            career_name = _compact_text(getattr(career, "name", None), limit=24) if career is not None else ""
            if not career_name:
                continue
            stage = max(_safe_int(sub_data.get("stage")) or 1, 1)
            _append_unique_entry(
                items,
                seen_keys,
                (char_name.lower(), career_name.lower()),
                label=f"{char_name}/{career_name}",
                summary=f"stage {stage}",
                limit=limit,
            )
            if len(items) >= limit:
                return tuple(items)
    return tuple(items)


async def build_project_continuity_ledger(db_session: AsyncSession, project_id: Optional[str], *, limit: int = 4) -> ProjectContinuityLedger:
    """构建项目 continuity ledger，汇总角色、关系、伏笔等关键连续性状态。"""
    if not project_id:
        return ProjectContinuityLedger()

    resolved_limit = max(1, int(limit or 4))
    cache_store = _resolve_session_cache_store(db_session)
    cache_key = _build_session_cache_key(db_session, str(project_id), resolved_limit)
    cached_ledger = cache_store.get(cache_key)
    if isinstance(cached_ledger, ProjectContinuityLedger):
        return cached_ledger

    character_result = await db_session.execute(select(Character).where(Character.project_id == project_id))
    characters = list(character_result.scalars().all())
    character_name_map = {
        character.id: _compact_text(character.name, limit=24)
        for character in characters
        if getattr(character, "id", None) and getattr(character, "name", None)
    }
    organization_result = await db_session.execute(select(Organization).where(Organization.project_id == project_id))
    organizations = list(organization_result.scalars().all())
    org_by_char_id = {organization.character_id: organization for organization in organizations}
    organization_pairs = [
        (character, org_by_char_id.get(character.id))
        for character in characters
        if getattr(character, "is_organization", False)
    ]
    relationship_result = await db_session.execute(
        select(CharacterRelationship).where(CharacterRelationship.project_id == project_id)
    )
    relationships = list(relationship_result.scalars().all())
    foreshadow_result = await db_session.execute(
        select(StoryMemory).where(
            StoryMemory.project_id == project_id,
            or_(StoryMemory.memory_type == "foreshadow", StoryMemory.is_foreshadow > 0),
            StoryMemory.foreshadow_resolved_at.is_(None),
            StoryMemory.is_foreshadow != 2,
        )
    )
    foreshadow_memories = list(foreshadow_result.scalars().all())
    analysis_result = await db_session.execute(
        select(PlotAnalysis)
        .join(Chapter, PlotAnalysis.chapter_id == Chapter.id)
        .where(Chapter.project_id == project_id)
        .order_by(Chapter.chapter_number.desc(), PlotAnalysis.created_at.desc())
        .limit(max(_MAX_RECENT_ANALYSES, resolved_limit * 3))
    )
    analyses = list(analysis_result.scalars().all())
    career_result = await db_session.execute(select(Career).where(Career.project_id == project_id))
    careers = list(career_result.scalars().all())
    career_map = {career.id: career for career in careers if getattr(career, "id", None)}
    character_career_result = await db_session.execute(
        select(CharacterCareer, Character, Career)
        .join(Character, CharacterCareer.character_id == Character.id)
        .join(Career, CharacterCareer.career_id == Career.id)
        .where(Character.project_id == project_id)
    )
    career_rows = list(character_career_result.all())
    ledger = ProjectContinuityLedger(
        character_state_ledger=_build_character_state_items(characters, analyses, limit=resolved_limit),
        relationship_state_ledger=_build_relationship_state_items(
            relationships,
            character_name_map,
            analyses,
            limit=resolved_limit,
        ),
        foreshadow_state_ledger=_build_foreshadow_state_items(
            foreshadow_memories,
            analyses,
            limit=resolved_limit,
        ),
        organization_state_ledger=_build_organization_state_items(
            organization_pairs,
            limit=resolved_limit,
        ),
        career_state_ledger=_build_career_state_items(
            career_rows,
            characters,
            career_map,
            limit=resolved_limit,
        ),
    )
    cache_store[cache_key] = ledger
    return ledger
