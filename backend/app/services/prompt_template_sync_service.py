"""Managed prompt template sync helpers.

This module keeps selected high-impact templates aligned with the latest
system defaults without overriding real user customizations.

Strategy:
- Only sync managed template keys.
- Only sync when user template content hash matches a known legacy default hash.
- Known current hashes are tracked explicitly so latest managed defaults never get
  mistaken for legacy copies.
- Never sync when content has diverged from both current and known legacy defaults.
"""
from __future__ import annotations

import json
import hashlib
from dataclasses import dataclass, field
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
    current_hashes: Set[str] = field(default_factory=set)

    def is_legacy_hash(self, content_hash: str) -> bool:
        return bool(content_hash) and content_hash in self.legacy_hashes and content_hash not in self.current_hashes


NOVEL_QUALITY_SYSTEM_PHASE1_TEMPLATE_KEYS: tuple[str, ...] = (
    "CHAPTER_GENERATION_ONE_TO_MANY",
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
    "CHAPTER_GENERATION_ONE_TO_ONE",
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    "CHAPTER_REGENERATION_SYSTEM",
    "PARTIAL_REGENERATE",
    "PLOT_ANALYSIS",
    "CHAPTER_TEXT_CHECKER",
    "CHAPTER_TEXT_REVISER",
)


# Keep this small and explicit. Add entries only when there is a verified need.
MANAGED_TEMPLATE_SYNC_RULES: Dict[str, TemplateSyncRule] = {
    # Legacy hashes from previous AI_DENOISING defaults.
    # If users still hold an untouched legacy copy, auto-upgrade to latest.
    "AI_DENOISING": TemplateSyncRule(
        legacy_hashes={"c40c0f000310940c", "15d418f68d68f7a6", "bec75d1005770577", "ec48a8f6a4c5797f"},
    ),
    # 第三版融合（主线模板）：当用户副本仍是历史默认内容时，自动升级到最新约束版本。
    "OUTLINE_CREATE": TemplateSyncRule(
        legacy_hashes={"3dac3838a8989e40", "8403d46239ac21f9", "0df51e236ddb2cf6"},
    ),
    "OUTLINE_CONTINUE": TemplateSyncRule(
        legacy_hashes={"32b6f497d1ba1fcb", "fbf485bc5ca6c990", "30348222dfec7056"},
    ),
    # novel-quality-system 一期：章节生成 / 再生成 / 分析 / checker / reviser
    # 旧 current hash 进入 legacy_hashes，确保仍持有上一版系统副本的用户可继续自动升级。
    "CHAPTER_GENERATION_ONE_TO_MANY": TemplateSyncRule(
        legacy_hashes={"fdfdb8c6b7619804", "f305d7120838aecd", "2a96431c1531b106", "13c9d99619a23a1c"},
        current_hashes={"d2bbe814848e5f9f"},
    ),
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": TemplateSyncRule(
        legacy_hashes={"9e3c0ccc044ffa31", "f0d0bb2c56983aed", "67a9162db2ebc58d", "432774bf3dce35bd"},
        current_hashes={"b2be5d86131f2ecb"},
    ),
    "CHAPTER_GENERATION_ONE_TO_ONE": TemplateSyncRule(
        legacy_hashes={"4ccddf983a56e3e9", "31a22bb8610a4cf1", "47f61ef943be70f0", "7f142c0d96b09f46"},
        current_hashes={"b6d8b5e856fd31b2"},
    ),
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": TemplateSyncRule(
        legacy_hashes={"cc18fe04ad17ac2b", "93979c953dcb6e64", "81d34cc949d6cbb5", "84052e4bffec16b5"},
        current_hashes={"b0e14d751b144ae4"},
    ),
    "PARTIAL_REGENERATE": TemplateSyncRule(
        legacy_hashes={"d4fc0961075e426e", "ed4e95fe9b82e4aa", "9da113b6496d8c12", "a68a83eac483fac3", "e67146339ab8be86"},
        current_hashes={"ef62c3f7a00746df"},
    ),
    "PLOT_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"dee21de2491c9c8b", "6048e506dedf3507", "a9535bf99e031fce"},
        current_hashes={"d2ca69f9780cad1c"},
    ),
    "CHAPTER_TEXT_CHECKER": TemplateSyncRule(
        legacy_hashes={"a001c74bdb617ddd", "2a45fbfc19da1bad"},
        current_hashes={"dcd2871dd7d5c7ad"},
    ),
    "CHAPTER_TEXT_REVISER": TemplateSyncRule(
        legacy_hashes={"5dcb73fff31d9af8"},
        current_hashes={"2de875f700923c91"},
    ),
    "OUTLINE_EXPAND_SINGLE": TemplateSyncRule(
        legacy_hashes={"819bf64ee54d5efe", "5bf412df5065cdb7"},
    ),
    "OUTLINE_EXPAND_MULTI": TemplateSyncRule(
        legacy_hashes={"5d90dbf35d2f3910", "731e6cef36332699"},
    ),
    "AUTO_CHARACTER_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"5f75f423f3f7effd", "fc86886d86feff26"},
    ),
    "AUTO_ORGANIZATION_ANALYSIS": TemplateSyncRule(
        legacy_hashes={"b85b57b470de0457", "f4d2e73b6dde0acd"},
    ),
    # 第三版融合（角色/组织/重写/职业扩展模板）
    "SINGLE_CHARACTER_GENERATION": TemplateSyncRule(
        legacy_hashes={"1c8caffaf3e86c1a", "472e253bfcaf0bb3"},
    ),
    "SINGLE_ORGANIZATION_GENERATION": TemplateSyncRule(
        legacy_hashes={"616ecf71799a153c", "d13588f0a44b4bb7"},
    ),
    "CHAPTER_REGENERATION_SYSTEM": TemplateSyncRule(
        legacy_hashes={"18cf26ca10811026", "7cf4aad2f74199ab", "bd65d4128fbeac8e", "0404450893fe057a"},
        current_hashes={"9602e895b5922da3"},
    ),
    "AUTO_CHARACTER_GENERATION": TemplateSyncRule(
        legacy_hashes={"0a789aa309633332", "4c34ae66825d3a36"},
    ),
    "AUTO_ORGANIZATION_GENERATION": TemplateSyncRule(
        legacy_hashes={"da5600e87099ab21", "58ffce61242e00f1"},
    ),
    "CAREER_SYSTEM_GENERATION": TemplateSyncRule(
        legacy_hashes={"d2d1b3971bcb05fc", "4a147e0226b82a66"},
    ),
    # 灵感模式模板：当用户仍是旧版系统默认副本时，自动升级到新版低AI生活化约束。
    "INSPIRATION_TITLE_SYSTEM": TemplateSyncRule(
        legacy_hashes={"174116db28ffe2cd", "ff42cc2ceac62d6c", "dd4dcec1cc7a4a89", "46d0630fcc38b042", "c6c6015d47c2ced6"},
    ),
    "INSPIRATION_TITLE_USER": TemplateSyncRule(
        legacy_hashes={"95f56808897a03a1", "17f7f36eed5bbf7f"},
    ),
    "INSPIRATION_DESCRIPTION_SYSTEM": TemplateSyncRule(
        legacy_hashes={"5d3349db809fe56f", "6027777447bb0156", "261b325cf3577e5f", "b4d06bf965dda8a6", "f4771e281767a9c6"},
    ),
    "INSPIRATION_DESCRIPTION_USER": TemplateSyncRule(
        legacy_hashes={"f0af139f8db3e24a", "0363da89559d6cb2"},
    ),
    "INSPIRATION_THEME_SYSTEM": TemplateSyncRule(
        legacy_hashes={"77fcd61b59597687", "9d784b8ce9a66177", "b83916e2eeab0f6d", "044b05ddb80e4729", "5e06f60ed7d63038"},
    ),
    "INSPIRATION_THEME_USER": TemplateSyncRule(
        legacy_hashes={"0c691111be925e7a", "6c2b5a57b8319894"},
    ),
    "INSPIRATION_GENRE_SYSTEM": TemplateSyncRule(
        legacy_hashes={"d7e550414969ffe2", "9258f1c8c0bfd99d", "8099dc8af8705f9b", "48e61aac5ed73b34", "cc08044e29ce7810"},
    ),
    "INSPIRATION_GENRE_USER": TemplateSyncRule(
        legacy_hashes={"c3a2ff6230a69e45", "e082b217449e54dd"},
    ),
    "INSPIRATION_QUICK_COMPLETE": TemplateSyncRule(
        legacy_hashes={"90d7050767ec4423", "859f27b2cd8f694e", "88892302ac1e0715", "b533c5afae9272ba", "527956ae06b2ae95"},
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
    is_legacy_default = bool(rule and is_diff and rule.is_legacy_hash(user_hash))
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
    if not rule.is_legacy_hash(current_hash):
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
