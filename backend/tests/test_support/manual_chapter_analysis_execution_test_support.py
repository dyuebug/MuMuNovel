"""章节分析后台执行冻结 runtime shim。

该文件保留给 rollback/source-map 和历史 patch surface 使用，
真实 owner 已收口到 Rust analysis runtime / trigger / persistence owner。
"""

from __future__ import annotations

import asyncio
import json
import uuid
from asyncio import Lock
from datetime import datetime
from functools import lru_cache
from pathlib import Path
from typing import TYPE_CHECKING, Any, Dict, List, Optional
import re

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active chapter-analysis background runtime contract; this "
    "Python module is kept only as frozen rollback/source-map material "
    "behind repointed analysis route shells."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_analysis_runtime_service.rs; "
    "backend-rs/src/services/chapter_analysis_runtime_service/trigger_runtime_owner.rs; "
    "backend-rs/src/services/chapter_analysis_runtime_service/persistence_owner.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "python_chapter_analysis_routes_fallback"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_packet_test_support import (
        StoryGenerationGuidance,
        StoryPacket,
    )
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload

from tests.test_support.retired_runtime_test_support import get_logger

logger = get_logger(__name__)
db_write_locks: dict[str, Lock] = {}
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)


async def update_careers_from_analysis(
    *,
    db,
    project_id: str,
    character_states: List[Dict[str, Any]],
    chapter_id: str,
    chapter_number: int,
) -> Dict[str, Any]:
    """根据章节分析结果更新角色职业信息。"""
    from sqlalchemy import select

    from migrator_app.models import Career, CharacterCareer
    from migrator_app.models.character import Character

    if not character_states:
        logger.info("📋 角色状态列表为空，跳过职业更新")
        return {"updated_count": 0, "changes": []}

    updated_count = 0
    changes_log: List[Dict[str, Any]] = []

    logger.info(f"🔍 开始分析第{chapter_number}章的角色职业变化...")

    async def _update_main_career_stage(
        *,
        character: Character,
        stage_change: int,
        career_changes: Dict[str, Any],
    ) -> bool:
        try:
            char_career_result = await db.execute(
                select(CharacterCareer).where(
                    CharacterCareer.character_id == character.id,
                    CharacterCareer.career_type == "main",
                )
            )
            char_career = char_career_result.scalar_one_or_none()

            if not char_career:
                logger.warning(f"  ⚠️ {character.name} 没有主职业关联记录")
                return False

            career_result = await db.execute(select(Career).where(Career.id == char_career.career_id))
            career = career_result.scalar_one_or_none()

            if not career:
                logger.warning(f"  ⚠️ 职业ID {char_career.career_id} 不存在")
                return False

            old_stage = char_career.current_stage
            new_stage = min(max(1, old_stage + stage_change), career.max_stage)

            if new_stage == old_stage:
                logger.info(f"  📊 {character.name} 的 {career.name} 已达到边界，无法变更")
                return False

            char_career.current_stage = new_stage
            character.main_career_stage = new_stage

            breakthrough_desc = career_changes.get("career_breakthrough", "")
            changes_log.append(
                {
                    "character": character.name,
                    "career": career.name,
                    "career_type": "main",
                    "old_stage": old_stage,
                    "new_stage": new_stage,
                    "change": stage_change,
                    "chapter": chapter_number,
                    "description": breakthrough_desc,
                }
            )

            change_desc = "晋升" if stage_change > 0 else "降级"
            logger.info(
                f"  ✨ {character.name} 的主职业 [{career.name}] "
                f"{old_stage}阶 → {new_stage}阶 ({change_desc})"
            )
            if breakthrough_desc:
                logger.info(f"     突破描述: {breakthrough_desc[:50]}...")
            return True
        except Exception as error:
            logger.error(f"  ❌ 更新主职业失败: {str(error)}")
            return False

    async def _update_sub_career_stage(
        *,
        character: Character,
        sub_change: Dict[str, Any],
    ) -> bool:
        try:
            career_name = sub_change.get("career_name")
            stage_change = sub_change.get("stage_change", 0)
            if not career_name or stage_change == 0:
                return False

            career_result = await db.execute(
                select(Career).where(
                    Career.name == career_name,
                    Career.project_id == project_id,
                    Career.type == "sub",
                )
            )
            career = career_result.scalar_one_or_none()
            if not career:
                logger.warning(f"  ⚠️ 副职业 [{career_name}] 不存在")
                return False

            char_career_result = await db.execute(
                select(CharacterCareer).where(
                    CharacterCareer.character_id == character.id,
                    CharacterCareer.career_id == career.id,
                    CharacterCareer.career_type == "sub",
                )
            )
            char_career = char_career_result.scalar_one_or_none()
            if not char_career:
                logger.warning(f"  ⚠️ {character.name} 没有 [{career_name}] 副职业")
                return False

            old_stage = char_career.current_stage
            new_stage = min(max(1, old_stage + stage_change), career.max_stage)
            if new_stage == old_stage:
                return False

            char_career.current_stage = new_stage
            sub_careers = json.loads(character.sub_careers) if character.sub_careers else []
            for sub_career in sub_careers:
                if sub_career.get("career_id") == career.id:
                    sub_career["stage"] = new_stage
                    break
            character.sub_careers = json.dumps(sub_careers, ensure_ascii=False)

            changes_log.append(
                {
                    "character": character.name,
                    "career": career.name,
                    "career_type": "sub",
                    "old_stage": old_stage,
                    "new_stage": new_stage,
                    "change": stage_change,
                    "chapter": chapter_number,
                }
            )
            logger.info(f"  ✨ {character.name} 的副职业 [{career.name}] {old_stage}阶 → {new_stage}阶")
            return True
        except Exception as error:
            logger.error(f"  ❌ 更新副职业失败: {str(error)}")
            return False

    async def _add_new_career(
        *,
        character: Character,
        career_name: str,
    ) -> bool:
        try:
            career_result = await db.execute(
                select(Career).where(
                    Career.name == career_name,
                    Career.project_id == project_id,
                )
            )
            career = career_result.scalar_one_or_none()
            if not career:
                logger.warning(f"  ⚠️ 职业 [{career_name}] 不存在，无法添加")
                return False

            existing_result = await db.execute(
                select(CharacterCareer).where(
                    CharacterCareer.character_id == character.id,
                    CharacterCareer.career_id == career.id,
                )
            )
            if existing_result.scalar_one_or_none():
                logger.info(f"  📋 {character.name} 已拥有 [{career_name}]，跳过")
                return False

            if career.type == "main":
                if character.main_career_id:
                    logger.warning(f"  ⚠️ {character.name} 已有主职业，无法添加新主职业")
                    return False

                db.add(
                    CharacterCareer(
                        id=str(uuid.uuid4()),
                        character_id=character.id,
                        career_id=career.id,
                        career_type="main",
                        current_stage=1,
                    )
                )
                character.main_career_id = career.id
                character.main_career_stage = 1
                logger.info(f"  ✨ {character.name} 获得新主职业 [{career_name}]")
            else:
                sub_count_result = await db.execute(
                    select(CharacterCareer).where(
                        CharacterCareer.character_id == character.id,
                        CharacterCareer.career_type == "sub",
                    )
                )
                if len(sub_count_result.scalars().all()) >= 2:
                    logger.warning(f"  ⚠️ {character.name} 的副职业已达上限(2个)")
                    return False

                db.add(
                    CharacterCareer(
                        id=str(uuid.uuid4()),
                        character_id=character.id,
                        career_id=career.id,
                        career_type="sub",
                        current_stage=1,
                    )
                )
                sub_careers = json.loads(character.sub_careers) if character.sub_careers else []
                sub_careers.append({"career_id": career.id, "stage": 1})
                character.sub_careers = json.dumps(sub_careers, ensure_ascii=False)
                logger.info(f"  ✨ {character.name} 获得新副职业 [{career_name}]")

            changes_log.append(
                {
                    "character": character.name,
                    "career": career.name,
                    "career_type": career.type,
                    "action": "new",
                    "chapter": chapter_number,
                }
            )
            return True
        except Exception as error:
            logger.error(f"  ❌ 添加新职业失败: {str(error)}")
            return False

    for char_state in character_states:
        char_name = char_state.get("character_name")
        career_changes = char_state.get("career_changes", {})

        if not career_changes or not isinstance(career_changes, dict):
            continue

        main_stage_change = career_changes.get("main_career_stage_change", 0)
        sub_career_changes = career_changes.get("sub_career_changes", [])
        new_careers = career_changes.get("new_careers", [])

        if main_stage_change == 0 and not sub_career_changes and not new_careers:
            continue

        logger.info(f"  👤 检测到角色 [{char_name}] 有职业变化")

        char_result = await db.execute(
            select(Character).where(
                Character.name == char_name,
                Character.project_id == project_id,
            )
        )
        character = char_result.scalar_one_or_none()
        if not character:
            logger.warning(f"  ⚠️ 角色不存在: {char_name}，跳过")
            continue

        if main_stage_change != 0 and character.main_career_id:
            if await _update_main_career_stage(
                character=character,
                stage_change=main_stage_change,
                career_changes=career_changes,
            ):
                updated_count += 1

        if sub_career_changes and isinstance(sub_career_changes, list):
            for sub_change in sub_career_changes:
                if await _update_sub_career_stage(character=character, sub_change=sub_change):
                    updated_count += 1

        if new_careers and isinstance(new_careers, list):
            for new_career_name in new_careers:
                if await _add_new_career(character=character, career_name=new_career_name):
                    updated_count += 1

    if updated_count > 0:
        await db.commit()
        logger.info(f"✅ 职业更新完成: 共更新了 {updated_count} 个角色的职业信息")
    else:
        logger.info("📋 本章没有角色职业变化")

    return {
        "updated_count": updated_count,
        "changes": changes_log,
    }


@lru_cache(maxsize=1)
def _load_analysis_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = (
        "PLOT_ANALYSIS",
        "CHAPTER_TEXT_CHECKER",
        "CHAPTER_TEXT_REVISER",
    )
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(
                f"manual chapter analysis test support 未找到模板常量: {template_key}"
            )
        templates[template_key] = match.group(1)
    return templates


def _analysis_template_lookup(template_key: str) -> Optional[str]:
    return _load_analysis_prompt_template_map().get(template_key)


class PromptService:
    PLOT_ANALYSIS = _load_analysis_prompt_template_map()["PLOT_ANALYSIS"]
    CHAPTER_TEXT_CHECKER = _load_analysis_prompt_template_map()[
        "CHAPTER_TEXT_CHECKER"
    ]
    CHAPTER_TEXT_REVISER = _load_analysis_prompt_template_map()[
        "CHAPTER_TEXT_REVISER"
    ]

    @staticmethod
    async def get_template(template_key: str, user_id: str, db_session):
        from tests.test_support.prompt_template_facade_test_support import get_template_for_owner

        return await get_template_for_owner(
            template_key,
            user_id,
            db_session,
            template_lookup=_analysis_template_lookup,
        )

    @staticmethod
    def format_prompt(template: str, **kwargs) -> str:
        from tests.test_support.prompt_template_facade_test_support import format_prompt

        return format_prompt(template, **kwargs)


def _prompt_service():
    return PromptService


_ORIGINAL_PROMPTSERVICE_GET_TEMPLATE = _prompt_service().get_template
_ORIGINAL_PROMPTSERVICE_FORMAT_PROMPT = _prompt_service().format_prompt


async def _get_analysis_template(template_key: str, user_id: str, db_session):
    prompt_service = _prompt_service()
    patched_impl = getattr(prompt_service, "get_template", None)
    if (
        patched_impl is not None
        and patched_impl is not _ORIGINAL_PROMPTSERVICE_GET_TEMPLATE
    ):
        return await patched_impl(template_key, user_id, db_session)

    return await prompt_service.get_template(template_key, user_id, db_session)


def _format_analysis_prompt(template: str, **kwargs) -> str:
    prompt_service = _prompt_service()
    patched_impl = getattr(prompt_service, "format_prompt", None)
    if (
        patched_impl is not None
        and patched_impl is not _ORIGINAL_PROMPTSERVICE_FORMAT_PROMPT
    ):
        return patched_impl(template, **kwargs)

    return prompt_service.format_prompt(template, **kwargs)


def _quality_context_service():
    from tests.test_support.story_packet_test_support import (
        build_prompt_quality_kwargs,
    )

    return build_prompt_quality_kwargs


def _generated_text_service():
    from tests.test_support.chapter_generated_text_test_support import (
        contains_chapter_workflow_meta_text,
        sanitize_generated_narrative_text,
    )

    return contains_chapter_workflow_meta_text, sanitize_generated_narrative_text


async def get_chapter_analysis_write_lock(user_id: str) -> Lock:
    lock = db_write_locks.get(user_id)
    if lock is None:
        lock = Lock()
        db_write_locks[user_id] = lock
    return lock


def normalize_checker_result(raw: Optional[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
    if not isinstance(raw, dict):
        return None

    severity_allow = {"critical", "major", "minor"}
    normalized_issues: List[Dict[str, str]] = []

    for item in (raw.get("issues") or []):
        if not isinstance(item, dict):
            continue
        severity = str(item.get("severity") or "minor").strip().lower()
        if severity not in severity_allow:
            severity = "minor"
        issue = {
            "severity": severity,
            "category": str(item.get("category") or "文风表达").strip()[:40],
            "location": str(item.get("location") or "未明确位置").strip()[:120],
            "evidence": str(item.get("evidence") or "").strip()[:240],
            "impact": str(item.get("impact") or "").strip()[:240],
            "suggestion": str(item.get("suggestion") or "").strip()[:240],
        }
        if issue["suggestion"]:
            normalized_issues.append(issue)
        if len(normalized_issues) >= 8:
            break

    severity_counts = {
        "critical": sum(1 for i in normalized_issues if i["severity"] == "critical"),
        "major": sum(1 for i in normalized_issues if i["severity"] == "major"),
        "minor": sum(1 for i in normalized_issues if i["severity"] == "minor"),
    }

    overall = str(raw.get("overall_assessment") or "").strip()
    if not overall:
        if severity_counts["critical"] > 0:
            overall = "存在严重问题"
        elif severity_counts["major"] >= 4:
            overall = "较差"
        elif severity_counts["major"] >= 2:
            overall = "一般"
        elif severity_counts["major"] >= 1 or severity_counts["minor"] >= 3:
            overall = "良好"
        else:
            overall = "优秀"

    priority_actions: List[str] = []
    for action in (raw.get("priority_actions") or []):
        if isinstance(action, str) and action.strip():
            priority_actions.append(action.strip()[:220])
        if len(priority_actions) >= 5:
            break
    if not priority_actions:
        priority_actions = [i["suggestion"] for i in normalized_issues[:3] if i["suggestion"]]

    revision_suggestions: List[str] = []
    seen: set[str] = set()
    for suggestion in (raw.get("revision_suggestions") or []):
        if not isinstance(suggestion, str):
            continue
        text = suggestion.strip()
        if not text or text in seen:
            continue
        seen.add(text)
        revision_suggestions.append(text[:220])
        if len(revision_suggestions) >= 8:
            break
    for issue in normalized_issues:
        text = issue["suggestion"]
        if text and text not in seen:
            seen.add(text)
            revision_suggestions.append(text[:220])
        if len(revision_suggestions) >= 8:
            break

    return {
        "overall_assessment": overall,
        "severity_counts": severity_counts,
        "issues": normalized_issues,
        "priority_actions": priority_actions,
        "revision_suggestions": revision_suggestions,
    }


def build_checker_report_text(checker_result: Optional[Dict[str, Any]]) -> str:
    if not checker_result:
        return ""

    counts = checker_result.get("severity_counts") or {}
    lines = [
        "【第三版正文质检】",
        f"- 总评：{checker_result.get('overall_assessment', '未给出')}",
        f"- 问题统计：严重{counts.get('critical', 0)} / 中等{counts.get('major', 0)} / 轻微{counts.get('minor', 0)}",
    ]
    actions = checker_result.get("priority_actions") or []
    if actions:
        lines.append("- 优先修复：")
        for idx, action in enumerate(actions[:3], start=1):
            lines.append(f"  {idx}. {action}")
    return "\n".join(lines)


def merge_checker_suggestions(
    analysis_suggestions: Optional[List[Any]],
    checker_result: Optional[Dict[str, Any]],
) -> List[str]:
    merged: List[str] = []
    seen: set[str] = set()

    for item in (analysis_suggestions or []):
        if not isinstance(item, str):
            continue
        text = item.strip()
        if text and text not in seen:
            seen.add(text)
            merged.append(text[:220])

    if not checker_result:
        return merged[:14]

    counts = checker_result.get("severity_counts") or {}
    if counts.get("critical", 0) > 0:
        top_line = "【质检优先级】先修复所有严重问题，再处理中等问题。"
        merged.insert(0, top_line)
        seen.add(top_line)

    for issue in (checker_result.get("issues") or []):
        if not isinstance(issue, dict):
            continue
        severity = issue.get("severity", "minor")
        if severity == "critical":
            prefix = "【质检-严重】"
        elif severity == "major":
            prefix = "【质检-中等】"
        else:
            prefix = "【质检-轻微】"
        category = str(issue.get("category") or "").strip()
        suggestion = str(issue.get("suggestion") or "").strip()
        text = f"{prefix}{category}：{suggestion}" if suggestion else ""
        if text and text not in seen:
            seen.add(text)
            merged.append(text[:220])
        if len(merged) >= 12:
            break

    for suggestion in (checker_result.get("revision_suggestions") or []):
        if not isinstance(suggestion, str):
            continue
        text = suggestion.strip()
        line = f"【质检建议】{text[:200]}"
        if text and line not in seen:
            seen.add(line)
            merged.append(line)
        if len(merged) >= 14:
            break

    return merged[:14]


def _collect_reviser_priority_issues(
    checker_result: Optional[Dict[str, Any]],
) -> List[Dict[str, Any]]:
    if not checker_result:
        return []

    priority_issues: List[Dict[str, Any]] = []
    for severity in ("critical", "major"):
        for issue in (checker_result.get("issues") or []):
            if isinstance(issue, dict) and issue.get("severity") == severity:
                priority_issues.append(issue)
    return priority_issues


def build_reviser_priority_issues_text(checker_result: Optional[Dict[str, Any]]) -> str:
    priority_issues = _collect_reviser_priority_issues(checker_result)
    if not priority_issues:
        return "（无高优先问题）"

    lines: List[str] = []
    for idx, issue in enumerate(priority_issues[:8], start=1):
        severity = str(issue.get("severity") or "major").lower()
        severity_label = "严重" if severity == "critical" else "中等"
        category = str(issue.get("category") or "未分类")
        location = str(issue.get("location") or "未定位")
        impact = str(issue.get("impact") or "").strip()
        suggestion = str(issue.get("suggestion") or "").strip()
        lines.append(f"{idx}. [{severity_label}][{category}] 位置: {location}")
        if impact:
            lines.append(f"   影响: {impact}")
        if suggestion:
            lines.append(f"   建议: {suggestion}")
    return "\n".join(lines)


def build_checker_history_payload(checker_result: Dict[str, Any]) -> str:
    payload = {
        "log_type": "chapter_text_checker_v1",
        "checker_result": checker_result,
        "checked_at": datetime.now().isoformat(),
    }
    return json.dumps(payload, ensure_ascii=False)


def build_reviser_history_payload(reviser_result: Dict[str, Any]) -> str:
    payload = {
        "log_type": "chapter_text_reviser_v1",
        "reviser_result": reviser_result,
        "revised_at": datetime.now().isoformat(),
    }
    return json.dumps(payload, ensure_ascii=False)


async def run_chapter_text_checker(
    *,
    ai_service: AIService,
    db_session,
    user_id: str,
    chapter_number: int,
    chapter_title: str,
    chapter_content: str,
    chapter_outline: str,
    characters_info: str,
    world_rules: str,
    quality_profile: Optional[Dict[str, Any]] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
) -> Optional[Dict[str, Any]]:
    try:
        prompt_service = _prompt_service()
        build_prompt_quality_kwargs = _quality_context_service()

        template = await _get_analysis_template(
            "CHAPTER_TEXT_CHECKER",
            user_id,
            db_session,
        )
        if not template:
            template = prompt_service.CHAPTER_TEXT_CHECKER

        prompt = _format_analysis_prompt(
            template,
            chapter_number=chapter_number,
            chapter_title=chapter_title or f"第{chapter_number}章",
            chapter_content=(chapter_content or "")[:12000],
            chapter_outline=(chapter_outline or "（无大纲信息）")[:3000],
            characters_info=(characters_info or "（无角色信息）")[:4000],
            world_rules=(world_rules or "（无世界规则）")[:1500],
            _template_key="CHAPTER_TEXT_CHECKER",
            **build_prompt_quality_kwargs(quality_profile, guidance=generation_guidance),
        )

        result = await ai_service.call_with_json_retry(
            prompt=prompt,
            max_retries=2,
            temperature=0.2,
            max_tokens=2200,
            expected_type="object",
            auto_mcp=False,
        )
        normalized = normalize_checker_result(result)
        if not normalized:
            return None

        counts = normalized.get("severity_counts") or {}
        logger.info(
            "✅ 第三版正文质检完成: critical=%s, major=%s, minor=%s",
            counts.get("critical", 0),
            counts.get("major", 0),
            counts.get("minor", 0),
        )
        return normalized
    except Exception as checker_error:
        logger.warning(f"⚠️ 第三版正文质检失败，已跳过: {checker_error}")
        return None


async def run_chapter_text_reviser(
    *,
    ai_service: AIService,
    db_session,
    user_id: str,
    chapter_number: int,
    chapter_title: str,
    chapter_content: str,
    checker_result: Dict[str, Any],
    quality_profile: Optional[Dict[str, Any]] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
) -> Optional[Dict[str, Any]]:
    counts = (checker_result or {}).get("severity_counts") or {}
    critical_count = int(counts.get("critical") or 0)
    major_count = int(counts.get("major") or 0)
    priority_issue_count = critical_count + major_count
    if priority_issue_count <= 0:
        return None

    try:
        prompt_service = _prompt_service()
        build_prompt_quality_kwargs = _quality_context_service()
        contains_chapter_workflow_meta_text, sanitize_generated_narrative_text = (
            _generated_text_service()
        )

        template = await _get_analysis_template(
            "CHAPTER_TEXT_REVISER",
            user_id,
            db_session,
        )
        if not template:
            template = prompt_service.CHAPTER_TEXT_REVISER

        checker_json = json.dumps(checker_result, ensure_ascii=False)
        prompt = _format_analysis_prompt(
            template,
            chapter_number=chapter_number,
            chapter_title=chapter_title or f"第{chapter_number}章",
            chapter_content=(chapter_content or "")[:12000],
            critical_issues_text=build_reviser_priority_issues_text(checker_result),
            checker_result_json=checker_json[:12000],
            _template_key="CHAPTER_TEXT_REVISER",
            **build_prompt_quality_kwargs(quality_profile, guidance=generation_guidance),
        )

        result = await ai_service.call_with_json_retry(
            prompt=prompt,
            max_retries=2,
            temperature=0.22,
            max_tokens=4200,
            expected_type="object",
            auto_mcp=False,
        )
        if not isinstance(result, dict):
            return None

        revised_text_raw = str(result.get("revised_text") or "").strip()
        revised_text, removed_meta_lines = sanitize_generated_narrative_text(revised_text_raw)
        if not revised_text:
            return None
        if contains_chapter_workflow_meta_text(revised_text):
            logger.warning("⚠️ 自动修订草稿包含流程化元文本，已丢弃")
            return None

        applied_issues: List[str] = []
        for item in (result.get("applied_issues") or []):
            if isinstance(item, str) and item.strip():
                applied_issues.append(item.strip()[:220])
            if len(applied_issues) >= 8:
                break
        if not applied_issues:
            applied_issues = [
                i.get("suggestion", "")
                for i in (checker_result.get("issues") or [])
                if isinstance(i, dict) and i.get("severity") in {"critical", "major"} and i.get("suggestion")
            ][:3]

        unresolved_issues: List[str] = []
        for item in (result.get("unresolved_issues") or []):
            if isinstance(item, str) and item.strip():
                unresolved_issues.append(item.strip()[:220])
            if len(unresolved_issues) >= 8:
                break

        reviser_result = {
            "critical_count": critical_count,
            "major_count": major_count,
            "priority_issue_count": priority_issue_count,
            "applied_critical_count": len(applied_issues),
            "applied_issue_count": len(applied_issues),
            "applied_issues": applied_issues,
            "unresolved_issues": unresolved_issues,
            "change_summary": str(
                result.get("change_summary") or "已根据高优先问题生成自动修订草稿"
            ).strip()[:220],
            "revised_word_count": len(revised_text),
            "meta_lines_removed": removed_meta_lines,
            "revised_text": revised_text,
            "revised_text_preview": revised_text[:500],
        }
        logger.info(
            "自动修订草稿已生成: priority=%s, critical=%s, major=%s, applied=%s, unresolved=%s",
            priority_issue_count,
            critical_count,
            major_count,
            reviser_result["applied_issue_count"],
            len(unresolved_issues),
        )
        return reviser_result
    except Exception as reviser_error:
        logger.warning(f"⚠️ 自动修订草稿生成失败，已跳过: {reviser_error}")
        return None


async def execute_chapter_analysis_background(
    chapter_id: str,
    user_id: str,
    project_id: str,
    task_id: str,
    ai_service: AIService,
    quality_profile: Optional[Dict[str, Any]] = None,
    story_packet: Optional[StoryPacket] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
    chapter_content_override: Optional[str] = None,
    chapter_word_count_override: Optional[int] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
) -> bool:
    """
    后台异步分析章节（支持并发，使用锁保护数据库写入）
    
    Args:
        chapter_id: 章节ID
        user_id: 用户ID
        project_id: 项目ID
        task_id: 任务ID
        ai_service: AI服务实例
        
    Returns:
        bool: True表示分析成功，False表示分析失败
    """
    from sqlalchemy import select

    from tests.test_support.database_test_support import get_session_factory
    from migrator_app.models.analysis_task import AnalysisTask
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.character import Character
    from migrator_app.models import GenerationHistory
    from migrator_app.models import PlotAnalysis, StoryMemory
    from migrator_app.models.outline import Outline
    from migrator_app.models.project import Project
    from tests.test_support.character_context_test_support import build_characters_info_with_careers
    from tests.test_support.story_packet_test_support import (
        StoryPacket,
    )
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity,
    )
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )
    from tests.test_support.foreshadow_test_support import foreshadow_service
    from tests.test_support.memory_service_test_support import memory_service
    from tests.test_support.plot_analyzer_test_support import PlotAnalyzer
    from tests.test_support.story_repair_payload_test_support import resolve_story_repair_prompt_kwargs

    db_session = None
    write_lock = await get_chapter_analysis_write_lock(user_id)
    resolved_story_repair_kwargs = resolve_story_repair_prompt_kwargs(
        story_repair_payload,
        summary=story_repair_summary,
        targets=story_repair_targets,
        strengths=story_preserve_strengths,
    )
    story_repair_summary = resolved_story_repair_kwargs.get("story_repair_summary")
    story_repair_targets = resolved_story_repair_kwargs.get("story_repair_targets")
    story_preserve_strengths = resolved_story_repair_kwargs.get("story_preserve_strengths")
    
    try:
        logger.info(f"🔍 开始分析章节: {chapter_id}, 任务ID: {task_id}")
        
        # 创建独立数据库会话
        AsyncSessionLocal = await get_session_factory(user_id)
        db_session = AsyncSessionLocal()
        
        # 1. 获取任务（读操作）
        task_result = await db_session.execute(
            select(AnalysisTask).where(AnalysisTask.id == task_id)
        )
        task = task_result.scalar_one_or_none()
        
        if not task:
            logger.error(f"❌ 任务不存在: {task_id}")
            return False
        
        # 更新任务状态（写操作，需要锁）
        async with write_lock:
            task.status = 'running'
            task.started_at = datetime.now()
            task.progress = 10
            await db_session.commit()
        
        # 2. 获取章节信息（读操作）
        chapter_result = await db_session.execute(
            select(Chapter).where(Chapter.id == chapter_id)
        )
        chapter = chapter_result.scalar_one_or_none()
        effective_chapter_content = chapter_content_override if chapter_content_override is not None else (chapter.content if chapter else None)
        if not chapter or not effective_chapter_content:
            async with write_lock:
                task.status = 'failed'
                task.error_message = '章节不存在或正文为空'
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ 章节不存在或正文为空: {chapter_id}")
            return False
        effective_chapter_word_count = int(chapter_word_count_override or chapter.word_count or len(effective_chapter_content))
        async with write_lock:
            task.progress = 20
            await db_session.commit()

        # 获取已埋入的伏笔列表（用于回收匹配，传入当前章节号以启用智能标记）
        project_result = await db_session.execute(
            select(Project).where(Project.id == project_id)
        )
        project = project_result.scalar_one_or_none()
        if not project:
            async with write_lock:
                task.status = 'failed'
                task.error_message = '项目不存在'
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ 项目不存在: {project_id}")
            return False

        chapter_outline_record = None
        chapter_outline_text = ""
        if chapter.outline_id:
            outline_result = await db_session.execute(
                select(Outline).where(Outline.id == chapter.outline_id)
            )
            chapter_outline_record = outline_result.scalar_one_or_none()
            if chapter_outline_record:
                chapter_outline_text = (chapter_outline_record.content or chapter_outline_record.title or "").strip()

        existing_foreshadows = await foreshadow_service.get_planted_foreshadows_for_analysis(
            db=db_session,
            project_id=project_id,
            current_chapter_number=chapter.chapter_number  # 传入当前章节号以启用智能标记
        )
        logger.info(f"📋 后台分析 - 已获取{len(existing_foreshadows)}个已埋入伏笔用于匹配（含智能回收标记）")
        
        # 获取项目角色信息（根据大纲/展开规划筛选本章相关角色）
        filter_character_names = None
        
        # 1-N模式：从expansion_plan中提取character_focus
        if chapter.expansion_plan:
            try:
                plan = json.loads(chapter.expansion_plan)
                focus_names = plan.get('character_focus', [])
                if focus_names:
                    filter_character_names = focus_names
                    logger.info(f"📋 从expansion_plan提取角色焦点: {filter_character_names}")
            except (json.JSONDecodeError, Exception):
                pass
        
        # 1-1模式：从outline.structure中提取characters
        if not filter_character_names and chapter_outline_record and chapter_outline_record.structure:
            try:
                structure = json.loads(chapter_outline_record.structure)
                raw_characters = structure.get('characters', [])
                if raw_characters:
                    filter_character_names = [
                        c['name'] if isinstance(c, dict) else c
                        for c in raw_characters
                    ]
                    logger.info(f"📋 从outline.structure提取角色: {filter_character_names}")
            except (json.JSONDecodeError, Exception):
                pass
        
        # 查询角色（根据筛选名单或全部）
        characters_query = select(Character).where(Character.project_id == project_id)
        if filter_character_names:
            characters_query = characters_query.where(Character.name.in_(filter_character_names))
        characters_result = await db_session.execute(characters_query)
        project_characters = characters_result.scalars().all()
        
        # 如果筛选后无角色，降级为全部角色
        if not project_characters and filter_character_names:
            logger.warning(f"⚠️ 筛选后无匹配角色，降级为全部角色")
            characters_result = await db_session.execute(
                select(Character).where(Character.project_id == project_id)
            )
            project_characters = characters_result.scalars().all()
            filter_character_names = None
        
        characters_info = await build_characters_info_with_careers(
            db=db_session,
            project_id=project_id,
            characters=project_characters,
            filter_character_names=filter_character_names
        )
        logger.info(f"📋 后台分析 - 已获取{len(project_characters)}个角色信息用于分析")

        analysis_quality_profile = quality_profile or await resolve_chapter_quality_profile(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=None,
            enable_mcp=True,
            prefer_project_default_style=True,
            log_prefix="章节分析",
        )
        analysis_story_packet = story_packet
        if analysis_story_packet is None and generation_guidance is not None:
            analysis_story_packet = StoryPacket.from_guidance(
                generation_guidance,
                source="legacy-analysis-guidance",
            )
        if analysis_story_packet is None:
            analysis_story_packet = await build_story_generation_packet_with_project_continuity(
                db_session,
                project,
                source_label="chapter-analysis-defaults",
            )
        analysis_guidance = analysis_story_packet.guidance

        # 定义重试回调函数，用于在重试时更新任务状态
        async def on_retry_callback(attempt: int, max_retries: int, wait_time: int, error_reason: str):
            """重试时更新任务状态，让前端能感知到重试进度"""
            try:
                async with write_lock:
                    # 重新获取任务（确保获取最新状态）
                    task_result_retry = await db_session.execute(
                        select(AnalysisTask).where(AnalysisTask.id == task_id)
                    )
                    task_retry = task_result_retry.scalar_one_or_none()
                    if task_retry:
                        # 更新任务状态，保持 running 但更新 started_at 以重置超时计时器
                        task_retry.status = 'running'
                        task_retry.started_at = datetime.now()  # 重置开始时间，防止超时检测误判
                        task_retry.progress = min(70, 35 + attempt * 15)  # 根据重试次数更新进度
                        task_retry.error_message = f"正在重试({attempt}/{max_retries})：{error_reason[:100]}"
                        await db_session.commit()
                        logger.info(f"🔄 分析任务重试状态已更新: 尝试 {attempt}/{max_retries}, 等待 {wait_time}s, 原因: {error_reason[:50]}...")
            except Exception as callback_error:
                logger.warning(f"⚠️ 更新重试状态失败: {callback_error}")
        
        # 3. 使用PlotAnalyzer分析章节（传入已有伏笔列表、角色信息和重试回调）
        async with write_lock:
            task.progress = 30
            task.error_message = '正在调用AI分析章节...'
            await db_session.commit()

        analyzer = PlotAnalyzer(ai_service)
        analysis_result = await analyzer.analyze_chapter(
            chapter_number=chapter.chapter_number,
            title=chapter.title,
            content=effective_chapter_content,
            word_count=effective_chapter_word_count,
            existing_foreshadows=existing_foreshadows,
            on_retry=on_retry_callback,
            characters_info=characters_info,
            **analysis_story_packet.build_analysis_quality_kwargs(analysis_quality_profile),
        )
        
        if not analysis_result:
            analysis_error_message = analyzer.last_error_message or '章节分析失败，请稍后重试'
            async with write_lock:
                task.status = 'failed'
                task.error_message = analysis_error_message[:500]
                task.completed_at = datetime.now()
                await db_session.commit()
            logger.error(f"❌ AI分析失败: {chapter_id}, 原因: {analysis_error_message}")
            return False
        
        skip_followup_enrichment = analysis_result.get("analysis_mode") == "heuristic_fallback"
        checker_result = None
        reviser_result = None
        if skip_followup_enrichment:
            logger.warning(
                "⚠️ 当前分析使用启发式回退，跳过后续检查与润色补强: %s",
                analysis_result.get("fallback_reason") or "unknown",
            )
        else:
            checker_result = await run_chapter_text_checker(
                ai_service=ai_service,
                db_session=db_session,
                user_id=user_id,
                chapter_number=chapter.chapter_number,
                chapter_title=chapter.title or "",
                chapter_content=effective_chapter_content,
                chapter_outline=chapter_outline_text,
                characters_info=characters_info,
                world_rules=project.world_rules or "",
                quality_profile=analysis_quality_profile,
                generation_guidance=analysis_guidance,
            )
            reviser_result = await run_chapter_text_reviser(
                ai_service=ai_service,
                db_session=db_session,
                user_id=user_id,
                chapter_number=chapter.chapter_number,
                chapter_title=chapter.title or "",
                chapter_content=effective_chapter_content,
                checker_result=checker_result or {},
                quality_profile=analysis_quality_profile,
                generation_guidance=analysis_guidance,
            )

        analysis_report_text = analyzer.generate_analysis_summary(analysis_result)
        checker_report_text = build_checker_report_text(checker_result)
        if checker_report_text:
            analysis_report_text = f"{analysis_report_text}\n\n{checker_report_text}"
        if reviser_result:
            draft_priority_issue_count = int(
                reviser_result.get("priority_issue_count")
                or (
                    int(reviser_result.get("critical_count") or 0)
                    + int(reviser_result.get("major_count") or 0)
                )
            )
            draft_applied_issue_count = int(
                reviser_result.get("applied_issue_count")
                or reviser_result.get("applied_critical_count")
                or 0
            )
            reviser_summary_lines = [
                "【第三版自动修订草稿】",
                f"- 高优先问题数：{draft_priority_issue_count}（严重{reviser_result.get('critical_count', 0)} / 中等{reviser_result.get('major_count', 0)}）",
                f"- 已处理问题数：{draft_applied_issue_count}",
                f"- 草稿字数：{reviser_result.get('revised_word_count', 0)}",
                f"- 说明：{reviser_result.get('change_summary', '已生成草稿')}",
            ]
            analysis_report_text = f"{analysis_report_text}\n\n" + "\n".join(reviser_summary_lines)

        merged_suggestions = merge_checker_suggestions(
            analysis_suggestions=analysis_result.get('suggestions', []),
            checker_result=checker_result,
        )
        if reviser_result:
            for unresolved in (reviser_result.get("unresolved_issues") or []):
                if isinstance(unresolved, str) and unresolved.strip():
                    merged_suggestions.append(f"【修订未完成】{unresolved.strip()[:200]}")
                if len(merged_suggestions) >= 16:
                    break
            merged_suggestions = merged_suggestions[:16]

        async with write_lock:
            task.progress = 60
            await db_session.commit()
        
        # 4. 保存分析结果到数据库（写操作，需要锁）
        async with write_lock:
            existing_analysis_result = await db_session.execute(
                select(PlotAnalysis).where(PlotAnalysis.chapter_id == chapter_id)
            )
            existing_analysis = existing_analysis_result.scalar_one_or_none()
            
            if existing_analysis:
                # 更新现有记录
                logger.info(f"  更新现有分析记录: {existing_analysis.id}")
                existing_analysis.plot_stage = analysis_result.get('plot_stage', '发展')
                existing_analysis.conflict_level = analysis_result.get('conflict', {}).get('level', 0)
                existing_analysis.conflict_types = analysis_result.get('conflict', {}).get('types', [])
                existing_analysis.emotional_tone = analysis_result.get('emotional_arc', {}).get('primary_emotion', '')
                existing_analysis.emotional_intensity = analysis_result.get('emotional_arc', {}).get('intensity', 0) / 10.0
                existing_analysis.hooks = analysis_result.get('hooks', [])
                existing_analysis.hooks_count = len(analysis_result.get('hooks', []))
                existing_analysis.foreshadows = analysis_result.get('foreshadows', [])
                existing_analysis.foreshadows_planted = sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'planted')
                existing_analysis.foreshadows_resolved = sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'resolved')
                existing_analysis.plot_points = analysis_result.get('plot_points', [])
                existing_analysis.plot_points_count = len(analysis_result.get('plot_points', []))
                existing_analysis.character_states = analysis_result.get('character_states', [])
                existing_analysis.scenes = analysis_result.get('scenes', [])
                existing_analysis.pacing = analysis_result.get('pacing', 'moderate')
                existing_analysis.overall_quality_score = analysis_result.get('scores', {}).get('overall', 0)
                existing_analysis.pacing_score = analysis_result.get('scores', {}).get('pacing', 0)
                existing_analysis.engagement_score = analysis_result.get('scores', {}).get('engagement', 0)
                existing_analysis.coherence_score = analysis_result.get('scores', {}).get('coherence', 0)
                existing_analysis.analysis_report = analysis_report_text
                existing_analysis.suggestions = merged_suggestions
                existing_analysis.dialogue_ratio = analysis_result.get('dialogue_ratio', 0)
                existing_analysis.description_ratio = analysis_result.get('description_ratio', 0)
            else:
                # 创建新记录
                logger.info(f"  创建新的分析记录")
                plot_analysis = PlotAnalysis(
                    chapter_id=chapter_id,
                    project_id=project_id,
                    plot_stage=analysis_result.get('plot_stage', '发展'),
                    conflict_level=analysis_result.get('conflict', {}).get('level', 0),
                    conflict_types=analysis_result.get('conflict', {}).get('types', []),
                    emotional_tone=analysis_result.get('emotional_arc', {}).get('primary_emotion', ''),
                    emotional_intensity=analysis_result.get('emotional_arc', {}).get('intensity', 0) / 10.0,
                    hooks=analysis_result.get('hooks', []),
                    hooks_count=len(analysis_result.get('hooks', [])),
                    foreshadows=analysis_result.get('foreshadows', []),
                    foreshadows_planted=sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'planted'),
                    foreshadows_resolved=sum(1 for f in analysis_result.get('foreshadows', []) if f.get('type') == 'resolved'),
                    plot_points=analysis_result.get('plot_points', []),
                    plot_points_count=len(analysis_result.get('plot_points', [])),
                    character_states=analysis_result.get('character_states', []),
                    scenes=analysis_result.get('scenes', []),
                    pacing=analysis_result.get('pacing', 'moderate'),
                    overall_quality_score=analysis_result.get('scores', {}).get('overall', 0),
                    pacing_score=analysis_result.get('scores', {}).get('pacing', 0),
                    engagement_score=analysis_result.get('scores', {}).get('engagement', 0),
                    coherence_score=analysis_result.get('scores', {}).get('coherence', 0),
                    analysis_report=analysis_report_text,
                    suggestions=merged_suggestions,
                    dialogue_ratio=analysis_result.get('dialogue_ratio', 0),
                    description_ratio=analysis_result.get('description_ratio', 0)
                )
                db_session.add(plot_analysis)

            if checker_result:
                checker_history = GenerationHistory(
                    project_id=project_id,
                    chapter_id=chapter_id,
                    prompt=f"章节质检: 第{chapter.chapter_number}章 {chapter.title or ''}",
                    generated_content=build_checker_history_payload(checker_result),
                    model="chapter_text_checker_v1",
                )
                db_session.add(checker_history)
            if reviser_result:
                reviser_history = GenerationHistory(
                    project_id=project_id,
                    chapter_id=chapter_id,
                    prompt=f"自动修订草稿: 第{chapter.chapter_number}章 {chapter.title or ''}",
                    generated_content=build_reviser_history_payload(reviser_result),
                    model="chapter_text_reviser_v1",
                )
                db_session.add(reviser_history)
            
            await db_session.commit()
            
            task.progress = 80
            await db_session.commit()
        
        # 5. 清理旧的分析伏笔（重新分析时需要先清理）
        try:
            async with write_lock:
                clean_result = await foreshadow_service.clean_chapter_analysis_foreshadows(
                    db=db_session,
                    project_id=project_id,
                    chapter_id=chapter_id
                )
            if clean_result['cleaned_count'] > 0:
                logger.info(f"🧹 重新分析前清理了 {clean_result['cleaned_count']} 个旧伏笔")
        except Exception as clean_error:
            logger.warning(f"⚠️ 清理旧伏笔失败（继续分析）: {str(clean_error)}")
        
        # 6. 提取记忆并保存到向量数据库（传入章节内容用于计算位置）
        memories = analyzer.extract_memories_from_analysis(
            analysis=analysis_result,
            chapter_id=chapter_id,
            chapter_number=chapter.chapter_number,
            chapter_content=effective_chapter_content,
            chapter_title=chapter.title or ""
        )
        
        # 先删除该章节的旧记忆（写操作，需要锁）
        async with write_lock:
            old_memories_result = await db_session.execute(
                select(StoryMemory).where(StoryMemory.chapter_id == chapter_id)
            )
            old_memories = old_memories_result.scalars().all()
            for old_mem in old_memories:
                await db_session.delete(old_mem)
            await db_session.commit()
            logger.info(f"  删除旧记忆: {len(old_memories)}条")
        
        # 准备批量添加的记忆数据（不需要锁）
        memory_records = []
        for mem in memories:
            memory_id = f"{chapter_id}_{mem['type']}_{len(memory_records)}"
            memory_records.append({
                'id': memory_id,
                'content': mem['content'],
                'type': mem['type'],
                'metadata': mem['metadata']
            })
            
        # 保存到关系数据库（写操作，需要锁）
        async with write_lock:
            for mem in memories:
                memory_id = memory_records[memories.index(mem)]['id']
                text_position = mem['metadata'].get('text_position', -1)
                text_length = mem['metadata'].get('text_length', 0)
                
                story_memory = StoryMemory(
                    id=memory_id,
                    project_id=project_id,
                    chapter_id=chapter_id,
                    memory_type=mem['type'],
                    content=mem['content'],
                    title=mem['title'],
                    importance_score=mem['metadata'].get('importance_score', 0.5),
                    tags=mem['metadata'].get('tags', []),
                    is_foreshadow=mem['metadata'].get('is_foreshadow', 0),
                    story_timeline=chapter.chapter_number,
                    chapter_position=text_position,
                    text_length=text_length,
                    related_characters=mem['metadata'].get('related_characters', []),
                    related_locations=mem['metadata'].get('related_locations', [])
                )
                db_session.add(story_memory)
                
                if text_position >= 0:
                    logger.debug(f"  保存记忆 {memory_id}: position={text_position}, length={text_length}")
            
            await db_session.commit()
        
        # 批量添加到向量数据库
        if memory_records:
            added_count = await memory_service.batch_add_memories(
                user_id=user_id,
                project_id=project_id,
                memories=memory_records
            )
            logger.info(f"✅ 添加{added_count}条记忆到向量库")
        
        # 💼 更新角色职业（根据分析结果）
        if analysis_result.get('character_states'):
            try:
                logger.info(f"💼 开始根据分析结果更新角色职业...")
                career_update_result = await update_careers_from_analysis(
                    db=db_session,
                    project_id=project_id,
                    character_states=analysis_result.get('character_states', []),
                    chapter_id=chapter_id,
                    chapter_number=chapter.chapter_number
                )
                
                if career_update_result['updated_count'] > 0:
                    logger.info(
                        f"✅ 更新了 {career_update_result['updated_count']} 个角色的职业信息"
                    )
                    if career_update_result['changes']:
                        for change in career_update_result['changes']:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无角色职业变化")
                    
            except Exception as career_error:
                # 职业更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新角色职业失败: {str(career_error)}", exc_info=True)
        else:
            logger.debug("📋 分析结果中无角色状态信息，跳过职业更新")
        
        # 👤 更新角色心理状态和关系（根据分析结果）
        if analysis_result.get('character_states'):
            try:
                from tests.test_support.character_state_update_test_support import (
                    CharacterStateUpdateService,
                )
                
                logger.info(f"👤 开始根据分析结果更新角色状态、关系和组织成员...")
                async with write_lock:
                    state_update_result = await CharacterStateUpdateService.update_from_analysis(
                        db=db_session,
                        project_id=project_id,
                        character_states=analysis_result.get('character_states', []),
                        chapter_id=chapter_id,
                        chapter_number=chapter.chapter_number
                    )
                
                total_state_changes = (
                    state_update_result['state_updated_count'] +
                    state_update_result['relationship_created_count'] +
                    state_update_result['relationship_updated_count'] +
                    state_update_result.get('org_updated_count', 0)
                )
                if total_state_changes > 0:
                    logger.info(
                        f"✅ 角色状态更新: 心理状态{state_update_result['state_updated_count']}个, "
                        f"新建关系{state_update_result['relationship_created_count']}个, "
                        f"更新关系{state_update_result['relationship_updated_count']}个, "
                        f"组织变动{state_update_result.get('org_updated_count', 0)}个"
                    )
                    if state_update_result['changes']:
                        for change in state_update_result['changes'][:8]:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无角色状态、关系或组织变化")
                    
            except Exception as state_error:
                # 角色状态更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新角色状态、关系和组织失败: {str(state_error)}", exc_info=True)
        
        # 🏛️ 更新组织自身状态（根据分析结果）
        if analysis_result.get('organization_states'):
            try:
                from tests.test_support.character_state_update_test_support import (
                    CharacterStateUpdateService,
                )
                
                logger.info(f"🏛️ 开始根据分析结果更新组织自身状态...")
                async with write_lock:
                    org_state_result = await CharacterStateUpdateService.update_organization_states(
                        db=db_session,
                        project_id=project_id,
                        organization_states=analysis_result.get('organization_states', []),
                        chapter_number=chapter.chapter_number
                    )
                
                if org_state_result['updated_count'] > 0:
                    logger.info(
                        f"✅ 组织状态更新: {org_state_result['updated_count']}个组织"
                    )
                    if org_state_result['changes']:
                        for change in org_state_result['changes'][:5]:
                            logger.info(f"  - {change}")
                else:
                    logger.info("ℹ️ 本章节无组织自身状态变化")
                    
            except Exception as org_state_error:
                # 组织状态更新失败不应影响整个分析流程
                logger.error(f"⚠️ 更新组织自身状态失败: {str(org_state_error)}", exc_info=True)
        
        # 🔮 自动更新伏笔状态（根据分析结果）
        if analysis_result.get('foreshadows'):
            try:
                logger.info(f"🔮 开始根据分析结果自动更新伏笔状态...")
                async with write_lock:
                    foreshadow_stats = await foreshadow_service.auto_update_from_analysis(
                        db=db_session,
                        project_id=project_id,
                        chapter_id=chapter_id,
                        chapter_number=chapter.chapter_number,
                        analysis_foreshadows=analysis_result.get('foreshadows', [])
                    )
                
                if foreshadow_stats['planted_count'] > 0 or foreshadow_stats['resolved_count'] > 0:
                    logger.info(
                        f"✅ 伏笔自动更新: 埋入{foreshadow_stats['planted_count']}个, "
                        f"回收{foreshadow_stats['resolved_count']}个"
                    )
                else:
                    logger.info("ℹ️ 本章节无新的伏笔状态变化")
                    
            except Exception as foreshadow_error:
                # 伏笔更新失败不应影响整个分析流程
                logger.error(f"⚠️ 自动更新伏笔失败: {str(foreshadow_error)}", exc_info=True)
        else:
            logger.debug("📋 分析结果中无伏笔信息，跳过伏笔自动更新")
        
        # 最终更新任务状态（写操作，需要锁）- 增加重试机制
        update_success = False
        for retry in range(3):
            try:
                async with write_lock:
                    task.progress = 100
                    task.status = 'completed'
                    task.error_message = None
                    task.completed_at = datetime.now()
                    await db_session.commit()
                    update_success = True
                    logger.info(f"✅ 章节分析完成: {chapter_id}, 提取{len(memories)}条记忆")
                    break
            except Exception as commit_error:
                logger.error(f"❌ 提交任务完成状态失败(重试{retry+1}/3): {str(commit_error)}")
                if retry < 2:
                    await asyncio.sleep(0.1)
                else:
                    logger.error(f"❌ 无法更新任务为completed状态: {task_id}")
                    # 即使失败也不抛出异常，因为分析本身已经完成
        
        if not update_success:
            logger.warning(f"⚠️  章节分析完成但状态更新失败: {chapter_id}")
        
        # 返回成功状态
        return True
        
    except Exception as e:
        logger.error(f"❌ 后台分析异常: {str(e)}", exc_info=True)
        # 确保任务状态被更新为failed（写操作，需要锁）
        if db_session:
            # 多次重试更新任务状态
            for retry in range(3):
                try:
                    async with write_lock:
                        # 重新获取任务（可能是旧会话导致的问题）
                        task_result = await db_session.execute(
                            select(AnalysisTask).where(AnalysisTask.id == task_id)
                        )
                        task = task_result.scalar_one_or_none()
                        if task:
                            task.status = 'failed'
                            task.error_message = str(e)[:500]
                            task.completed_at = datetime.now()
                            task.progress = 0
                            await db_session.commit()
                            logger.info(f"✅ 任务状态已更新为failed: {task_id} (重试{retry+1}次)")
                            break
                        else:
                            logger.error(f"❌ 无法找到任务进行状态更新: {task_id}")
                            break
                except Exception as update_error:
                    logger.error(f"❌ 更新任务状态失败(重试{retry+1}/3): {str(update_error)}")
                    if retry < 2:
                        await asyncio.sleep(0.1)  # 短暂等待后重试
                    else:
                        logger.error(f"❌ 任务状态更新失败，已达到最大重试次数: {task_id}")
        
        # 返回失败状态
        return False
        
    finally:
        if db_session:
            await db_session.close()



