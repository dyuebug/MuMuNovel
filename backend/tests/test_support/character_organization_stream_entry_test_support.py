from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
import re
from typing import Any, AsyncGenerator, Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.api_common_test_support import verify_project_access
from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models import (
    Career,
    CharacterCareer,
    CharacterRelationship,
    Organization,
    OrganizationMember,
    RelationshipType,
)
from migrator_app.models.character import Character
from migrator_app.models import GenerationHistory
from migrator_app.models.project import Project
from tests.test_support.character_schema_test_support import CharacterGenerateRequest
from tests.test_support.ai_gateway.ai_service import AIService
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)
from tests.test_support.utils.sse_response import SSEResponse, WizardProgressTracker

logger = get_logger(__name__)
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)


@lru_cache(maxsize=1)
def _load_character_organization_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = (
        "SINGLE_CHARACTER_GENERATION",
        "SINGLE_ORGANIZATION_GENERATION",
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
                f"character/organization test support 未找到模板常量: {template_key}"
            )
        templates[template_key] = match.group(1)
    return templates


def _character_organization_template_lookup(template_key: str) -> Optional[str]:
    return _load_character_organization_prompt_template_map().get(template_key)


async def _default_get_character_organization_template(
    template_key: str,
    user_id: Optional[str],
    db: AsyncSession,
):
    return await get_template_for_owner(
        template_key,
        user_id,
        db,
        template_lookup=_character_organization_template_lookup,
    )


def _default_format_character_organization_prompt(template: str, **kwargs) -> str:
    return _facade_format_prompt(template, **kwargs)


class PromptService:
    get_template = staticmethod(_default_get_character_organization_template)
    format_prompt = staticmethod(_default_format_character_organization_prompt)


async def get_template(*args, **kwargs):
    return await _default_get_character_organization_template(*args, **kwargs)


def format_prompt(*args, **kwargs):
    return _default_format_character_organization_prompt(*args, **kwargs)


_ORIGINAL_PROMPTSERVICE_GET_TEMPLATE = PromptService.get_template
_ORIGINAL_PROMPTSERVICE_FORMAT_PROMPT = PromptService.format_prompt


async def _get_character_organization_template(
    template_key: str,
    user_id: Optional[str],
    db: AsyncSession,
):
    patched_impl = globals().get("get_template")
    if patched_impl is None:
        raise RuntimeError("character/organization get_template 未定义")
    return await patched_impl(template_key, user_id, db)


def _format_character_organization_prompt(template: str, **kwargs) -> str:
    patched_impl = globals().get("format_prompt")
    if patched_impl is None:
        raise RuntimeError("character/organization format_prompt 未定义")
    return patched_impl(template, **kwargs)


@dataclass(slots=True)
class OrganizationGenerateRequest:
    project_id: str
    name: Optional[str] = None
    organization_type: Optional[str] = None
    background: Optional[str] = None
    requirements: Optional[str] = None
    enable_mcp: bool = True

    @classmethod
    def model_validate(cls, data: dict[str, Any]) -> "OrganizationGenerateRequest":
        project_id = str(data.get("project_id") or "").strip()
        if not project_id:
            raise ValueError("project_id is required")

        def optional_text(key: str) -> Optional[str]:
            value = data.get(key)
            if value is None:
                return None
            text = str(value).strip()
            return text or None

        enable_mcp_raw = data.get("enable_mcp", True)
        enable_mcp = enable_mcp_raw if isinstance(enable_mcp_raw, bool) else str(
            enable_mcp_raw
        ).strip().lower() not in {"0", "false", "no", ""}

        return cls(
            project_id=project_id,
            name=optional_text("name"),
            organization_type=optional_text("organization_type"),
            background=optional_text("background"),
            requirements=optional_text("requirements"),
            enable_mcp=enable_mcp,
        )


def _build_existing_character_context(existing_characters: list[Character]) -> str:
    character_list: list[str] = []
    organization_list: list[str] = []

    for character in existing_characters[:10]:
        if character.is_organization:
            organization_list.append(
                f"- {character.name} [{character.organization_type or '组织'}]"
            )
        else:
            character_list.append(f"- {character.name}（{character.role_type or '未知'}）")

    sections: list[str] = []
    if character_list:
        sections.append("已有角色：\n" + "\n".join(character_list))
    if organization_list:
        sections.append("已有组织：\n" + "\n".join(organization_list))
    if not sections:
        return ""
    return "\n" + "\n\n".join(sections)


def _build_character_careers_context(careers: list[Career]) -> str:
    if not careers:
        return "\n\n⚠️ 项目中暂无职业设定"

    main_careers = [career for career in careers if career.type == "main"]
    sub_careers = [career for career in careers if career.type == "sub"]
    sections: list[str] = []

    if main_careers:
        lines = ["可用主职业列表（请在career_info中填写职业名称，系统会自动匹配ID）："]
        for career in main_careers:
            try:
                stages = json.loads(career.stages) if career.stages else []
                stage_names = [
                    stage.get("name", f'阶段{stage.get("level")}')
                    for stage in stages[:3]
                ]
                stage_info = " → ".join(stage_names)
                if len(stages) > 3:
                    stage_info += " → ..."
            except Exception:
                stage_info = f"共{career.max_stage}个阶段"

            description = f", 描述: {career.description[:50]}" if career.description else ""
            lines.append(f"- 名称: {career.name}{description}, 阶段: {stage_info}")
        sections.append("\n".join(lines))

    if sub_careers:
        lines = ["可用副职业列表（请在career_info中填写职业名称，系统会自动匹配ID）："]
        for career in sub_careers[:5]:
            description = f", 描述: {career.description[:50]}" if career.description else ""
            lines.append(f"- 名称: {career.name}{description}")
        sections.append("\n".join(lines))

    return "\n\n" + "\n\n".join(sections)


def _build_project_context(
    project: Project,
    *,
    existing_info: str,
    careers_info: str = "",
) -> str:
    return f"""
项目信息：
- 书名：{project.title}
- 主题：{project.theme or '未设定'}
- 类型：{project.genre or '未设定'}
- 时间背景：{project.world_time_period or '未设定'}
- 地理位置：{project.world_location or '未设定'}
- 氛围基调：{project.world_atmosphere or '未设定'}
- 世界规则：{project.world_rules or '未设定'}
{existing_info}
{careers_info}
"""


def _build_organization_generation_request_options(
    ai_service: AIService,
) -> dict[str, object]:
    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(getattr(retry_cfg, "max_retries", 2) or 2)
    provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    request_options: dict[str, object] = {
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


async def create_character_generate_stream(
    *,
    data: dict[str, Any] | CharacterGenerateRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
):
    request = data
    if isinstance(data, Mapping):
        request = CharacterGenerateRequest.model_validate(dict(data))

    async def generate() -> AsyncGenerator[str, None]:
        tracker = WizardProgressTracker("角色")
        try:
            user_id = request_user_id
            project = await verify_project_access(request.project_id, user_id, db)

            yield await tracker.start()
            yield await tracker.loading("获取项目上下文...", 0.3)

            existing_chars_result = await db.execute(
                select(Character)
                .where(Character.project_id == request.project_id)
                .order_by(Character.created_at.desc())
            )
            existing_characters = existing_chars_result.scalars().all()
            existing_info = _build_existing_character_context(existing_characters)

            careers_result = await db.execute(
                select(Career)
                .where(Career.project_id == request.project_id)
                .order_by(Career.type, Career.name)
            )
            careers = careers_result.scalars().all()
            careers_info = _build_character_careers_context(careers)

            project_context = _build_project_context(
                project,
                existing_info=existing_info,
                careers_info=careers_info,
            )
            user_input = f"""
用户要求：
- 角色名称：{request.name or '请AI生成'}
- 角色定位：{request.role_type or 'supporting'}
- 背景设定：{request.background or '无特殊要求'}
- 其他要求：{request.requirements or '无'}
"""

            yield await tracker.loading("项目上下文准备完成", 0.7)
            yield await tracker.preparing("构建AI提示词...")

            template = await _get_character_organization_template(
                "SINGLE_CHARACTER_GENERATION",
                user_id,
                db,
            )
            prompt = _format_character_organization_prompt(
                template,
                project_context=project_context,
                user_input=user_input,
            )

            estimated_total = max(3000, len(prompt) * 8)
            yield await tracker.generating(0, estimated_total, "调用AI服务生成角色...")
            logger.info(f"🎯 开始为项目 {request.project_id} 生成角色（SSE流式）")

            try:
                ai_response = ""
                progress_tick = 0

                logger.info("🎯 开始生成角色（流式模式）...")
                yield await tracker.generating(0, estimated_total, "开始生成角色...")

                async for chunk in user_ai_service.generate_text_stream(
                    prompt=prompt,
                    tool_choice="required",
                ):
                    content = chunk.get("content", "") if isinstance(chunk, dict) else chunk
                    if not content:
                        continue

                    ai_response += content
                    yield await SSEResponse.send_chunk(content)

                    current_len = len(ai_response)
                    if current_len >= progress_tick * 500:
                        progress_tick += 1
                        yield await tracker.generating(current_len, estimated_total)

                    if progress_tick % 20 == 0:
                        yield await tracker.heartbeat()

            except Exception as ai_error:
                logger.error(f"❌ AI服务调用异常：{str(ai_error)}")
                yield await tracker.error(f"AI服务调用失败：{str(ai_error)}")
                return

            if not ai_response or not ai_response.strip():
                yield await tracker.error("AI服务返回空响应")
                return

            yield await tracker.parsing("解析AI响应...", 0.5)

            try:
                cleaned_response = user_ai_service._clean_json_response(ai_response)
                character_data = json.loads(cleaned_response)
                logger.info("✅ 角色JSON解析成功")
            except json.JSONDecodeError as error:
                logger.error(f"❌ 角色JSON解析失败: {error}")
                logger.error(f"   原始响应预览: {ai_response[:200]}")
                yield await tracker.error(f"AI返回的内容无法解析为JSON：{str(error)}")
                return

            yield await tracker.saving("创建角色记录...", 0.3)

            traits_json = (
                json.dumps(character_data.get("traits", []), ensure_ascii=False)
                if character_data.get("traits")
                else None
            )
            is_organization = character_data.get("is_organization", False)
            career_info = character_data.get("career_info", {})
            raw_main_career_name = career_info.get("main_career_name") if career_info else None
            main_career_stage = career_info.get("main_career_stage", 1) if career_info else None
            raw_sub_careers_data = career_info.get("sub_careers", []) if career_info else []

            logger.info(f"🔍 提取职业信息 - career_info: {career_info}")
            logger.info(
                f"🔍 raw_main_career_name: {raw_main_career_name}, main_career_stage: {main_career_stage}"
            )
            logger.info(
                f"🔍 raw_sub_careers_data类型: {type(raw_sub_careers_data)}, 内容: {raw_sub_careers_data}"
            )

            main_career_id = None
            sub_careers_data: list[dict[str, Any]] = []

            if raw_main_career_name and not is_organization:
                career_check = await db.execute(
                    select(Career).where(
                        Career.name == raw_main_career_name,
                        Career.project_id == request.project_id,
                        Career.type == "main",
                    )
                )
                matched_career = career_check.scalar_one_or_none()
                if matched_career:
                    main_career_id = matched_career.id
                    logger.info(
                        f"✅ 主职业名称匹配成功: {raw_main_career_name} -> ID: {main_career_id}"
                    )
                else:
                    logger.warning(f"⚠️ AI返回的主职业名称未找到: {raw_main_career_name}")

            if raw_sub_careers_data and not is_organization and isinstance(raw_sub_careers_data, list):
                for sub_data in raw_sub_careers_data[:2]:
                    if not isinstance(sub_data, dict):
                        continue
                    career_name = sub_data.get("career_name")
                    if not career_name:
                        continue

                    career_check = await db.execute(
                        select(Career).where(
                            Career.name == career_name,
                            Career.project_id == request.project_id,
                            Career.type == "sub",
                        )
                    )
                    matched_career = career_check.scalar_one_or_none()
                    if matched_career:
                        sub_careers_data.append(
                            {
                                "career_id": matched_career.id,
                                "stage": sub_data.get("stage", 1),
                            }
                        )
                        logger.info(
                            f"✅ 副职业名称匹配成功: {career_name} -> ID: {matched_career.id}"
                        )
                    else:
                        logger.warning(f"⚠️ AI返回的副职业名称未找到: {career_name}")

            character = Character(
                project_id=request.project_id,
                name=character_data.get("name", request.name or "未命名角色"),
                age=str(character_data.get("age", "")),
                gender=character_data.get("gender"),
                is_organization=is_organization,
                role_type=request.role_type or "supporting",
                personality=character_data.get("personality", ""),
                background=character_data.get("background", ""),
                appearance=character_data.get("appearance", ""),
                organization_type=character_data.get("organization_type")
                if is_organization
                else None,
                organization_purpose=character_data.get("organization_purpose")
                if is_organization
                else None,
                traits=traits_json,
                main_career_id=main_career_id,
                main_career_stage=main_career_stage if main_career_id else None,
                sub_careers=json.dumps(sub_careers_data, ensure_ascii=False)
                if sub_careers_data
                else None,
            )
            db.add(character)
            await db.flush()

            logger.info(f"✅ 角色创建成功：{character.name} (ID: {character.id})")

            if main_career_id and not is_organization:
                career_result = await db.execute(
                    select(Career).where(
                        Career.id == main_career_id,
                        Career.project_id == request.project_id,
                        Career.type == "main",
                    )
                )
                career = career_result.scalar_one_or_none()
                if career:
                    db.add(
                        CharacterCareer(
                            character_id=character.id,
                            career_id=main_career_id,
                            career_type="main",
                            current_stage=main_career_stage,
                            stage_progress=0,
                        )
                    )
                    logger.info(f"✅ AI生成角色-创建主职业关联：{character.name} -> {career.name}")
                else:
                    logger.warning(f"⚠️ AI返回的主职业ID不存在: {main_career_id}")

            if sub_careers_data and not is_organization:
                logger.info(f"🔍 开始处理副职业关联，数据: {sub_careers_data}")
                if not isinstance(sub_careers_data, list):
                    logger.warning(f"⚠️ sub_careers_data不是列表类型: {type(sub_careers_data)}")
                    sub_careers_data = []

                for index, sub_data in enumerate(sub_careers_data[:2]):
                    logger.info(
                        f"🔍 处理第{index + 1}个副职业，数据: {sub_data}, 类型: {type(sub_data)}"
                    )
                    if not isinstance(sub_data, dict):
                        logger.warning(f"⚠️ 副职业数据格式错误，应为dict: {sub_data}")
                        continue

                    career_id = sub_data.get("career_id")
                    stage = sub_data.get("stage", 1)
                    if not career_id:
                        logger.warning("⚠️ 副职业数据缺少career_id字段")
                        continue

                    logger.info(
                        f"🔍 查询副职业: career_id={career_id}, project_id={request.project_id}"
                    )
                    career_result = await db.execute(
                        select(Career).where(
                            Career.id == career_id,
                            Career.project_id == request.project_id,
                            Career.type == "sub",
                        )
                    )
                    career = career_result.scalar_one_or_none()
                    if career:
                        db.add(
                            CharacterCareer(
                                character_id=character.id,
                                career_id=career_id,
                                career_type="sub",
                                current_stage=stage,
                                stage_progress=0,
                            )
                        )
                        logger.info(
                            f"✅ AI生成角色-创建副职业关联：{character.name} -> {career.name} (阶段{stage})"
                        )
                    else:
                        logger.warning(
                            f"⚠️ AI返回的副职业ID不存在: {career_id} (项目ID: {request.project_id})"
                        )

            if is_organization:
                yield await tracker.saving("创建组织详情...", 0.6)
                org_check = await db.execute(
                    select(Organization).where(Organization.character_id == character.id)
                )
                existing_org = org_check.scalar_one_or_none()
                if not existing_org:
                    db.add(
                        Organization(
                            character_id=character.id,
                            project_id=request.project_id,
                            member_count=0,
                            power_level=character_data.get("power_level", 50),
                            location=character_data.get("location"),
                            motto=character_data.get("motto"),
                            color=character_data.get("color"),
                        )
                    )
                    await db.flush()

            if not is_organization:
                relationships_data = character_data.get("relationships", [])
                if relationships_data and isinstance(relationships_data, list):
                    logger.info(f"📊 开始处理 {len(relationships_data)} 条关系数据")
                    created_rels = 0

                    for relationship_data in relationships_data:
                        try:
                            target_name = relationship_data.get("target_character_name")
                            if not target_name:
                                logger.debug("  ⚠️  关系缺少target_character_name，跳过")
                                continue

                            target_result = await db.execute(
                                select(Character).where(
                                    Character.project_id == request.project_id,
                                    Character.name == target_name,
                                )
                            )
                            target_char = target_result.scalar_one_or_none()

                            if not target_char:
                                logger.warning(f"  ⚠️  目标角色不存在：{target_name}")
                                continue

                            existing_rel = await db.execute(
                                select(CharacterRelationship).where(
                                    CharacterRelationship.project_id == request.project_id,
                                    CharacterRelationship.character_from_id == character.id,
                                    CharacterRelationship.character_to_id == target_char.id,
                                )
                            )
                            if existing_rel.scalar_one_or_none():
                                logger.debug(
                                    f"  ℹ️  关系已存在：{character.name} -> {target_name}"
                                )
                                continue

                            relationship = CharacterRelationship(
                                project_id=request.project_id,
                                character_from_id=character.id,
                                character_to_id=target_char.id,
                                relationship_name=relationship_data.get(
                                    "relationship_type",
                                    "未知关系",
                                ),
                                intimacy_level=relationship_data.get("intimacy_level", 50),
                                description=relationship_data.get("description", ""),
                                started_at=relationship_data.get("started_at"),
                                source="ai",
                            )

                            rel_type_result = await db.execute(
                                select(RelationshipType).where(
                                    RelationshipType.name
                                    == relationship_data.get("relationship_type")
                                )
                            )
                            rel_type = rel_type_result.scalar_one_or_none()
                            if rel_type:
                                relationship.relationship_type_id = rel_type.id

                            db.add(relationship)
                            created_rels += 1
                            logger.info(
                                f"  ✅ 创建关系：{character.name} -> {target_name} ({relationship_data.get('relationship_type')})"
                            )
                        except Exception as rel_error:
                            logger.warning(f"  ❌ 创建关系失败：{str(rel_error)}")

                    logger.info(f"✅ 成功创建 {created_rels} 条关系记录")

            if not is_organization:
                org_memberships = character_data.get("organization_memberships", [])
                if org_memberships and isinstance(org_memberships, list):
                    logger.info(f"🏢 开始处理 {len(org_memberships)} 条组织成员关系")
                    created_members = 0

                    for membership in org_memberships:
                        try:
                            org_name = membership.get("organization_name")
                            if not org_name:
                                logger.debug("  ⚠️  组织成员关系缺少organization_name，跳过")
                                continue

                            org_char_result = await db.execute(
                                select(Character).where(
                                    Character.project_id == request.project_id,
                                    Character.name == org_name,
                                    Character.is_organization == True,
                                )
                            )
                            org_char = org_char_result.scalar_one_or_none()

                            if not org_char:
                                logger.warning(f"  ⚠️  组织不存在：{org_name}")
                                continue

                            org_result = await db.execute(
                                select(Organization).where(Organization.character_id == org_char.id)
                            )
                            org = org_result.scalar_one_or_none()
                            if not org:
                                org = Organization(
                                    character_id=org_char.id,
                                    project_id=request.project_id,
                                    member_count=0,
                                )
                                db.add(org)
                                await db.flush()
                                logger.info(f"  ℹ️  自动创建缺失的组织详情：{org_name}")

                            existing_member = await db.execute(
                                select(OrganizationMember).where(
                                    OrganizationMember.organization_id == org.id,
                                    OrganizationMember.character_id == character.id,
                                )
                            )
                            if existing_member.scalar_one_or_none():
                                logger.debug(
                                    f"  ℹ️  成员关系已存在：{character.name} -> {org_name}"
                                )
                                continue

                            db.add(
                                OrganizationMember(
                                    organization_id=org.id,
                                    character_id=character.id,
                                    position=membership.get("position", "成员"),
                                    rank=membership.get("rank", 0),
                                    loyalty=membership.get("loyalty", 50),
                                    joined_at=membership.get("joined_at"),
                                    status=membership.get("status", "active"),
                                    source="ai",
                                )
                            )
                            org.member_count += 1
                            created_members += 1
                            logger.info(
                                f"  ✅ 添加成员：{character.name} -> {org_name} ({membership.get('position')})"
                            )
                        except Exception as org_error:
                            logger.warning(f"  ❌ 添加组织成员失败：{str(org_error)}")

                    logger.info(f"✅ 成功创建 {created_members} 条组织成员记录")

            yield await tracker.saving("保存生成历史...", 0.9)

            db.add(
                GenerationHistory(
                    project_id=request.project_id,
                    prompt=prompt,
                    generated_content=ai_response,
                    model=user_ai_service.default_model,
                )
            )

            await db.commit()
            await db.refresh(character)

            logger.info(f"🎉 成功生成角色: {character.name}")
            yield await tracker.complete("角色生成完成！")
            yield await tracker.result(
                {
                    "character": {
                        "id": character.id,
                        "name": character.name,
                        "role_type": character.role_type,
                        "is_organization": character.is_organization,
                    }
                }
            )
            yield await tracker.done()

        except HTTPException as error:
            logger.error(f"HTTP异常: {error.detail}")
            yield await tracker.error(error.detail, error.status_code)
        except Exception as error:
            logger.error(f"生成角色失败: {str(error)}")
            yield await tracker.error(f"生成角色失败: {str(error)}")

    return generate()


async def create_character_generation_stream(
    *,
    request: CharacterGenerateRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
):
    return await create_character_generate_stream(
        data=request,
        request_user_id=request_user_id,
        db=db,
        user_ai_service=user_ai_service,
    )


async def create_organization_generate_stream(
    *,
    data: dict[str, Any] | OrganizationGenerateRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
):
    request = data
    if isinstance(data, Mapping):
        request = OrganizationGenerateRequest.model_validate(dict(data))

    async def generate() -> AsyncGenerator[str, None]:
        tracker = WizardProgressTracker("组织")
        try:
            user_id = request_user_id
            project = await verify_project_access(request.project_id, user_id, db)

            yield await tracker.start()
            yield await tracker.loading("获取项目上下文...", 0.3)

            existing_chars_result = await db.execute(
                select(Character)
                .where(Character.project_id == request.project_id)
                .order_by(Character.created_at.desc())
            )
            existing_characters = existing_chars_result.scalars().all()
            existing_info = _build_existing_character_context(existing_characters)
            project_context = _build_project_context(project, existing_info=existing_info)

            user_input = f"""
用户要求：
- 组织名称：{request.name or '请AI生成'}
- 组织类型：{request.organization_type or '请AI根据世界观决定'}
- 背景设定：{request.background or '无特殊要求'}
- 其他要求：{request.requirements or '无'}
"""

            yield await tracker.loading("项目上下文准备完成", 0.7)
            yield await tracker.preparing("构建AI提示词...")

            template = await _get_character_organization_template(
                "SINGLE_ORGANIZATION_GENERATION",
                user_id,
                db,
            )
            prompt = _format_character_organization_prompt(
                template,
                project_context=project_context,
                user_input=user_input,
            )

            estimated_total = max(3000, len(prompt) * 8)
            yield await tracker.generating(0, estimated_total, "调用AI服务生成组织...")
            logger.info(f"🎯 开始为项目 {request.project_id} 生成组织（SSE流式）")

            try:
                ai_content = ""
                chunk_count = 0
                request_options = _build_organization_generation_request_options(
                    user_ai_service
                )

                async for chunk in user_ai_service.generate_text_stream(
                    prompt=prompt,
                    request_options=request_options,
                ):
                    ai_content += chunk
                    chunk_count += 1

                    yield await SSEResponse.send_chunk(chunk)
                    if chunk_count % 5 == 0:
                        yield await tracker.generating(len(ai_content), estimated_total)
                    if chunk_count % 20 == 0:
                        yield await tracker.heartbeat()

            except Exception as ai_error:
                logger.error(f"❌ AI服务调用异常：{str(ai_error)}")
                yield await tracker.error(f"AI服务调用失败：{str(ai_error)}")
                return

            if not ai_content or not ai_content.strip():
                yield await tracker.error("AI服务返回空响应")
                return

            yield await tracker.parsing("解析AI响应...", 0.5)

            try:
                cleaned_response = user_ai_service._clean_json_response(ai_content)
                organization_data = json.loads(cleaned_response)
                logger.info("✅ 组织JSON解析成功")
            except json.JSONDecodeError as error:
                logger.error(f"❌ 组织JSON解析失败: {error}")
                logger.error(f"   原始响应预览: {ai_content[:200]}")
                yield await tracker.error(f"AI返回的内容无法解析为JSON：{str(error)}")
                return

            yield await tracker.saving("创建组织记录...", 0.3)

            character = Character(
                project_id=request.project_id,
                name=organization_data.get("name", request.name or "未命名组织"),
                is_organization=True,
                role_type="supporting",
                personality=organization_data.get("personality", ""),
                background=organization_data.get("background", ""),
                appearance=organization_data.get("appearance", ""),
                organization_type=organization_data.get("organization_type"),
                organization_purpose=organization_data.get("organization_purpose"),
                traits=json.dumps(organization_data.get("traits", []), ensure_ascii=False),
            )
            db.add(character)
            await db.flush()

            logger.info(f"✅ 组织角色创建成功：{character.name} (ID: {character.id})")
            yield await tracker.saving("创建组织详情...", 0.6)

            organization = Organization(
                character_id=character.id,
                project_id=request.project_id,
                member_count=0,
                power_level=organization_data.get("power_level", 50),
                location=organization_data.get("location"),
                motto=organization_data.get("motto"),
                color=organization_data.get("color"),
            )
            db.add(organization)
            await db.flush()

            logger.info(f"✅ 组织详情创建成功：{character.name} (Org ID: {organization.id})")
            yield await tracker.saving("保存生成历史...", 0.9)

            db.add(
                GenerationHistory(
                    project_id=request.project_id,
                    prompt=prompt,
                    generated_content=ai_content,
                    model=user_ai_service.default_model,
                )
            )

            await db.commit()
            await db.refresh(character)

            logger.info(f"🎉 成功生成组织: {character.name}")
            yield await tracker.complete("组织生成完成！")
            yield await tracker.result(
                {
                    "character": {
                        "id": character.id,
                        "name": character.name,
                        "organization_type": character.organization_type,
                        "is_organization": character.is_organization,
                    }
                }
            )
            yield await tracker.done()

        except HTTPException as error:
            logger.error(f"HTTP异常: {error.detail}")
            yield await tracker.error(error.detail, error.status_code)
        except Exception as error:
            logger.error(f"生成组织失败: {str(error)}")
            yield await tracker.error(f"生成组织失败: {str(error)}")

    return generate()


async def create_organization_generation_stream(
    *,
    request: OrganizationGenerateRequest,
    request_user_id: Optional[str],
    db: AsyncSession,
    user_ai_service: AIService,
):
    return await create_organization_generate_stream(
        data=request,
        request_user_id=request_user_id,
        db=db,
        user_ai_service=user_ai_service,
    )




