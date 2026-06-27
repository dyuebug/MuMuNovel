"""Test-only support for retired chapter regeneration prompt assembly."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Dict, Optional

from tests.test_support.prompt_template_facade_test_support import get_template_for_owner
from tests.test_support.story_prompt_block_test_support import build_quality_runtime_blocks
from tests.test_support.prompt_template_render_test_support import (
    inject_quality_contract,
    prepare_template_content,
)

_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)


def _load_regeneration_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_key = "CHAPTER_REGENERATION_SYSTEM"
    match = re.search(
        rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
        source,
        flags=re.MULTILINE | re.DOTALL,
    )
    if not match:
        raise RuntimeError(f"regeneration prompt test support 未找到模板常量: {template_key}")
    return {template_key: match.group(1)}


def _regeneration_template_lookup(template_key: str) -> str | None:
    return _load_regeneration_prompt_template_map().get(template_key)


async def build_chapter_regeneration_prompt(
    *,
    chapter_number: int,
    title: str,
    word_count: int,
    content: str,
    modification_instructions: str,
    project_context: Dict[str, Any],
    style_content: str,
    target_word_count: int,
    user_id: Optional[str] = None,
    db: Any = None,
) -> str:
    """构建章节重写提示词（支持用户自定义模板）。"""
    default_system_template = _regeneration_template_lookup(
        "CHAPTER_REGENERATION_SYSTEM"
    )
    if default_system_template is None:
        raise RuntimeError("regeneration prompt test support 缺少系统模板")

    if user_id and db:
        system_template = await get_template_for_owner(
            "CHAPTER_REGENERATION_SYSTEM",
            user_id,
            db,
            template_lookup=_regeneration_template_lookup,
        )
    else:
        system_template = prepare_template_content(
            "CHAPTER_REGENERATION_SYSTEM",
            default_system_template,
        )

    prompt_parts = [system_template]
    prompt_parts.append(
        f"""## 📖 原始章节信息

**章节**：第{chapter_number}章
**标题**：{title}
**字数**：{word_count}字

**原始内容**：
{content}

---
"""
    )
    prompt_parts.append(modification_instructions)
    prompt_parts.append("\n---\n")
    prompt_parts.append(
        f"""## 🌍 项目背景信息

**小说标题**：{project_context.get('project_title', '未知')}
**题材**：{project_context.get('genre', '未设定')}
**主题**：{project_context.get('theme', '未设定')}
**叙事视角**：{project_context.get('narrative_perspective', '第三人称')}
**世界观设定**：
- 时代背景：{project_context.get('time_period', '未设定')}
- 地理位置：{project_context.get('location', '未设定')}
- 氛围基调：{project_context.get('atmosphere', '未设定')}

---
"""
    )

    if project_context.get("characters_info"):
        prompt_parts.append(
            f"""## 👥 角色信息

{project_context['characters_info']}

---
"""
        )

    if project_context.get("chapter_outline"):
        prompt_parts.append(
            f"""## 📝 本章大纲

{project_context['chapter_outline']}

---
"""
        )

    if project_context.get("previous_context"):
        prompt_parts.append(
            f"""## 📚 前置章节上下文

{project_context['previous_context']}

---
"""
        )

    if style_content:
        prompt_parts.append(
            f"""## 🎨 写作风格要求

{style_content}

请在重新创作时贴合上述写作风格。

---
"""
        )

    prompt_parts.append(
        f"""## ✨ 创作要求

1. **解决问题**：针对上述修改指令中提到的所有问题进行改进
2. **保持连贯**：确保与前后章节的情节、人物、风格保持一致
3. **提升质量**：在节奏、情感、描写等方面明显优于原版
4. **保留精华**：保持原章节中优秀的部分和关键情节
5. **字数控制**：目标字数约{target_word_count}字（可适当浮动±20%）
{f'6. **风格一致**：按上述写作风格创作，语气保持自然' if style_content else ''}

---

## 🎬 开始创作

请现在开始创作改进后的新版本章节内容。

**重要提示**：
- 直接输出章节正文内容，从故事内容开始写
- **不要**输出章节标题（如"第X章"、"第X章：XXX"等）
- **不要**输出任何额外的说明、注释或元数据
- 只需要纯粹的故事正文内容

现在开始：
"""
    )

    prompt_text = "\n".join(prompt_parts)
    quality_kwargs = project_context.get("prompt_quality_kwargs") or {
        "genre": project_context.get("genre"),
        "style_name": project_context.get("style_name"),
        "style_preset_id": project_context.get("style_preset_id"),
        "style_content": style_content,
        "external_assets": project_context.get("external_assets"),
        "reference_assets": project_context.get("reference_assets"),
        "mcp_references": project_context.get("mcp_references"),
        "mcp_guard": project_context.get("mcp_guard"),
    }
    return inject_quality_contract(
        template=prompt_text,
        template_key="CHAPTER_REGENERATION_SYSTEM",
        build_quality_runtime_blocks=build_quality_runtime_blocks,
        **quality_kwargs,
    )
