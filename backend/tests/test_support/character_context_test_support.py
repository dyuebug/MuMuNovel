from __future__ import annotations

from typing import TYPE_CHECKING, Optional

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.character import Character

logger = get_logger(__name__)


async def build_characters_info_with_careers(
    db: "AsyncSession",
    project_id: str,
    characters: list["Character"],
    filter_character_names: Optional[list[str]] = None,
) -> str:
    """构建角色上下文与关联信息。"""
    from sqlalchemy import or_, select

    from migrator_app.models import (
        Career,
        CharacterCareer,
        CharacterRelationship,
        Organization,
        OrganizationMember,
    )
    from migrator_app.models.character import Character

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


