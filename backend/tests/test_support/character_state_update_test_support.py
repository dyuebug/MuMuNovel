from __future__ import annotations

from functools import lru_cache
from importlib import import_module
from typing import TYPE_CHECKING, Any, Dict, List
import uuid

from sqlalchemy import and_, or_, select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from migrator_app.models.character import Character
    from migrator_app.models import CharacterRelationship, Organization, OrganizationMember

logger = get_logger(__name__)


@lru_cache(maxsize=1)
def _character_state_models() -> tuple[type[Any], type[Any], type[Any], type[Any]]:
    character_module = import_module("migrator_app.models.character")
    models_module = import_module("migrator_app.models")
    return (
        character_module.Character,
        models_module.CharacterRelationship,
        models_module.Organization,
        models_module.OrganizationMember,
    )

# 亲密度调整关键词映射
INTIMACY_ADJUSTMENTS = {
    # 正向变化
    "改善": +10, "加深": +15, "信任": +10, "亲近": +15,
    "友好": +10, "认可": +10, "合作": +5, "和解": +20,
    "喜欢": +15, "爱": +20, "尊敬": +10, "感激": +10,
    "好转": +10, "增进": +10, "亲密": +15, "忠诚": +10,
    # 负向变化
    "恶化": -10, "疏远": -15, "背叛": -30, "敌对": -25,
    "矛盾": -10, "冲突": -15, "怀疑": -10, "不信任": -15,
    "厌恶": -20, "仇恨": -25, "决裂": -30, "猜忌": -10,
    "紧张": -5, "破裂": -25, "反目": -25, "嫉妒": -10,
    # 特殊变化
    "初识": 0, "相遇": 0, "结盟": +10, "分离": -5,
}


class CharacterStateUpdateService:
    """测试辅助宿主：根据章节分析结果更新角色心理状态和关系。"""

    @staticmethod
    async def update_from_analysis(
        db: AsyncSession,
        project_id: str,
        character_states: List[Dict[str, Any]],
        chapter_id: str,
        chapter_number: int
    ) -> Dict[str, Any]:
        """根据章节分析结果更新角色状态和关系。"""
        if not character_states:
            logger.info("📋 角色状态列表为空，跳过状态和关系更新")
            return {
                "state_updated_count": 0,
                "relationship_created_count": 0,
                "relationship_updated_count": 0,
                "org_updated_count": 0,
                "changes": []
            }

        result = {
            "state_updated_count": 0,
            "relationship_created_count": 0,
            "relationship_updated_count": 0,
            "org_updated_count": 0,
            "changes": []
        }

        logger.info(f"🔍 开始分析第{chapter_number}章的角色状态、关系和组织变化...")
        Character, _, Organization, _ = _character_state_models()

        all_characters_result = await db.execute(
            select(Character).where(Character.project_id == project_id)
        )
        all_characters = all_characters_result.scalars().all()

        characters_by_name: Dict[str, Character] = {
            character.name: character
            for character in all_characters
            if not character.is_organization
        }

        orgs_result = await db.execute(
            select(Organization).where(Organization.project_id == project_id)
        )
        all_orgs = orgs_result.scalars().all()

        char_id_to_name: Dict[str, str] = {character.id: character.name for character in all_characters}

        org_by_name: Dict[str, Organization] = {}
        for org in all_orgs:
            org_char_name = char_id_to_name.get(org.character_id)
            if org_char_name:
                org_by_name[org_char_name] = org

        for char_state in character_states:
            char_name = char_state.get("character_name")
            if not char_name:
                continue

            character = characters_by_name.get(char_name)
            if not character:
                logger.warning(f"  ⚠️ 角色不存在: {char_name}，跳过状态更新")
                continue

            survival_status = char_state.get("survival_status")
            if survival_status and survival_status in ("deceased", "missing", "retired"):
                await CharacterStateUpdateService._update_survival_status(
                    db=db,
                    project_id=project_id,
                    character=character,
                    new_status=survival_status,
                    chapter_number=chapter_number,
                    key_event=char_state.get("key_event", ""),
                    changes=result["changes"],
                )
                result["state_updated_count"] += 1
                continue

            state_updated = await CharacterStateUpdateService._update_psychological_state(
                character=character,
                char_state=char_state,
                chapter_number=chapter_number,
                changes=result["changes"],
            )
            if state_updated:
                result["state_updated_count"] += 1

            relationship_changes = char_state.get("relationship_changes", {})
            if relationship_changes and isinstance(relationship_changes, dict):
                created, updated = await CharacterStateUpdateService._update_relationships(
                    db=db,
                    project_id=project_id,
                    character=character,
                    relationship_changes=relationship_changes,
                    chapter_number=chapter_number,
                    chapter_id=chapter_id,
                    characters_by_name=characters_by_name,
                    changes=result["changes"],
                )
                result["relationship_created_count"] += created
                result["relationship_updated_count"] += updated

            organization_changes = char_state.get("organization_changes", [])
            if organization_changes and isinstance(organization_changes, list):
                org_updated = await CharacterStateUpdateService._update_organization_memberships(
                    db=db,
                    project_id=project_id,
                    character=character,
                    organization_changes=organization_changes,
                    chapter_number=chapter_number,
                    org_by_name=org_by_name,
                    changes=result["changes"],
                )
                result["org_updated_count"] += org_updated

        total_changes = (
            result["state_updated_count"]
            + result["relationship_created_count"]
            + result["relationship_updated_count"]
            + result["org_updated_count"]
        )
        if total_changes > 0:
            await db.commit()
            logger.info(
                f"✅ 角色状态更新完成: "
                f"心理状态{result['state_updated_count']}个, "
                f"新建关系{result['relationship_created_count']}个, "
                f"更新关系{result['relationship_updated_count']}个, "
                f"组织变动{result['org_updated_count']}个"
            )
        else:
            logger.info("📋 本章没有角色状态或关系变化")

        return result

    @staticmethod
    async def _update_survival_status(
        db: AsyncSession,
        project_id: str,
        character: Character,
        new_status: str,
        chapter_number: int,
        key_event: str,
        changes: List[str],
    ) -> None:
        """更新角色存活状态及级联影响。"""
        status_desc_map = {
            "deceased": "死亡",
            "missing": "失踪",
            "retired": "退场",
        }

        status_desc = status_desc_map.get(new_status, new_status)

        if (
            character.status_changed_chapter is not None
            and chapter_number < character.status_changed_chapter
        ):
            logger.info(
                f"  ⏭️ {character.name} 状态已在第{character.status_changed_chapter}章变更，跳过"
            )
            return

        old_status = character.status or "active"
        character.status = new_status
        character.status_changed_chapter = chapter_number
        character.current_state = f"{status_desc}（第{chapter_number}章）"
        character.state_updated_chapter = chapter_number

        event_desc = f"：{key_event[:50]}" if key_event else ""
        changes.append(f"💀 {character.name} {status_desc}{event_desc}")
        logger.info(f"  💀 {character.name} 状态: {old_status} → {new_status}")
        _, CharacterRelationship, _, OrganizationMember = _character_state_models()

        rels_result = await db.execute(
            select(CharacterRelationship).where(
                and_(
                    CharacterRelationship.project_id == project_id,
                    CharacterRelationship.status == "active",
                    or_(
                        CharacterRelationship.character_from_id == character.id,
                        CharacterRelationship.character_to_id == character.id,
                    ),
                )
            )
        )
        active_rels = rels_result.scalars().all()
        for rel in active_rels:
            rel.status = "past"
            rel.ended_at = f"第{chapter_number}章"
        if active_rels:
            logger.info(f"  📋 {character.name} {status_desc}，{len(active_rels)}条关系标记为past")

        member_status = "deceased" if new_status == "deceased" else "retired"
        members_result = await db.execute(
            select(OrganizationMember).where(
                and_(
                    OrganizationMember.character_id == character.id,
                    OrganizationMember.status == "active",
                )
            )
        )
        active_members = members_result.scalars().all()
        for member in active_members:
            member.status = member_status
            member.left_at = f"第{chapter_number}章"
            member.notes = (
                f"{member.notes or ''}\n[第{chapter_number}章] 角色{status_desc}"
            ).strip()
        if active_members:
            logger.info(
                f"  📋 {character.name} {status_desc}，{len(active_members)}个组织身份标记为{member_status}"
            )

    @staticmethod
    async def _update_psychological_state(
        character: Character,
        char_state: Dict[str, Any],
        chapter_number: int,
        changes: List[str],
    ) -> bool:
        """更新角色心理状态。"""
        state_after = char_state.get("state_after")
        if not state_after:
            return False

        if (
            character.state_updated_chapter is not None
            and chapter_number < character.state_updated_chapter
        ):
            logger.info(
                f"  ⏭️ {character.name} 的心理状态已被第{character.state_updated_chapter}章更新，"
                f"跳过第{chapter_number}章的更新"
            )
            return False

        state_before = char_state.get("state_before", "未知")
        psychological_change = char_state.get("psychological_change", "")

        character.current_state = state_after
        character.state_updated_chapter = chapter_number

        change_desc = f"👤 {character.name} 心理状态: {state_before} → {state_after}"
        if psychological_change:
            change_desc += f" ({psychological_change[:50]})"
        changes.append(change_desc)

        logger.info(f"  ✅ {character.name} 心理状态更新: {state_before} → {state_after}")
        return True

    @staticmethod
    async def _update_relationships(
        db: AsyncSession,
        project_id: str,
        character: Character,
        relationship_changes: Dict[str, Any],
        chapter_number: int,
        chapter_id: str,
        characters_by_name: Dict[str, Character],
        changes: List[str],
    ) -> tuple[int, int]:
        """更新角色关系。"""
        created_count = 0
        updated_count = 0
        _, CharacterRelationship, _, _ = _character_state_models()

        for target_name, change_info in relationship_changes.items():
            try:
                if isinstance(change_info, str):
                    change_desc = change_info
                elif isinstance(change_info, dict):
                    change_desc = change_info.get("change", str(change_info))
                else:
                    change_desc = str(change_info)

                if not change_desc:
                    continue

                target_character = characters_by_name.get(target_name)
                if not target_character:
                    logger.warning(f"  ⚠️ 关系目标角色不存在: {target_name}，跳过")
                    continue

                if character.id == target_character.id:
                    continue

                existing_rel_result = await db.execute(
                    select(CharacterRelationship).where(
                        and_(
                            CharacterRelationship.project_id == project_id,
                            or_(
                                and_(
                                    CharacterRelationship.character_from_id == character.id,
                                    CharacterRelationship.character_to_id == target_character.id,
                                ),
                                and_(
                                    CharacterRelationship.character_from_id == target_character.id,
                                    CharacterRelationship.character_to_id == character.id,
                                ),
                            ),
                        )
                    )
                )
                existing_rel = existing_rel_result.scalar_one_or_none()

                intimacy_delta = CharacterStateUpdateService._calculate_intimacy_delta(change_desc)

                if existing_rel:
                    existing_rel.relationship_name = change_desc

                    chapter_note = f"[第{chapter_number}章] {change_desc}"
                    if existing_rel.description:
                        existing_rel.description = f"{existing_rel.description}\n{chapter_note}"
                    else:
                        existing_rel.description = chapter_note

                    if intimacy_delta != 0:
                        old_intimacy = existing_rel.intimacy_level or 0
                        new_intimacy = max(-100, min(100, old_intimacy + intimacy_delta))
                        existing_rel.intimacy_level = new_intimacy
                        logger.info(
                            f"  📊 {character.name}↔{target_name} 亲密度: "
                            f"{old_intimacy} → {new_intimacy} "
                            f"({'+' if intimacy_delta > 0 else ''}{intimacy_delta})"
                        )

                    updated_count += 1
                    changes.append(f"🔄 {character.name}↔{target_name} 关系更新: {change_desc}")
                    logger.info(f"  ✅ 更新关系: {character.name}↔{target_name} - {change_desc}")

                else:
                    initial_intimacy = max(-100, min(100, 50 + intimacy_delta))

                    new_relationship = CharacterRelationship(
                        id=str(uuid.uuid4()),
                        project_id=project_id,
                        character_from_id=character.id,
                        character_to_id=target_character.id,
                        relationship_type_id=None,
                        relationship_name=change_desc,
                        intimacy_level=initial_intimacy,
                        status="active",
                        description=f"[第{chapter_number}章] {change_desc}",
                        source="analysis",
                    )
                    db.add(new_relationship)

                    created_count += 1
                    changes.append(f"✨ {character.name}→{target_name} 新关系: {change_desc}")
                    logger.info(
                        f"  ✅ 创建关系: {character.name}→{target_name} "
                        f"({change_desc}, 亲密度:{initial_intimacy})"
                    )

            except Exception as item_error:
                logger.error(
                    f"  ❌ 更新 {character.name}→{target_name} 关系失败: {str(item_error)}"
                )

        return created_count, updated_count

    @staticmethod
    async def _update_organization_memberships(
        db: AsyncSession,
        project_id: str,
        character: Character,
        organization_changes: List[Dict[str, Any]],
        chapter_number: int,
        org_by_name: Dict[str, Organization],
        changes: List[str],
    ) -> int:
        """更新角色的组织成员关系。"""
        updated_count = 0
        _, _, _, OrganizationMember = _character_state_models()

        loyalty_adjustments = {
            "提升": +10, "增强": +10, "坚定": +15, "忠心": +15,
            "动摇": -15, "怀疑": -10, "不满": -10, "降低": -10,
            "背叛": -50, "叛变": -50, "反感": -20, "失望": -15,
        }

        for org_change in organization_changes:
            try:
                org_name = org_change.get("organization_name")
                change_type = org_change.get("change_type", "")
                new_position = org_change.get("new_position")
                loyalty_change_desc = org_change.get("loyalty_change", "")
                description = org_change.get("description", "")

                if not org_name:
                    continue

                organization = org_by_name.get(org_name)
                if not organization:
                    logger.warning(f"  ⚠️ 组织不存在: {org_name}，跳过组织变动更新")
                    continue

                existing_member_result = await db.execute(
                    select(OrganizationMember).where(
                        and_(
                            OrganizationMember.organization_id == organization.id,
                            OrganizationMember.character_id == character.id,
                        )
                    )
                )
                existing_member = existing_member_result.scalar_one_or_none()

                loyalty_delta = 0
                if loyalty_change_desc:
                    for keyword, adjustment in loyalty_adjustments.items():
                        if keyword in loyalty_change_desc:
                            loyalty_delta += adjustment
                    loyalty_delta = max(-50, min(50, loyalty_delta))

                if change_type == "joined":
                    if existing_member:
                        if existing_member.status != "active":
                            existing_member.status = "active"
                            existing_member.left_at = None
                            if new_position:
                                existing_member.position = new_position
                            existing_member.notes = (
                                f"{existing_member.notes or ''}\n[第{chapter_number}章] 重新加入: {description}"
                            ).strip()
                            updated_count += 1
                            changes.append(f"🏛️ {character.name} 重新加入 {org_name}")
                            logger.info(f"  ✅ {character.name} 重新加入 {org_name}")
                    else:
                        new_member = OrganizationMember(
                            id=str(uuid.uuid4()),
                            organization_id=organization.id,
                            character_id=character.id,
                            position=new_position or "成员",
                            rank=0,
                            loyalty=max(0, min(100, 50 + loyalty_delta)),
                            status="active",
                            joined_at=f"第{chapter_number}章",
                            source="analysis",
                            notes=f"[第{chapter_number}章] {description}" if description else None,
                        )
                        db.add(new_member)
                        organization.member_count = (organization.member_count or 0) + 1
                        updated_count += 1
                        changes.append(f"🏛️ {character.name} 加入 {org_name}({new_position or '成员'})")
                        logger.info(f"  ✅ {character.name} 加入 {org_name} 为 {new_position or '成员'}")

                elif change_type in ("left", "expelled", "betrayed"):
                    if existing_member and existing_member.status == "active":
                        status_map = {
                            "left": "retired",
                            "expelled": "expelled",
                            "betrayed": "expelled",
                        }
                        existing_member.status = status_map.get(change_type, "retired")
                        existing_member.left_at = f"第{chapter_number}章"
                        if loyalty_delta != 0:
                            existing_member.loyalty = max(
                                0,
                                min(100, (existing_member.loyalty or 50) + loyalty_delta),
                            )
                        existing_member.notes = (
                            f"{existing_member.notes or ''}\n[第{chapter_number}章] {change_type}: {description}"
                        ).strip()
                        updated_count += 1
                        type_desc = {"left": "离开", "expelled": "被开除", "betrayed": "叛变"}
                        changes.append(f"🏛️ {character.name} {type_desc.get(change_type, change_type)} {org_name}")
                        logger.info(f"  ✅ {character.name} {type_desc.get(change_type, change_type)} {org_name}")

                elif change_type == "promoted":
                    if existing_member:
                        old_position = existing_member.position
                        if new_position:
                            existing_member.position = new_position
                        existing_member.rank = (existing_member.rank or 0) + 1
                        if loyalty_delta != 0:
                            existing_member.loyalty = max(
                                0,
                                min(100, (existing_member.loyalty or 50) + loyalty_delta),
                            )
                        else:
                            existing_member.loyalty = max(
                                0,
                                min(100, (existing_member.loyalty or 50) + 5),
                            )
                        existing_member.notes = (
                            f"{existing_member.notes or ''}\n[第{chapter_number}章] 晋升: "
                            f"{old_position} → {new_position or '更高职位'}: {description}"
                        ).strip()
                        updated_count += 1
                        changes.append(
                            f"🏛️ {character.name} 在 {org_name} 晋升: "
                            f"{old_position} → {new_position or '更高职位'}"
                        )
                        logger.info(f"  ✅ {character.name} 在 {org_name} 晋升为 {new_position or '更高职位'}")
                    else:
                        logger.warning(f"  ⚠️ {character.name} 不是 {org_name} 的成员，无法晋升")

                elif change_type == "demoted":
                    if existing_member:
                        old_position = existing_member.position
                        if new_position:
                            existing_member.position = new_position
                        existing_member.rank = max(0, (existing_member.rank or 0) - 1)
                        if loyalty_delta != 0:
                            existing_member.loyalty = max(
                                0,
                                min(100, (existing_member.loyalty or 50) + loyalty_delta),
                            )
                        else:
                            existing_member.loyalty = max(
                                0,
                                min(100, (existing_member.loyalty or 50) - 5),
                            )
                        existing_member.notes = (
                            f"{existing_member.notes or ''}\n[第{chapter_number}章] 降级: "
                            f"{old_position} → {new_position or '更低职位'}: {description}"
                        ).strip()
                        updated_count += 1
                        changes.append(
                            f"🏛️ {character.name} 在 {org_name} 降级: "
                            f"{old_position} → {new_position or '更低职位'}"
                        )
                        logger.info(f"  ✅ {character.name} 在 {org_name} 降级为 {new_position or '更低职位'}")
                    else:
                        logger.warning(f"  ⚠️ {character.name} 不是 {org_name} 的成员，无法降级")

                else:
                    if existing_member and loyalty_delta != 0:
                        old_loyalty = existing_member.loyalty or 50
                        existing_member.loyalty = max(0, min(100, old_loyalty + loyalty_delta))
                        existing_member.notes = (
                            f"{existing_member.notes or ''}\n[第{chapter_number}章] {change_type}: {description}"
                        ).strip()
                        updated_count += 1
                        changes.append(
                            f"🏛️ {character.name} 在 {org_name} 忠诚度变化: "
                            f"{old_loyalty} → {existing_member.loyalty}"
                        )
                        logger.info(
                            f"  ✅ {character.name} 在 {org_name} 忠诚度: "
                            f"{old_loyalty} → {existing_member.loyalty}"
                        )

            except Exception as item_error:
                logger.error(
                    f"  ❌ 更新 {character.name} 的组织 "
                    f"{org_change.get('organization_name', '未知')} 变动失败: {str(item_error)}"
                )

        return updated_count

    @staticmethod
    async def update_organization_states(
        db: AsyncSession,
        project_id: str,
        organization_states: List[Dict[str, Any]],
        chapter_number: int,
    ) -> Dict[str, Any]:
        """根据章节分析结果更新组织自身状态。"""
        if not organization_states:
            return {"updated_count": 0, "changes": []}

        result = {"updated_count": 0, "changes": []}

        logger.info(f"🏛️ 开始更新第{chapter_number}章的组织自身状态...")
        Character, _, Organization, OrganizationMember = _character_state_models()

        all_chars_result = await db.execute(
            select(Character).where(
                Character.project_id == project_id,
                Character.is_organization == True,
            )
        )
        org_chars = all_chars_result.scalars().all()
        org_char_by_name: Dict[str, Character] = {character.name: character for character in org_chars}

        char_ids = [character.id for character in org_chars]
        if not char_ids:
            logger.info("🏛️ 项目中无组织，跳过组织状态更新")
            return result

        orgs_result = await db.execute(
            select(Organization).where(Organization.character_id.in_(char_ids))
        )
        all_orgs = orgs_result.scalars().all()
        org_by_char_id: Dict[str, Organization] = {
            organization.character_id: organization for organization in all_orgs
        }

        for org_state in organization_states:
            try:
                org_name = org_state.get("organization_name")
                if not org_name:
                    continue

                org_char = org_char_by_name.get(org_name)
                if not org_char:
                    logger.warning(f"  ⚠️ 组织不存在: {org_name}，跳过状态更新")
                    continue

                organization = org_by_char_id.get(org_char.id)
                if not organization:
                    logger.warning(f"  ⚠️ 组织 {org_name} 无详情记录，跳过状态更新")
                    continue

                updated = False
                change_parts = []

                is_destroyed = org_state.get("is_destroyed", False)
                if is_destroyed:
                    org_char.status = "destroyed"
                    org_char.status_changed_chapter = chapter_number
                    org_char.current_state = f"覆灭（第{chapter_number}章）"
                    org_char.state_updated_chapter = chapter_number
                    organization.power_level = 0

                    members_result = await db.execute(
                        select(OrganizationMember).where(
                            and_(
                                OrganizationMember.organization_id == organization.id,
                                OrganizationMember.status == "active",
                            )
                        )
                    )
                    active_members = members_result.scalars().all()
                    for member in active_members:
                        member.status = "retired"
                        member.left_at = f"第{chapter_number}章"
                        member.notes = (
                            f"{member.notes or ''}\n[第{chapter_number}章] 组织覆灭"
                        ).strip()

                    key_event = org_state.get("key_event", "")
                    event_desc = f"：{key_event[:40]}" if key_event else ""
                    result["updated_count"] += 1
                    change_summary = f"💀 {org_name} 覆灭{event_desc}，{len(active_members)}名成员受影响"
                    result["changes"].append(change_summary)
                    logger.info(f"  💀 {change_summary}")
                    continue

                power_change = org_state.get("power_change", 0)
                if power_change and isinstance(power_change, (int, float)):
                    old_power = organization.power_level or 50
                    new_power = max(0, min(100, old_power + int(power_change)))
                    if new_power != old_power:
                        organization.power_level = new_power
                        change_parts.append(f"势力:{old_power}→{new_power}")
                        updated = True

                new_location = org_state.get("new_location")
                if new_location and isinstance(new_location, str):
                    old_location = organization.location or "未设定"
                    organization.location = new_location
                    change_parts.append(f"据点:{old_location}→{new_location}")
                    updated = True

                new_purpose = org_state.get("new_purpose")
                if new_purpose and isinstance(new_purpose, str):
                    org_char.organization_purpose = new_purpose
                    change_parts.append("宗旨变更")
                    updated = True

                status_desc = org_state.get("status_description")
                if status_desc and isinstance(status_desc, str):
                    org_char.current_state = status_desc
                    org_char.state_updated_chapter = chapter_number
                    if not change_parts:
                        change_parts.append(f"状态:{status_desc[:30]}")
                    updated = True

                if updated:
                    result["updated_count"] += 1
                    key_event = org_state.get("key_event", "")
                    change_summary = f"🏛️ {org_name} 状态变化: {', '.join(change_parts)}"
                    if key_event:
                        change_summary += f" (因:{key_event[:40]})"
                    result["changes"].append(change_summary)
                    logger.info(f"  ✅ {change_summary}")

            except Exception as item_error:
                logger.error(
                    f"  ❌ 更新组织 {org_state.get('organization_name', '未知')} "
                    f"状态失败: {str(item_error)}"
                )

        if result["updated_count"] > 0:
            await db.commit()
            logger.info(f"✅ 组织状态更新完成: {result['updated_count']}个组织")

        return result

    @staticmethod
    def _calculate_intimacy_delta(change_desc: str) -> int:
        """根据变化描述计算亲密度调整值。"""
        delta = 0
        matched = False
        for keyword, adjustment in INTIMACY_ADJUSTMENTS.items():
            if keyword in change_desc:
                delta += adjustment
                matched = True

        if matched:
            delta = max(-30, min(30, delta))

        return delta



