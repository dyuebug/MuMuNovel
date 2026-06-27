"""Chapter regeneration API routes."""

from __future__ import annotations

import asyncio
import difflib
import json
from dataclasses import dataclass
from datetime import datetime
from functools import lru_cache
from pathlib import Path
import re
from typing import TYPE_CHECKING, Any, AsyncGenerator, AsyncIterator, Awaitable, Callable, Dict, Iterable, List, Optional, Sequence

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active chapter regeneration route group; this Python "
    "module is kept only as frozen rollback/source-map material after "
    "route-shell closeout promotion."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/chapter_regeneration_routes.rs"
SOURCE_MAP_ROLLBACK_FLAG = "legacy_chapter_regeneration_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Query, Request

from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.schemas.generation_payload import build_chapter_regeneration_stream_result_payload
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)
from tests.test_support.chapter_web_research_test_support import (
    chapter_web_research_service,
)
from tests.test_support.utils.sse_response import SSEResponse, WizardProgressTracker, create_sse_response
from tests.test_support.chapter_regeneration_schema_test_support import (
    ChapterRegenerateRequest,
    PartialRegenerateRequest,
)

router = APIRouter(prefix='/chapters', tags=['章节管理'])

logger = get_logger(__name__)

if TYPE_CHECKING:
    from migrator_app.models import PlotAnalysis
    from migrator_app.models import User
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.chapter import Chapter
    from migrator_app.models.outline import Outline
    from migrator_app.models.project import Project
    from tests.test_support.ai_gateway.ai_service import AIService
else:
    AsyncSession = Any
    AIService = Any
    Chapter = Any
    PlotAnalysis = Any
    Outline = Any
    Project = Any
    User = Any


_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)

@lru_cache(maxsize=1)
def _load_regeneration_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = ("CHAPTER_REGENERATION_SYSTEM", "PARTIAL_REGENERATE")
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(f"regeneration test adapter 未找到模板常量: {template_key}")
        templates[template_key] = match.group(1)
    return templates


def _regeneration_template_lookup(template_key: str) -> Optional[str]:
    return _load_regeneration_prompt_template_map().get(template_key)


CHAPTER_REGENERATION_SYSTEM_TEMPLATE = _regeneration_template_lookup(
    "CHAPTER_REGENERATION_SYSTEM"
)
PARTIAL_REGENERATE_TEMPLATE = _regeneration_template_lookup("PARTIAL_REGENERATE")


async def _default_get_regeneration_template(
    template_key: str,
    user_id: str,
    db_session: Any,
):
    return await get_template_for_owner(
        template_key,
        user_id,
        db_session,
        template_lookup=_regeneration_template_lookup,
    )


def _default_format_regeneration_prompt(template: str, **kwargs) -> str:
    return _facade_format_prompt(template, **kwargs)


async def get_template(template_key: str, user_id: str, db_session: Any):
    return await _default_get_regeneration_template(template_key, user_id, db_session)


def format_prompt(template: str, **kwargs) -> str:
    return _default_format_regeneration_prompt(template, **kwargs)


async def _get_regeneration_template(template_key: str, user_id: str, db_session: Any):
    return await get_template(template_key, user_id, db_session)


def _format_regeneration_prompt(template: str, **kwargs) -> str:
    return format_prompt(template, **kwargs)


def require_login(request: Request) -> User:
    from tests.test_support.ai_dependencies_test_support import require_login as app_require_login

    return app_require_login(request)


async def get_db(request: Request):
    from tests.test_support.database_test_support import get_db as app_get_db

    async for session in app_get_db(request):
        yield session


async def get_user_ai_service(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db),
):
    from tests.test_support.ai_dependencies_test_support import (
        get_user_ai_service as app_get_user_ai_service,
    )

    return await app_get_user_ai_service(user=user, db=db)


def require_authenticated_user_id(request: Request) -> str:
    from tests.test_support.chapter_route_helpers_test_support import (
        require_authenticated_user_id as app_require_authenticated_user_id,
    )

    return app_require_authenticated_user_id(request)


async def load_accessible_chapter_or_404(*args, **kwargs):
    from tests.test_support.chapter_route_helpers_test_support import (
        load_accessible_chapter_or_404 as app_load_accessible_chapter_or_404,
    )

    return await app_load_accessible_chapter_or_404(*args, **kwargs)


async def build_characters_info_with_careers(*args, **kwargs):
    from sqlalchemy import or_, select

    from migrator_app.models import (
        Career,
        CharacterCareer,
        CharacterRelationship,
        Organization,
        OrganizationMember,
    )
    from migrator_app.models.character import Character

    db = args[0] if args else kwargs["db"]
    project_id = args[1] if len(args) > 1 else kwargs["project_id"]
    characters = args[2] if len(args) > 2 else kwargs["characters"]
    filter_character_names = (
        args[3] if len(args) > 3 else kwargs.get("filter_character_names")
    )

    if not characters:
        return '暂无相关角色'

    if filter_character_names:
        filtered_characters = [character for character in characters if character.name in filter_character_names]
        if not filtered_characters:
            logger.warning(f"角色过滤后未命中，回退到全部角色: {filter_character_names}")
            filtered_characters = characters
        else:
            logger.info(
                f"角色过滤命中 {len(filtered_characters)}/{len(characters)} 个角色: "
                f"{[character.name for character in filtered_characters]}"
            )
        characters = filtered_characters

    careers_result = await db.execute(
        select(Career).where(Career.project_id == project_id)
    )
    careers_map = {career.id: career for career in careers_result.scalars().all()}

    character_ids = [character.id for character in characters]
    if not character_ids:
        return '暂无相关角色'

    all_chars_result = await db.execute(
        select(Character.id, Character.name).where(Character.project_id == project_id)
    )
    all_char_name_map = {row.id: row.name for row in all_chars_result.all()}

    character_careers_result = await db.execute(
        select(CharacterCareer).where(CharacterCareer.character_id.in_(character_ids))
    )
    character_careers = character_careers_result.scalars().all()

    rels_result = await db.execute(
        select(CharacterRelationship).where(
            CharacterRelationship.project_id == project_id,
            or_(
                CharacterRelationship.character_from_id.in_(character_ids),
                CharacterRelationship.character_to_id.in_(character_ids),
            ),
        )
    )
    all_relationships = rels_result.scalars().all()

    char_rels_map: dict[str, list] = {character_id: [] for character_id in character_ids}
    for relationship in all_relationships:
        if relationship.character_from_id in char_rels_map:
            char_rels_map[relationship.character_from_id].append(relationship)
        if relationship.character_to_id in char_rels_map:
            char_rels_map[relationship.character_to_id].append(relationship)

    orgs_result = await db.execute(
        select(Organization).where(Organization.project_id == project_id)
    )
    all_orgs = orgs_result.scalars().all()

    org_name_map: dict[str, str] = {}
    char_id_to_org: dict[str, Organization] = {}
    for organization in all_orgs:
        org_name_map[organization.id] = all_char_name_map.get(organization.character_id, '未知角色')
        char_id_to_org[organization.character_id] = organization

    org_ids = [organization.id for organization in all_orgs]
    all_org_members: list[OrganizationMember] = []
    if org_ids:
        all_org_members_result = await db.execute(
            select(OrganizationMember).where(
                OrganizationMember.organization_id.in_(org_ids)
            )
        )
        all_org_members = all_org_members_result.scalars().all()

    org_members_map: dict[str, list] = {organization_id: [] for organization_id in org_ids}
    for member in all_org_members:
        if member.organization_id in org_members_map:
            org_members_map[member.organization_id].append(member)

    non_org_char_ids = [character.id for character in characters if not character.is_organization]
    char_org_map: dict[str, list] = {character_id: [] for character_id in non_org_char_ids}
    for member in all_org_members:
        if member.character_id in char_org_map:
            char_org_map[member.character_id].append(member)

    char_career_map: dict[str, dict[str, object]] = {}
    for character_career in character_careers:
        if character_career.character_id not in char_career_map:
            char_career_map[character_career.character_id] = {'main': None, 'sub': []}

        career = careers_map.get(character_career.career_id)
        if not career:
            continue

        career_info = {
            'name': career.name,
            'stage': character_career.current_stage,
            'max_stage': career.max_stage,
            'stage_progress': character_career.stage_progress,
        }

        if character_career.career_type == 'main':
            char_career_map[character_career.character_id]['main'] = career_info
        else:
            char_career_map[character_career.character_id]['sub'].append(career_info)

    characters_info_parts: list[str] = []
    for character in characters:
        entity_type = '组织' if character.is_organization else '角色'
        status_marker = ""
        char_status = getattr(character, 'status', None) or 'active'
        if char_status != 'active':
            status_markers = {
                'deceased': '已死亡',
                'missing': '失踪',
                'retired': '已退场',
                'destroyed': '已毁灭',
            }
            status_marker = f" [{status_markers.get(char_status, char_status)}]"
        base_info = f"- {character.name}({entity_type}, {character.role_type}){status_marker}"

        org_detail_str = ""
        if character.is_organization and character.id in char_id_to_org:
            organization = char_id_to_org[character.id]
            org_detail_parts: list[str] = []
            if character.organization_type:
                org_detail_parts.append(f"类型:{character.organization_type}")
            if character.organization_purpose:
                purpose_preview = (
                    character.organization_purpose[:60]
                    if len(character.organization_purpose) > 60
                    else character.organization_purpose
                )
                org_detail_parts.append(f"目的:{purpose_preview}")
            if organization.power_level is not None:
                org_detail_parts.append(f"势力:{organization.power_level}")
            if organization.location:
                org_detail_parts.append(f"地点:{organization.location}")
            if organization.motto:
                org_detail_parts.append(f"格言:{organization.motto}")
            if organization.member_count:
                org_detail_parts.append(f"成员:{organization.member_count}")
            if org_detail_parts:
                org_detail_str = f" | {', '.join(org_detail_parts)}"

            if organization.id in org_members_map and org_members_map[organization.id]:
                member_parts: list[str] = []
                for member in sorted(org_members_map[organization.id], key=lambda item: -(item.rank or 0))[:5]:
                    member_name = all_char_name_map.get(member.character_id, '未知角色')
                    member_desc = f"{member_name}({member.position})"
                    if member.status and member.status != 'active':
                        member_desc += f"[{member.status}]"
                    member_parts.append(member_desc)
                if member_parts:
                    org_detail_str += f" | 成员: {', '.join(member_parts)}"

        career_info_str = ""
        if character.id in char_career_map:
            career_data = char_career_map[character.id]
            main_career = career_data['main']
            if main_career:
                stage_desc = f"{main_career['stage']}/{main_career['max_stage']}阶"
                career_info_str += f" | 主职业: {main_career['name']}({stage_desc})"

            sub_careers = career_data['sub']
            if sub_careers:
                sub_list: list[str] = []
                for sub_career in sub_careers:
                    stage_desc = f"{sub_career['stage']}/{sub_career['max_stage']}阶"
                    sub_list.append(f"{sub_career['name']}({stage_desc})")
                career_info_str += f" | 副职业: {', '.join(sub_list)}"

        state_str = ""
        if character.current_state:
            state_preview = character.current_state[:50] if len(character.current_state) > 50 else character.current_state
            state_str = f" | 当前状态: {state_preview}"
            if character.state_updated_chapter:
                state_str += f"(第{character.state_updated_chapter}章)"

        org_str = ""
        if not character.is_organization and character.id in char_org_map and char_org_map[character.id]:
            org_parts: list[str] = []
            for member in char_org_map[character.id][:3]:
                organization_name = org_name_map.get(member.organization_id, '未知组织')
                org_desc = f"{organization_name}({member.position})"
                if member.loyalty is not None and member.loyalty != 50:
                    org_desc += f"[忠诚:{member.loyalty}]"
                if member.status and member.status != 'active':
                    org_desc += f"[{member.status}]"
                org_parts.append(org_desc)
            if org_parts:
                org_str = f" | 组织归属: {', '.join(org_parts)}"

        rel_str = ""
        if character.id in char_rels_map and char_rels_map[character.id]:
            rel_parts: list[str] = []
            seen_pairs: set[tuple[str, str]] = set()
            for relationship in char_rels_map[character.id][:5]:
                if relationship.character_from_id == character.id:
                    other_name = all_char_name_map.get(relationship.character_to_id, '未知角色')
                    other_id = relationship.character_to_id
                else:
                    other_name = all_char_name_map.get(relationship.character_from_id, '未知角色')
                    other_id = relationship.character_from_id

                pair_key = tuple(sorted([character.id, other_id]))
                if pair_key in seen_pairs:
                    continue
                seen_pairs.add(pair_key)

                rel_name = relationship.relationship_name or '未知关系'
                rel_desc = f"{other_name}({rel_name})"
                if relationship.intimacy_level is not None and relationship.intimacy_level != 50:
                    rel_desc += f"[亲密:{relationship.intimacy_level}]"
                rel_parts.append(rel_desc)

            if rel_parts:
                rel_str = f" | 关系: {', '.join(rel_parts)}"

        personality_str = ""
        if character.personality:
            personality_preview = character.personality[:100] if len(character.personality) > 100 else character.personality
            personality_str = f": {personality_preview}"

        full_info = base_info + org_detail_str + career_info_str + state_str + org_str + rel_str + personality_str
        characters_info_parts.append(full_info)
    return "\n".join(characters_info_parts)


def contains_chapter_workflow_meta_text(*args, **kwargs):
    return chapter_narrative_contains_workflow_meta_text(*args, **kwargs)


def sanitize_generated_narrative_text(*args, **kwargs):
    return chapter_narrative_sanitize_generated_text(*args, **kwargs)


_CHAPTER_WORKFLOW_META_PATTERNS = (
    r"^\s*(?:步骤|step)\s*\d+\b",
    r"^\s*执行\s*\d+(?:\.\d+)*\b",
    r"调用\s*agent",
    r"(?:流程|步骤)\s*(?:说明|日志|总结|复盘|评审)",
    r"(?:方案对比|方案评审|复盘结论|执行计划)",
    r"^\s*(?:作为|身为)\s*(?:ai|助手|模型)[：:?,，]",
)
_CHAPTER_META_PREFIXES = {
    "以下是章节正文：",
    "以下是正文：",
    "章节正文：",
    "正文：",
}

_LIGHT_TEMPLATE_SENTENCE_LEADS = (
    "下一秒",
    "那一瞬",
)

_LIGHT_TEMPLATE_SIMILE_PATTERN = re.compile(
    r"像(?P<body>[^，。！？；\n]{1,18})一样"
)


def _is_likely_chapter_meta_line(line: str) -> bool:
    stripped = (line or "").strip()
    if not stripped:
        return False
    if stripped.startswith("```"):
        return True
    if stripped in _CHAPTER_META_PREFIXES:
        return True
    return any(
        re.search(pattern, stripped, flags=re.IGNORECASE)
        for pattern in _CHAPTER_WORKFLOW_META_PATTERNS
    )


def chapter_narrative_contains_workflow_meta_text(text: str) -> bool:
    if not text:
        return False
    return any(_is_likely_chapter_meta_line(line) for line in text.splitlines())


def _lightly_polish_template_phrases(text: str) -> str:
    cleaned = text

    sentence_boundary_pattern = r"""(^|[。！？!?；;\n])([?"'\u201c\u201d\u2018\u2019(]*)"""
    sentence_lead_suffix_pattern = r"""(?:[，、,]\s*)?"""
    leading_punctuation_pattern = re.compile(
        r"""(^|[。！？!?；;\n])([?"'\u201c\u201d\u2018\u2019(]*)[，、,]\s*""",
        flags=re.MULTILINE,
    )

    for sentence_lead in _LIGHT_TEMPLATE_SENTENCE_LEADS:
        pattern = re.compile(
            sentence_boundary_pattern
            + re.escape(sentence_lead)
            + sentence_lead_suffix_pattern,
            flags=re.MULTILINE,
        )
        seen_count = 0

        def _replace_sentence_lead(match: re.Match[str]) -> str:
            nonlocal seen_count
            seen_count += 1
            if seen_count <= 1:
                return match.group(0)
            return f"{match.group(1)}{match.group(2)}"

        cleaned = pattern.sub(_replace_sentence_lead, cleaned)

    simile_seen_count = 0

    def _replace_simile(match: re.Match[str]) -> str:
        nonlocal simile_seen_count
        simile_seen_count += 1
        if simile_seen_count <= 2:
            return match.group(0)

        body = (match.group("body") or "").strip()
        if not body:
            return match.group(0)
        return f"像{body}那样"

    cleaned = _LIGHT_TEMPLATE_SIMILE_PATTERN.sub(_replace_simile, cleaned)
    cleaned = re.sub("像是有什么", "像有", cleaned)
    cleaned = re.sub("像有什么", "像有", cleaned)
    cleaned = leading_punctuation_pattern.sub(
        lambda match: f"{match.group(1)}{match.group(2)}",
        cleaned,
    )
    return cleaned


def chapter_narrative_trim_text_to_sentence_boundary(
    text: str,
    *,
    hard_limit: int,
    lookback_chars: int = 220,
) -> str:
    normalized_text = str(text or "")
    if hard_limit <= 0 or len(normalized_text) <= hard_limit:
        return normalized_text.strip()

    search_start = max(0, hard_limit - max(int(lookback_chars or 0), 80))
    best_boundary_index = -1
    for boundary_char in ("。", "！", "？", "!", "?", "；", ";", "\n"):
        boundary_index = normalized_text.rfind(
            boundary_char,
            search_start,
            hard_limit + 1,
        )
        if boundary_index > best_boundary_index:
            best_boundary_index = boundary_index

    if best_boundary_index >= search_start:
        return normalized_text[: best_boundary_index + 1].strip()

    trimmed_text = normalized_text[:hard_limit].rstrip("，,、 ")
    if trimmed_text and trimmed_text[-1] not in {"。", "！", "？", "!", "?", "；", ";"}:
        trimmed_text += "。"
    return trimmed_text.strip()


def chapter_narrative_sanitize_generated_text(text: str) -> tuple[str, int]:
    original = (text or "").replace("\r\n", "\n").strip()
    if not original:
        return "", 0

    removed_line_count = 0
    kept_lines: list[str] = []

    for raw_line in original.split("\n"):
        stripped = raw_line.strip()
        if not stripped:
            kept_lines.append("")
            continue

        if _is_likely_chapter_meta_line(stripped):
            removed_line_count += 1
            continue

        kept_lines.append(raw_line)

    cleaned = re.sub(r"\n{3,}", "\n\n", "\n".join(kept_lines)).strip()
    cleaned = _lightly_polish_template_phrases(cleaned)
    cleaned = re.sub(r"\n{3,}", "\n\n", cleaned).strip()
    return cleaned, removed_line_count


@dataclass(frozen=True)
class ChapterContentApplyResult:
    old_word_count: int
    new_word_count: int


async def apply_chapter_content_update(
    db: AsyncSession,
    *,
    chapter: Chapter,
    content: str,
    history_entry: Any = None,
    refresh_chapter: bool = True,
) -> ChapterContentApplyResult:
    old_word_count = chapter.word_count or len(chapter.content or "")
    new_word_count = len(content)

    chapter.content = content
    chapter.word_count = new_word_count

    from sqlalchemy import select

    from migrator_app.models.project import Project as ProjectModel

    project_result = await db.execute(
        select(ProjectModel).where(ProjectModel.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project:
        current_words = project.current_words or 0
        project.current_words = max(0, current_words - old_word_count + new_word_count)

    if history_entry is not None:
        db.add(history_entry)

    await db.commit()
    if refresh_chapter:
        await db.refresh(chapter)

    return ChapterContentApplyResult(
        old_word_count=old_word_count,
        new_word_count=new_word_count,
    )


def build_chapter_generation_runtime_bundle(*args, **kwargs):
    from tests.test_support.story_packet_test_support import (
        build_chapter_generation_runtime_bundle as impl,
    )

    return impl(*args, **kwargs)


async def build_story_generation_packet_with_project_continuity(*args, **kwargs):
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity as impl,
    )

    return await impl(*args, **kwargs)


async def resolve_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile as impl,
    )

    return await impl(*args, **kwargs)


def _build_outline_structure_runtime_sources(*args, **kwargs):
    outline = args[0] if args else kwargs.get("outline")
    structure_text = getattr(outline, "structure", None)
    if not isinstance(structure_text, str) or not structure_text.strip():
        return {}

    try:
        structure = json.loads(structure_text)
    except (TypeError, ValueError, json.JSONDecodeError):
        return {}

    if not isinstance(structure, dict):
        return {}

    character_focus: list[str] = []
    character_state_ledger: list[str] = []
    organization_state_ledger: list[str] = []
    for item in structure.get("characters") or []:
        if not isinstance(item, dict):
            continue
        name = str(item.get("name") or "").strip()
        item_type = str(item.get("type") or "character").strip().lower()
        if not name:
            continue
        if item_type == "organization":
            if name not in organization_state_ledger:
                organization_state_ledger.append(f"{name}: active in this chapter outline")
            continue
        if name not in character_focus:
            character_focus.append(name)
        entry = f"{name}: active in this chapter outline"
        if entry not in character_state_ledger:
            character_state_ledger.append(entry)

    runtime_sources: Dict[str, Any] = {}
    if character_focus:
        runtime_sources["character_focus"] = character_focus[:4]
    if character_state_ledger:
        runtime_sources["character_state_ledger"] = character_state_ledger[:4]
    if organization_state_ledger:
        runtime_sources["organization_state_ledger"] = organization_state_ledger[:4]
    return runtime_sources


def normalize_story_repair_payload(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        normalize_story_repair_payload as impl,
    )

    return impl(*args, **kwargs)


async def resolve_generation_story_repair_state_for_chapter(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_chapter as impl,
    )

    return await impl(*args, **kwargs)


def story_repair_payload_to_prompt_kwargs(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        story_repair_payload_to_prompt_kwargs as impl,
    )

    return impl(*args, **kwargs)


@dataclass(frozen=True)
class ChapterRegenerationPreparation:
    effective_regenerate_request: ChapterRegenerateRequest
    style_content: str
    style_id: Optional[int]
    project_context: Dict[str, Any]
    story_runtime_contract: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterRegenerationStreamContext:
    chapter: Chapter
    analysis: Optional["PlotAnalysis"]
    user_id: str
    regenerate_request: ChapterRegenerateRequest
    effective_regenerate_request: ChapterRegenerateRequest
    project_context: Dict[str, Any]
    style_content: str
    style_id: Optional[int]
    story_runtime_contract: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterRegenerationSanitizedContent:
    full_content: str
    removed_meta_lines: int


@dataclass(frozen=True)
class ChapterRegenerationCompletion:
    word_count: int
    diff_stats: Dict[str, Any]
    result_payload: Dict[str, Any]


@dataclass(frozen=True)
class ChapterRegenerationEmissionStep:
    kind: str
    payload: Optional[Dict[str, Any]] = None
    message: Optional[str] = None
    event: Optional[str] = None
    progress: Optional[float] = None


@dataclass
class ChapterRegenerationStreamingState:
    full_content: str = ""


@dataclass(frozen=True)
class PartialRegenerationPreparation:
    start_position: int
    end_position: int
    original_text: str
    original_word_count: int
    style_id: Optional[int]
    style_content: str
    prompt: str
    target_words: int
    max_tokens: int


def _build_generation_request_options(ai_service: AIService) -> Dict[str, Any]:
    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(getattr(retry_cfg, "max_retries", 2) or 2)
    provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    request_options: Dict[str, Any] = {
        "transport_max_retries": max(1, min(configured_retry_budget, 2)),
    }
    if provider in {"sub2api", "openai_responses"}:
        request_options.update(
            {
                "prefer_chat_completions": True,
                "prefer_normalized_v1_candidate": True,
                "first_chunk_timeout": 20.0,
                "allow_non_stream_fallback": False,
            }
        )
    return request_options


FOCUS_AREA_LABELS: Dict[str, str] = {
    "pacing": "节奏把控 - 调整叙事速度，避免拖沓或过快",
    "emotion": "情感渲染 - 深化人物情感表达，增强感染力",
    "description": "场景描写 - 丰富环境细节，增强画面感",
    "dialogue": "对话质量 - 让对话更自然真实，推动剧情",
    "conflict": "冲突强度 - 强化矛盾冲突，提升戏剧张力",
    "outline": "大纲贴合 - 确保当前章节命中本轮目标、变化与收束",
    "rule_grounding": "规则落地 - 把设定限制、代价与结果写进动作链",
    "opening": "开场钩子 - 开头尽快出现目标、异常或受阻",
    "payoff": "回报兑现 - 回收承诺、伏笔或阶段性爽点",
    "cliffhanger": "章尾钩子 - 章末保留更尖锐的未决问题或新失衡",
}

FOCUS_AREA_REPAIR_TARGETS: Dict[str, str] = {
    "pacing": "调整场景节拍，让推进、停顿和转折更均衡。",
    "emotion": "补强角色情绪触发与外露的连续变化。",
    "description": "用场景细节和动作反馈承载信息。",
    "dialogue": "删掉解释型对白，改成带潜台词和立场碰撞的说话方式。",
    "conflict": "补强正面冲突与升级代价，让人物付出真实后果。",
    "outline": "回扣本轮大纲关键任务、变化与收束。",
    "rule_grounding": "把规则限制、风险代价和结果反馈落到动作链里。",
    "opening": "把开头改成更快入场的异常 / 目标 / 受阻起手。",
    "payoff": "回收前文承诺、伏笔或阶段性期待。",
    "cliffhanger": "章尾保留未决选择、新失衡或更高一级的问题。",
}

AUTO_FOCUS_KEYWORDS: Dict[str, tuple[str, ...]] = {
    "pacing": ("节奏", "拖沓", "过快", "冗长", "跳切"),
    "emotion": ("情感", "情绪", "感染力", "共鸣"),
    "description": ("描写", "场景", "画面", "环境细节"),
    "dialogue": ("对白", "对话", "台词", "说话"),
    "conflict": ("冲突", "矛盾", "对抗", "张力", "代价"),
    "outline": ("大纲", "主线", "偏离", "跑题", "结构"),
    "rule_grounding": ("规则", "设定", "世界观", "逻辑", "约束"),
    "opening": ("开头", "开场", "起手", "前300字"),
    "payoff": ("回收", "回报", "兑现", "爽点", "伏笔回收"),
    "cliffhanger": ("章尾", "结尾", "悬念", "收束", "牵引"),
}

_PARTIAL_REGENERATE_PREFIXES_TO_REMOVE = [
    "重写后：",
    "重写后:",
    "改写后：",
    "改写后:",
    "以下是重写后的内容：",
    "以下是重写后的内容:",
    "重写内容：",
    "重写内容:",
]


def _normalize_focus_areas(areas: Iterable[str]) -> List[str]:
    normalized: List[str] = []
    seen: set[str] = set()
    for area in areas:
        value = str(area or "").strip().lower()
        if not value or value not in FOCUS_AREA_LABELS or value in seen:
            continue
        seen.add(value)
        normalized.append(value)
    return normalized


def _infer_focus_areas_from_texts(texts: Iterable[str]) -> List[str]:
    combined = "\n".join(str(text or "").strip().lower() for text in texts if str(text or "").strip())
    if not combined:
        return []

    inferred: List[str] = []
    for area, keywords in AUTO_FOCUS_KEYWORDS.items():
        if any(keyword in combined for keyword in keywords):
            inferred.append(area)
    return _normalize_focus_areas(inferred)


class ChapterRegenerator:
    """章节重新生成服务。"""

    def __init__(self, ai_service: AIService):
        self.ai_service = ai_service
        logger.info("✅ ChapterRegenerator初始化成功")

    async def regenerate_with_feedback(
        self,
        chapter: Chapter,
        analysis: Optional["PlotAnalysis"],
        regenerate_request: ChapterRegenerateRequest,
        project_context: Dict[str, Any],
        style_content: str = "",
        user_id: Optional[str] = None,
        db: Optional[AsyncSession] = None,
    ) -> AsyncGenerator[Dict[str, Any], None]:
        """根据反馈重新生成章节（流式）。"""
        try:
            logger.info(f"🔄 开始重新生成章节: 第{chapter.chapter_number}章")

            yield {"type": "progress", "progress": 5, "message": "正在构建修改指令..."}
            modification_instructions = self._build_modification_instructions(
                analysis=analysis,
                regenerate_request=regenerate_request,
            )
            logger.info(f"📝 修改指令构建完成，长度: {len(modification_instructions)}字符")

            yield {"type": "progress", "progress": 10, "message": "正在构建生成提示词..."}
            full_prompt = await self._build_regeneration_prompt(
                chapter=chapter,
                modification_instructions=modification_instructions,
                project_context=project_context,
                regenerate_request=regenerate_request,
                style_content=style_content,
                user_id=user_id,
                db=db,
            )

            logger.info("🎯 提示词构建完成，开始AI生成")
            yield {"type": "progress", "progress": 15, "message": "开始AI生成内容..."}

            system_prompt_with_style = None
            if style_content:
                extra_guard = ""
                if "连载感" in style_content:
                    extra_guard = (
                        "\n连载优化要求：\n"
                        "- 情绪推进至少经历“触发→压住/回避→外露”中的两个阶段\n"
                        "- 对话要区分角色声线，避免全员同一种书面表达\n"
                        "- 章末保留自然压力点，不要硬反转"
                    )
                elif "生活化" in style_content:
                    extra_guard = (
                        "\n生活化优化要求：\n"
                        "- 用动作和反应传达情绪，少写抽象总结\n"
                        "- 允许少量口语毛边，避免句句工整"
                    )

                system_prompt_with_style = f"""【🎨 写作风格参考】

{style_content}

请优先贴合上述写作风格进行重写。
整体语气尽量保持一致，自然表达，不要写成模板腔。{extra_guard}"""
                logger.info(f"✅ 已将写作风格注入系统提示词（{len(style_content)}字符）")

            target_word_count = regenerate_request.target_word_count
            accumulated_length = 0
            request_options = _build_generation_request_options(self.ai_service)

            async for chunk in self.ai_service.generate_text_stream(
                prompt=full_prompt,
                system_prompt=system_prompt_with_style,
                temperature=0.7,
                request_options=request_options,
            ):
                yield {"type": "chunk", "content": chunk}

                accumulated_length += len(chunk)
                generation_progress = min(15 + (accumulated_length / target_word_count) * 80, 95)
                yield {
                    "type": "progress",
                    "progress": int(generation_progress),
                    "word_count": accumulated_length,
                }

            logger.info(f"✅ 章节重新生成完成，共生成 {accumulated_length} 字")
            yield {"type": "progress", "progress": 100, "message": "生成完成"}

        except Exception as exc:
            logger.error(f"❌ 重新生成失败: {str(exc)}", exc_info=True)
            raise

    def _build_modification_instructions(
        self,
        analysis: Optional["PlotAnalysis"],
        regenerate_request: ChapterRegenerateRequest,
    ) -> str:
        instructions: List[str] = []
        instructions.append("# 修改要求\n")

        selected_suggestions: List[str] = []
        if analysis and regenerate_request.selected_suggestion_indices and analysis.suggestions:
            for idx in regenerate_request.selected_suggestion_indices:
                if 0 <= idx < len(analysis.suggestions):
                    selected_suggestions.append(str(analysis.suggestions[idx]).strip())

        custom_instructions = str(regenerate_request.custom_instructions or "").strip()
        explicit_focus_areas = _normalize_focus_areas(regenerate_request.focus_areas)
        inferred_focus_areas = _infer_focus_areas_from_texts([
            *selected_suggestions,
            custom_instructions,
        ])

        if analysis:
            if getattr(analysis, "conflict_level", None) is not None and (analysis.conflict_level or 0) < 6:
                inferred_focus_areas.append("conflict")
            if getattr(analysis, "pacing_score", None) is not None and (analysis.pacing_score or 0) < 6.5:
                inferred_focus_areas.append("pacing")
            if getattr(analysis, "coherence_score", None) is not None and (analysis.coherence_score or 0) < 6.5:
                inferred_focus_areas.append("outline")
            if getattr(analysis, "dialogue_ratio", None) is not None and (analysis.dialogue_ratio or 0) < 0.18:
                inferred_focus_areas.append("dialogue")

        effective_focus_areas = _normalize_focus_areas([*explicit_focus_areas, *inferred_focus_areas])
        auto_added_focus_areas = [area for area in effective_focus_areas if area not in explicit_focus_areas]

        repair_payload = normalize_story_repair_payload(
            getattr(regenerate_request, "story_repair_summary", None),
            getattr(regenerate_request, "story_repair_targets", []) or [],
            getattr(regenerate_request, "story_preserve_strengths", []) or [],
        )
        repair_summary = repair_payload.summary if repair_payload is not None else ""
        repair_targets = list(repair_payload.targets) if repair_payload is not None else []
        preserve_strengths = list(repair_payload.strengths) if repair_payload is not None else []

        if not repair_targets and auto_added_focus_areas:
            repair_targets = [
                FOCUS_AREA_REPAIR_TARGETS[area]
                for area in auto_added_focus_areas
                if area in FOCUS_AREA_REPAIR_TARGETS
            ][:3]

        if not repair_summary and repair_targets:
            repair_summary = f"本轮优先修复：{repair_targets[0]}，不要只做表面润色。"

        if selected_suggestions:
            instructions.append("## 选中的 AI 建议\n")
            for idx, suggestion in enumerate(selected_suggestions, start=1):
                instructions.append(f"{idx}. {suggestion}")
            instructions.append("")

        if custom_instructions:
            instructions.append("## 额外修改要求\n")
            instructions.append(custom_instructions)
            instructions.append("")

        if repair_summary or repair_targets or preserve_strengths:
            instructions.append("## 🩺 剧情质量修复目标：\n")
            if repair_summary:
                instructions.append(f"- 修复摘要：{repair_summary}")
            if repair_targets:
                instructions.append("- 本轮优先修复：")
                for target in repair_targets:
                    instructions.append(f"  * {target}")
            if preserve_strengths:
                instructions.append("- 需要保留的优势：")
                for strength in preserve_strengths:
                    instructions.append(f"  * {strength}")
            instructions.append("")

        if effective_focus_areas:
            section_title = "## 🎯 重点优化方向（含自动补充）：\n" if auto_added_focus_areas else "## 🎯 重点优化方向：\n"
            instructions.append(section_title)
            for area in effective_focus_areas:
                focus_label = FOCUS_AREA_LABELS.get(area)
                if focus_label:
                    instructions.append(f"- {focus_label}")
            instructions.append("")

        if regenerate_request.preserve_elements:
            preserve = regenerate_request.preserve_elements
            instructions.append("## 需要保留的内容\n")

            if preserve.preserve_structure:
                instructions.append("- 保留原有段落结构和主要事件顺序")

            if preserve.preserve_dialogues:
                instructions.append("- 保留以下对白：")
                for dialogue in preserve.preserve_dialogues:
                    instructions.append(f"  * {dialogue}")

            if preserve.preserve_plot_points:
                instructions.append("- 保留以下关键情节点：")
                for plot in preserve.preserve_plot_points:
                    instructions.append(f"  * {plot}")

            if preserve.preserve_character_traits:
                instructions.append("- 保留人物既有性格、语气与核心关系定位")

            instructions.append("")

        return "\n".join(instructions)

    async def _build_regeneration_prompt(
        self,
        chapter: Chapter,
        modification_instructions: str,
        project_context: Dict[str, Any],
        regenerate_request: ChapterRegenerateRequest,
        style_content: str = "",
        user_id: Optional[str] = None,
        db: Optional[AsyncSession] = None,
    ) -> str:
        return await build_chapter_regeneration_prompt(
            chapter_number=chapter.chapter_number,
            title=chapter.title,
            word_count=chapter.word_count,
            content=chapter.content,
            modification_instructions=modification_instructions,
            project_context=project_context,
            style_content=style_content,
            target_word_count=regenerate_request.target_word_count,
            user_id=user_id,
            db=db,
        )

    def calculate_content_diff(
        self,
        original_content: str,
        new_content: str,
    ) -> Dict[str, Any]:
        diff_stats = {
            "original_length": len(original_content),
            "new_length": len(new_content),
            "length_change": len(new_content) - len(original_content),
            "length_change_percent": round((len(new_content) - len(original_content)) / len(original_content) * 100, 2) if len(original_content) > 0 else 0,
        }

        similarity = difflib.SequenceMatcher(None, original_content, new_content).ratio()
        diff_stats["similarity"] = round(similarity * 100, 2)
        diff_stats["difference"] = round((1 - similarity) * 100, 2)

        original_paragraphs = [p for p in original_content.split("\n\n") if p.strip()]
        new_paragraphs = [p for p in new_content.split("\n\n") if p.strip()]
        diff_stats["original_paragraph_count"] = len(original_paragraphs)
        diff_stats["new_paragraph_count"] = len(new_paragraphs)

        return diff_stats


# 保持 route 模块上的 patch surface 稳定，测试直接 monkeypatch 这里。
REGENERATOR_FACTORY = ChapterRegenerator


async def create_regeneration_task(
    db_session: AsyncSession,
    *,
    chapter,
    analysis,
    user_id: str,
    regenerate_request: ChapterRegenerateRequest,
    style_id: Optional[int],
):
    from migrator_app.models.regeneration_task import RegenerationTask

    regeneration_task = RegenerationTask(
        chapter_id=chapter.id,
        analysis_id=analysis.id if analysis else None,
        user_id=user_id,
        project_id=chapter.project_id,
        modification_instructions="",
        original_suggestions=analysis.suggestions if analysis else None,
        selected_suggestion_indices=regenerate_request.selected_suggestion_indices,
        custom_instructions=regenerate_request.custom_instructions,
        style_id=style_id,
        target_word_count=regenerate_request.target_word_count,
        focus_areas=regenerate_request.focus_areas,
        preserve_elements=(
            regenerate_request.preserve_elements.model_dump()
            if regenerate_request.preserve_elements
            else None
        ),
        status="running",
        original_content=chapter.content,
        original_word_count=chapter.word_count or len(chapter.content or ""),
        version_note=regenerate_request.version_note,
        started_at=datetime.now(),
    )
    db_session.add(regeneration_task)
    await db_session.commit()
    await db_session.refresh(regeneration_task)
    return regeneration_task


async def mark_latest_regeneration_task_failed(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    error_message: str,
):
    from sqlalchemy import select

    from migrator_app.models.regeneration_task import RegenerationTask

    task_result = await db_session.execute(
        select(RegenerationTask)
        .where(RegenerationTask.chapter_id == chapter_id)
        .order_by(RegenerationTask.created_at.desc())
        .limit(1)
    )
    regeneration_task = task_result.scalar_one_or_none()
    if regeneration_task is None:
        return None

    regeneration_task.status = "failed"
    regeneration_task.error_message = str(error_message)[:500]
    regeneration_task.completed_at = datetime.now()
    await db_session.commit()
    return regeneration_task


async def load_regeneration_tasks_payload(
    *,
    db_session: AsyncSession,
    chapter_id: str,
    limit: int,
) -> Dict[str, object]:
    from sqlalchemy import select

    from migrator_app.models.regeneration_task import RegenerationTask

    result = await db_session.execute(
        select(RegenerationTask)
        .where(RegenerationTask.chapter_id == chapter_id)
        .order_by(RegenerationTask.created_at.desc())
        .limit(limit)
    )
    tasks = result.scalars().all()
    return {
        "chapter_id": chapter_id,
        "total": len(tasks),
        "tasks": [
            {
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": task.created_at.isoformat() if task.created_at else None,
                "completed_at": task.completed_at.isoformat() if task.completed_at else None,
            }
            for task in tasks
        ],
    }


def resolve_chapter_regeneration_estimated_total(
    context: ChapterRegenerationStreamContext,
) -> int:
    return int(
        context.effective_regenerate_request.target_word_count
        or context.regenerate_request.target_word_count
        or len(context.chapter.content or "")
    )


def sanitize_chapter_regeneration_content(
    full_content: str,
    *,
    chapter_id: str,
    sanitize_generated_text: Callable[[str], tuple[str, int]],
    contains_workflow_meta_text: Callable[[str], bool],
) -> ChapterRegenerationSanitizedContent:
    sanitized_content, removed_meta_lines = sanitize_generated_text(full_content)
    if removed_meta_lines > 0:
        logger.warning(
            f"章节重写检测到流程化元文本，已清理 {removed_meta_lines} 行: chapter_id={chapter_id}"
        )
    if not sanitized_content.strip():
        raise ValueError("重写结果为空或仅包含流程化元文本")
    if contains_workflow_meta_text(sanitized_content):
        raise ValueError("重写结果包含流程化元文本")
    return ChapterRegenerationSanitizedContent(
        full_content=sanitized_content,
        removed_meta_lines=removed_meta_lines,
    )


def finalize_chapter_regeneration_completion(
    *,
    regeneration_task: Any,
    original_content: Optional[str],
    regenerated_content: str,
    regenerator: Any,
    regenerate_request: ChapterRegenerateRequest,
    story_runtime_contract: Optional[Dict[str, Any]],
    build_result_payload_fn: Callable[..., Dict[str, Any]] = build_chapter_regeneration_stream_result_payload,
) -> ChapterRegenerationCompletion:
    regeneration_task.status = "completed"
    regeneration_task.regenerated_content = regenerated_content
    regeneration_task.regenerated_word_count = len(regenerated_content)
    regeneration_task.completed_at = datetime.now()

    diff_stats = regenerator.calculate_content_diff(
        original_content,
        regenerated_content,
    )
    result_payload = build_result_payload_fn(
        task_id=regeneration_task.id,
        word_count=len(regenerated_content),
        version_number=regeneration_task.version_number,
        auto_applied=regenerate_request.auto_apply,
        diff_stats=diff_stats,
        story_runtime_contract=story_runtime_contract,
    )
    return ChapterRegenerationCompletion(
        word_count=len(regenerated_content),
        diff_stats=diff_stats if isinstance(diff_stats, dict) else {},
        result_payload=result_payload,
    )


def build_chapter_regeneration_emission_plan(
    *,
    result_payload: Dict[str, Any],
) -> List[ChapterRegenerationEmissionStep]:
    return [
        ChapterRegenerationEmissionStep(kind="tracker_saving", message="正在保存", progress=0.9),
        ChapterRegenerationEmissionStep(kind="tracker_complete", message="章节重写完成"),
        ChapterRegenerationEmissionStep(kind="tracker_result", payload=result_payload),
        ChapterRegenerationEmissionStep(kind="tracker_done"),
    ]


async def emit_chapter_regeneration_plan(
    *,
    emission_plan: Sequence[ChapterRegenerationEmissionStep],
    tracker_saving_fn: Callable[[str, float], Awaitable[Any]],
    tracker_complete_fn: Callable[[str], Awaitable[Any]],
    tracker_result_fn: Callable[[Dict[str, Any]], Awaitable[Any]],
    tracker_done_fn: Callable[[], Awaitable[Any]],
) -> AsyncIterator[Any]:
    for emission_step in emission_plan:
        if emission_step.kind == "tracker_saving":
            yield await tracker_saving_fn(emission_step.message or "", float(emission_step.progress or 0))
        elif emission_step.kind == "tracker_complete":
            yield await tracker_complete_fn(emission_step.message or "")
        elif emission_step.kind == "tracker_result":
            yield await tracker_result_fn(emission_step.payload or {})
        elif emission_step.kind == "tracker_done":
            yield await tracker_done_fn()


async def handle_chapter_regeneration_failure(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    error_message: str,
    mark_failed_fn: Callable[..., Awaitable[Any]] = mark_latest_regeneration_task_failed,
) -> None:
    await mark_failed_fn(
        db_session,
        chapter_id=chapter_id,
        error_message=error_message,
    )


async def stream_chapter_regeneration_feedback(
    *,
    regenerator: Any,
    context: ChapterRegenerationStreamContext,
    db_session: AsyncSession,
    estimated_total: int,
    streaming_state: ChapterRegenerationStreamingState,
    tracker_generating_chunk_fn: Callable[[str], Awaitable[Any]],
    tracker_preparing_fn: Callable[[str], Awaitable[Any]],
    tracker_generating_fn: Callable[..., Awaitable[Any]],
    tracker_parsing_fn: Callable[[str], Awaitable[Any]],
) -> AsyncIterator[Any]:
    async for event in regenerator.regenerate_with_feedback(
        chapter=context.chapter,
        analysis=context.analysis,
        regenerate_request=context.effective_regenerate_request,
        project_context=context.project_context,
        style_content=context.style_content,
        user_id=context.user_id,
        db=db_session,
    ):
        if event["type"] == "chunk":
            chunk = str(event.get("content") or "")
            streaming_state.full_content += chunk
            yield await tracker_generating_chunk_fn(chunk)

            if streaming_state.full_content and len(streaming_state.full_content) % 500 == 0:
                yield await tracker_generating_fn(
                    current_chars=len(streaming_state.full_content),
                    estimated_total=estimated_total,
                    message=f"正在重写中... 已生成 {len(streaming_state.full_content)} 字",
                )
        elif event["type"] == "progress":
            progress = float(event.get("progress") or 0)
            message = str(event.get("message") or "")
            if progress < 20:
                yield await tracker_preparing_fn(message)
            elif progress < 85:
                yield await tracker_generating_fn(
                    current_chars=len(streaming_state.full_content),
                    estimated_total=estimated_total,
                    message=message,
                )
            else:
                yield await tracker_parsing_fn(message)

        await asyncio.sleep(0)


async def build_chapter_regeneration_event_stream(
    *,
    db_session_source: Callable[[], AsyncGenerator[AsyncSession, None]],
    context: ChapterRegenerationStreamContext,
    user_ai_service: AIService,
    regenerator_factory: Callable[[AIService], Any],
    sanitize_generated_text: Callable[[str], tuple[str, int]],
    contains_workflow_meta_text: Callable[[str], bool],
) -> AsyncGenerator[str, None]:
    tracker = WizardProgressTracker("章节重写")
    yield await tracker.start()

    async for db_session in db_session_source():
        db_committed = False
        try:
            yield await tracker.loading("正在准备重写上下文...", 0.5)

            regeneration_task = await create_regeneration_task(
                db_session,
                chapter=context.chapter,
                analysis=context.analysis,
                user_id=context.user_id,
                regenerate_request=context.regenerate_request,
                style_id=context.style_id,
            )
            task_id = regeneration_task.id
            logger.info(f"已创建章节重写任务: {task_id}")

            yield await tracker.preparing("正在生成重写提示词...")
            yield await SSEResponse.send_event(
                event="task_created",
                data={"task_id": task_id},
            )

            regenerator = regenerator_factory(user_ai_service)
            streaming_state = ChapterRegenerationStreamingState()
            estimated_total = resolve_chapter_regeneration_estimated_total(context)

            yield await tracker.generating(
                current_chars=0,
                estimated_total=estimated_total,
            )

            async for streamed_payload in stream_chapter_regeneration_feedback(
                regenerator=regenerator,
                context=context,
                db_session=db_session,
                estimated_total=estimated_total,
                streaming_state=streaming_state,
                tracker_generating_chunk_fn=tracker.generating_chunk,
                tracker_preparing_fn=tracker.preparing,
                tracker_generating_fn=tracker.generating,
                tracker_parsing_fn=tracker.parsing,
            ):
                yield streamed_payload

            yield await tracker.saving("正在保存重写结果...", 0.5)

            full_content = streaming_state.full_content
            sanitized_content = sanitize_chapter_regeneration_content(
                full_content,
                chapter_id=context.chapter.id,
                sanitize_generated_text=sanitize_generated_text,
                contains_workflow_meta_text=contains_workflow_meta_text,
            )
            full_content = sanitized_content.full_content

            completion = finalize_chapter_regeneration_completion(
                regeneration_task=regeneration_task,
                original_content=context.chapter.content,
                regenerated_content=full_content,
                regenerator=regenerator,
                regenerate_request=context.regenerate_request,
                story_runtime_contract=context.story_runtime_contract,
            )

            await db_session.commit()
            db_committed = True

            emission_plan = build_chapter_regeneration_emission_plan(
                result_payload=completion.result_payload,
            )
            async for emitted_payload in emit_chapter_regeneration_plan(
                emission_plan=emission_plan,
                tracker_saving_fn=tracker.saving,
                tracker_complete_fn=tracker.complete,
                tracker_result_fn=tracker.result,
                tracker_done_fn=tracker.done,
            ):
                yield emitted_payload

            logger.info(f"章节重写完成: {context.chapter.id}, 任务: {task_id}")
        except Exception as exc:
            logger.error(f"章节重写失败: {str(exc)}", exc_info=True)

            if not db_committed:
                try:
                    await handle_chapter_regeneration_failure(
                        db_session,
                        chapter_id=context.chapter.id,
                        error_message=str(exc),
                    )
                except Exception as update_error:
                    logger.error(f"更新章节重写任务状态失败: {str(update_error)}")

            yield await tracker.error(str(exc))
        break


async def _load_regeneration_outline(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
) -> Optional[Outline]:
    from sqlalchemy import select

    from migrator_app.models.outline import Outline

    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline).where(Outline.id == chapter.outline_id)
        )
        return outline_result.scalar_one_or_none()

    outline_result = await db_session.execute(
        select(Outline)
        .where(Outline.project_id == chapter.project_id)
        .where(Outline.order_index == chapter.chapter_number)
    )
    return outline_result.scalar_one_or_none()


def _resolve_regeneration_filter_character_names(
    *,
    chapter: Chapter,
    outline_mode: str,
    outline: Optional[Outline],
) -> Optional[list[str]]:
    if outline_mode == "one-to-one":
        structure_text = getattr(outline, "structure", None)
        if not structure_text:
            return None
        try:
            structure = json.loads(structure_text)
        except json.JSONDecodeError:
            return None
        if not isinstance(structure, dict):
            return None
        filter_character_names = structure.get("characters", [])
        if filter_character_names:
            return filter_character_names
        return None

    if not chapter.expansion_plan:
        return None

    try:
        plan = json.loads(chapter.expansion_plan)
    except json.JSONDecodeError:
        return None
    if not isinstance(plan, dict):
        return None
    filter_character_names = plan.get("character_focus", [])
    if filter_character_names:
        return filter_character_names
    return None


async def prepare_chapter_regeneration_context(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    regenerate_request: ChapterRegenerateRequest,
    user_id: str,
) -> ChapterRegenerationPreparation:
    from sqlalchemy import select

    from migrator_app.models.character import Character
    from migrator_app.models.project import Project

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise ValueError(f"Project not found for chapter regeneration: {chapter.project_id}")

    outline_mode = project.outline_mode or "one-to-many"
    outline = await _load_regeneration_outline(db_session, chapter=chapter)

    filter_character_names = _resolve_regeneration_filter_character_names(
        chapter=chapter,
        outline_mode=outline_mode,
        outline=outline,
    )

    characters_result = await db_session.execute(
        select(Character).where(Character.project_id == chapter.project_id)
    )
    characters = characters_result.scalars().all()
    characters_info_with_careers = await build_characters_info_with_careers(
        db_session,
        chapter.project_id,
        characters,
        filter_character_names,
    )

    quality_profile = await resolve_chapter_quality_profile(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=regenerate_request.style_id,
        enable_mcp=True,
        prefer_project_default_style=not bool(regenerate_request.style_id),
        log_prefix="章节重写",
    )
    story_repair_state = await resolve_generation_story_repair_state_for_chapter(
        db_session,
        chapter=chapter,
        story_repair_summary=getattr(regenerate_request, "story_repair_summary", None),
        story_repair_targets=getattr(regenerate_request, "story_repair_targets", None),
        story_preserve_strengths=getattr(regenerate_request, "story_preserve_strengths", None),
    )
    story_repair_payload = story_repair_state.get("payload")
    effective_regenerate_request = regenerate_request.model_copy(
        update=story_repair_payload_to_prompt_kwargs(story_repair_payload),
        deep=True,
    )
    regeneration_story_packet = await build_story_generation_packet_with_project_continuity(
        db_session,
        project,
        source=effective_regenerate_request,
        source_label="chapter-regenerate-request",
    )
    web_research_bundle = await chapter_web_research_service.collect_for_chapter(
        user_id=user_id,
        db_session=db_session,
        project=project,
        chapter=chapter,
        outline=outline,
        story_creation_brief=effective_regenerate_request.story_creation_brief,
        enable_web_research=effective_regenerate_request.enable_web_research,
        web_research_query=effective_regenerate_request.web_research_query,
    )
    web_research_assets = list(web_research_bundle.get("assets") or [])

    outline_runtime_sources = _build_outline_structure_runtime_sources(outline)
    generation_runtime = build_chapter_generation_runtime_bundle(
        story_packet=regeneration_story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=effective_regenerate_request.target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=story_repair_state.get("active_story_repair_payload"),
        character_focus_source=outline_runtime_sources or None,
        character_state_source=(
            {**outline_runtime_sources, "chapter_characters": characters_info_with_careers}
            if outline_runtime_sources
            else characters_info_with_careers
        ),
        relationship_state_source=characters_info_with_careers,
        foreshadow_state_source=outline.content if outline else chapter.summary,
        organization_state_source=outline_runtime_sources or None,
    )

    style_content = quality_profile.get("style_content") or ""
    style_id = quality_profile.get("resolved_style_id")

    project_context = {
        "project_title": project.title if project else "未命名项目",
        "genre": project.genre if project else "未提供",
        "theme": project.theme if project else "未提供",
        "narrative_perspective": project.narrative_perspective if project else "第三人称",
        "time_period": project.world_time_period if project else "未提供",
        "location": project.world_location if project else "未提供",
        "atmosphere": project.world_atmosphere if project else "未提供",
        "characters_info": characters_info_with_careers,
        "chapter_outline": outline.content if outline else chapter.summary or "暂无大纲",
        "previous_context": "",
        "external_assets": web_research_assets,
        "reference_assets": web_research_assets,
        "prompt_quality_kwargs": generation_runtime.prompt_quality_kwargs,
    }

    return ChapterRegenerationPreparation(
        effective_regenerate_request=effective_regenerate_request,
        style_content=style_content,
        style_id=style_id,
        project_context=project_context,
        story_runtime_contract=generation_runtime.story_runtime_contract,
    )


async def prepare_chapter_regeneration_stream_context(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    regenerate_request: ChapterRegenerateRequest,
    user_id: str,
) -> ChapterRegenerationStreamContext:
    from sqlalchemy import select

    from migrator_app.models import PlotAnalysis

    if not chapter.content or not chapter.content.strip():
        raise ValueError("当前章节缺少可重写的原始内容")

    analysis = None
    if regenerate_request.modification_source in {"analysis_suggestions", "mixed"}:
        analysis_result = await db_session.execute(
            select(PlotAnalysis)
            .where(PlotAnalysis.chapter_id == chapter.id)
            .order_by(PlotAnalysis.created_at.desc())
            .limit(1)
        )
        analysis = analysis_result.scalar_one_or_none()
        if analysis is None:
            raise LookupError("未找到对应的章节分析")

    preparation = await prepare_chapter_regeneration_context(
        db_session,
        chapter=chapter,
        regenerate_request=regenerate_request,
        user_id=user_id,
    )
    return ChapterRegenerationStreamContext(
        chapter=chapter,
        analysis=analysis,
        user_id=user_id,
        regenerate_request=regenerate_request,
        effective_regenerate_request=preparation.effective_regenerate_request,
        project_context=preparation.project_context,
        style_content=preparation.style_content,
        style_id=preparation.style_id,
        story_runtime_contract=preparation.story_runtime_contract,
    )


def _build_partial_web_research_grounding_block(assets: list[dict]) -> str:
    newline = "\n"
    lines: list[str] = []
    for index, asset in enumerate(assets or [], start=1):
        title = str(asset.get("title") or asset.get("source") or f"Reference {index}").strip()
        summary = str(
            asset.get("summary")
            or asset.get("snippet")
            or asset.get("text")
            or asset.get("raw_content")
            or ""
        ).strip()
        usage_hint = str(asset.get("usage_hint") or "").strip()
        url = str(asset.get("url") or "").strip()
        item_lines = [f"{index}. {title}"]
        if summary:
            item_lines.append(f"   - Summary: {summary}")
        if usage_hint:
            item_lines.append(f"   - Usage: {usage_hint}")
        if url:
            item_lines.append(f"   - Link: {url}")
        lines.append(newline.join(item_lines))
    if not lines:
        return ""
    return (
        f"{newline}{newline}[Web Research References]{newline}"
        "Use the following references to improve factual texture and scene grounding, but integrate them naturally:\n"
        + newline.join(lines)
    )


async def _load_partial_regeneration_project_bundle(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
) -> tuple[Project, Optional[Outline]]:
    from sqlalchemy import select

    from migrator_app.models.outline import Outline
    from migrator_app.models.project import Project

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise HTTPException(status_code=404, detail="章节不存在")

    outline = None
    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline).where(Outline.id == chapter.outline_id)
        )
        outline = outline_result.scalar_one_or_none()
    else:
        outline_result = await db_session.execute(
            select(Outline)
            .where(Outline.project_id == chapter.project_id)
            .where(Outline.order_index == chapter.chapter_number)
        )
        outline = outline_result.scalar_one_or_none()
    return project, outline


def _normalize_partial_selection(
    *,
    chapter_content: str,
    partial_request,
) -> tuple[int, int, str]:
    content_length = len(chapter_content)
    start_position = partial_request.start_position
    end_position = partial_request.end_position

    if start_position >= content_length:
        raise HTTPException(status_code=400, detail="请先选中需要重写的内容")
    if end_position > content_length:
        raise HTTPException(status_code=400, detail="请提供有效的选中文本")
    if start_position >= end_position:
        raise HTTPException(status_code=400, detail="选中文本与原文不匹配，请重试")

    actual_selected = chapter_content[start_position:end_position]
    selected_text = partial_request.selected_text
    if actual_selected == selected_text:
        return start_position, end_position, selected_text

    search_start = max(0, start_position - 50)
    search_end = min(content_length, end_position + 50)
    search_area = chapter_content[search_start:search_end]
    if selected_text not in search_area:
        raise HTTPException(
            status_code=400,
            detail="未找到对应章节的大纲上下文，无法执行局部重写",
        )

    offset = search_area.find(selected_text)
    corrected_start = search_start + offset
    corrected_end = corrected_start + len(selected_text)
    logger.info(f"局部重写选区已校正: {corrected_start}-{corrected_end}")
    return corrected_start, corrected_end, selected_text


async def _resolve_partial_style_content(
    db_session: AsyncSession,
    *,
    project_id: str,
    requested_style_id: Optional[int],
    user_id: str,
) -> tuple[Optional[int], str]:
    from sqlalchemy import select

    from migrator_app.models import ProjectDefaultStyle, WritingStyle
    from tests.test_support.chapter_prompt_quality_test_support import sync_low_ai_presets

    await sync_low_ai_presets(db_session)

    style_id = requested_style_id
    if not style_id:
        default_style_result = await db_session.execute(
            select(ProjectDefaultStyle.style_id)
            .where(ProjectDefaultStyle.project_id == project_id)
        )
        default_style_id = default_style_result.scalar_one_or_none()
        if default_style_id:
            style_id = default_style_id
            logger.info(f"局部重写 - 使用项目默认风格ID: {style_id}")

    if not style_id:
        return None, ""

    style_result = await db_session.execute(
        select(WritingStyle).where(WritingStyle.id == style_id)
    )
    style = style_result.scalar_one_or_none()
    if style is None:
        return style_id, ""
    if style.user_id is not None and style.user_id != user_id:
        logger.warning(f"风格 {style_id} 不属于当前用户，已忽略")
        return style_id, ""

    style_content = style.prompt_content or ""
    style_type = "系统风格" if style.user_id is None else "用户风格"
    logger.info(f"局部重写 - 使用风格: {style.name} ({style_type})")
    return style_id, style_content


def _build_partial_length_requirement(
    *,
    length_mode: Optional[str],
    target_word_count: Optional[int],
    original_word_count: int,
) -> str:
    if length_mode == "similar":
        min_words = int(original_word_count * 0.8)
        max_words = int(original_word_count * 1.2)
        return f"尽量保持与原文接近，原文约 {original_word_count} 字，目标 {min_words}-{max_words} 字"
    if length_mode == "expand":
        min_words = int(original_word_count * 1.2)
        max_words = int(original_word_count * 2.0)
        return f"建议扩写至 {min_words}-{max_words} 字"
    if length_mode == "condense":
        min_words = int(original_word_count * 0.5)
        max_words = int(original_word_count * 0.8)
        return f"建议压缩至 {min_words}-{max_words} 字"
    if length_mode == "custom" and target_word_count:
        return f"目标长度约 {target_word_count} 字，允许上下浮动 20%"
    return f"默认按接近原文长度处理，原文约 {original_word_count} 字"


def _calculate_partial_target_words(
    *,
    length_mode: Optional[str],
    target_word_count: Optional[int],
    original_word_count: int,
) -> int:
    if length_mode == "expand":
        return int(original_word_count * 2.0)
    if length_mode == "custom" and target_word_count:
        return target_word_count
    return int(original_word_count * 1.5)


async def prepare_partial_regeneration(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    partial_request: PartialRegenerateRequest,
    user_id: str,
) -> PartialRegenerationPreparation:
    chapter_content = chapter.content or ""
    if not chapter_content.strip():
        raise HTTPException(status_code=400, detail="章节内容为空")

    start_position, end_position, selected_text = _normalize_partial_selection(
        chapter_content=chapter_content,
        partial_request=partial_request,
    )
    original_word_count = len(selected_text)

    context_chars = partial_request.context_chars
    context_before_start = max(0, start_position - context_chars)
    context_before = chapter_content[context_before_start:start_position]
    context_after_end = min(len(chapter_content), end_position + context_chars)
    context_after = chapter_content[end_position:context_after_end]
    logger.info(
        f"局部重写上下文 - 原文: {original_word_count}字, 前文: {len(context_before)}字, 后文: {len(context_after)}字"
    )

    style_id, style_content = await _resolve_partial_style_content(
        db_session,
        project_id=chapter.project_id,
        requested_style_id=partial_request.style_id,
        user_id=user_id,
    )
    project, outline = await _load_partial_regeneration_project_bundle(
        db_session,
        chapter=chapter,
    )
    web_research_bundle = await chapter_web_research_service.collect_for_chapter(
        user_id=user_id,
        db_session=db_session,
        project=project,
        chapter=chapter,
        outline=outline,
        story_creation_brief=None,
        enable_web_research=partial_request.enable_web_research,
        web_research_query=partial_request.web_research_query,
    )
    web_research_grounding_block = _build_partial_web_research_grounding_block(
        list(web_research_bundle.get("assets") or [])
    )

    length_requirement = _build_partial_length_requirement(
        length_mode=partial_request.length_mode,
        target_word_count=partial_request.target_word_count,
        original_word_count=original_word_count,
    )
    template = await _get_regeneration_template("PARTIAL_REGENERATE", user_id, db_session)
    if not template:
        template = PARTIAL_REGENERATE_TEMPLATE
    if not template:
        raise RuntimeError("regeneration partial prompt 默认模板缺失")

    prompt = _format_regeneration_prompt(
        template,
        context_before=context_before if context_before else "（无前文上下文）",
        original_word_count=original_word_count,
        selected_text=selected_text,
        context_after=context_after if context_after else "（无后文上下文）",
        user_instructions=(partial_request.user_instructions or "") + web_research_grounding_block,
        length_requirement=length_requirement,
        style_content=style_content if style_content else "（未提供风格约束）",
    )

    target_words = _calculate_partial_target_words(
        length_mode=partial_request.length_mode,
        target_word_count=partial_request.target_word_count,
        original_word_count=original_word_count,
    )
    max_tokens = max(500, min(int(target_words * 3), 8000))

    return PartialRegenerationPreparation(
        start_position=start_position,
        end_position=end_position,
        original_text=selected_text,
        original_word_count=original_word_count,
        style_id=style_id,
        style_content=style_content,
        prompt=prompt,
        target_words=target_words,
        max_tokens=max_tokens,
    )


def normalize_partial_regeneration_output(text: str) -> str:
    cleaned = (text or "").strip()
    for prefix in _PARTIAL_REGENERATE_PREFIXES_TO_REMOVE:
        if cleaned.startswith(prefix):
            cleaned = cleaned[len(prefix):].strip()
            break

    if (cleaned.startswith('"') and cleaned.endswith('"')) or (
        cleaned.startswith("'") and cleaned.endswith("'")
    ):
        cleaned = cleaned[1:-1]
    if (cleaned.startswith("「") and cleaned.endswith("」")) or (
        cleaned.startswith("『") and cleaned.endswith("』")
    ):
        cleaned = cleaned[1:-1]
    return cleaned.strip()


async def regenerate_chapter_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    regenerate_request: ChapterRegenerateRequest,
    background_tasks: BackgroundTasks,
    db_session,
    user_ai_service: AIService,
):
    _ = background_tasks
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    try:
        regeneration_context = await prepare_chapter_regeneration_stream_context(
            db_session,
            chapter=chapter,
            regenerate_request=regenerate_request,
            user_id=user_id,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except LookupError as exc:
        raise HTTPException(status_code=404, detail=str(exc)) from exc

    return create_sse_response(
        build_chapter_regeneration_event_stream(
            db_session_source=lambda: get_db(request),
            context=regeneration_context,
            user_ai_service=user_ai_service,
            regenerator_factory=lambda ai_service: REGENERATOR_FACTORY(ai_service),
            sanitize_generated_text=sanitize_generated_narrative_text,
            contains_workflow_meta_text=contains_chapter_workflow_meta_text,
        )
    )


async def partial_regenerate_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db_session,
    user_ai_service: AIService,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    try:
        preparation = await prepare_partial_regeneration(
            db_session,
            chapter=chapter,
            partial_request=partial_request,
            user_id=user_id,
        )
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    async def event_generator():
        tracker = WizardProgressTracker("Partial Rewrite")

        try:
            yield await tracker.start()
            yield await tracker.loading("Preparing rewrite context...", 0.3)
            yield await tracker.preparing("Starting generation...")

            full_content = ""
            chunk_count = 0

            yield await tracker.generating(
                current_chars=0,
                estimated_total=preparation.target_words,
            )

            async for chunk in user_ai_service.generate_text_stream(
                prompt=preparation.prompt,
                max_tokens=preparation.max_tokens,
            ):
                full_content += chunk
                chunk_count += 1

                yield await tracker.generating_chunk(chunk)

                if chunk_count % 5 == 0:
                    yield await tracker.generating(
                        current_chars=len(full_content),
                        estimated_total=preparation.target_words,
                        message=f"Generating rewrite... {len(full_content)} chars",
                    )

                await asyncio.sleep(0)

            full_content = normalize_partial_regeneration_output(full_content)
            full_content, removed_meta_lines = sanitize_generated_narrative_text(
                full_content
            )
            if removed_meta_lines > 0:
                logger.warning(
                    "Partial regeneration removed %s workflow meta lines: chapter_id=%s",
                    removed_meta_lines,
                    chapter_id,
                )
            if not full_content.strip():
                raise ValueError("Rewrite result is empty after sanitization")
            if contains_chapter_workflow_meta_text(full_content):
                raise ValueError("Rewrite result still contains workflow meta text")

            new_word_count = len(full_content)
            logger.info(
                "Partial regeneration completed: %s chars -> %s chars",
                preparation.original_word_count,
                new_word_count,
            )

            yield await tracker.complete("Rewrite complete")
            yield await tracker.result(
                {
                    "new_text": full_content,
                    "word_count": new_word_count,
                    "original_word_count": preparation.original_word_count,
                    "start_position": preparation.start_position,
                    "end_position": preparation.end_position,
                }
            )
            yield await tracker.done()
        except Exception as exc:
            logger.error("Partial regeneration failed: %s", str(exc), exc_info=True)
            yield await tracker.error(str(exc))

    return create_sse_response(event_generator())


async def apply_partial_regenerate_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db_session,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    new_text_raw = str(apply_request.get("new_text", "") or "")
    start_position = apply_request.get("start_position", 0)
    end_position = apply_request.get("end_position", 0)

    new_text, removed_meta_lines = sanitize_generated_narrative_text(new_text_raw)
    if removed_meta_lines > 0:
        logger.warning(
            "Partial regenerate apply removed %s workflow meta lines: chapter_id=%s",
            removed_meta_lines,
            chapter_id,
        )
    if not new_text:
        raise HTTPException(status_code=400, detail="改写内容为空")
    if contains_chapter_workflow_meta_text(new_text):
        raise HTTPException(status_code=400, detail="改写内容仍包含工作流提示文本")

    content_length = len(chapter.content or "")
    if start_position < 0 or end_position > content_length or start_position >= end_position:
        raise HTTPException(status_code=400, detail="改写位置非法")

    new_content = (
        (chapter.content or "")[:start_position]
        + new_text
        + (chapter.content or "")[end_position:]
    )
    apply_result = await apply_chapter_content_update(
        db_session,
        chapter=chapter,
        content=new_content,
    )

    logger.info(
        "Partial regenerate applied: chapter_id=%s, %s -> %s",
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
    )

    return {
        "success": True,
        "chapter_id": chapter_id,
        "word_count": apply_result.new_word_count,
        "old_word_count": apply_result.old_word_count,
        "message": "局部改写已应用",
    }


@router.post('/{chapter_id}/regenerate-stream', summary='Regenerate chapter stream')
async def regenerate_chapter_stream(
    chapter_id: str,
    request: Request,
    regenerate_request: ChapterRegenerateRequest,
    background_tasks: BackgroundTasks,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """Run regeneration with SSE streaming output."""
    return await regenerate_chapter_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        regenerate_request=regenerate_request,
        background_tasks=background_tasks,
        db_session=db,
        user_ai_service=user_ai_service,
    )


@router.post("/{chapter_id}/partial-regenerate-stream", summary="局部重写章节片段")
async def partial_regenerate_stream(
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """对章节选中片段进行局部重写并返回 SSE 流。"""
    return await partial_regenerate_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        partial_request=partial_request,
        db_session=db,
        user_ai_service=user_ai_service,
    )


@router.post("/{chapter_id}/apply-partial-regenerate", summary="应用局部改写")
async def apply_partial_regenerate(
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db: AsyncSession = Depends(get_db),
):
    """将局部重写结果写回到章节内容。"""
    return await apply_partial_regenerate_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        apply_request=apply_request,
        db_session=db,
    )


@router.get('/{chapter_id}/regeneration/tasks', summary='Get regeneration task history')
async def get_regeneration_tasks(
    chapter_id: str,
    request: Request,
    limit: int = Query(10, ge=1, le=50),
    db: AsyncSession = Depends(get_db),
):
    """Return regeneration task history for one chapter."""
    user_id = require_authenticated_user_id(request)
    await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )
    return await load_regeneration_tasks_payload(
        db_session=db,
        chapter_id=chapter_id,
        limit=limit,
    )
from tests.test_support.chapter_regeneration_prompt_test_support import (
    build_chapter_regeneration_prompt,
)





