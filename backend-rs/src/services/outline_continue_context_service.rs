use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;

use crate::models::{
    career, character, character_career, foreshadow, organization, organization_member, outline,
    relationship, story_memory,
};

pub(crate) const OUTLINE_CONTINUE_RECENT_LIMIT: usize = 8;
const OUTLINE_CONTINUE_SUMMARY_LIMIT: usize = 220;
const OUTLINE_CONTINUE_DETAIL_CHARACTER_LIMIT: usize = 10;
const OUTLINE_CONTINUE_RELATION_LIMIT: usize = 4;
const OUTLINE_CONTINUE_MEMBER_LIMIT: usize = 4;
const OUTLINE_CONTINUE_OVERVIEW_LIMIT: usize = 8;
const OUTLINE_CONTINUE_MEMORY_CHARACTER_LIMIT: usize = 8;
const OUTLINE_CONTINUE_MEMORY_BLOCK_LIMIT: usize = 800;
const OUTLINE_CONTINUE_RECENT_MEMORY_LIMIT: usize = 12;
const OUTLINE_CONTINUE_CHARACTER_MEMORY_LIMIT: usize = 8;
const OUTLINE_CONTINUE_FORESHADOW_MEMORY_LIMIT: usize = 5;
const OUTLINE_CONTINUE_PLOT_POINT_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineContinuePromptContext {
    pub recent_outlines: String,
    pub characters_info: String,
    pub memory_guidance: String,
    pub focus_names: Vec<String>,
    pub foreshadow_payoff_plan: Vec<String>,
    pub foreshadow_state_ledger: Vec<String>,
    pub character_state_ledger: Vec<String>,
    pub relationship_state_ledger: Vec<String>,
    pub organization_state_ledger: Vec<String>,
    pub career_state_ledger: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutlineContinueCharacterContext {
    characters_info: String,
    character_state_ledger: Vec<String>,
    relationship_state_ledger: Vec<String>,
    organization_state_ledger: Vec<String>,
    career_state_ledger: Vec<String>,
}

pub(crate) async fn build_outline_continue_prompt_context(
    db: &DatabaseConnection,
    project_id: &str,
    existing_outlines: &[outline::Model],
    current_chapter: i32,
    story_direction: Option<&str>,
    requirements: Option<&str>,
) -> Result<OutlineContinuePromptContext, String> {
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .order_by_asc(character::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| format!("加载角色信息失败: {}", error))?;

    let recent_outlines = build_recent_outlines_context(existing_outlines);
    let focus_names = collect_outline_focus_names(
        existing_outlines,
        &characters,
        story_direction,
        requirements,
        OUTLINE_CONTINUE_MEMORY_CHARACTER_LIMIT,
    );
    let memory_character_names = collect_outline_memory_character_names(
        existing_outlines,
        &characters,
        story_direction,
        requirements,
        OUTLINE_CONTINUE_MEMORY_CHARACTER_LIMIT,
    );
    let character_context = build_outline_continue_characters_context(
        db,
        project_id,
        &characters,
        existing_outlines,
        story_direction,
        requirements,
    )
    .await?;
    let unresolved_foreshadows =
        load_unresolved_outline_foreshadows(db, project_id, current_chapter).await?;
    let foreshadow_payoff_plan =
        build_outline_foreshadow_payoff_plan_items(&unresolved_foreshadows);
    let memory_guidance = build_outline_memory_guidance(
        db,
        project_id,
        current_chapter,
        &memory_character_names,
        &unresolved_foreshadows,
    )
    .await?;

    Ok(OutlineContinuePromptContext {
        recent_outlines,
        characters_info: character_context.characters_info,
        memory_guidance,
        focus_names,
        foreshadow_payoff_plan,
        foreshadow_state_ledger: build_outline_foreshadow_state_ledger_items(
            &unresolved_foreshadows,
        ),
        character_state_ledger: character_context.character_state_ledger,
        relationship_state_ledger: character_context.relationship_state_ledger,
        organization_state_ledger: character_context.organization_state_ledger,
        career_state_ledger: character_context.career_state_ledger,
    })
}

pub(crate) fn outline_continue_stage_instruction(plot_stage: &str) -> &'static str {
    match plot_stage {
        "development" => "继续展开情节，深化角色关系，推进主线冲突",
        "climax" => "进入故事高潮，矛盾激化，关键冲突爆发",
        "ending" => "解决主要冲突，收束伏笔，给出结局",
        _ => "",
    }
}

pub(crate) fn build_recent_outlines_context(existing_outlines: &[outline::Model]) -> String {
    if existing_outlines.is_empty() {
        return "【最近大纲详情】\n暂无已有大纲".to_string();
    }

    let recent = if existing_outlines.len() > OUTLINE_CONTINUE_RECENT_LIMIT {
        &existing_outlines[existing_outlines.len() - OUTLINE_CONTINUE_RECENT_LIMIT..]
    } else {
        existing_outlines
    };

    let mut sections = vec![format!("【最近{}章大纲详情】", recent.len())];
    for outline_model in recent {
        let chapter_number = outline_model.order_index.unwrap_or_default();
        let mut block = format!("\n第{}章《{}》", chapter_number, outline_model.title);

        if let Some(structure) = outline_model.structure.as_deref() {
            if let Ok(structure_data) = serde_json::from_str::<Value>(structure) {
                let summary = truncate_text(
                    pick_outline_text(&structure_data, &["summary", "content"]).as_deref(),
                    OUTLINE_CONTINUE_SUMMARY_LIMIT,
                );
                if !summary.is_empty() {
                    block.push_str(&format!("\n  概要：{}", summary));
                }

                let key_points = format_outline_context_value(
                    structure_data.get("key_points").unwrap_or(&Value::Null),
                    3,
                    36,
                    120,
                );
                if !key_points.is_empty() {
                    block.push_str(&format!("\n  关键事件：{}", key_points));
                }

                let (character_names, organization_names) = extract_outline_characters_from_payload(
                    structure_data.get("characters").unwrap_or(&Value::Null),
                    4,
                );
                if !character_names.is_empty() {
                    block.push_str(&format!("\n  重点角色：{}", character_names.join("、")));
                }
                if !organization_names.is_empty() {
                    block.push_str(&format!("\n  涉及组织：{}", organization_names.join("、")));
                }

                let emotion =
                    truncate_text(structure_data.get("emotion").and_then(Value::as_str), 40);
                if !emotion.is_empty() {
                    block.push_str(&format!("\n  情感基调：{}", emotion));
                }

                let goal = truncate_text(structure_data.get("goal").and_then(Value::as_str), 80);
                if !goal.is_empty() {
                    block.push_str(&format!("\n  叙事目标：{}", goal));
                }

                let scenes = format_outline_context_value(
                    structure_data.get("scenes").unwrap_or(&Value::Null),
                    2,
                    24,
                    72,
                );
                if !scenes.is_empty() {
                    block.push_str(&format!("\n  场景：{}", scenes));
                }
            } else {
                let content = truncate_text(
                    outline_model.content.as_deref(),
                    OUTLINE_CONTINUE_SUMMARY_LIMIT,
                );
                if !content.is_empty() {
                    block.push_str(&format!("\n  内容：{}", content));
                }
            }
        } else {
            let content = truncate_text(
                outline_model.content.as_deref(),
                OUTLINE_CONTINUE_SUMMARY_LIMIT,
            );
            if !content.is_empty() {
                block.push_str(&format!("\n  内容：{}", content));
            }
        }

        sections.push(block);
    }

    sections.join("\n")
}

async fn build_outline_continue_characters_context(
    db: &DatabaseConnection,
    project_id: &str,
    characters: &[character::Model],
    existing_outlines: &[outline::Model],
    story_direction: Option<&str>,
    requirements: Option<&str>,
) -> Result<OutlineContinueCharacterContext, String> {
    if characters.is_empty() {
        return Ok(OutlineContinueCharacterContext {
            characters_info: "【角色信息】\n暂无角色信息".to_string(),
            character_state_ledger: Vec::new(),
            relationship_state_ledger: Vec::new(),
            organization_state_ledger: Vec::new(),
            career_state_ledger: Vec::new(),
        });
    }

    let character_name_map = characters
        .iter()
        .map(|item| (item.id.clone(), item.name.clone()))
        .collect::<HashMap<_, _>>();

    let character_ids = characters
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();

    let relationships = if character_ids.is_empty() {
        Vec::new()
    } else {
        relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .filter(relationship::Column::Status.eq("active"))
            .filter(
                relationship::Column::CharacterFromId
                    .is_in(character_ids.clone())
                    .or(relationship::Column::CharacterToId.is_in(character_ids.clone())),
            )
            .order_by_asc(relationship::Column::CreatedAt)
            .order_by_asc(relationship::Column::Id)
            .all(db)
            .await
            .map_err(|error| format!("加载角色关系失败: {}", error))?
    };

    let mut relationships_by_character_id: HashMap<String, Vec<relationship::Model>> =
        HashMap::new();
    for item in &relationships {
        relationships_by_character_id
            .entry(item.character_from_id.clone())
            .or_default()
            .push(item.clone());
        if item.character_to_id != item.character_from_id {
            relationships_by_character_id
                .entry(item.character_to_id.clone())
                .or_default()
                .push(item.clone());
        }
    }

    let organization_character_ids = characters
        .iter()
        .filter(|item| item.is_organization)
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let organizations = if organization_character_ids.is_empty() {
        Vec::new()
    } else {
        organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .filter(organization::Column::CharacterId.is_in(organization_character_ids))
            .order_by_asc(organization::Column::CreatedAt)
            .order_by_asc(organization::Column::Id)
            .all(db)
            .await
            .map_err(|error| format!("加载组织信息失败: {}", error))?
    };
    let organization_by_character_id = organizations
        .iter()
        .map(|item| (item.character_id.clone(), item.clone()))
        .collect::<HashMap<_, _>>();
    let organization_ids = organizations
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();

    let organization_members = if organization_ids.is_empty() {
        Vec::new()
    } else {
        organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.is_in(organization_ids))
            .filter(organization_member::Column::Status.eq("active"))
            .order_by_asc(organization_member::Column::CreatedAt)
            .order_by_asc(organization_member::Column::Id)
            .all(db)
            .await
            .map_err(|error| format!("加载组织成员失败: {}", error))?
    };
    let mut members_by_organization_id: HashMap<String, Vec<(organization_member::Model, String)>> =
        HashMap::new();
    for item in organization_members {
        let member_name = character_name_map
            .get(&item.character_id)
            .cloned()
            .unwrap_or_else(|| "未知".to_string());
        members_by_organization_id
            .entry(item.organization_id.clone())
            .or_default()
            .push((item, member_name));
    }

    let non_organization_character_ids = characters
        .iter()
        .filter(|item| !item.is_organization)
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let character_careers = if non_organization_character_ids.is_empty() {
        Vec::new()
    } else {
        character_career::Entity::find()
            .filter(character_career::Column::CharacterId.is_in(non_organization_character_ids))
            .order_by_asc(character_career::Column::CreatedAt)
            .order_by_asc(character_career::Column::Id)
            .all(db)
            .await
            .map_err(|error| format!("加载角色职业失败: {}", error))?
    };
    let mut career_ids = character_careers
        .iter()
        .map(|item| item.career_id.clone())
        .collect::<HashSet<_>>();
    for item in characters {
        if let Some(career_id) = item.main_career_id.as_ref() {
            career_ids.insert(career_id.clone());
        }
    }
    let careers = if career_ids.is_empty() {
        Vec::new()
    } else {
        career::Entity::find()
            .filter(career::Column::Id.is_in(career_ids.into_iter().collect::<Vec<_>>()))
            .all(db)
            .await
            .map_err(|error| format!("加载职业详情失败: {}", error))?
    };
    let careers_by_id = careers
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<HashMap<_, _>>();

    let mut primary_career_by_character_id: HashMap<String, character_career::Model> =
        HashMap::new();
    for item in character_careers {
        primary_career_by_character_id
            .entry(item.character_id.clone())
            .and_modify(|existing| {
                if existing.career_type != "main" && item.career_type == "main" {
                    *existing = item.clone();
                }
            })
            .or_insert(item);
    }

    let focus_names = collect_outline_focus_names(
        existing_outlines,
        characters,
        story_direction,
        requirements,
        8,
    );
    let focus_order = focus_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut ordered_characters = characters
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &character::Model)>>();
    ordered_characters.sort_by_key(|(index, item)| {
        let is_focus = focus_order.contains_key(&item.name);
        (
            if is_focus { 0 } else { 1 },
            focus_order.get(&item.name).copied().unwrap_or(10_000),
            if item.role_type.as_deref() == Some("protagonist") {
                0
            } else {
                1
            },
            if item.is_organization { 1 } else { 0 },
            *index,
        )
    });

    let detailed_characters = ordered_characters
        .iter()
        .take(OUTLINE_CONTINUE_DETAIL_CHARACTER_LIMIT)
        .map(|(_, item)| *item)
        .collect::<Vec<_>>();
    let compacted_characters = ordered_characters
        .iter()
        .skip(OUTLINE_CONTINUE_DETAIL_CHARACTER_LIMIT)
        .map(|(_, item)| *item)
        .collect::<Vec<_>>();

    let mut char_texts = vec!["【角色信息】".to_string()];
    for item in detailed_characters {
        let mut char_text = format!("\n{}", build_outline_character_overview(item));

        if let Some(value) = trimmed_non_empty(item.personality.as_deref()) {
            char_text.push_str(&format!("\n  性格特点：{}", truncate_text(Some(value), 72)));
        }
        if let Some(value) = trimmed_non_empty(item.background.as_deref()) {
            char_text.push_str(&format!("\n  背景故事：{}", truncate_text(Some(value), 96)));
        }
        if let Some(value) = trimmed_non_empty(item.appearance.as_deref()) {
            char_text.push_str(&format!("\n  外貌描述：{}", truncate_text(Some(value), 56)));
        }
        if let Some(value) = trimmed_non_empty(item.traits.as_deref()) {
            char_text.push_str(&format!("\n  特征标签：{}", truncate_text(Some(value), 56)));
        }

        if let Some(rels) = relationships_by_character_id.get(&item.id) {
            let rel_parts = rels
                .iter()
                .take(OUTLINE_CONTINUE_RELATION_LIMIT)
                .map(|relation_model| {
                    let target_name = if relation_model.character_from_id == item.id {
                        character_name_map
                            .get(&relation_model.character_to_id)
                            .cloned()
                            .unwrap_or_else(|| "未知".to_string())
                    } else {
                        character_name_map
                            .get(&relation_model.character_from_id)
                            .cloned()
                            .unwrap_or_else(|| "未知".to_string())
                    };
                    let rel_name = truncate_text(
                        relation_model
                            .relationship_name
                            .as_deref()
                            .or(relation_model.description.as_deref()),
                        24,
                    );
                    format!(
                        "与{}：{}",
                        target_name,
                        if rel_name.is_empty() {
                            "相关"
                        } else {
                            rel_name.as_str()
                        }
                    )
                })
                .collect::<Vec<_>>();
            if !rel_parts.is_empty() {
                char_text.push_str(&format!(
                    "\n  关系网络：{}{}",
                    rel_parts.join("；"),
                    if rels.len() > OUTLINE_CONTINUE_RELATION_LIMIT {
                        format!("；其余{}条略", rels.len() - OUTLINE_CONTINUE_RELATION_LIMIT)
                    } else {
                        String::new()
                    }
                ));
            }
        }

        if item.is_organization {
            if let Some(value) = trimmed_non_empty(item.organization_type.as_deref()) {
                char_text.push_str(&format!("\n  组织类型：{}", truncate_text(Some(value), 36)));
            }
            if let Some(value) = trimmed_non_empty(item.organization_purpose.as_deref()) {
                char_text.push_str(&format!("\n  组织宗旨：{}", truncate_text(Some(value), 72)));
            }

            if let Some(organization_model) = organization_by_character_id.get(&item.id) {
                if let Some(members) = members_by_organization_id.get(&organization_model.id) {
                    let member_parts = members
                        .iter()
                        .take(OUTLINE_CONTINUE_MEMBER_LIMIT)
                        .map(|(member_model, member_name)| {
                            format!("{}（{}）", member_name, member_model.position)
                        })
                        .collect::<Vec<_>>();
                    if !member_parts.is_empty() {
                        char_text.push_str(&format!(
                            "\n  组织成员：{}{}",
                            member_parts.join("；"),
                            if members.len() > OUTLINE_CONTINUE_MEMBER_LIMIT {
                                format!(
                                    "；其余{}人略",
                                    members.len() - OUTLINE_CONTINUE_MEMBER_LIMIT
                                )
                            } else {
                                String::new()
                            }
                        ));
                    }
                }
            }
        } else {
            let resolved_career = if let Some(career_id) = item.main_career_id.as_ref() {
                careers_by_id.get(career_id).map(|career_model| {
                    (
                        career_model.clone(),
                        item.main_career_stage,
                        Some("main".to_string()),
                    )
                })
            } else {
                primary_career_by_character_id
                    .get(&item.id)
                    .and_then(|relation_model| {
                        careers_by_id
                            .get(&relation_model.career_id)
                            .map(|career_model| {
                                (
                                    career_model.clone(),
                                    Some(relation_model.current_stage),
                                    Some(relation_model.career_type.clone()),
                                )
                            })
                    })
            };

            if let Some((career_model, current_stage, career_type)) = resolved_career {
                let mut career_line = format!(
                    "\n  职业：{}",
                    truncate_text(Some(career_model.name.as_str()), 32)
                );
                if let Some(stage) = current_stage {
                    if stage > 0 {
                        career_line.push_str(&format!("（{}阶段）", stage));
                    }
                }
                char_text.push_str(&career_line);

                if let Some(career_type) = career_type.as_deref() {
                    if !career_type.trim().is_empty() {
                        char_text.push_str(&format!(
                            "\n  职业类型：{}",
                            truncate_text(Some(career_type), 20)
                        ));
                    }
                }
            }
        }

        char_texts.push(char_text);
    }

    if !compacted_characters.is_empty() {
        let overview_characters = compacted_characters
            .iter()
            .take(OUTLINE_CONTINUE_OVERVIEW_LIMIT)
            .copied()
            .collect::<Vec<_>>();
        let overview_text = overview_characters
            .iter()
            .map(|item| build_outline_character_overview(item))
            .collect::<Vec<_>>()
            .join("；");
        let omitted_count = compacted_characters
            .len()
            .saturating_sub(overview_characters.len());
        char_texts.push(format!(
            "\n其余角色速览：{}{}",
            overview_text,
            if omitted_count > 0 {
                format!("；其余{}个角色略", omitted_count)
            } else {
                String::new()
            }
        ));
    }

    Ok(OutlineContinueCharacterContext {
        characters_info: char_texts.join("\n"),
        character_state_ledger: build_outline_character_state_ledger_items(characters),
        relationship_state_ledger: build_outline_relationship_state_ledger_items(
            &relationships,
            &character_name_map,
        ),
        organization_state_ledger: build_outline_organization_state_ledger_items(
            characters,
            &organization_by_character_id,
        ),
        career_state_ledger: build_outline_career_state_ledger_items(
            characters,
            &primary_career_by_character_id,
            &careers_by_id,
        ),
    })
}

fn build_outline_character_state_ledger_items(characters: &[character::Model]) -> Vec<String> {
    let mut ranked_characters = characters
        .iter()
        .filter(|item| !item.is_organization)
        .collect::<Vec<_>>();
    ranked_characters.sort_by_key(|item| {
        (
            std::cmp::Reverse(item.state_updated_chapter.unwrap_or(-1)),
            std::cmp::Reverse(item.status_changed_chapter.unwrap_or(-1)),
            item.name.clone(),
        )
    });

    let mut items = Vec::new();
    for item in ranked_characters {
        let name = truncate_text(Some(item.name.as_str()), 32);
        if name.is_empty() {
            continue;
        }

        let mut fragments = Vec::new();
        let current_state = truncate_text(item.current_state.as_deref(), 72);
        if !current_state.is_empty() {
            fragments.push(current_state);
        }
        let summary = fragments
            .into_iter()
            .fold(Vec::<String>::new(), |mut acc, fragment| {
                if !acc.contains(&fragment) {
                    acc.push(fragment);
                }
                acc
            })
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("; ");
        let status = item.status.trim().to_ascii_lowercase();
        let has_status =
            !status.is_empty() && !matches!(status.as_str(), "active" | "alive" | "normal");

        if summary.is_empty() && !has_status {
            continue;
        }

        let mut entry = name;
        if !summary.is_empty() {
            entry.push_str(&format!(": {}", summary));
        }
        if has_status {
            if !summary.is_empty() {
                entry.push_str(&format!("; status={}", status));
            } else {
                entry.push_str(&format!(": status={}", status));
            }
        }
        if !items.contains(&entry) {
            items.push(entry);
        }
        if items.len() >= 4 {
            break;
        }
    }

    items
}

fn build_outline_relationship_state_ledger_items(
    relationships: &[relationship::Model],
    character_name_map: &HashMap<String, String>,
) -> Vec<String> {
    let mut ranked_relationships = relationships.iter().collect::<Vec<_>>();
    ranked_relationships.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.intimacy_level.abs().cmp(&left.intimacy_level.abs()))
    });

    let mut seen_pairs = HashSet::new();
    let mut items = Vec::new();
    for item in ranked_relationships {
        let from_name = truncate_text(
            character_name_map
                .get(&item.character_from_id)
                .map(String::as_str),
            24,
        );
        let to_name = truncate_text(
            character_name_map
                .get(&item.character_to_id)
                .map(String::as_str),
            24,
        );
        if from_name.is_empty() || to_name.is_empty() || from_name == to_name {
            continue;
        }

        let mut pair_key = [from_name.to_ascii_lowercase(), to_name.to_ascii_lowercase()];
        pair_key.sort();
        if !seen_pairs.insert((pair_key[0].clone(), pair_key[1].clone())) {
            continue;
        }

        let relationship_name = truncate_text(item.relationship_name.as_deref(), 40);
        let description = truncate_text(item.description.as_deref(), 72);
        let mut fragments = Vec::new();
        if !relationship_name.is_empty() {
            fragments.push(relationship_name.clone());
        }
        if !description.is_empty()
            && description.to_ascii_lowercase() != relationship_name.to_ascii_lowercase()
        {
            fragments.push(description);
        }
        if fragments.is_empty() {
            fragments.push(format!("intimacy={}", item.intimacy_level));
        }
        let summary = fragments
            .into_iter()
            .fold(Vec::<String>::new(), |mut acc, fragment| {
                if !acc.contains(&fragment) {
                    acc.push(fragment);
                }
                acc
            })
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("; ");
        let status = item.status.trim().to_ascii_lowercase();
        let has_status =
            !status.is_empty() && !matches!(status.as_str(), "active" | "alive" | "normal");

        if summary.is_empty() && !has_status {
            continue;
        }

        let mut entry = format!("{}/{}", from_name, to_name);
        if !summary.is_empty() {
            entry.push_str(&format!(": {}", summary));
        }
        if has_status {
            if !summary.is_empty() {
                entry.push_str(&format!("; status={}", status));
            } else {
                entry.push_str(&format!(": status={}", status));
            }
        }
        items.push(entry);
        if items.len() >= 4 {
            break;
        }
    }

    items
}

fn build_outline_organization_state_ledger_items(
    characters: &[character::Model],
    organization_by_character_id: &HashMap<String, organization::Model>,
) -> Vec<String> {
    let mut ranked_organizations = characters
        .iter()
        .filter(|item| item.is_organization)
        .collect::<Vec<_>>();
    ranked_organizations.sort_by_key(|item| {
        (
            std::cmp::Reverse(item.state_updated_chapter.unwrap_or(-1)),
            std::cmp::Reverse(item.status_changed_chapter.unwrap_or(-1)),
        )
    });

    let mut items = Vec::new();
    for item in ranked_organizations {
        let name = truncate_text(Some(item.name.as_str()), 36);
        if name.is_empty() {
            continue;
        }

        let mut fragments = Vec::new();
        let current_state = truncate_text(item.current_state.as_deref(), 72);
        if !current_state.is_empty() {
            fragments.push(current_state);
        }
        if let Some(organization_model) = organization_by_character_id.get(&item.id) {
            if organization_model.power_level > 0 {
                fragments.push(format!("power={}", organization_model.power_level));
            }
            let location = truncate_text(organization_model.location.as_deref(), 36);
            if !location.is_empty() {
                fragments.push(format!("location={}", location));
            }
        }

        let summary = fragments
            .into_iter()
            .fold(Vec::<String>::new(), |mut acc, fragment| {
                if !acc.contains(&fragment) {
                    acc.push(fragment);
                }
                acc
            })
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("; ");
        let status = item.status.trim().to_ascii_lowercase();
        let has_status =
            !status.is_empty() && !matches!(status.as_str(), "active" | "alive" | "normal");

        if summary.is_empty() && !has_status {
            continue;
        }

        let mut entry = name;
        if !summary.is_empty() {
            entry.push_str(&format!(": {}", summary));
        }
        if has_status {
            if !summary.is_empty() {
                entry.push_str(&format!("; status={}", status));
            } else {
                entry.push_str(&format!(": status={}", status));
            }
        }
        if !items.contains(&entry) {
            items.push(entry);
        }
        if items.len() >= 4 {
            break;
        }
    }

    items
}

fn build_outline_career_state_ledger_items(
    characters: &[character::Model],
    primary_career_by_character_id: &HashMap<String, character_career::Model>,
    careers_by_id: &HashMap<String, career::Model>,
) -> Vec<String> {
    let mut ranked_characters = characters
        .iter()
        .filter(|item| !item.is_organization)
        .collect::<Vec<_>>();
    ranked_characters.sort_by_key(|item| {
        (
            std::cmp::Reverse(if item.main_career_id.is_some() { 1 } else { 0 }),
            std::cmp::Reverse(item.main_career_stage.unwrap_or(0)),
            item.name.clone(),
        )
    });

    let mut items = Vec::new();
    for item in ranked_characters {
        let char_name = truncate_text(Some(item.name.as_str()), 24);
        if char_name.is_empty() {
            continue;
        }

        let resolved_career = if let Some(career_id) = item.main_career_id.as_ref() {
            careers_by_id
                .get(career_id)
                .map(|career_model| (career_model, item.main_career_stage, None::<&str>))
        } else {
            primary_career_by_character_id
                .get(&item.id)
                .and_then(|relation_model| {
                    careers_by_id
                        .get(&relation_model.career_id)
                        .map(|career_model| {
                            (
                                career_model,
                                Some(relation_model.current_stage),
                                relation_model.notes.as_deref(),
                            )
                        })
                })
        };

        let Some((career_model, current_stage, notes)) = resolved_career else {
            continue;
        };
        let career_name = truncate_text(Some(career_model.name.as_str()), 24);
        if career_name.is_empty() {
            continue;
        }

        let stage = current_stage.unwrap_or(1).max(1);
        let mut fragments = vec![format!("stage {}", stage)];
        let notes = truncate_text(notes, 48);
        if !notes.is_empty() {
            fragments.push(notes);
        }
        let summary = fragments
            .into_iter()
            .fold(Vec::<String>::new(), |mut acc, fragment| {
                if !acc.contains(&fragment) {
                    acc.push(fragment);
                }
                acc
            })
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("; ");
        let entry = format!("{}/{}: {}", char_name, career_name, summary);
        if !items.contains(&entry) {
            items.push(entry);
        }
        if items.len() >= 4 {
            break;
        }
    }

    items
}

fn trimmed_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn truncate_text(value: Option<&str>, limit: usize) -> String {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        return String::new();
    }

    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= limit {
        return text.to_string();
    }

    let truncated = chars.into_iter().take(limit).collect::<String>();
    format!("{}...", truncated)
}

fn pick_outline_text(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let candidate = value.get(*key).and_then(Value::as_str).map(str::trim);
        if let Some(candidate) = candidate {
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

fn dedupe_outline_names(names: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            unique.push(trimmed.to_string());
        }
        if unique.len() >= limit {
            break;
        }
    }

    unique
}

fn extract_outline_characters_from_payload(
    raw_characters: &Value,
    limit: usize,
) -> (Vec<String>, Vec<String>) {
    let mut character_names = Vec::new();
    let mut organization_names = Vec::new();

    match raw_characters {
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::Object(map) => {
                        let name = map
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty());
                        if let Some(name) = name {
                            if map.get("type").and_then(Value::as_str) == Some("organization") {
                                organization_names.push(name.to_string());
                            } else {
                                character_names.push(name.to_string());
                            }
                        }
                    }
                    Value::String(value) => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            character_names.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Object(map) => {
            let name = map
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(name) = name {
                if map.get("type").and_then(Value::as_str) == Some("organization") {
                    organization_names.push(name.to_string());
                } else {
                    character_names.push(name.to_string());
                }
            }
        }
        Value::String(value) => {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                character_names.push(trimmed.to_string());
            }
        }
        _ => {}
    }

    (
        dedupe_outline_names(character_names, limit),
        dedupe_outline_names(organization_names, limit),
    )
}

fn format_outline_context_value(
    value: &Value,
    max_items: usize,
    item_limit: usize,
    total_limit: usize,
) -> String {
    match value {
        Value::Array(items) => {
            let compact_items = items
                .iter()
                .take(max_items)
                .map(|item| truncate_text(item.as_str(), item_limit))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>();
            truncate_text(Some(compact_items.join("；").as_str()), total_limit)
        }
        Value::Object(map) => {
            let parts = map
                .iter()
                .take(max_items)
                .filter_map(|(key, item)| {
                    let compact_value = truncate_text(item.as_str(), item_limit);
                    if compact_value.is_empty() {
                        None
                    } else {
                        Some(format!("{}:{}", key, compact_value))
                    }
                })
                .collect::<Vec<_>>();
            truncate_text(Some(parts.join("；").as_str()), total_limit)
        }
        Value::String(value) => truncate_text(Some(value.as_str()), total_limit),
        _ => String::new(),
    }
}

fn collect_outline_focus_names(
    existing_outlines: &[outline::Model],
    characters: &[character::Model],
    story_direction: Option<&str>,
    requirements: Option<&str>,
    limit: usize,
) -> Vec<String> {
    let mut focus_names = Vec::new();
    let recent_slice = if existing_outlines.len() > 6 {
        &existing_outlines[existing_outlines.len() - 6..]
    } else {
        existing_outlines
    };

    for outline_model in recent_slice.iter().rev() {
        let Some(structure) = outline_model.structure.as_deref() else {
            continue;
        };
        let Ok(structure_data) = serde_json::from_str::<Value>(structure) else {
            continue;
        };
        let (character_names, organization_names) = extract_outline_characters_from_payload(
            structure_data.get("characters").unwrap_or(&Value::Null),
            limit,
        );
        focus_names.extend(character_names);
        focus_names.extend(organization_names);
    }

    let query_text = [story_direction.unwrap_or(""), requirements.unwrap_or("")]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if !query_text.is_empty() {
        for item in characters {
            if !item.name.trim().is_empty() && query_text.contains(item.name.as_str()) {
                focus_names.push(item.name.clone());
            }
        }
    }

    dedupe_outline_names(focus_names, limit)
}

fn build_outline_character_overview(character_model: &character::Model) -> String {
    let entity_type = if character_model.is_organization {
        "organization"
    } else {
        "character"
    };
    let role_type = character_model
        .role_type
        .as_deref()
        .unwrap_or("unknown")
        .trim();
    format!("{}|{}|{}|", character_model.name, entity_type, role_type)
}

fn collect_outline_memory_character_names(
    existing_outlines: &[outline::Model],
    characters: &[character::Model],
    story_direction: Option<&str>,
    requirements: Option<&str>,
    limit: usize,
) -> Vec<String> {
    let focus_names = collect_outline_focus_names(
        existing_outlines,
        characters,
        story_direction,
        requirements,
        limit,
    );
    let focus_order = focus_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut ordered_characters = characters
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &character::Model)>>();
    ordered_characters.sort_by_key(|(index, item)| {
        let is_focus = focus_order.contains_key(&item.name);
        (
            if is_focus { 0 } else { 1 },
            focus_order.get(&item.name).copied().unwrap_or(10_000),
            if item.role_type.as_deref() == Some("protagonist") {
                0
            } else {
                1
            },
            if item.is_organization { 1 } else { 0 },
            *index,
        )
    });

    ordered_characters
        .into_iter()
        .take(limit)
        .filter_map(|(_, item)| {
            let name = item.name.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn normalize_outline_memory_block(text: impl AsRef<str>, limit: usize) -> String {
    let normalized = text.as_ref().trim();
    if normalized.is_empty() || normalized.contains("暂无相关记忆") {
        return String::new();
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    if chars.len() <= limit {
        return normalized.to_string();
    }

    format!(
        "{}...",
        chars
            .into_iter()
            .take(limit.saturating_sub(3))
            .collect::<String>()
            .trim_end()
    )
}

fn outline_memory_related_name_hit_count(
    memory: &story_memory::Model,
    related_names: &[String],
) -> usize {
    if related_names.is_empty() {
        return 0;
    }

    let content = memory.content.to_lowercase();
    let title = memory.title.as_deref().unwrap_or_default().to_lowercase();
    let related_characters = memory
        .related_characters
        .as_ref()
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.to_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    related_names
        .iter()
        .filter(|name| {
            let lowered = name.to_lowercase();
            content.contains(&lowered)
                || title.contains(&lowered)
                || related_characters
                    .iter()
                    .any(|item| item.contains(&lowered))
        })
        .count()
}

fn memory_type_priority(memory_type: &str) -> i32 {
    match memory_type {
        "chapter_summary" => 5,
        "plot_point" => 4,
        "character_event" => 3,
        "hook" => 2,
        "world_detail" => 1,
        _ => 0,
    }
}

fn sort_outline_memories_for_generation(
    memories: &mut [(story_memory::Model, usize)],
    current_chapter: i32,
) {
    memories.sort_by(|left, right| {
        let left_gap = (current_chapter - left.0.story_timeline).abs();
        let right_gap = (current_chapter - right.0.story_timeline).abs();
        right
            .1
            .cmp(&left.1)
            .then_with(|| {
                memory_type_priority(&right.0.memory_type)
                    .cmp(&memory_type_priority(&left.0.memory_type))
            })
            .then_with(|| {
                right
                    .0
                    .importance_score
                    .partial_cmp(&left.0.importance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left_gap.cmp(&right_gap))
            .then_with(|| right.0.story_timeline.cmp(&left.0.story_timeline))
            .then_with(|| right.0.chapter_position.cmp(&left.0.chapter_position))
    });
}

fn format_outline_memory_entries(
    memories: &[(story_memory::Model, usize)],
    section_title: &str,
) -> String {
    if memories.is_empty() {
        return String::new();
    }

    let mut lines = vec![format!("【{}】", section_title)];
    for (index, (memory, _)) in memories.iter().enumerate() {
        let chapter_label = if memory.story_timeline > 0 {
            format!("第{}章", memory.story_timeline)
        } else {
            "近期".to_string()
        };
        let memory_type = memory.memory_type.trim();
        let importance = memory.importance_score.unwrap_or(0.5);
        let title = memory.title.as_deref().unwrap_or("").trim();
        let content_limit = if title.is_empty() { 150 } else { 100 };
        let content_preview =
            normalize_outline_memory_block(memory.content.as_str(), content_limit);
        if content_preview.is_empty() {
            continue;
        }

        let mut line = format!(
            "{}. [{}-{}★{:.1}]",
            index + 1,
            chapter_label,
            if memory_type.is_empty() {
                "未知"
            } else {
                memory_type
            },
            importance
        );
        if title.is_empty() {
            line.push_str(&format!(" {}", content_preview));
        } else {
            line.push_str(&format!(
                " {}: {}",
                normalize_outline_memory_block(title, 32),
                content_preview
            ));
        }
        lines.push(line);
    }

    if lines.len() <= 1 {
        return String::new();
    }
    lines.join("\n")
}

fn format_outline_foreshadow_entries(items: &[foreshadow::Model]) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【未完结伏笔】".to_string()];
    for (index, item) in items.iter().enumerate() {
        let chapter_label = item
            .plant_chapter_number
            .map(|value| format!("第{}章", value))
            .unwrap_or_else(|| "未知章节".to_string());
        let title = normalize_outline_memory_block(item.title.as_str(), 32);
        let content = normalize_outline_memory_block(
            item.hint_text
                .as_deref()
                .or(item.notes.as_deref())
                .unwrap_or(item.content.as_str()),
            120,
        );
        if content.is_empty() {
            continue;
        }

        let mut line = format!(
            "{}. [{}-伏笔★{:.1}]",
            index + 1,
            chapter_label,
            item.importance
        );
        if title.is_empty() {
            line.push_str(&format!(" {}", content));
        } else {
            line.push_str(&format!(" {}: {}", title, content));
        }
        lines.push(line);
    }

    if lines.len() <= 1 {
        return String::new();
    }
    lines.join("\n")
}

fn build_outline_foreshadow_payoff_plan_items(items: &[foreshadow::Model]) -> Vec<String> {
    let mut plan_items = Vec::new();
    for item in items.iter().take(3) {
        let chapter_label = item
            .plant_chapter_number
            .map(|value| format!("第{}章", value))
            .unwrap_or_else(|| "未知章节".to_string());
        let title = normalize_outline_memory_block(item.title.as_str(), 24);
        let content = normalize_outline_memory_block(
            item.hint_text
                .as_deref()
                .or(item.notes.as_deref())
                .unwrap_or(item.content.as_str()),
            72,
        );
        if content.is_empty() {
            continue;
        }

        let plan = if title.is_empty() {
            format!("{}埋下的伏笔：{}", chapter_label, content)
        } else {
            format!("{}《{}》：{}", chapter_label, title, content)
        };
        if !plan_items.contains(&plan) {
            plan_items.push(plan);
        }
    }
    plan_items
}

fn build_outline_foreshadow_state_ledger_items(items: &[foreshadow::Model]) -> Vec<String> {
    let mut state_items = Vec::new();
    for item in items.iter().take(4) {
        let title = normalize_outline_memory_block(item.title.as_str(), 36);
        let detail = normalize_outline_memory_block(item.content.as_str(), 72);
        let status = item.status.trim().to_ascii_lowercase();
        let has_status = !status.is_empty() && status != "resolved";
        if title.is_empty() && detail.is_empty() && !has_status {
            continue;
        }

        let mut entry = if title.is_empty() {
            detail.clone()
        } else if detail.is_empty() || detail == title {
            title.clone()
        } else {
            format!("{}: {}", title, detail)
        };
        if has_status {
            if entry.is_empty() {
                entry = format!("status={}", status);
            } else {
                entry.push_str(&format!("; status={}", status));
            }
        }
        if !entry.is_empty() && !state_items.contains(&entry) {
            state_items.push(entry);
        }
    }
    state_items
}

async fn load_unresolved_outline_foreshadows(
    db: &DatabaseConnection,
    project_id: &str,
    current_chapter: i32,
) -> Result<Vec<foreshadow::Model>, String> {
    let unresolved_foreshadows = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(project_id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .filter(foreshadow::Column::Status.ne("abandoned"))
        .filter(
            foreshadow::Column::PlantChapterNumber
                .is_null()
                .or(foreshadow::Column::PlantChapterNumber.lt(current_chapter)),
        )
        .order_by_desc(foreshadow::Column::Importance)
        .order_by_desc(foreshadow::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| format!("加载续写未完结伏笔失败: {}", error))?;

    Ok(unresolved_foreshadows
        .into_iter()
        .take(OUTLINE_CONTINUE_FORESHADOW_MEMORY_LIMIT)
        .collect::<Vec<_>>())
}

async fn build_outline_memory_guidance(
    db: &DatabaseConnection,
    project_id: &str,
    current_chapter: i32,
    related_names: &[String],
    unresolved_foreshadows: &[foreshadow::Model],
) -> Result<String, String> {
    let recent_memories = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::StoryTimeline.lt(current_chapter))
        .filter(
            story_memory::Column::MemoryType
                .eq("chapter_summary")
                .or(story_memory::Column::MemoryType.eq("plot_point"))
                .or(story_memory::Column::MemoryType.eq("character_event")),
        )
        .order_by_desc(story_memory::Column::StoryTimeline)
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::ChapterPosition)
        .all(db)
        .await
        .map_err(|error| format!("加载续写最近记忆失败: {}", error))?;

    let mut ranked_recent = recent_memories
        .into_iter()
        .map(|memory| {
            let related_hits = outline_memory_related_name_hit_count(&memory, related_names);
            (memory, related_hits)
        })
        .collect::<Vec<_>>();
    sort_outline_memories_for_generation(&mut ranked_recent, current_chapter);
    ranked_recent.truncate(OUTLINE_CONTINUE_RECENT_MEMORY_LIMIT);

    let character_memories = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::StoryTimeline.lt(current_chapter))
        .filter(
            story_memory::Column::MemoryType
                .eq("character_event")
                .or(story_memory::Column::MemoryType.eq("plot_point")),
        )
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::StoryTimeline)
        .order_by_desc(story_memory::Column::ChapterPosition)
        .all(db)
        .await
        .map_err(|error| format!("加载续写角色记忆失败: {}", error))?;

    let mut ranked_character_memories = character_memories
        .into_iter()
        .map(|memory| {
            let related_hits = outline_memory_related_name_hit_count(&memory, related_names);
            (memory, related_hits)
        })
        .filter(|(_, hits)| *hits > 0)
        .collect::<Vec<_>>();
    sort_outline_memories_for_generation(&mut ranked_character_memories, current_chapter);
    ranked_character_memories.truncate(OUTLINE_CONTINUE_CHARACTER_MEMORY_LIMIT);

    let plot_points = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::StoryTimeline.lt(current_chapter))
        .filter(
            story_memory::Column::MemoryType
                .eq("plot_point")
                .or(story_memory::Column::MemoryType.eq("hook")),
        )
        .filter(story_memory::Column::ImportanceScore.gte(0.7))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::StoryTimeline)
        .order_by_desc(story_memory::Column::ChapterPosition)
        .all(db)
        .await
        .map_err(|error| format!("加载续写重要情节点失败: {}", error))?;
    let mut ranked_plot_points = plot_points
        .into_iter()
        .map(|memory| {
            let related_hits = outline_memory_related_name_hit_count(&memory, related_names);
            (memory, related_hits)
        })
        .collect::<Vec<_>>();
    sort_outline_memories_for_generation(&mut ranked_plot_points, current_chapter);
    ranked_plot_points.truncate(OUTLINE_CONTINUE_PLOT_POINT_LIMIT);

    let mut parts = Vec::new();
    for block in [
        format_outline_memory_entries(&ranked_recent, "最近章节记忆"),
        format_outline_memory_entries(&ranked_character_memories, "角色相关记忆"),
        format_outline_foreshadow_entries(unresolved_foreshadows),
        format_outline_memory_entries(&ranked_plot_points, "重要情节点"),
    ] {
        let normalized = normalize_outline_memory_block(block, OUTLINE_CONTINUE_MEMORY_BLOCK_LIMIT);
        if !normalized.is_empty() {
            parts.push(normalized);
        }
    }

    if parts.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("【连载记忆与伏笔约束】\n{}", parts.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDateTime, Utc};
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
    };

    use super::{
        build_outline_continue_prompt_context, build_recent_outlines_context,
        OutlineContinuePromptContext,
    };
    use crate::models::{
        career, character, character_career, foreshadow, organization, organization_member,
        outline, relationship, story_memory,
    };

    fn outline_model(
        id: &str,
        order_index: i32,
        title: &str,
        structure: Option<&str>,
    ) -> outline::Model {
        outline::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            content: Some("章节内容".to_string()),
            structure: structure.map(str::to_string),
            order_index: Some(order_index),
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        }
    }

    async fn setup_outline_continue_context_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);

        for entity in [
            builder.build(&schema.create_table_from_entity(character::Entity)),
            builder.build(&schema.create_table_from_entity(relationship::Entity)),
            builder.build(&schema.create_table_from_entity(organization::Entity)),
            builder.build(&schema.create_table_from_entity(organization_member::Entity)),
            builder.build(&schema.create_table_from_entity(career::Entity)),
            builder.build(&schema.create_table_from_entity(character_career::Entity)),
            builder.build(&schema.create_table_from_entity(story_memory::Entity)),
            builder.build(&schema.create_table_from_entity(foreshadow::Entity)),
        ] {
            db.execute(entity).await.expect("create test table");
        }

        db
    }

    #[test]
    fn recent_outlines_context_includes_organization_and_scene_fields() {
        let outlines = vec![outline_model(
            "outline-1",
            3,
            "第三章",
            Some(
                r#"{
                    "summary":"主角在雨夜截住押送车队，逼出城门背后的内应名单。",
                    "key_points":["押送车队现身","截杀失败后反追踪"],
                    "characters":[
                        {"name":"沈夜","type":"character"},
                        {"name":"顾寒舟","type":"character"},
                        {"name":"夜巡司","type":"organization"}
                    ],
                    "emotion":"紧绷",
                    "goal":"拿到名单",
                    "scenes":["城门雨巷","夜巡司偏厅"]
                }"#,
            ),
        )];

        let context = build_recent_outlines_context(&outlines);
        assert!(context.contains("第3章《第三章》"));
        assert!(context.contains("概要：主角在雨夜截住押送车队"));
        assert!(context.contains("关键事件：押送车队现身"));
        assert!(context.contains("重点角色：沈夜、顾寒舟"));
        assert!(context.contains("涉及组织：夜巡司"));
        assert!(context.contains("叙事目标：拿到名单"));
        assert!(context.contains("场景：城门雨巷；夜巡司偏厅"));
    }

    #[tokio::test]
    async fn outline_continue_prompt_context_builds_rich_character_details() {
        let db = setup_outline_continue_context_db().await;
        let now = Utc::now().naive_utc();

        character::ActiveModel {
            id: Set("char-hero".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("林川".to_string()),
            age: Set(None),
            gender: Set(None),
            is_organization: Set(false),
            role_type: Set(Some("protagonist".to_string())),
            personality: Set(Some("冷静果断，擅长在高压局面下拆解线索。".to_string())),
            background: Set(Some("出身夜巡司外勤，常年负责裂隙清剿。".to_string())),
            appearance: Set(None),
            relationships: Set(None),
            organization_type: Set(None),
            organization_purpose: Set(None),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(Some("正沿裂隙源头追查内应线索。".to_string())),
            state_updated_chapter: Set(Some(1)),
            main_career_id: Set(Some("career-1".to_string())),
            main_career_stage: Set(Some(2)),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(Some("敏锐,执着".to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert hero");

        character::ActiveModel {
            id: Set("char-ally".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("苏槿".to_string()),
            age: Set(None),
            gender: Set(None),
            is_organization: Set(false),
            role_type: Set(Some("supporting".to_string())),
            personality: Set(Some("谨慎细致，擅长追踪异常回响。".to_string())),
            background: Set(Some("旧城区情报线出身。".to_string())),
            appearance: Set(None),
            relationships: Set(None),
            organization_type: Set(None),
            organization_purpose: Set(None),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert ally");

        character::ActiveModel {
            id: Set("char-org".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("夜巡司".to_string()),
            age: Set(None),
            gender: Set(None),
            is_organization: Set(true),
            role_type: Set(Some("supporting".to_string())),
            personality: Set(Some("纪律森严，优先维持城防稳定。".to_string())),
            background: Set(Some("负责边城异象与裂隙治理。".to_string())),
            appearance: Set(None),
            relationships: Set(None),
            organization_type: Set(Some("官方机构".to_string())),
            organization_purpose: Set(Some("守住裂隙外溢，维持城防秩序。".to_string())),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert org character");

        relationship::ActiveModel {
            id: Set("rel-1".to_string()),
            project_id: Set("project-1".to_string()),
            character_from_id: Set("char-hero".to_string()),
            character_to_id: Set("char-ally".to_string()),
            relationship_type_id: Set(None),
            relationship_name: Set(Some("盟友".to_string())),
            intimacy_level: Set(80),
            status: Set("active".to_string()),
            description: Set(None),
            started_at: Set(None),
            ended_at: Set(None),
            source: Set("test".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert relationship");

        organization::ActiveModel {
            id: Set("org-1".to_string()),
            character_id: Set("char-org".to_string()),
            project_id: Set("project-1".to_string()),
            parent_org_id: Set(None),
            level: Set(1),
            power_level: Set(85),
            member_count: Set(1),
            location: Set(None),
            motto: Set(None),
            color: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert organization");

        organization_member::ActiveModel {
            id: Set("member-1".to_string()),
            organization_id: Set("org-1".to_string()),
            character_id: Set("char-hero".to_string()),
            position: Set("统领".to_string()),
            rank: Set(8),
            status: Set("active".to_string()),
            joined_at: Set(None),
            left_at: Set(None),
            loyalty: Set(92),
            contribution: Set(0),
            source: Set("test".to_string()),
            notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert organization member");

        career::ActiveModel {
            id: Set("career-1".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("夜巡人".to_string()),
            career_type: Set("main".to_string()),
            description: Set(None),
            category: Set(None),
            stages: Set("[]".to_string()),
            max_stage: Set(6),
            requirements: Set(None),
            special_abilities: Set(None),
            worldview_rules: Set(None),
            attribute_bonuses: Set(None),
            source: Set("test".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert career");

        character_career::ActiveModel {
            id: Set("career-rel-1".to_string()),
            character_id: Set("char-hero".to_string()),
            career_id: Set("career-1".to_string()),
            career_type: Set("main".to_string()),
            current_stage: Set(2),
            stage_progress: Set(None),
            started_at: Set(None),
            reached_current_stage_at: Set(None),
            notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert character career");

        story_memory::ActiveModel {
            id: Set("memory-summary-1".to_string()),
            project_id: Set("project-1".to_string()),
            chapter_id: Set(Some("chapter-1".to_string())),
            memory_type: Set("chapter_summary".to_string()),
            title: Set(Some("裂隙追查".to_string())),
            content: Set("夜巡司已发现城南裂隙，林川和苏槿决定顺线追查。".to_string()),
            full_context: Set(None),
            related_characters: Set(Some(serde_json::json!(["林川", "苏槿"]))),
            related_locations: Set(None),
            tags: Set(None),
            importance_score: Set(Some(0.82)),
            story_timeline: Set(1),
            chapter_position: Set(10),
            text_length: Set(28),
            is_foreshadow: Set(0),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(None),
            vector_id: Set(None),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert recent memory");

        story_memory::ActiveModel {
            id: Set("memory-character-1".to_string()),
            project_id: Set("project-1".to_string()),
            chapter_id: Set(Some("chapter-1".to_string())),
            memory_type: Set("character_event".to_string()),
            title: Set(Some("苏槿回收线索".to_string())),
            content: Set("苏槿确认怀表异响与裂隙回响同步，提醒林川警惕内应。".to_string()),
            full_context: Set(None),
            related_characters: Set(Some(serde_json::json!(["林川", "苏槿"]))),
            related_locations: Set(None),
            tags: Set(None),
            importance_score: Set(Some(0.91)),
            story_timeline: Set(1),
            chapter_position: Set(18),
            text_length: Set(31),
            is_foreshadow: Set(0),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(None),
            vector_id: Set(None),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert character memory");

        story_memory::ActiveModel {
            id: Set("memory-plot-1".to_string()),
            project_id: Set("project-1".to_string()),
            chapter_id: Set(Some("chapter-1".to_string())),
            memory_type: Set("plot_point".to_string()),
            title: Set(Some("裂隙正在扩散".to_string())),
            content: Set("关键矛盾：裂隙正在扩散，如果不及时封锁将波及整座边城。".to_string()),
            full_context: Set(None),
            related_characters: Set(Some(serde_json::json!(["林川"]))),
            related_locations: Set(None),
            tags: Set(None),
            importance_score: Set(Some(0.95)),
            story_timeline: Set(1),
            chapter_position: Set(26),
            text_length: Set(32),
            is_foreshadow: Set(0),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(None),
            vector_id: Set(None),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert plot point memory");

        foreshadow::ActiveModel {
            id: Set("foreshadow-1".to_string()),
            project_id: Set("project-1".to_string()),
            title: Set("怀表异响".to_string()),
            content: Set("怀表异响尚未回收".to_string()),
            hint_text: Set(Some("怀表异响尚未回收".to_string())),
            resolution_text: Set(None),
            source_type: Set("analysis".to_string()),
            source_memory_id: Set(None),
            source_analysis_id: Set(None),
            plant_chapter_id: Set(Some("chapter-1".to_string())),
            plant_chapter_number: Set(Some(1)),
            target_resolve_chapter_id: Set(None),
            target_resolve_chapter_number: Set(None),
            actual_resolve_chapter_id: Set(None),
            actual_resolve_chapter_number: Set(None),
            status: Set("planted".to_string()),
            is_long_term: Set(false),
            importance: Set(0.93),
            strength: Set(8),
            subtlety: Set(4),
            urgency: Set(7),
            related_characters: Set(Some(serde_json::json!(["林川", "苏槿"]))),
            related_foreshadow_ids: Set(None),
            tags: Set(None),
            category: Set(None),
            notes: Set(Some("等待在续写段落中回收".to_string())),
            resolution_notes: Set(None),
            auto_remind: Set(true),
            remind_before_chapters: Set(1),
            include_in_context: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            planted_at: Set(None),
            resolved_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert foreshadow");

        let outlines = vec![outline_model(
            "outline-1",
            1,
            "第一章",
            Some(
                r#"{
                    "summary":"夜巡司首次锁定异常源头。",
                    "characters":[
                        {"name":"林川","type":"character"},
                        {"name":"苏槿","type":"character"},
                        {"name":"夜巡司","type":"organization"}
                    ]
                }"#,
            ),
        )];

        let context: OutlineContinuePromptContext = build_outline_continue_prompt_context(
            &db,
            "project-1",
            &outlines,
            2,
            Some("追查裂隙源头"),
            Some("强化角色协作"),
        )
        .await
        .expect("build outline continue prompt context");

        assert!(context.recent_outlines.contains("夜巡司首次锁定异常源头"));
        assert!(context.characters_info.contains("与苏槿：盟友"));
        assert!(context.characters_info.contains("职业：夜巡人（2阶段）"));
        assert!(context.characters_info.contains("组织成员：林川（统领）"));
        assert!(context
            .characters_info
            .contains("林川|character|protagonist|"));
        assert!(context
            .characters_info
            .contains("夜巡司|organization|supporting|"));
        assert!(context.memory_guidance.contains("【连载记忆与伏笔约束】"));
        assert!(context.memory_guidance.contains("【最近章节记忆】"));
        assert!(context.memory_guidance.contains("【角色相关记忆】"));
        assert!(context.memory_guidance.contains("【未完结伏笔】"));
        assert!(context.memory_guidance.contains("【重要情节点】"));
        assert!(context.memory_guidance.contains("怀表异响尚未回收"));
        assert_eq!(context.foreshadow_payoff_plan.len(), 1);
        assert!(context.foreshadow_payoff_plan[0].contains("怀表异响"));
        assert_eq!(context.foreshadow_state_ledger.len(), 1);
        assert!(context.foreshadow_state_ledger[0].contains("怀表异响"));
        assert!(context.foreshadow_state_ledger[0].contains("status=planted"));
        assert_eq!(context.character_state_ledger.len(), 1);
        assert!(context.character_state_ledger[0].contains("林川"));
        assert!(context.character_state_ledger[0].contains("正沿裂隙源头追查内应线索"));
        assert_eq!(context.relationship_state_ledger.len(), 1);
        assert!(context.relationship_state_ledger[0].contains("林川/苏槿"));
        assert!(context.relationship_state_ledger[0].contains("盟友"));
        assert_eq!(context.organization_state_ledger.len(), 1);
        assert!(context.organization_state_ledger[0].contains("夜巡司"));
        assert!(context.organization_state_ledger[0].contains("power=85"));
        assert_eq!(context.career_state_ledger.len(), 1);
        assert!(context.career_state_ledger[0].contains("林川/夜巡人"));
        assert!(context.career_state_ledger[0].contains("stage 2"));
    }
}
