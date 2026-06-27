from __future__ import annotations

import json
import re
from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Dict, Mapping, Optional

from sqlalchemy import event, or_, select
from sqlalchemy.orm import Session

from migrator_app.models.chapter import Chapter
from migrator_app.models.career import Career, CharacterCareer
from migrator_app.models.character import Character
from migrator_app.models.memory_analysis import PlotAnalysis, StoryMemory
from migrator_app.models.organization import Organization
from migrator_app.models.relationship import CharacterRelationship

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.project import Project
    from tests.test_support.story_packet_test_support import StoryPacket


ProjectContinuityLedgerItem = str | Dict[str, Any]

_MAX_RECENT_PROJECT_CONTINUITY_ANALYSES = 12
_PROJECT_CONTINUITY_EMPTY_TIME = datetime.min
_PROJECT_CONTINUITY_SESSION_CACHE_KEY = "project_continuity_ledger_cache"


def _resolve_project_continuity_session_cache_store(
    db_session: "AsyncSession",
) -> dict[Any, "ProjectContinuityLedger"]:
    sync_session = getattr(db_session, "sync_session", None)
    info = getattr(sync_session, "info", None)
    if not isinstance(info, dict):
        return {}
    cache_store = info.get(_PROJECT_CONTINUITY_SESSION_CACHE_KEY)
    if isinstance(cache_store, dict):
        return cache_store
    cache_store = {}
    info[_PROJECT_CONTINUITY_SESSION_CACHE_KEY] = cache_store
    return cache_store


def _build_project_continuity_session_cache_key(
    project_id: str,
    limit: int,
) -> tuple[str, int]:
    return str(project_id), int(limit)


@event.listens_for(Session, "after_commit")
@event.listens_for(Session, "after_rollback")
def _clear_project_continuity_ledger_cache(sync_session: Session) -> None:
    info = getattr(sync_session, "info", None)
    if isinstance(info, dict):
        info.pop(_PROJECT_CONTINUITY_SESSION_CACHE_KEY, None)


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


def _compact_project_continuity_text(value: Any, *, limit: int = 72) -> str:
    text = re.sub(r"\s+", " ", str(value or "")).strip()
    if not text:
        return ""
    if len(text) <= limit:
        return text
    return f"{text[:limit - 3].rstrip()}..."


def _normalize_project_continuity_status_label(value: Any) -> str:
    normalized = _compact_project_continuity_text(value, limit=24).lower()
    if normalized in {"", "active", "alive", "normal"}:
        return ""
    return normalized


def _append_unique_project_continuity_entry(
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

    normalized_label = _compact_project_continuity_text(label, limit=36)
    normalized_summary = _compact_project_continuity_text(summary, limit=72)
    normalized_status = _normalize_project_continuity_status_label(status)

    entry: Dict[str, Any] = {}
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


def _safe_project_continuity_int(value: Any) -> Optional[int]:
    try:
        if value in (None, ""):
            return None
        return int(value)
    except (TypeError, ValueError):
        return None


def _safe_project_continuity_json_list(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if isinstance(value, str):
        try:
            loaded = json.loads(value)
        except Exception:
            return []
        return loaded if isinstance(loaded, list) else []
    return []


def _normalize_project_continuity_mapping_list(
    value: Any,
) -> list[Mapping[str, Any]]:
    if isinstance(value, list):
        return [item for item in value if isinstance(item, Mapping)]
    if isinstance(value, Mapping):
        return [value]
    return []


def _project_continuity_sort_time(value: Any) -> datetime:
    return value if isinstance(value, datetime) else _PROJECT_CONTINUITY_EMPTY_TIME


def _build_project_continuity_relationship_pair_key(
    name_a: str,
    name_b: str,
) -> tuple[str, str]:
    return tuple(sorted((name_a.lower(), name_b.lower())))


def _build_project_continuity_character_state_items(
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
            _safe_project_continuity_int(getattr(character, "state_updated_chapter", None)) or -1,
            _safe_project_continuity_int(getattr(character, "status_changed_chapter", None)) or -1,
            _project_continuity_sort_time(getattr(character, "updated_at", None)),
            _project_continuity_sort_time(getattr(character, "created_at", None)),
        ),
        reverse=True,
    )
    for character in ranked_characters:
        if getattr(character, "is_organization", False):
            continue
        name = _compact_project_continuity_text(getattr(character, "name", None), limit=32)
        if not name:
            continue
        fragments: list[str] = []
        current_state = _compact_project_continuity_text(
            getattr(character, "current_state", None),
            limit=72,
        )
        if current_state:
            fragments.append(current_state)
        status = _normalize_project_continuity_status_label(getattr(character, "status", None))
        if not fragments and not status:
            continue
        _append_unique_project_continuity_entry(
            items,
            seen_names,
            name.lower(),
            label=name,
            summary="; ".join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    for analysis in analyses:
        for state in reversed(
            _normalize_project_continuity_mapping_list(
                getattr(analysis, "character_states", None)
            )
        ):
            name = _compact_project_continuity_text(
                state.get("character_name") or state.get("name"),
                limit=32,
            )
            if not name or name.lower() in seen_names:
                continue
            state_text = _compact_project_continuity_text(
                state.get("state_after")
                or state.get("psychological_change")
                or state.get("current_state")
                or state.get("state"),
                limit=72,
            )
            if not state_text:
                continue
            _append_unique_project_continuity_entry(
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


def _build_project_continuity_relationship_state_items(
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
            _project_continuity_sort_time(getattr(relationship, "updated_at", None)),
            _project_continuity_sort_time(getattr(relationship, "created_at", None)),
            abs(_safe_project_continuity_int(getattr(relationship, "intimacy_level", None)) or 0),
        ),
        reverse=True,
    )
    for relationship in ranked_relationships:
        from_name = _compact_project_continuity_text(
            character_name_map.get(getattr(relationship, "character_from_id", "")),
            limit=24,
        )
        to_name = _compact_project_continuity_text(
            character_name_map.get(getattr(relationship, "character_to_id", "")),
            limit=24,
        )
        if not from_name or not to_name or from_name == to_name:
            continue
        pair_key = _build_project_continuity_relationship_pair_key(from_name, to_name)
        if pair_key in seen_pairs:
            continue
        fragments: list[str] = []
        relationship_name = _compact_project_continuity_text(
            getattr(relationship, "relationship_name", None),
            limit=40,
        )
        description = _compact_project_continuity_text(
            getattr(relationship, "description", None),
            limit=72,
        )
        if relationship_name:
            fragments.append(relationship_name)
        if description and description.lower() != relationship_name.lower():
            fragments.append(description)
        intimacy_level = _safe_project_continuity_int(getattr(relationship, "intimacy_level", None))
        if not fragments and intimacy_level is not None:
            fragments.append(f"intimacy={intimacy_level}")
        status = _normalize_project_continuity_status_label(getattr(relationship, "status", None))
        if not fragments and not status:
            continue
        _append_unique_project_continuity_entry(
            items,
            seen_pairs,
            pair_key,
            label=f"{from_name}/{to_name}",
            summary="; ".join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    for analysis in analyses:
        for state in reversed(
            _normalize_project_continuity_mapping_list(
                getattr(analysis, "character_states", None)
            )
        ):
            base_name = _compact_project_continuity_text(
                state.get("character_name") or state.get("name"),
                limit=24,
            )
            relationship_changes = state.get("relationship_changes")
            if not base_name or not isinstance(relationship_changes, Mapping):
                continue
            for other_name_raw, change_raw in relationship_changes.items():
                other_name = _compact_project_continuity_text(other_name_raw, limit=24)
                change_text = _compact_project_continuity_text(change_raw, limit=72)
                if not other_name or not change_text:
                    continue
                pair_key = _build_project_continuity_relationship_pair_key(base_name, other_name)
                if pair_key in seen_pairs:
                    continue
                _append_unique_project_continuity_entry(
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


def _build_project_continuity_foreshadow_state_items(
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
            _safe_project_continuity_int(getattr(memory, "story_timeline", None)) or -1,
            _project_continuity_sort_time(getattr(memory, "updated_at", None)),
            _project_continuity_sort_time(getattr(memory, "created_at", None)),
        ),
        reverse=True,
    )
    for memory in ranked_memories:
        head = _compact_project_continuity_text(getattr(memory, "title", None), limit=36) or _compact_project_continuity_text(
            getattr(memory, "content", None),
            limit=36,
        )
        if not head:
            continue
        detail = _compact_project_continuity_text(
            getattr(memory, "content", None),
            limit=72,
        )
        _append_unique_project_continuity_entry(
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
        for foreshadow in reversed(
            _normalize_project_continuity_mapping_list(getattr(analysis, "foreshadows", None))
        ):
            foreshadow_type = _compact_project_continuity_text(
                foreshadow.get("type"),
                limit=16,
            ).lower()
            if foreshadow_type == "resolved":
                continue
            head = _compact_project_continuity_text(
                foreshadow.get("content") or foreshadow.get("title"),
                limit=36,
            )
            if not head:
                continue
            _append_unique_project_continuity_entry(
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


def _build_project_continuity_organization_state_items(
    organizations: list[tuple[Character, Optional[Organization]]],
    *,
    limit: int,
) -> tuple[ProjectContinuityLedgerItem, ...]:
    items: list[ProjectContinuityLedgerItem] = []
    seen_names: set[str] = set()
    ranked_orgs = sorted(
        organizations,
        key=lambda pair: (
            _safe_project_continuity_int(getattr(pair[0], "state_updated_chapter", None)) or -1,
            _safe_project_continuity_int(getattr(pair[0], "status_changed_chapter", None)) or -1,
            _project_continuity_sort_time(getattr(pair[0], "updated_at", None)),
            _project_continuity_sort_time(getattr(pair[1], "updated_at", None) if pair[1] else None),
        ),
        reverse=True,
    )
    for org_char, organization in ranked_orgs:
        name = _compact_project_continuity_text(getattr(org_char, "name", None), limit=36)
        if not name:
            continue
        fragments: list[str] = []
        current_state = _compact_project_continuity_text(
            getattr(org_char, "current_state", None),
            limit=72,
        )
        if current_state:
            fragments.append(current_state)
        status = _normalize_project_continuity_status_label(getattr(org_char, "status", None))
        if organization is not None:
            power_level = _safe_project_continuity_int(getattr(organization, "power_level", None))
            if power_level is not None:
                fragments.append(f"power={power_level}")
            location = _compact_project_continuity_text(
                getattr(organization, "location", None),
                limit=36,
            )
            if location:
                fragments.append(f"location={location}")
        if not fragments and not status:
            continue
        _append_unique_project_continuity_entry(
            items,
            seen_names,
            name.lower(),
            label=name,
            summary="; ".join(list(dict.fromkeys(fragments))[:2]),
            status=status,
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    return tuple(items)


def _build_project_continuity_career_state_items(
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
            _project_continuity_sort_time(getattr(row[0], "updated_at", None)),
            _safe_project_continuity_int(getattr(row[0], "current_stage", None)) or 0,
        ),
        reverse=True,
    )
    for character_career, character, career in ranked_rows:
        if getattr(character, "is_organization", False):
            continue
        char_name = _compact_project_continuity_text(getattr(character, "name", None), limit=24)
        career_name = _compact_project_continuity_text(getattr(career, "name", None), limit=24)
        if not char_name or not career_name:
            continue
        dedupe_key = (char_name.lower(), career_name.lower())
        fragments = [f"stage {max(_safe_project_continuity_int(getattr(character_career, 'current_stage', None)) or 1, 1)}"]
        progress = _safe_project_continuity_int(getattr(character_career, "stage_progress", None))
        if progress:
            fragments.append(f"progress {progress}%")
        notes = _compact_project_continuity_text(getattr(character_career, "notes", None), limit=48)
        if notes:
            fragments.append(notes)
        _append_unique_project_continuity_entry(
            items,
            seen_keys,
            dedupe_key,
            label=f"{char_name}/{career_name}",
            summary="; ".join(list(dict.fromkeys(fragments))[:2]),
            limit=limit,
        )
        if len(items) >= limit:
            return tuple(items)
    if items:
        return tuple(items)
    for character in characters:
        if getattr(character, "is_organization", False):
            continue
        char_name = _compact_project_continuity_text(getattr(character, "name", None), limit=24)
        if not char_name:
            continue
        main_career = career_map.get(getattr(character, "main_career_id", None) or "")
        if main_career is not None:
            career_name = _compact_project_continuity_text(getattr(main_career, "name", None), limit=24)
            stage = max(_safe_project_continuity_int(getattr(character, "main_career_stage", None)) or 1, 1)
            _append_unique_project_continuity_entry(
                items,
                seen_keys,
                (char_name.lower(), career_name.lower()),
                label=f"{char_name}/{career_name}",
                summary=f"stage {stage}",
                limit=limit,
            )
            if len(items) >= limit:
                return tuple(items)
        for sub_data in _safe_project_continuity_json_list(getattr(character, "sub_careers", None)):
            if not isinstance(sub_data, Mapping):
                continue
            career = career_map.get(str(sub_data.get("career_id") or ""))
            career_name = _compact_project_continuity_text(getattr(career, "name", None), limit=24) if career is not None else ""
            if not career_name:
                continue
            stage = max(_safe_project_continuity_int(sub_data.get("stage")) or 1, 1)
            _append_unique_project_continuity_entry(
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


async def build_project_continuity_ledger(
    db_session: "AsyncSession",
    project_id: Optional[str],
    *,
    limit: int = 4,
) -> ProjectContinuityLedger:
    """构建项目 continuity ledger，汇总角色、关系、伏笔等关键连续性状态。"""
    if not project_id:
        return ProjectContinuityLedger()

    resolved_limit = max(1, int(limit or 4))
    cache_store = _resolve_project_continuity_session_cache_store(db_session)
    cache_key = _build_project_continuity_session_cache_key(str(project_id), resolved_limit)
    cached_ledger = cache_store.get(cache_key)
    if isinstance(cached_ledger, ProjectContinuityLedger):
        return cached_ledger

    character_result = await db_session.execute(select(Character).where(Character.project_id == project_id))
    characters = list(character_result.scalars().all())
    character_name_map = {
        character.id: _compact_project_continuity_text(character.name, limit=24)
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
        .limit(max(_MAX_RECENT_PROJECT_CONTINUITY_ANALYSES, resolved_limit * 3))
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
        character_state_ledger=_build_project_continuity_character_state_items(characters, analyses, limit=resolved_limit),
        relationship_state_ledger=_build_project_continuity_relationship_state_items(
            relationships,
            character_name_map,
            analyses,
            limit=resolved_limit,
        ),
        foreshadow_state_ledger=_build_project_continuity_foreshadow_state_items(
            foreshadow_memories,
            analyses,
            limit=resolved_limit,
        ),
        organization_state_ledger=_build_project_continuity_organization_state_items(
            organization_pairs,
            limit=resolved_limit,
        ),
        career_state_ledger=_build_project_continuity_career_state_items(
            career_rows,
            characters,
            career_map,
            limit=resolved_limit,
        ),
    )
    cache_store[cache_key] = ledger
    return ledger


async def enrich_story_packet_with_project_continuity(
    db_session: "AsyncSession",
    project: Optional["Project"],
    story_packet: StoryPacket,
) -> StoryPacket:
    project_id = getattr(project, "id", None)
    if not project_id:
        return story_packet

    blueprint = story_packet.blueprint
    missing_character_state = not blueprint.character_state_ledger
    missing_relationship_state = not blueprint.relationship_state_ledger
    missing_foreshadow_state = not blueprint.foreshadow_state_ledger
    missing_organization_state = not blueprint.organization_state_ledger
    missing_career_state = not blueprint.career_state_ledger
    if not (
        missing_character_state
        or missing_relationship_state
        or missing_foreshadow_state
        or missing_organization_state
        or missing_career_state
    ):
        return story_packet

    continuity_ledger = await build_project_continuity_ledger(db_session, project_id)
    if not continuity_ledger.has_any_entries():
        return story_packet

    return story_packet.with_blueprint(
        character_state_source=(
            {"story_character_state_ledger": continuity_ledger.character_state_ledger}
            if missing_character_state and continuity_ledger.character_state_ledger
            else None
        ),
        relationship_state_source=(
            {"story_relationship_state_ledger": continuity_ledger.relationship_state_ledger}
            if missing_relationship_state and continuity_ledger.relationship_state_ledger
            else None
        ),
        foreshadow_state_source=(
            {"story_foreshadow_state_ledger": continuity_ledger.foreshadow_state_ledger}
            if missing_foreshadow_state and continuity_ledger.foreshadow_state_ledger
            else None
        ),
        organization_state_source=(
            {"story_organization_state_ledger": continuity_ledger.organization_state_ledger}
            if missing_organization_state and continuity_ledger.organization_state_ledger
            else None
        ),
        career_state_source=(
            {"story_career_state_ledger": continuity_ledger.career_state_ledger}
            if missing_career_state and continuity_ledger.career_state_ledger
            else None
        ),
    )


async def build_story_generation_packet_with_project_continuity(
    db_session: "AsyncSession",
    project: Optional["Project"],
    source: Optional[Any] = None,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    source_label: Optional[str] = None,
) -> StoryPacket:
    from tests.test_support.story_packet_test_support import build_story_generation_packet

    packet = build_story_generation_packet(
        project,
        source=source,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        source_label=source_label,
    )
    return await enrich_story_packet_with_project_continuity(
        db_session,
        project,
        packet,
    )


__all__ = [
    "ProjectContinuityLedger",
    "ProjectContinuityLedgerItem",
    "build_project_continuity_ledger",
    "build_story_generation_packet_with_project_continuity",
    "enrich_story_packet_with_project_continuity",
]
