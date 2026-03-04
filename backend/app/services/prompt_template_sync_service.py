"""Managed prompt template sync helpers.

This module keeps selected high-impact templates aligned with the latest
system defaults without overriding real user customizations.

Strategy:
- Only sync managed template keys.
- Only sync when user template content hash matches a known legacy default hash.
- Never sync when content has diverged from both current and known legacy defaults.
"""
from __future__ import annotations

import json
import hashlib
from dataclasses import dataclass
from typing import Any, Dict, Optional, Set

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.prompt_template import PromptTemplate

logger = get_logger(__name__)


@dataclass(frozen=True)
class TemplateSyncRule:
    """Auto-sync rule for a managed template key."""

    legacy_hashes: Set[str]


# Keep this small and explicit. Add entries only when there is a verified need.
MANAGED_TEMPLATE_SYNC_RULES: Dict[str, TemplateSyncRule] = {
    # Legacy hashes from previous AI_DENOISING defaults.
    # If users still hold an untouched legacy copy, auto-upgrade to latest.
    "AI_DENOISING": TemplateSyncRule(
        legacy_hashes={"c40c0f000310940c", "15d418f68d68f7a6"},
    ),
    # 第三版融合（主线模板）：当用户副本仍是历史默认内容时，自动升级到最新约束版本。
    "OUTLINE_CREATE": TemplateSyncRule(
        legacy_hashes={"3dac3838a8989e40"},
    ),
    "OUTLINE_CONTINUE": TemplateSyncRule(
        legacy_hashes={"32b6f497d1ba1fcb"},
    ),
    "CHAPTER_GENERATION_ONE_TO_MANY": TemplateSyncRule(
        legacy_hashes={"fdfdb8c6b7619804"},
    ),
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": TemplateSyncRule(
        legacy_hashes={"9e3c0ccc044ffa31"},
    ),
    "CHAPTER_GENERATION_ONE_TO_ONE": TemplateSyncRule(
        legacy_hashes={"4ccddf983a56e3e9"},
    ),
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": TemplateSyncRule(
        legacy_hashes={"cc18fe04ad17ac2b"},
    ),
    "PARTIAL_REGENERATE": TemplateSyncRule(
        legacy_hashes={"d4fc0961075e426e"},
    ),
    "PLOT_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"dee21de2491c9c8b"},
    ),
    "OUTLINE_EXPAND_SINGLE": TemplateSyncRule(
        legacy_hashes={"819bf64ee54d5efe"},
    ),
    "OUTLINE_EXPAND_MULTI": TemplateSyncRule(
        legacy_hashes={"5d90dbf35d2f3910"},
    ),
    "AUTO_CHARACTER_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"5f75f423f3f7effd"},
    ),
    "AUTO_ORGANIZATION_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"b85b57b470de0457"},
    ),
    # 灵感模式模板：当用户仍是旧版系统默认副本时，自动升级到新版低AI生活化约束。
    "INSPIRATION_TITLE_SYSTEM": TemplateSyncRule(
        legacy_hashes={"174116db28ffe2cd", "ff42cc2ceac62d6c", "dd4dcec1cc7a4a89"},
    ),
    "INSPIRATION_TITLE_USER": TemplateSyncRule(
        legacy_hashes={"95f56808897a03a1"},
    ),
    "INSPIRATION_DESCRIPTION_SYSTEM": TemplateSyncRule(
        legacy_hashes={"5d3349db809fe56f", "6027777447bb0156", "261b325cf3577e5f"},
    ),
    "INSPIRATION_DESCRIPTION_USER": TemplateSyncRule(
        legacy_hashes={"f0af139f8db3e24a"},
    ),
    "INSPIRATION_THEME_SYSTEM": TemplateSyncRule(
        legacy_hashes={"77fcd61b59597687", "9d784b8ce9a66177", "b83916e2eeab0f6d"},
    ),
    "INSPIRATION_THEME_USER": TemplateSyncRule(
        legacy_hashes={"0c691111be925e7a"},
    ),
    "INSPIRATION_GENRE_SYSTEM": TemplateSyncRule(
        legacy_hashes={"d7e550414969ffe2", "9258f1c8c0bfd99d", "8099dc8af8705f9b"},
    ),
    "INSPIRATION_GENRE_USER": TemplateSyncRule(
        legacy_hashes={"c3a2ff6230a69e45"},
    ),
    "INSPIRATION_QUICK_COMPLETE": TemplateSyncRule(
        legacy_hashes={"90d7050767ec4423", "859f27b2cd8f694e", "88892302ac1e0715"},
    ),
}


def managed_template_keys() -> list[str]:
    """Return managed template keys for sync checks."""

    return list(MANAGED_TEMPLATE_SYNC_RULES.keys())


def calculate_content_hash(content: str) -> str:
    """Compute a stable short hash for template content."""

    normalized = (content or "").strip()
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()[:16]


def build_template_sync_status(
    *,
    template_key: str,
    system_template_content: Optional[str],
    system_template_info: Optional[Dict[str, Any]] = None,
    user_template: Optional[PromptTemplate] = None,
) -> Dict[str, Any]:
    """Build sync status payload for frontend rendering.

    Status values:
    - system_default: user has no custom template and uses system default.
    - up_to_date: user custom template exists and matches system default.
    - legacy_default: user custom template is an old known default copy.
    - customized: user custom template differs from system and is user-modified.
    - system_template_missing: template key has no current system default content.
    """

    template_name = (
        (system_template_info or {}).get("template_name")
        or (system_template_info or {}).get("name")
        or template_key
    )
    category = (system_template_info or {}).get("category")
    system_hash = calculate_content_hash(system_template_content or "")

    if not system_template_content:
        return {
            "template_key": template_key,
            "template_name": template_name,
            "category": category,
            "has_custom_template": user_template is not None,
            "is_active": bool(user_template.is_active) if user_template else True,
            "sync_status": "system_template_missing",
            "is_diff_from_system": False,
            "is_legacy_default": False,
            "can_auto_sync": False,
            "can_sync_to_default": user_template is not None,
            "user_content_hash": calculate_content_hash(user_template.template_content) if user_template else None,
            "system_content_hash": None,
            "updated_at": user_template.updated_at if user_template else None,
        }

    if user_template is None:
        return {
            "template_key": template_key,
            "template_name": template_name,
            "category": category,
            "has_custom_template": False,
            "is_active": True,
            "sync_status": "system_default",
            "is_diff_from_system": False,
            "is_legacy_default": False,
            "can_auto_sync": False,
            "can_sync_to_default": False,
            "user_content_hash": None,
            "system_content_hash": system_hash,
            "updated_at": None,
        }

    user_hash = calculate_content_hash(user_template.template_content or "")
    is_diff = user_hash != system_hash

    rule = MANAGED_TEMPLATE_SYNC_RULES.get(template_key)
    is_legacy_default = bool(rule and is_diff and user_hash in rule.legacy_hashes)
    can_auto_sync = is_legacy_default

    if not is_diff:
        sync_status = "up_to_date"
    elif is_legacy_default:
        sync_status = "legacy_default"
    else:
        sync_status = "customized"

    return {
        "template_key": template_key,
        "template_name": template_name,
        "category": category,
        "has_custom_template": True,
        "is_active": bool(user_template.is_active),
        "sync_status": sync_status,
        "is_diff_from_system": is_diff,
        "is_legacy_default": is_legacy_default,
        "can_auto_sync": can_auto_sync,
        "can_sync_to_default": True,
        "user_content_hash": user_hash,
        "system_content_hash": system_hash,
        "updated_at": user_template.updated_at,
    }


async def sync_managed_template_if_legacy(
    *,
    db: AsyncSession,
    user_id: str,
    template_key: str,
    system_template_content: Optional[str],
    system_template_info: Optional[Dict[str, Any]] = None,
) -> bool:
    """Sync one managed user template when it still matches legacy default.

    Returns:
    - True: database row updated and committed
    - False: no change
    """

    if not user_id or not template_key:
        return False

    rule = MANAGED_TEMPLATE_SYNC_RULES.get(template_key)
    if not rule:
        return False

    if not system_template_content:
        return False

    result = await db.execute(
        select(PromptTemplate).where(
            PromptTemplate.user_id == user_id,
            PromptTemplate.template_key == template_key,
        )
    )
    user_template = result.scalar_one_or_none()
    if not user_template:
        return False

    current_hash = calculate_content_hash(user_template.template_content or "")
    if current_hash not in rule.legacy_hashes:
        return False

    normalized_user = (user_template.template_content or "").strip()
    normalized_system = system_template_content.strip()
    if normalized_user == normalized_system:
        return False

    user_template.template_content = system_template_content

    # Refresh metadata from current system definition when available.
    if system_template_info:
        system_name = system_template_info.get("template_name")
        if system_name:
            user_template.template_name = system_name

        system_desc = system_template_info.get("description")
        if system_desc is not None:
            user_template.description = system_desc

        system_category = system_template_info.get("category")
        if system_category is not None:
            user_template.category = system_category

        system_parameters = system_template_info.get("parameters")
        if system_parameters is not None:
            user_template.parameters = json.dumps(system_parameters, ensure_ascii=False)

    await db.commit()
    logger.info(
        "Synced managed template from legacy default: user_id=%s, template_key=%s, from_hash=%s",
        user_id,
        template_key,
        current_hash,
    )
    return True
