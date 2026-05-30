use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{career, character, character_career, organization, organization_member, relationship};

const INTIMACY_ADJUSTMENTS: [(&str, i32); 30] = [
    ("改善", 10),
    ("加深", 15),
    ("信任", 10),
    ("亲近", 15),
    ("友好", 10),
    ("认可", 10),
    ("合作", 5),
    ("和解", 20),
    ("喜欢", 15),
    ("爱", 20),
    ("尊敬", 10),
    ("感激", 10),
    ("好转", 10),
    ("增进", 10),
    ("亲密", 15),
    ("忠诚", 10),
    ("恶化", -10),
    ("疏远", -15),
    ("背叛", -30),
    ("敌对", -25),
    ("矛盾", -10),
    ("冲突", -15),
    ("怀疑", -10),
    ("不信任", -15),
    ("厌恶", -20),
    ("仇恨", -25),
    ("决裂", -30),
    ("猜忌", -10),
    ("紧张", -5),
    ("破裂", -25),
];

const LOYALTY_ADJUSTMENTS: [(&str, i32); 12] = [
    ("提升", 10),
    ("增强", 10),
    ("坚定", 15),
    ("忠心", 15),
    ("动摇", -15),
    ("怀疑", -10),
    ("不满", -10),
    ("降低", -10),
    ("背叛", -50),
    ("叛变", -50),
    ("反感", -20),
    ("失望", -15),
];

fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn should_skip_state_update(existing_updated_chapter: Option<i32>, chapter_number: i32) -> bool {
    existing_updated_chapter
        .map(|updated_chapter| chapter_number < updated_chapter)
        .unwrap_or(false)
}

fn should_skip_status_update(existing_changed_chapter: Option<i32>, chapter_number: i32) -> bool {
    existing_changed_chapter
        .map(|changed_chapter| chapter_number < changed_chapter)
        .unwrap_or(false)
}

fn is_supported_survival_status(status: &str) -> bool {
    matches!(status, "deceased" | "missing" | "retired")
}

fn normalized_relationship_change_description(change_info: &Value) -> Option<String> {
    match change_info {
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Object(_) => normalized_non_empty_string(change_info.get("change"))
            .or_else(|| {
                let rendered = change_info.to_string();
                if rendered == "{}" {
                    None
                } else {
                    Some(rendered)
                }
            }),
        Value::Null => None,
        _ => Some(change_info.to_string()),
    }
}

fn calculate_intimacy_delta(change_desc: &str) -> i32 {
    let mut delta = 0;
    let mut matched = false;

    for (keyword, adjustment) in INTIMACY_ADJUSTMENTS {
        if change_desc.contains(keyword) {
            delta += adjustment;
            matched = true;
        }
    }

    if matched {
        delta.clamp(-30, 30)
    } else {
        0
    }
}

fn calculate_loyalty_delta(change_desc: &str) -> i32 {
    let mut delta = 0;

    for (keyword, adjustment) in LOYALTY_ADJUSTMENTS {
        if change_desc.contains(keyword) {
            delta += adjustment;
        }
    }

    delta.clamp(-50, 50)
}

fn append_note(existing_notes: Option<&str>, note: String) -> Option<String> {
    existing_notes
        .map(|value| format!("{}\n{}", value, note))
        .or(Some(note))
}

fn normalized_i32_value(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| match value {
        Value::Number(number) => number
            .as_i64()
            .map(|number| number.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        _ => None,
    })
}

fn normalized_bool_value(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

fn survival_status_description(status: &str) -> &str {
    match status {
        "deceased" => "死亡",
        "missing" => "失踪",
        "retired" => "退场",
        _ => status,
    }
}

fn parse_sub_careers_json(raw: Option<&str>) -> Vec<Value> {
    raw.and_then(|text| serde_json::from_str::<Vec<Value>>(text).ok())
        .unwrap_or_default()
}

fn serialize_sub_careers_json(items: &[Value]) -> Option<String> {
    if items.is_empty() {
        None
    } else {
        serde_json::to_string(items).ok()
    }
}

pub async fn sync_character_states_from_analysis(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_number: i32,
    character_states: &[Value],
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    if character_states.is_empty() {
        return Ok(json!({
            "state_updated_count": 0,
            "status_updated_count": 0,
            "skipped_count": 0,
            "changes": [],
            "skipped_reasons": [],
        }));
    }

    let now = Utc::now().naive_utc();
    let mut state_updated_count = 0_i64;
    let mut status_updated_count = 0_i64;
    let mut relationship_created_count = 0_i64;
    let mut relationship_updated_count = 0_i64;
    let mut organization_updated_count = 0_i64;
    let mut skipped_reasons = Vec::new();
    let mut changes = Vec::new();

    for item in character_states {
        let Some(character_name) = normalized_non_empty_string(item.get("character_name")) else {
            skipped_reasons.push("skip character state without character_name".to_string());
            continue;
        };

        let Some(existing) = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .filter(character::Column::Name.eq(character_name.clone()))
            .filter(character::Column::IsOrganization.eq(false))
            .one(db)
            .await?
        else {
            skipped_reasons.push(format!(
                "skip character state without matching character: {}",
                character_name
            ));
            continue;
        };

        let survival_status = normalized_non_empty_string(item.get("survival_status"));
        let state_after = normalized_non_empty_string(item.get("state_after"));
        let state_before = normalized_non_empty_string(item.get("state_before"));
        let psychological_change = normalized_non_empty_string(item.get("psychological_change"));
        let key_event = normalized_non_empty_string(item.get("key_event"));

        let mut active: character::ActiveModel = existing.clone().into();
        let mut changed = false;

        if let Some(ref status) = survival_status {
            if is_supported_survival_status(status) {
                if should_skip_status_update(existing.status_changed_chapter, chapter_number) {
                    skipped_reasons.push(format!(
                        "skip outdated survival status update for {} at chapter {}",
                        character_name, chapter_number
                    ));
                } else if existing.status != *status
                    || existing.status_changed_chapter != Some(chapter_number)
                {
                    let status_desc = survival_status_description(status);
                    active.status = Set(status.clone());
                    active.status_changed_chapter = Set(Some(chapter_number));
                    active.current_state = Set(Some(format!("{}（第{}章）", status_desc, chapter_number)));
                    active.state_updated_chapter = Set(Some(chapter_number));
                    active.updated_at = Set(Some(now));
                    status_updated_count += 1;
                    changes.push(format!(
                        "status {}: {} -> {}",
                        character_name, existing.status, status_desc
                    ));

                    let active_relationships = relationship::Entity::find()
                        .filter(relationship::Column::ProjectId.eq(project_id))
                        .filter(relationship::Column::Status.eq("active"))
                        .filter(
                            relationship::Column::CharacterFromId
                                .eq(existing.id.clone())
                                .or(relationship::Column::CharacterToId.eq(existing.id.clone())),
                        )
                        .all(db)
                        .await?;

                    for item in active_relationships {
                        let mut active_relationship: relationship::ActiveModel = item.into();
                        active_relationship.status = Set("past".to_string());
                        active_relationship.ended_at =
                            Set(Some(format!("第{}章", chapter_number)));
                        active_relationship.updated_at = Set(Some(now));
                        active_relationship.update(db).await?;
                    }

                    let member_status = if status == "deceased" {
                        "deceased"
                    } else {
                        "retired"
                    };
                    let active_members = organization_member::Entity::find()
                        .filter(organization_member::Column::CharacterId.eq(existing.id.clone()))
                        .filter(organization_member::Column::Status.eq("active"))
                        .all(db)
                        .await?;

                    for item in active_members {
                        let existing_notes = item.notes.clone();
                        let mut active_member: organization_member::ActiveModel = item.into();
                        active_member.status = Set(member_status.to_string());
                        active_member.left_at = Set(Some(format!("第{}章", chapter_number)));
                        active_member.notes = Set(append_note(
                            existing_notes.as_deref(),
                            format!("[第{}章] 角色{}", chapter_number, status_desc),
                        ));
                        active_member.updated_at = Set(Some(now));
                        active_member.update(db).await?;
                    }
                    continue;
                }
            } else {
                skipped_reasons.push(format!(
                    "skip unsupported survival status for {}: {}",
                    character_name, status
                ));
            }
        }

        if let Some(state_after) = state_after {
            if should_skip_state_update(existing.state_updated_chapter, chapter_number) {
                skipped_reasons.push(format!(
                    "skip outdated character state update for {} at chapter {}",
                    character_name, chapter_number
                ));
            } else if existing.current_state.as_deref() != Some(state_after.as_str())
                || existing.state_updated_chapter != Some(chapter_number)
            {
                active.current_state = Set(Some(state_after.clone()));
                active.state_updated_chapter = Set(Some(chapter_number));
                active.updated_at = Set(Some(now));
                changed = true;
                state_updated_count += 1;
                let mut change_line = format!(
                    "state {}: {} -> {}",
                    character_name,
                    state_before.unwrap_or_else(|| "未知".to_string()),
                    state_after
                );
                if let Some(psychological_change) = psychological_change {
                    change_line.push_str(&format!(" ({})", psychological_change));
                } else if let Some(key_event) = key_event {
                    change_line.push_str(&format!(" [{}]", key_event));
                }
                changes.push(change_line);
            }
        }

        if let Some(relationship_changes) =
            item.get("relationship_changes").and_then(Value::as_object)
        {
            for (target_name, change_info) in relationship_changes {
                let Some(change_desc) = normalized_relationship_change_description(change_info) else {
                    continue;
                };
                if target_name.trim().is_empty() {
                    continue;
                }

                let Some(target_character) = character::Entity::find()
                    .filter(character::Column::ProjectId.eq(project_id))
                    .filter(character::Column::Name.eq(target_name.trim().to_string()))
                    .filter(character::Column::IsOrganization.eq(false))
                    .one(db)
                    .await?
                else {
                    skipped_reasons.push(format!(
                        "skip relationship target without matching character: {} -> {}",
                        character_name,
                        target_name.trim()
                    ));
                    continue;
                };

                if existing.id == target_character.id {
                    continue;
                }

                let intimacy_delta = calculate_intimacy_delta(&change_desc);
                let existing_rel = relationship::Entity::find()
                    .filter(relationship::Column::ProjectId.eq(project_id))
                    .filter(
                        relationship::Column::CharacterFromId
                            .eq(existing.id.clone())
                            .and(relationship::Column::CharacterToId.eq(target_character.id.clone()))
                            .or(
                                relationship::Column::CharacterFromId
                                    .eq(target_character.id.clone())
                                    .and(relationship::Column::CharacterToId.eq(existing.id.clone())),
                            ),
                    )
                    .one(db)
                    .await?;

                if let Some(existing_rel) = existing_rel {
                    let old_intimacy = existing_rel.intimacy_level;
                    let new_intimacy = (old_intimacy + intimacy_delta).clamp(-100, 100);
                    let chapter_note = format!("[第{}章] {}", chapter_number, change_desc);
                    let next_description = existing_rel
                        .description
                        .as_deref()
                        .map(|description| format!("{}\n{}", description, chapter_note))
                        .unwrap_or(chapter_note);

                    let mut active_rel: relationship::ActiveModel = existing_rel.into();
                    active_rel.relationship_name = Set(Some(change_desc.clone()));
                    active_rel.description = Set(Some(next_description));
                    if intimacy_delta != 0 {
                        active_rel.intimacy_level = Set(new_intimacy);
                    }
                    active_rel.updated_at = Set(Some(now));
                    active_rel.update(db).await?;

                    relationship_updated_count += 1;
                    changes.push(format!(
                        "relationship {} <-> {}: {}",
                        character_name,
                        target_name.trim(),
                        change_desc
                    ));
                } else {
                    let initial_intimacy = (50 + intimacy_delta).clamp(-100, 100);
                    relationship::ActiveModel {
                        id: Set(Uuid::new_v4().to_string()),
                        project_id: Set(project_id.to_string()),
                        character_from_id: Set(existing.id.clone()),
                        character_to_id: Set(target_character.id),
                        relationship_type_id: Set(None),
                        relationship_name: Set(Some(change_desc.clone())),
                        intimacy_level: Set(initial_intimacy),
                        status: Set("active".to_string()),
                        description: Set(Some(format!(
                            "[第{}章] {}",
                            chapter_number, change_desc
                        ))),
                        started_at: Set(Some(format!("第{}章", chapter_number))),
                        ended_at: Set(None),
                        source: Set("analysis".to_string()),
                        created_at: Set(now),
                        updated_at: Set(Some(now)),
                    }
                    .insert(db)
                    .await?;

                    relationship_created_count += 1;
                    changes.push(format!(
                        "relationship {} -> {}: {}",
                        character_name,
                        target_name.trim(),
                        change_desc
                    ));
                }
            }
        }

        if let Some(career_changes) = item.get("career_changes").and_then(Value::as_object) {
            let main_career_stage_change = normalized_i32_value(career_changes.get("main_career_stage_change"))
                .unwrap_or_default();
            let sub_career_changes = career_changes
                .get("sub_career_changes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let new_careers = career_changes
                .get("new_careers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let career_breakthrough =
                normalized_non_empty_string(career_changes.get("career_breakthrough"));

            if main_career_stage_change != 0 || !sub_career_changes.is_empty() || !new_careers.is_empty() {
                if main_career_stage_change != 0 {
                    if let Some(main_career_id) = existing.main_career_id.clone() {
                        if let Some(main_char_career) = character_career::Entity::find()
                            .filter(character_career::Column::CharacterId.eq(existing.id.clone()))
                            .filter(character_career::Column::CareerType.eq("main"))
                            .one(db)
                            .await?
                        {
                            if let Some(main_career_model) = career::Entity::find()
                                .filter(career::Column::Id.eq(main_career_id))
                                .one(db)
                                .await?
                            {
                                let old_stage = main_char_career.current_stage;
                                let new_stage = (old_stage + main_career_stage_change)
                                    .clamp(1, main_career_model.max_stage.max(1));
                                if new_stage != old_stage {
                                    let mut active_main: character_career::ActiveModel =
                                        main_char_career.into();
                                    active_main.current_stage = Set(new_stage);
                                    active_main.updated_at = Set(Some(now));
                                    active_main.update(db).await?;

                                    active.main_career_stage = Set(Some(new_stage));
                                    active.updated_at = Set(Some(now));
                                    changed = true;
                                    changes.push(format!(
                                        "career main {}: {} {}阶→{}阶{}",
                                        character_name,
                                        main_career_model.name,
                                        old_stage,
                                        new_stage,
                                        career_breakthrough
                                            .as_ref()
                                            .map(|text| format!(" [{}]", text))
                                            .unwrap_or_default()
                                    ));
                                }
                            }
                        } else {
                            skipped_reasons.push(format!(
                                "skip main career stage change without character career record: {}",
                                character_name
                            ));
                        }
                    } else {
                        skipped_reasons.push(format!(
                            "skip main career stage change without main career id: {}",
                            character_name
                        ));
                    }
                }

                if !sub_career_changes.is_empty() {
                    let mut sub_careers_json = parse_sub_careers_json(existing.sub_careers.as_deref());
                    let mut sub_careers_json_changed = false;

                    for sub_change in sub_career_changes {
                        let Some(career_name) =
                            normalized_non_empty_string(sub_change.get("career_name"))
                        else {
                            continue;
                        };
                        let stage_change =
                            normalized_i32_value(sub_change.get("stage_change")).unwrap_or_default();
                        if stage_change == 0 {
                            continue;
                        }

                        let Some(sub_career_model) = career::Entity::find()
                            .filter(career::Column::ProjectId.eq(project_id))
                            .filter(career::Column::Name.eq(career_name.clone()))
                            .filter(career::Column::CareerType.eq("sub"))
                            .one(db)
                            .await?
                        else {
                            skipped_reasons.push(format!(
                                "skip missing sub career for {}: {}",
                                character_name, career_name
                            ));
                            continue;
                        };

                        let Some(sub_char_career) = character_career::Entity::find()
                            .filter(character_career::Column::CharacterId.eq(existing.id.clone()))
                            .filter(character_career::Column::CareerId.eq(sub_career_model.id.clone()))
                            .filter(character_career::Column::CareerType.eq("sub"))
                            .one(db)
                            .await?
                        else {
                            skipped_reasons.push(format!(
                                "skip sub career stage change for non-owned sub career: {} -> {}",
                                character_name, career_name
                            ));
                            continue;
                        };

                        let old_stage = sub_char_career.current_stage;
                        let new_stage =
                            (old_stage + stage_change).clamp(1, sub_career_model.max_stage.max(1));
                        if new_stage == old_stage {
                            continue;
                        }

                        let mut active_sub: character_career::ActiveModel = sub_char_career.into();
                        active_sub.current_stage = Set(new_stage);
                        active_sub.updated_at = Set(Some(now));
                        active_sub.update(db).await?;

                        for item in &mut sub_careers_json {
                            if item
                                .get("career_id")
                                .and_then(Value::as_str)
                                == Some(sub_career_model.id.as_str())
                            {
                                if let Some(object) = item.as_object_mut() {
                                    object.insert("stage".to_string(), json!(new_stage));
                                    sub_careers_json_changed = true;
                                }
                                break;
                            }
                        }

                        changes.push(format!(
                            "career sub {}: {} {}阶→{}阶",
                            character_name, career_name, old_stage, new_stage
                        ));
                    }

                    if sub_careers_json_changed {
                        active.sub_careers = Set(serialize_sub_careers_json(&sub_careers_json));
                        active.updated_at = Set(Some(now));
                        changed = true;
                    }
                }

                if !new_careers.is_empty() {
                    let mut sub_careers_json = parse_sub_careers_json(existing.sub_careers.as_deref());
                    let existing_sub_count = character_career::Entity::find()
                        .filter(character_career::Column::CharacterId.eq(existing.id.clone()))
                        .filter(character_career::Column::CareerType.eq("sub"))
                        .count(db)
                        .await? as usize;
                    let mut current_sub_count = existing_sub_count;
                    let mut sub_careers_json_changed = false;

                    for item in new_careers {
                        let Some(career_name) = item.as_str().map(str::trim).filter(|value| !value.is_empty()) else {
                            continue;
                        };

                        let Some(career_model) = career::Entity::find()
                            .filter(career::Column::ProjectId.eq(project_id))
                            .filter(career::Column::Name.eq(career_name.to_string()))
                            .one(db)
                            .await?
                        else {
                            skipped_reasons.push(format!(
                                "skip missing new career for {}: {}",
                                character_name, career_name
                            ));
                            continue;
                        };

                        let already_owned = character_career::Entity::find()
                            .filter(character_career::Column::CharacterId.eq(existing.id.clone()))
                            .filter(character_career::Column::CareerId.eq(career_model.id.clone()))
                            .one(db)
                            .await?;
                        if already_owned.is_some() {
                            continue;
                        }

                        if career_model.career_type == "main" {
                            if existing.main_career_id.is_some() {
                                skipped_reasons.push(format!(
                                    "skip new main career because character already has one: {}",
                                    character_name
                                ));
                                continue;
                            }

                            character_career::ActiveModel {
                                id: Set(Uuid::new_v4().to_string()),
                                character_id: Set(existing.id.clone()),
                                career_id: Set(career_model.id.clone()),
                                career_type: Set("main".to_string()),
                                current_stage: Set(1),
                                stage_progress: Set(None),
                                started_at: Set(Some(format!("第{}章", chapter_number))),
                                reached_current_stage_at: Set(Some(format!("第{}章", chapter_number))),
                                notes: Set(None),
                                created_at: Set(now),
                                updated_at: Set(Some(now)),
                            }
                            .insert(db)
                            .await?;

                            active.main_career_id = Set(Some(career_model.id));
                            active.main_career_stage = Set(Some(1));
                            active.updated_at = Set(Some(now));
                            changed = true;
                            changes.push(format!(
                                "career new main {}: {}",
                                character_name, career_name
                            ));
                        } else {
                            if current_sub_count >= 2 {
                                skipped_reasons.push(format!(
                                    "skip new sub career because limit reached: {}",
                                    character_name
                                ));
                                continue;
                            }

                            character_career::ActiveModel {
                                id: Set(Uuid::new_v4().to_string()),
                                character_id: Set(existing.id.clone()),
                                career_id: Set(career_model.id.clone()),
                                career_type: Set("sub".to_string()),
                                current_stage: Set(1),
                                stage_progress: Set(None),
                                started_at: Set(Some(format!("第{}章", chapter_number))),
                                reached_current_stage_at: Set(Some(format!("第{}章", chapter_number))),
                                notes: Set(None),
                                created_at: Set(now),
                                updated_at: Set(Some(now)),
                            }
                            .insert(db)
                            .await?;

                            sub_careers_json.push(json!({
                                "career_id": career_model.id,
                                "stage": 1
                            }));
                            sub_careers_json_changed = true;
                            current_sub_count += 1;
                            changes.push(format!(
                                "career new sub {}: {}",
                                character_name, career_name
                            ));
                        }
                    }

                    if sub_careers_json_changed {
                        active.sub_careers = Set(serialize_sub_careers_json(&sub_careers_json));
                        active.updated_at = Set(Some(now));
                        changed = true;
                    }
                }
            }
        }

        if let Some(organization_changes) =
            item.get("organization_changes").and_then(Value::as_array)
        {
            for org_change in organization_changes {
                let Some(org_name) = normalized_non_empty_string(org_change.get("organization_name"))
                else {
                    continue;
                };
                let change_type = normalized_non_empty_string(org_change.get("change_type"))
                    .unwrap_or_default();
                let new_position = normalized_non_empty_string(org_change.get("new_position"));
                let loyalty_change_desc =
                    normalized_non_empty_string(org_change.get("loyalty_change"))
                        .unwrap_or_default();
                let description = normalized_non_empty_string(org_change.get("description"));

                let Some(org_character) = character::Entity::find()
                    .filter(character::Column::ProjectId.eq(project_id))
                    .filter(character::Column::Name.eq(org_name.clone()))
                    .filter(character::Column::IsOrganization.eq(true))
                    .one(db)
                    .await?
                else {
                    skipped_reasons.push(format!(
                        "skip organization change without matching organization character: {} -> {}",
                        character_name, org_name
                    ));
                    continue;
                };

                let Some(org_model) = organization::Entity::find()
                    .filter(organization::Column::ProjectId.eq(project_id))
                    .filter(organization::Column::CharacterId.eq(org_character.id.clone()))
                    .one(db)
                    .await?
                else {
                    skipped_reasons.push(format!(
                        "skip organization change without organization detail record: {}",
                        org_name
                    ));
                    continue;
                };

                let existing_member = organization_member::Entity::find()
                    .filter(organization_member::Column::OrganizationId.eq(org_model.id.clone()))
                    .filter(organization_member::Column::CharacterId.eq(existing.id.clone()))
                    .one(db)
                    .await?;

                let loyalty_delta = calculate_loyalty_delta(&loyalty_change_desc);

                match change_type.as_str() {
                    "joined" => {
                        if let Some(existing_member) = existing_member {
                            if existing_member.status != "active" {
                                let existing_loyalty = existing_member.loyalty;
                                let existing_notes = existing_member.notes.clone();
                                let mut active_member: organization_member::ActiveModel =
                                    existing_member.into();
                                active_member.status = Set("active".to_string());
                                active_member.left_at = Set(None);
                                if let Some(new_position) = new_position.clone() {
                                    active_member.position = Set(new_position);
                                }
                                if loyalty_delta != 0 {
                                    active_member.loyalty =
                                        Set((existing_loyalty + loyalty_delta).clamp(0, 100));
                                }
                                if let Some(description) = description.clone() {
                                    let note = format!("[第{}章] 重新加入: {}", chapter_number, description);
                                    active_member.notes = Set(append_note(existing_notes.as_deref(), note));
                                }
                                active_member.updated_at = Set(Some(now));
                                active_member.update(db).await?;
                                organization_updated_count += 1;
                                changes.push(format!(
                                    "organization {} rejoined {}",
                                    character_name, org_name
                                ));
                            }
                        } else {
                            organization_member::ActiveModel {
                                id: Set(Uuid::new_v4().to_string()),
                                organization_id: Set(org_model.id.clone()),
                                character_id: Set(existing.id.clone()),
                                position: Set(new_position.unwrap_or_else(|| "成员".to_string())),
                                rank: Set(0),
                                status: Set("active".to_string()),
                                joined_at: Set(Some(format!("第{}章", chapter_number))),
                                left_at: Set(None),
                                loyalty: Set((50 + loyalty_delta).clamp(0, 100)),
                                contribution: Set(0),
                                source: Set("analysis".to_string()),
                                notes: Set(description
                                    .clone()
                                    .map(|desc| format!("[第{}章] {}", chapter_number, desc))),
                                created_at: Set(now),
                                updated_at: Set(Some(now)),
                            }
                            .insert(db)
                            .await?;

                            let mut active_org: organization::ActiveModel = org_model.into();
                            active_org.member_count = Set(active_org.member_count.unwrap() + 1);
                            active_org.updated_at = Set(Some(now));
                            active_org.update(db).await?;

                            organization_updated_count += 1;
                            changes.push(format!(
                                "organization {} joined {}",
                                character_name, org_name
                            ));
                        }
                    }
                    "left" | "expelled" | "betrayed" => {
                        if let Some(existing_member) = existing_member {
                            if existing_member.status == "active" {
                                let existing_loyalty = existing_member.loyalty;
                                let existing_notes = existing_member.notes.clone();
                                let mut active_member: organization_member::ActiveModel =
                                    existing_member.into();
                                let next_status = if change_type == "left" {
                                    "retired"
                                } else {
                                    "expelled"
                                };
                                active_member.status = Set(next_status.to_string());
                                active_member.left_at = Set(Some(format!("第{}章", chapter_number)));
                                if loyalty_delta != 0 {
                                    active_member.loyalty =
                                        Set((existing_loyalty + loyalty_delta).clamp(0, 100));
                                }
                                if let Some(description) = description.clone() {
                                    let note =
                                        format!("[第{}章] {}: {}", chapter_number, change_type, description);
                                    active_member.notes = Set(append_note(existing_notes.as_deref(), note));
                                }
                                active_member.updated_at = Set(Some(now));
                                active_member.update(db).await?;
                                organization_updated_count += 1;
                                changes.push(format!(
                                    "organization {} {} {}",
                                    character_name, change_type, org_name
                                ));
                            }
                        }
                    }
                    "promoted" | "demoted" => {
                        if let Some(existing_member) = existing_member {
                            let old_position = existing_member.position.clone();
                            let old_rank = existing_member.rank;
                            let loyalty_base = existing_member.loyalty;
                            let existing_notes = existing_member.notes.clone();
                            let mut active_member: organization_member::ActiveModel =
                                existing_member.into();
                            if let Some(new_position) = new_position.clone() {
                                active_member.position = Set(new_position);
                            }
                            active_member.rank = Set(if change_type == "promoted" {
                                old_rank + 1
                            } else {
                                old_rank.saturating_sub(1)
                            });
                            let loyalty_adjustment = if loyalty_delta != 0 {
                                loyalty_delta
                            } else if change_type == "promoted" {
                                5
                            } else {
                                -5
                            };
                            active_member.loyalty =
                                Set((loyalty_base + loyalty_adjustment).clamp(0, 100));
                            if let Some(description) = description.clone() {
                                let note = format!(
                                    "[第{}章] {}: {} → {}: {}",
                                    chapter_number,
                                    if change_type == "promoted" { "晋升" } else { "降级" },
                                    old_position,
                                    active_member.position.clone().unwrap(),
                                    description
                                );
                                active_member.notes = Set(append_note(existing_notes.as_deref(), note));
                            }
                            active_member.updated_at = Set(Some(now));
                            active_member.update(db).await?;
                            organization_updated_count += 1;
                            changes.push(format!(
                                "organization {} {} {}",
                                character_name, change_type, org_name
                            ));
                        } else {
                            skipped_reasons.push(format!(
                                "skip organization {} change for non-member {}",
                                change_type, character_name
                            ));
                        }
                    }
                    _ => {
                        if let Some(existing_member) = existing_member {
                            if loyalty_delta != 0 {
                                let old_loyalty = existing_member.loyalty;
                                let existing_notes = existing_member.notes.clone();
                                let mut active_member: organization_member::ActiveModel =
                                    existing_member.into();
                                active_member.loyalty =
                                    Set((old_loyalty + loyalty_delta).clamp(0, 100));
                                if let Some(description) = description.clone() {
                                    let note = format!(
                                        "[第{}章] {}: {}",
                                        chapter_number, change_type, description
                                    );
                                    active_member.notes = Set(append_note(existing_notes.as_deref(), note));
                                }
                                active_member.updated_at = Set(Some(now));
                                active_member.update(db).await?;
                                organization_updated_count += 1;
                                changes.push(format!(
                                    "organization loyalty {} -> {}",
                                    character_name, org_name
                                ));
                            }
                        }
                    }
                }
            }
        }

        if changed {
            active.update(db).await?;
        }
    }

    Ok(json!({
        "state_updated_count": state_updated_count,
        "status_updated_count": status_updated_count,
        "relationship_created_count": relationship_created_count,
        "relationship_updated_count": relationship_updated_count,
        "organization_updated_count": organization_updated_count,
        "skipped_count": skipped_reasons.len(),
        "changes": changes,
        "skipped_reasons": skipped_reasons,
    }))
}

pub async fn sync_organization_states_from_analysis(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_number: i32,
    organization_states: &[Value],
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    if organization_states.is_empty() {
        return Ok(json!({
            "updated_count": 0,
            "skipped_count": 0,
            "changes": [],
            "skipped_reasons": [],
        }));
    }

    let now = Utc::now().naive_utc();
    let mut updated_count = 0_i64;
    let mut skipped_reasons = Vec::new();
    let mut changes = Vec::new();

    for item in organization_states {
        let Some(org_name) = normalized_non_empty_string(item.get("organization_name")) else {
            skipped_reasons.push("skip organization state without organization_name".to_string());
            continue;
        };

        let Some(org_character) = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .filter(character::Column::Name.eq(org_name.clone()))
            .filter(character::Column::IsOrganization.eq(true))
            .one(db)
            .await?
        else {
            skipped_reasons.push(format!(
                "skip organization state without matching organization character: {}",
                org_name
            ));
            continue;
        };

        let Some(org_model) = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .filter(organization::Column::CharacterId.eq(org_character.id.clone()))
            .one(db)
            .await?
        else {
            skipped_reasons.push(format!(
                "skip organization state without organization detail record: {}",
                org_name
            ));
            continue;
        };

        let mut org_character_active: character::ActiveModel = org_character.clone().into();
        let mut org_active: organization::ActiveModel = org_model.clone().into();
        let mut org_character_changed = false;
        let mut org_changed = false;

        if normalized_bool_value(item.get("is_destroyed")) {
            org_character_active.status = Set("destroyed".to_string());
            org_character_active.status_changed_chapter = Set(Some(chapter_number));
            org_character_active.current_state = Set(Some(format!("覆灭（第{}章）", chapter_number)));
            org_character_active.state_updated_chapter = Set(Some(chapter_number));
            org_character_active.updated_at = Set(Some(now));
            org_character_changed = true;

            org_active.power_level = Set(0);
            org_active.updated_at = Set(Some(now));
            org_changed = true;

            let active_members = organization_member::Entity::find()
                .filter(organization_member::Column::OrganizationId.eq(org_model.id.clone()))
                .filter(organization_member::Column::Status.eq("active"))
                .all(db)
                .await?;

            let impacted_member_count = active_members.len();
            for member in active_members {
                let existing_notes = member.notes.clone();
                let mut active_member: organization_member::ActiveModel = member.into();
                active_member.status = Set("retired".to_string());
                active_member.left_at = Set(Some(format!("第{}章", chapter_number)));
                active_member.notes = Set(append_note(
                    existing_notes.as_deref(),
                    format!("[第{}章] 组织覆灭", chapter_number),
                ));
                active_member.updated_at = Set(Some(now));
                active_member.update(db).await?;
            }

            if org_character_changed {
                org_character_active.update(db).await?;
            }
            if org_changed {
                org_active.update(db).await?;
            }

            updated_count += 1;
            let key_event = normalized_non_empty_string(item.get("key_event"));
            let mut change_summary = format!(
                "organization {} destroyed, impacted {} active members",
                org_name, impacted_member_count
            );
            if let Some(key_event) = key_event {
                change_summary.push_str(&format!(" [{}]", key_event));
            }
            changes.push(change_summary);
            continue;
        }

        let mut change_parts = Vec::new();

        if let Some(power_change) = normalized_i32_value(item.get("power_change")) {
            if power_change != 0 {
                let old_power = org_model.power_level;
                let new_power = (old_power + power_change).clamp(0, 100);
                if new_power != old_power {
                    org_active.power_level = Set(new_power);
                    org_active.updated_at = Set(Some(now));
                    org_changed = true;
                    change_parts.push(format!("势力:{}→{}", old_power, new_power));
                }
            }
        }

        if let Some(new_location) = normalized_non_empty_string(item.get("new_location")) {
            if org_model.location.as_deref() != Some(new_location.as_str()) {
                let old_location = org_model.location.as_deref().unwrap_or("未设定");
                org_active.location = Set(Some(new_location.clone()));
                org_active.updated_at = Set(Some(now));
                org_changed = true;
                change_parts.push(format!("据点:{}→{}", old_location, new_location));
            }
        }

        if let Some(new_purpose) = normalized_non_empty_string(item.get("new_purpose")) {
            if org_character.organization_purpose.as_deref() != Some(new_purpose.as_str()) {
                org_character_active.organization_purpose = Set(Some(new_purpose));
                org_character_active.updated_at = Set(Some(now));
                org_character_changed = true;
                change_parts.push("宗旨变更".to_string());
            }
        }

        if let Some(status_description) = normalized_non_empty_string(item.get("status_description"))
        {
            if !should_skip_state_update(org_character.state_updated_chapter, chapter_number)
                && (org_character.current_state.as_deref() != Some(status_description.as_str())
                    || org_character.state_updated_chapter != Some(chapter_number))
            {
                org_character_active.current_state = Set(Some(status_description.clone()));
                org_character_active.state_updated_chapter = Set(Some(chapter_number));
                org_character_active.updated_at = Set(Some(now));
                org_character_changed = true;
                if change_parts.is_empty() {
                    change_parts.push(format!("状态:{}", status_description));
                }
            }
        }

        if org_character_changed {
            org_character_active.update(db).await?;
        }
        if org_changed {
            org_active.update(db).await?;
        }

        if org_character_changed || org_changed {
            updated_count += 1;
            let key_event = normalized_non_empty_string(item.get("key_event"));
            let mut change_summary = format!(
                "organization {} state changed: {}",
                org_name,
                change_parts.join(", ")
            );
            if let Some(key_event) = key_event {
                change_summary.push_str(&format!(" [{}]", key_event));
            }
            changes.push(change_summary);
        }
    }

    Ok(json!({
        "updated_count": updated_count,
        "skipped_count": skipped_reasons.len(),
        "changes": changes,
        "skipped_reasons": skipped_reasons,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        calculate_intimacy_delta, calculate_loyalty_delta, is_supported_survival_status,
        normalized_bool_value, normalized_i32_value, normalized_non_empty_string,
        normalized_relationship_change_description, parse_sub_careers_json,
        serialize_sub_careers_json, should_skip_state_update, should_skip_status_update,
        survival_status_description,
    };

    #[test]
    fn should_normalize_non_empty_string_values() {
        assert_eq!(
            normalized_non_empty_string(Some(&json!("  角色甲  "))),
            Some("角色甲".to_string())
        );
        assert_eq!(normalized_non_empty_string(Some(&json!("   "))), None);
        assert_eq!(normalized_non_empty_string(Some(&json!(1))), None);
        assert_eq!(normalized_non_empty_string(None), None);
    }

    #[test]
    fn should_skip_outdated_state_updates() {
        assert!(should_skip_state_update(Some(8), 7));
        assert!(!should_skip_state_update(Some(8), 8));
        assert!(!should_skip_state_update(Some(8), 9));
        assert!(!should_skip_state_update(None, 3));
    }

    #[test]
    fn should_skip_outdated_status_updates() {
        assert!(should_skip_status_update(Some(12), 11));
        assert!(!should_skip_status_update(Some(12), 12));
        assert!(!should_skip_status_update(None, 1));
    }

    #[test]
    fn should_recognize_supported_survival_statuses() {
        assert!(is_supported_survival_status("deceased"));
        assert!(is_supported_survival_status("missing"));
        assert!(is_supported_survival_status("retired"));
        assert!(!is_supported_survival_status("active"));
        assert!(!is_supported_survival_status("unknown"));
    }

    #[test]
    fn should_normalize_relationship_change_description() {
        assert_eq!(
            normalized_relationship_change_description(&json!("  关系改善  ")),
            Some("关系改善".to_string())
        );
        assert_eq!(
            normalized_relationship_change_description(&json!({"change": "  关系恶化  "})),
            Some("关系恶化".to_string())
        );
        assert_eq!(
            normalized_relationship_change_description(&json!({"note": "缺少change"})),
            Some("{\"note\":\"缺少change\"}".to_string())
        );
        assert_eq!(normalized_relationship_change_description(&Value::Null), None);
    }

    #[test]
    fn should_calculate_intimacy_delta_from_keywords() {
        assert_eq!(calculate_intimacy_delta("关系改善，开始信任对方"), 20);
        assert_eq!(calculate_intimacy_delta("关系恶化并且敌对"), -30);
        assert_eq!(calculate_intimacy_delta("普通互动，没有明显变化"), 0);
    }

    #[test]
    fn should_calculate_loyalty_delta_from_keywords() {
        assert_eq!(calculate_loyalty_delta("忠心提升并且更加坚定"), 40);
        assert_eq!(calculate_loyalty_delta("动摇且失望"), -30);
        assert_eq!(calculate_loyalty_delta("普通组织互动"), 0);
    }

    #[test]
    fn should_normalize_i32_value() {
        assert_eq!(normalized_i32_value(Some(&json!(12))), Some(12));
        assert_eq!(normalized_i32_value(Some(&json!(-7))), Some(-7));
        assert_eq!(normalized_i32_value(Some(&json!("12"))), None);
        assert_eq!(normalized_i32_value(None), None);
    }

    #[test]
    fn should_normalize_bool_value() {
        assert!(normalized_bool_value(Some(&json!(true))));
        assert!(!normalized_bool_value(Some(&json!(false))));
        assert!(!normalized_bool_value(Some(&json!("true"))));
        assert!(!normalized_bool_value(None));
    }

    #[test]
    fn should_map_survival_status_description() {
        assert_eq!(survival_status_description("deceased"), "死亡");
        assert_eq!(survival_status_description("missing"), "失踪");
        assert_eq!(survival_status_description("retired"), "退场");
        assert_eq!(survival_status_description("active"), "active");
    }

    #[test]
    fn should_parse_and_serialize_sub_careers_json() {
        let items = parse_sub_careers_json(Some("[{\"career_id\":\"c1\",\"stage\":2}]"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("career_id").and_then(Value::as_str), Some("c1"));
        assert_eq!(
            serialize_sub_careers_json(&items),
            Some("[{\"career_id\":\"c1\",\"stage\":2}]".to_string())
        );
        assert_eq!(serialize_sub_careers_json(&[]), None);
    }
}
