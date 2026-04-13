"""Analysis support helpers shared by chapter analysis flows."""

from __future__ import annotations

import json
from asyncio import Lock
from datetime import datetime
from typing import Any, Dict, List, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.services.ai_service import AIService
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_quality_context_service import (
    StoryGenerationGuidance,
    build_prompt_quality_kwargs,
)
from app.services.prompt_service import PromptService

logger = get_logger(__name__)

db_write_locks: dict[str, Lock] = {}


async def get_chapter_analysis_write_lock(user_id: str) -> Lock:
    lock = db_write_locks.get(user_id)
    if lock is None:
        lock = Lock()
        db_write_locks[user_id] = lock
    return lock


def normalize_checker_result(raw: Optional[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
    """规范化正文质检结果，确保字段稳定。"""
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
    """将结构化质检结果转为可读摘要文本。"""
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
    """合并分析建议和质检建议，去重后输出。"""
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
    """提取自动修订高优先问题文本，作为自动修订输入。"""
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
    db_session: AsyncSession,
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
    """运行第三版正文质检。失败时返回None，不阻断主流程。"""
    try:
        template = await PromptService.get_template("CHAPTER_TEXT_CHECKER", user_id, db_session)
        if not template:
            template = PromptService.CHAPTER_TEXT_CHECKER

        prompt = PromptService.format_prompt(
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
    db_session: AsyncSession,
    user_id: str,
    chapter_number: int,
    chapter_title: str,
    chapter_content: str,
    checker_result: Dict[str, Any],
    quality_profile: Optional[Dict[str, Any]] = None,
    generation_guidance: Optional[StoryGenerationGuidance] = None,
) -> Optional[Dict[str, Any]]:
    """根据质检结果生成自动修订草稿，仅建议，不覆盖正文。"""
    counts = (checker_result or {}).get("severity_counts") or {}
    critical_count = int(counts.get("critical") or 0)
    major_count = int(counts.get("major") or 0)
    priority_issue_count = critical_count + major_count
    if priority_issue_count <= 0:
        return None

    try:
        template = await PromptService.get_template("CHAPTER_TEXT_REVISER", user_id, db_session)
        if not template:
            template = PromptService.CHAPTER_TEXT_REVISER

        checker_json = json.dumps(checker_result, ensure_ascii=False)
        prompt = PromptService.format_prompt(
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
