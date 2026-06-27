use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::models::{
    career, chapter, character, character_career, organization, plot_analysis, relationship,
    story_memory,
};

const MAX_RECENT_PROJECT_CONTINUITY_ANALYSES: u64 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectContinuityLedgerEntry {
    pub(crate) label: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) target_chapter: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectContinuityLedger {
    pub(crate) character_state_ledger: Vec<ProjectContinuityLedgerEntry>,
    pub(crate) relationship_state_ledger: Vec<ProjectContinuityLedgerEntry>,
    pub(crate) foreshadow_state_ledger: Vec<ProjectContinuityLedgerEntry>,
    pub(crate) organization_state_ledger: Vec<ProjectContinuityLedgerEntry>,
    pub(crate) career_state_ledger: Vec<ProjectContinuityLedgerEntry>,
}

impl ProjectContinuityLedger {
    pub(crate) fn fill_missing_story_packet_ledgers(&self, packet: &mut Map<String, Value>) {
        insert_missing_story_packet_ledger(
            packet,
            "character_state_ledger",
            &self.character_state_ledger,
        );
        insert_missing_story_packet_ledger(
            packet,
            "relationship_state_ledger",
            &self.relationship_state_ledger,
        );
        insert_missing_story_packet_ledger(
            packet,
            "foreshadow_state_ledger",
            &self.foreshadow_state_ledger,
        );
        insert_missing_story_packet_ledger(
            packet,
            "organization_state_ledger",
            &self.organization_state_ledger,
        );
        insert_missing_story_packet_ledger(
            packet,
            "career_state_ledger",
            &self.career_state_ledger,
        );
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityCharacterStateSource {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) is_organization: bool,
    pub(crate) status: Option<String>,
    pub(crate) current_state: Option<String>,
    pub(crate) state_updated_chapter: Option<i32>,
    pub(crate) status_changed_chapter: Option<i32>,
    pub(crate) main_career_id: Option<String>,
    pub(crate) main_career_stage: Option<i32>,
    pub(crate) sub_careers_json: Option<Value>,
    pub(crate) sub_careers_text: Option<String>,
    pub(crate) created_at: Option<NaiveDateTime>,
    pub(crate) updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityRelationshipStateSource {
    pub(crate) character_from_id: String,
    pub(crate) character_to_id: String,
    pub(crate) relationship_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) intimacy_level: Option<i32>,
    pub(crate) status: Option<String>,
    pub(crate) created_at: Option<NaiveDateTime>,
    pub(crate) updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityForeshadowMemorySource {
    pub(crate) title: Option<String>,
    pub(crate) content: String,
    pub(crate) importance_score: Option<f64>,
    pub(crate) foreshadow_strength: Option<f64>,
    pub(crate) story_timeline: Option<i32>,
    pub(crate) created_at: Option<NaiveDateTime>,
    pub(crate) updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityAnalysisSource {
    pub(crate) character_states: Option<Value>,
    pub(crate) foreshadows: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityOrganizationSource {
    pub(crate) character: ContinuityCharacterStateSource,
    pub(crate) power_level: Option<i32>,
    pub(crate) location: Option<String>,
    pub(crate) organization_updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityCareerSource {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityCharacterCareerSource {
    pub(crate) character_career: ContinuityCharacterCareerStateSource,
    pub(crate) character: ContinuityCharacterStateSource,
    pub(crate) career: ContinuityCareerSource,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContinuityCharacterCareerStateSource {
    pub(crate) career_type: Option<String>,
    pub(crate) current_stage: Option<i32>,
    pub(crate) stage_progress: Option<i32>,
    pub(crate) notes: Option<String>,
    pub(crate) updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ProjectContinuityLedgerSources {
    pub(crate) characters: Vec<ContinuityCharacterStateSource>,
    pub(crate) relationships: Vec<ContinuityRelationshipStateSource>,
    pub(crate) foreshadow_memories: Vec<ContinuityForeshadowMemorySource>,
    pub(crate) analyses: Vec<ContinuityAnalysisSource>,
    pub(crate) organizations: Vec<ContinuityOrganizationSource>,
    pub(crate) careers: Vec<ContinuityCareerSource>,
    pub(crate) character_careers: Vec<ContinuityCharacterCareerSource>,
}

pub(crate) fn build_project_continuity_ledger_from_sources(
    sources: &ProjectContinuityLedgerSources,
    limit: usize,
) -> ProjectContinuityLedger {
    let resolved_limit = limit.max(1);
    let character_name_map: HashMap<String, String> = sources
        .characters
        .iter()
        .filter_map(|character| {
            let name = compact_project_continuity_text(&character.name, 24);
            (!character.id.is_empty() && !name.is_empty()).then(|| (character.id.clone(), name))
        })
        .collect();
    let career_map: HashMap<String, ContinuityCareerSource> = sources
        .careers
        .iter()
        .filter(|career| !career.id.is_empty())
        .map(|career| (career.id.clone(), career.clone()))
        .collect();

    ProjectContinuityLedger {
        character_state_ledger: build_project_continuity_character_state_items(
            &sources.characters,
            &sources.analyses,
            resolved_limit,
        ),
        relationship_state_ledger: build_project_continuity_relationship_state_items(
            &sources.relationships,
            &character_name_map,
            &sources.analyses,
            resolved_limit,
        ),
        foreshadow_state_ledger: build_project_continuity_foreshadow_state_items(
            &sources.foreshadow_memories,
            &sources.analyses,
            resolved_limit,
        ),
        organization_state_ledger: build_project_continuity_organization_state_items(
            &sources.organizations,
            resolved_limit,
        ),
        career_state_ledger: build_project_continuity_career_state_items(
            &sources.character_careers,
            &sources.characters,
            &career_map,
            resolved_limit,
        ),
    }
}

pub(crate) async fn load_project_continuity_ledger(
    db: &DatabaseConnection,
    project_id: Option<&str>,
    limit: usize,
) -> Result<ProjectContinuityLedger, String> {
    let Some(project_id) = project_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(ProjectContinuityLedger::default());
    };
    let resolved_limit = limit.max(1);
    let sources = load_project_continuity_ledger_sources(db, project_id, resolved_limit).await?;
    Ok(build_project_continuity_ledger_from_sources(
        &sources,
        resolved_limit,
    ))
}

async fn load_project_continuity_ledger_sources(
    db: &DatabaseConnection,
    project_id: &str,
    limit: usize,
) -> Result<ProjectContinuityLedgerSources, String> {
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| format!("load continuity characters failed: {error}"))?;

    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| format!("load continuity organizations failed: {error}"))?;

    let relationships = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| format!("load continuity relationships failed: {error}"))?;

    let foreshadow_memories = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(
            Condition::any()
                .add(story_memory::Column::MemoryType.eq("foreshadow"))
                .add(story_memory::Column::IsForeshadow.gt(0)),
        )
        .filter(story_memory::Column::ForeshadowResolvedAt.is_null())
        .filter(story_memory::Column::IsForeshadow.ne(2))
        .all(db)
        .await
        .map_err(|error| format!("load continuity foreshadow memories failed: {error}"))?;

    let analysis_limit = usize::try_from(MAX_RECENT_PROJECT_CONTINUITY_ANALYSES)
        .unwrap_or(12)
        .max(limit * 3);
    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| format!("load continuity chapters failed: {error}"))?;
    let chapter_number_by_id = chapters
        .iter()
        .map(|chapter| (chapter.id.clone(), chapter.chapter_number))
        .collect::<HashMap<_, _>>();
    let mut analyses = if chapter_number_by_id.is_empty() {
        Vec::new()
    } else {
        plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ChapterId.is_in(chapter_number_by_id.keys().cloned()))
            .all(db)
            .await
            .map_err(|error| format!("load continuity plot analyses failed: {error}"))?
    };
    analyses.sort_by(|left, right| {
        (
            chapter_number_by_id
                .get(&right.chapter_id)
                .copied()
                .unwrap_or_default(),
            right.created_at,
        )
            .cmp(&(
                chapter_number_by_id
                    .get(&left.chapter_id)
                    .copied()
                    .unwrap_or_default(),
                left.created_at,
            ))
    });
    analyses.truncate(analysis_limit);

    let careers = career::Entity::find()
        .filter(career::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| format!("load continuity careers failed: {error}"))?;

    let character_by_id = characters
        .iter()
        .map(|character| (character.id.clone(), character.clone()))
        .collect::<HashMap<_, _>>();
    let character_careers = if character_by_id.is_empty() {
        Vec::new()
    } else {
        character_career::Entity::find()
            .filter(character_career::Column::CharacterId.is_in(character_by_id.keys().cloned()))
            .all(db)
            .await
            .map_err(|error| format!("load continuity character careers failed: {error}"))?
    };
    let career_by_id = careers
        .iter()
        .map(|career| (career.id.clone(), career.clone()))
        .collect::<HashMap<_, _>>();
    let character_career_sources = character_careers
        .into_iter()
        .filter_map(|character_career_model| {
            let character_model = character_by_id
                .get(&character_career_model.character_id)?
                .clone();
            let career_model = career_by_id.get(&character_career_model.career_id)?.clone();
            Some(ContinuityCharacterCareerSource {
                character_career: character_career_model.into(),
                character: character_model.into(),
                career: career_model.into(),
            })
        })
        .collect();

    let organization_by_character_id = organizations
        .iter()
        .map(|organization| (organization.character_id.clone(), organization.clone()))
        .collect::<HashMap<_, _>>();
    let organization_sources = characters
        .iter()
        .filter(|character| character.is_organization)
        .map(|character| {
            let organization = organization_by_character_id.get(&character.id);
            ContinuityOrganizationSource {
                character: character.clone().into(),
                power_level: organization.map(|organization| organization.power_level),
                location: organization.and_then(|organization| organization.location.clone()),
                organization_updated_at: organization
                    .and_then(|organization| organization.updated_at),
            }
        })
        .collect();

    Ok(ProjectContinuityLedgerSources {
        characters: characters.into_iter().map(Into::into).collect(),
        relationships: relationships.into_iter().map(Into::into).collect(),
        foreshadow_memories: foreshadow_memories.into_iter().map(Into::into).collect(),
        analyses: analyses.into_iter().map(Into::into).collect(),
        organizations: organization_sources,
        careers: careers.into_iter().map(Into::into).collect(),
        character_careers: character_career_sources,
    })
}

pub(crate) fn build_story_continuity_ledger_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service::story_continuity_ledger_owner",
        "scope": "project_story_continuity_ledger_db_backed_aggregation",
        "python_source_map": [],
        "historical_python_test_support": [
            "backend/tests/test_support/story_continuity_ledger_test_support.py"
        ],
        "rust_target_file": "backend-rs/src/services/chapter_generation_runtime_service/story_continuity_ledger_owner.rs",
        "behavior_contract": {
            "ledger_sections": [
                "character_state_ledger",
                "relationship_state_ledger",
                "foreshadow_state_ledger",
                "organization_state_ledger",
                "career_state_ledger"
            ],
            "preserves_python_entry_compaction": true,
            "preserves_python_dedupe_keys": true,
            "preserves_python_limit_gate": true,
            "db_query_wiring_completed": true,
            "loads": [
                "characters",
                "character_relationships",
                "story_memories",
                "plot_analysis_joined_to_chapters",
                "organizations",
                "careers",
                "character_careers_joined_to_characters"
            ]
        },
        "validation_boundary": {
            "focused_test": "story_continuity_ledger_owner",
            "cargo_test_filter": "story_continuity_ledger"
        },
        "service_runtime_closeout_status": {
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "logged_in_story_packet_smoke_passed": true,
            "python_service_file_deleted": true,
            "remaining_cutover_gate": "story_continuity_ledger_service.py deleted from backend/app/services; Python reference implementation retained only under tests/test_support",
            "status": "rust_story_continuity_ledger_db_backed_owner_with_python_service_deleted"
        },
        "rollback_boundary": {
            "source_map_policy": "production Python story_continuity_ledger_service.py deleted; rollback reference is test-support fixture plus Rust owner tests",
            "rollback_owner": "backend/tests/test_support/story_continuity_ledger_test_support.py"
        }
    })
}

impl From<character::Model> for ContinuityCharacterStateSource {
    fn from(character: character::Model) -> Self {
        Self {
            id: character.id,
            name: character.name,
            is_organization: character.is_organization,
            status: Some(character.status),
            current_state: character.current_state,
            state_updated_chapter: character.state_updated_chapter,
            status_changed_chapter: character.status_changed_chapter,
            main_career_id: character.main_career_id,
            main_career_stage: character.main_career_stage,
            sub_careers_json: None,
            sub_careers_text: character.sub_careers,
            created_at: Some(character.created_at),
            updated_at: character.updated_at,
        }
    }
}

impl From<relationship::Model> for ContinuityRelationshipStateSource {
    fn from(relationship: relationship::Model) -> Self {
        Self {
            character_from_id: relationship.character_from_id,
            character_to_id: relationship.character_to_id,
            relationship_name: relationship.relationship_name,
            description: relationship.description,
            intimacy_level: Some(relationship.intimacy_level),
            status: Some(relationship.status),
            created_at: Some(relationship.created_at),
            updated_at: relationship.updated_at,
        }
    }
}

impl From<story_memory::Model> for ContinuityForeshadowMemorySource {
    fn from(memory: story_memory::Model) -> Self {
        Self {
            title: memory.title,
            content: memory.content,
            importance_score: memory.importance_score,
            foreshadow_strength: memory.foreshadow_strength,
            story_timeline: Some(memory.story_timeline),
            created_at: memory.created_at,
            updated_at: memory.updated_at,
        }
    }
}

impl From<plot_analysis::Model> for ContinuityAnalysisSource {
    fn from(analysis: plot_analysis::Model) -> Self {
        Self {
            character_states: analysis.character_states,
            foreshadows: analysis.foreshadows,
        }
    }
}

impl From<career::Model> for ContinuityCareerSource {
    fn from(career: career::Model) -> Self {
        Self {
            id: career.id,
            name: career.name,
        }
    }
}

impl From<character_career::Model> for ContinuityCharacterCareerStateSource {
    fn from(character_career: character_career::Model) -> Self {
        Self {
            career_type: Some(character_career.career_type),
            current_stage: Some(character_career.current_stage),
            stage_progress: character_career.stage_progress,
            notes: character_career.notes,
            updated_at: character_career.updated_at,
        }
    }
}

fn build_project_continuity_character_state_items(
    characters: &[ContinuityCharacterStateSource],
    analyses: &[ContinuityAnalysisSource],
    limit: usize,
) -> Vec<ProjectContinuityLedgerEntry> {
    let mut items = Vec::new();
    let mut seen_names = HashSet::new();
    let mut ranked_characters = characters.to_vec();
    ranked_characters.sort_by(|left, right| compare_character_state_rank(right, left));

    for character in &ranked_characters {
        if character.is_organization {
            continue;
        }
        let name = compact_project_continuity_text(&character.name, 32);
        if name.is_empty() {
            continue;
        }
        let current_state = compact_option_text(character.current_state.as_deref(), 72);
        let mut fragments = Vec::new();
        if !current_state.is_empty() {
            fragments.push(current_state);
        }
        let status = normalize_project_continuity_status_label(character.status.as_deref());
        if fragments.is_empty() && status.is_empty() {
            continue;
        }
        append_unique_project_continuity_entry(
            &mut items,
            &mut seen_names,
            name.to_lowercase(),
            Some(name),
            Some(unique_join(&fragments, 2)),
            Some(status),
            None,
            limit,
        );
        if items.len() >= limit {
            return items;
        }
    }

    for analysis in analyses {
        for state in reversed_mapping_list(analysis.character_states.as_ref()) {
            let name = compact_value_fields(state, &["character_name", "name"], 32);
            if name.is_empty() || seen_names.contains(&name.to_lowercase()) {
                continue;
            }
            let state_text = compact_value_fields(
                state,
                &[
                    "state_after",
                    "psychological_change",
                    "current_state",
                    "state",
                ],
                72,
            );
            if state_text.is_empty() {
                continue;
            }
            append_unique_project_continuity_entry(
                &mut items,
                &mut seen_names,
                name.to_lowercase(),
                Some(name),
                Some(state_text),
                None,
                None,
                limit,
            );
            if items.len() >= limit {
                return items;
            }
        }
    }

    items
}

fn build_project_continuity_relationship_state_items(
    relationships: &[ContinuityRelationshipStateSource],
    character_name_map: &HashMap<String, String>,
    analyses: &[ContinuityAnalysisSource],
    limit: usize,
) -> Vec<ProjectContinuityLedgerEntry> {
    let mut items = Vec::new();
    let mut seen_pairs = HashSet::new();
    let mut ranked_relationships = relationships.to_vec();
    ranked_relationships.sort_by(|left, right| compare_relationship_rank(right, left));

    for relationship in &ranked_relationships {
        let from_name = character_name_map
            .get(&relationship.character_from_id)
            .map(|value| compact_project_continuity_text(value, 24))
            .unwrap_or_default();
        let to_name = character_name_map
            .get(&relationship.character_to_id)
            .map(|value| compact_project_continuity_text(value, 24))
            .unwrap_or_default();
        if from_name.is_empty() || to_name.is_empty() || from_name == to_name {
            continue;
        }
        let pair_key = build_project_continuity_relationship_pair_key(&from_name, &to_name);
        if seen_pairs.contains(&pair_key) {
            continue;
        }
        let relationship_name = compact_option_text(relationship.relationship_name.as_deref(), 40);
        let description = compact_option_text(relationship.description.as_deref(), 72);
        let mut fragments = Vec::new();
        if !relationship_name.is_empty() {
            fragments.push(relationship_name.clone());
        }
        if !description.is_empty() && description.to_lowercase() != relationship_name.to_lowercase()
        {
            fragments.push(description);
        }
        if fragments.is_empty() {
            if let Some(intimacy_level) = relationship.intimacy_level {
                fragments.push(format!("intimacy={intimacy_level}"));
            }
        }
        let status = normalize_project_continuity_status_label(relationship.status.as_deref());
        if fragments.is_empty() && status.is_empty() {
            continue;
        }
        append_unique_project_continuity_entry(
            &mut items,
            &mut seen_pairs,
            pair_key,
            Some(format!("{from_name}/{to_name}")),
            Some(unique_join(&fragments, 2)),
            Some(status),
            None,
            limit,
        );
        if items.len() >= limit {
            return items;
        }
    }

    for analysis in analyses {
        for state in reversed_mapping_list(analysis.character_states.as_ref()) {
            let base_name = compact_value_fields(state, &["character_name", "name"], 24);
            let relationship_changes = state.get("relationship_changes").and_then(Value::as_object);
            let Some(relationship_changes) = relationship_changes else {
                continue;
            };
            if base_name.is_empty() {
                continue;
            }
            for (other_name_raw, change_raw) in relationship_changes {
                let other_name = compact_project_continuity_text(other_name_raw, 24);
                let change_text = compact_value_text(change_raw, 72);
                if other_name.is_empty() || change_text.is_empty() {
                    continue;
                }
                let pair_key =
                    build_project_continuity_relationship_pair_key(&base_name, &other_name);
                if seen_pairs.contains(&pair_key) {
                    continue;
                }
                append_unique_project_continuity_entry(
                    &mut items,
                    &mut seen_pairs,
                    pair_key,
                    Some(format!("{base_name}/{other_name}")),
                    Some(change_text),
                    None,
                    None,
                    limit,
                );
                if items.len() >= limit {
                    return items;
                }
            }
        }
    }

    items
}

fn build_project_continuity_foreshadow_state_items(
    foreshadow_memories: &[ContinuityForeshadowMemorySource],
    analyses: &[ContinuityAnalysisSource],
    limit: usize,
) -> Vec<ProjectContinuityLedgerEntry> {
    let mut items = Vec::new();
    let mut seen_heads = HashSet::new();
    let mut ranked_memories = foreshadow_memories.to_vec();
    ranked_memories.sort_by(|left, right| compare_foreshadow_memory_rank(right, left));

    for memory in &ranked_memories {
        let head = compact_option_text(memory.title.as_deref(), 36);
        let head = if head.is_empty() {
            compact_project_continuity_text(&memory.content, 36)
        } else {
            head
        };
        if head.is_empty() {
            continue;
        }
        let detail = compact_project_continuity_text(&memory.content, 72);
        let summary = if !detail.is_empty() && detail.to_lowercase() != head.to_lowercase() {
            Some(detail)
        } else {
            None
        };
        append_unique_project_continuity_entry(
            &mut items,
            &mut seen_heads,
            head.to_lowercase(),
            Some(head),
            summary,
            Some("planted".to_string()),
            None,
            limit,
        );
        if items.len() >= limit {
            return items;
        }
    }

    for analysis in analyses {
        for foreshadow in reversed_mapping_list(analysis.foreshadows.as_ref()) {
            let foreshadow_type = compact_value_fields(foreshadow, &["type"], 16).to_lowercase();
            if foreshadow_type == "resolved" {
                continue;
            }
            let head = compact_value_fields(foreshadow, &["content", "title"], 36);
            if head.is_empty() {
                continue;
            }
            append_unique_project_continuity_entry(
                &mut items,
                &mut seen_heads,
                head.to_lowercase(),
                Some(head),
                None,
                (!foreshadow_type.is_empty()).then_some(foreshadow_type),
                None,
                limit,
            );
            if items.len() >= limit {
                return items;
            }
        }
    }

    items
}

fn build_project_continuity_organization_state_items(
    organizations: &[ContinuityOrganizationSource],
    limit: usize,
) -> Vec<ProjectContinuityLedgerEntry> {
    let mut items = Vec::new();
    let mut seen_names = HashSet::new();
    let mut ranked_orgs = organizations.to_vec();
    ranked_orgs.sort_by(|left, right| compare_organization_rank(right, left));

    for organization in &ranked_orgs {
        let name = compact_project_continuity_text(&organization.character.name, 36);
        if name.is_empty() {
            continue;
        }
        let mut fragments = Vec::new();
        let current_state =
            compact_option_text(organization.character.current_state.as_deref(), 72);
        if !current_state.is_empty() {
            fragments.push(current_state);
        }
        let status =
            normalize_project_continuity_status_label(organization.character.status.as_deref());
        if let Some(power_level) = organization.power_level {
            fragments.push(format!("power={power_level}"));
        }
        let location = compact_option_text(organization.location.as_deref(), 36);
        if !location.is_empty() {
            fragments.push(format!("location={location}"));
        }
        if fragments.is_empty() && status.is_empty() {
            continue;
        }
        append_unique_project_continuity_entry(
            &mut items,
            &mut seen_names,
            name.to_lowercase(),
            Some(name),
            Some(unique_join(&fragments, 2)),
            Some(status),
            None,
            limit,
        );
        if items.len() >= limit {
            return items;
        }
    }

    items
}

fn build_project_continuity_career_state_items(
    career_rows: &[ContinuityCharacterCareerSource],
    characters: &[ContinuityCharacterStateSource],
    career_map: &HashMap<String, ContinuityCareerSource>,
    limit: usize,
) -> Vec<ProjectContinuityLedgerEntry> {
    let mut items = Vec::new();
    let mut seen_keys = HashSet::new();
    let mut ranked_rows = career_rows.to_vec();
    ranked_rows.sort_by(|left, right| compare_character_career_rank(right, left));

    for row in &ranked_rows {
        if row.character.is_organization {
            continue;
        }
        let character_name = compact_project_continuity_text(&row.character.name, 24);
        let career_name = compact_project_continuity_text(&row.career.name, 24);
        if character_name.is_empty() || career_name.is_empty() {
            continue;
        }
        let dedupe_key = (character_name.to_lowercase(), career_name.to_lowercase());
        let mut fragments = vec![format!(
            "stage {}",
            row.character_career.current_stage.unwrap_or(1).max(1)
        )];
        if let Some(progress) = row
            .character_career
            .stage_progress
            .filter(|value| *value != 0)
        {
            fragments.push(format!("progress {progress}%"));
        }
        let notes = compact_option_text(row.character_career.notes.as_deref(), 48);
        if !notes.is_empty() {
            fragments.push(notes);
        }
        append_unique_project_continuity_entry(
            &mut items,
            &mut seen_keys,
            dedupe_key,
            Some(format!("{character_name}/{career_name}")),
            Some(unique_join(&fragments, 2)),
            None,
            None,
            limit,
        );
        if items.len() >= limit {
            return items;
        }
    }

    if !items.is_empty() {
        return items;
    }

    for character in characters {
        if character.is_organization {
            continue;
        }
        let character_name = compact_project_continuity_text(&character.name, 24);
        if character_name.is_empty() {
            continue;
        }
        if let Some(main_career_id) = character.main_career_id.as_deref() {
            if let Some(career) = career_map.get(main_career_id) {
                let career_name = compact_project_continuity_text(&career.name, 24);
                if !career_name.is_empty() {
                    let stage = character.main_career_stage.unwrap_or(1).max(1);
                    append_unique_project_continuity_entry(
                        &mut items,
                        &mut seen_keys,
                        (character_name.to_lowercase(), career_name.to_lowercase()),
                        Some(format!("{character_name}/{career_name}")),
                        Some(format!("stage {stage}")),
                        None,
                        None,
                        limit,
                    );
                    if items.len() >= limit {
                        return items;
                    }
                }
            }
        }

        for sub_career in safe_project_continuity_json_list(character) {
            let Some(career_id) = sub_career
                .get("career_id")
                .and_then(value_as_compact_string)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some(career) = career_map.get(&career_id) else {
                continue;
            };
            let career_name = compact_project_continuity_text(&career.name, 24);
            if career_name.is_empty() {
                continue;
            }
            let stage = sub_career
                .get("stage")
                .and_then(safe_project_continuity_i32)
                .unwrap_or(1)
                .max(1);
            append_unique_project_continuity_entry(
                &mut items,
                &mut seen_keys,
                (character_name.to_lowercase(), career_name.to_lowercase()),
                Some(format!("{character_name}/{career_name}")),
                Some(format!("stage {stage}")),
                None,
                None,
                limit,
            );
            if items.len() >= limit {
                return items;
            }
        }
    }

    items
}

fn insert_missing_story_packet_ledger(
    packet: &mut Map<String, Value>,
    field_name: &str,
    entries: &[ProjectContinuityLedgerEntry],
) {
    if entries.is_empty() || !story_packet_ledger_missing(packet.get(field_name)) {
        return;
    }
    packet.insert(
        field_name.to_string(),
        Value::Array(entries.iter().map(project_continuity_entry_value).collect()),
    );
}

fn story_packet_ledger_missing(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => items.is_empty(),
        _ => false,
    }
}

fn project_continuity_entry_value(entry: &ProjectContinuityLedgerEntry) -> Value {
    let mut value = Map::new();
    if let Some(label) = entry.label.as_deref().filter(|value| !value.is_empty()) {
        value.insert("label".to_string(), json!(label));
    }
    if let Some(summary) = entry.summary.as_deref().filter(|value| !value.is_empty()) {
        value.insert("summary".to_string(), json!(summary));
    }
    if let Some(status) = entry.status.as_deref().filter(|value| !value.is_empty()) {
        value.insert("status".to_string(), json!(status));
    }
    if let Some(target_chapter) = entry.target_chapter.filter(|value| *value > 0) {
        value.insert("target_chapter".to_string(), json!(target_chapter));
    }
    Value::Object(value)
}

fn compact_project_continuity_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return String::new();
    }
    if normalized.chars().count() <= limit {
        return normalized;
    }
    let take = limit.saturating_sub(3);
    format!(
        "{}...",
        normalized.chars().take(take).collect::<String>().trim_end()
    )
}

fn compact_option_text(value: Option<&str>, limit: usize) -> String {
    value
        .map(|text| compact_project_continuity_text(text, limit))
        .unwrap_or_default()
}

fn compact_value_text(value: &Value, limit: usize) -> String {
    let text = match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    compact_project_continuity_text(&text, limit)
}

fn compact_value_fields(map: &Map<String, Value>, fields: &[&str], limit: usize) -> String {
    fields
        .iter()
        .filter_map(|field| map.get(*field))
        .map(|value| compact_value_text(value, limit))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn normalize_project_continuity_status_label(value: Option<&str>) -> String {
    let normalized = compact_option_text(value, 24).to_lowercase();
    match normalized.as_str() {
        "" | "active" | "alive" | "normal" => String::new(),
        _ => normalized,
    }
}

fn append_unique_project_continuity_entry<K>(
    items: &mut Vec<ProjectContinuityLedgerEntry>,
    seen_keys: &mut HashSet<K>,
    dedupe_key: K,
    label: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    target_chapter: Option<i32>,
    limit: usize,
) where
    K: Eq + Hash,
{
    if items.len() >= limit || seen_keys.contains(&dedupe_key) {
        return;
    }

    let normalized_label = label
        .as_deref()
        .map(|value| compact_project_continuity_text(value, 36))
        .filter(|value| !value.is_empty());
    let normalized_summary = summary
        .as_deref()
        .map(|value| compact_project_continuity_text(value, 72))
        .filter(|value| !value.is_empty());
    let normalized_status = status
        .as_deref()
        .map(|value| normalize_project_continuity_status_label(Some(value)))
        .filter(|value| !value.is_empty());
    let normalized_target_chapter = target_chapter.filter(|value| *value > 0);

    if normalized_label.is_none()
        && normalized_summary.is_none()
        && normalized_status.is_none()
        && normalized_target_chapter.is_none()
    {
        return;
    }

    seen_keys.insert(dedupe_key);
    items.push(ProjectContinuityLedgerEntry {
        label: normalized_label,
        summary: normalized_summary,
        status: normalized_status,
        target_chapter: normalized_target_chapter,
    });
}

fn value_as_compact_string(value: &Value) -> Option<String> {
    let text = compact_value_text(value, usize::MAX / 2);
    (!text.is_empty()).then_some(text)
}

fn safe_project_continuity_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(text) => text.parse::<i32>().ok(),
        _ => None,
    }
}

fn safe_project_continuity_json_list(
    character: &ContinuityCharacterStateSource,
) -> Vec<Map<String, Value>> {
    if let Some(Value::Array(items)) = character.sub_careers_json.as_ref() {
        return items.iter().filter_map(Value::as_object).cloned().collect();
    }
    if let Some(text) = character.sub_careers_text.as_deref() {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
            return items
                .into_iter()
                .filter_map(|value| value.as_object().cloned())
                .collect();
        }
    }
    Vec::new()
}

fn reversed_mapping_list(value: Option<&Value>) -> Vec<&Map<String, Value>> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).rev().collect(),
        Some(Value::Object(map)) => vec![map],
        _ => Vec::new(),
    }
}

fn unique_join(values: &[String], limit: usize) -> String {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_lowercase()))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ")
}

fn build_project_continuity_relationship_pair_key(name_a: &str, name_b: &str) -> (String, String) {
    let mut names = [name_a.to_lowercase(), name_b.to_lowercase()];
    names.sort();
    (names[0].clone(), names[1].clone())
}

fn compare_option_time(left: Option<NaiveDateTime>, right: Option<NaiveDateTime>) -> Ordering {
    left.cmp(&right)
}

fn compare_option_f64(left: Option<f64>, right: Option<f64>) -> Ordering {
    left.unwrap_or(0.0)
        .partial_cmp(&right.unwrap_or(0.0))
        .unwrap_or(Ordering::Equal)
}

fn compare_character_state_rank(
    left: &ContinuityCharacterStateSource,
    right: &ContinuityCharacterStateSource,
) -> Ordering {
    (
        left.state_updated_chapter.unwrap_or(-1),
        left.status_changed_chapter.unwrap_or(-1),
        left.updated_at,
        left.created_at,
    )
        .cmp(&(
            right.state_updated_chapter.unwrap_or(-1),
            right.status_changed_chapter.unwrap_or(-1),
            right.updated_at,
            right.created_at,
        ))
}

fn compare_relationship_rank(
    left: &ContinuityRelationshipStateSource,
    right: &ContinuityRelationshipStateSource,
) -> Ordering {
    let time_order = (left.updated_at, left.created_at).cmp(&(right.updated_at, right.created_at));
    if time_order != Ordering::Equal {
        return time_order;
    }
    left.intimacy_level
        .unwrap_or(0)
        .abs()
        .cmp(&right.intimacy_level.unwrap_or(0).abs())
}

fn compare_foreshadow_memory_rank(
    left: &ContinuityForeshadowMemorySource,
    right: &ContinuityForeshadowMemorySource,
) -> Ordering {
    compare_option_f64(left.importance_score, right.importance_score)
        .then_with(|| compare_option_f64(left.foreshadow_strength, right.foreshadow_strength))
        .then_with(|| {
            left.story_timeline
                .unwrap_or(-1)
                .cmp(&right.story_timeline.unwrap_or(-1))
        })
        .then_with(|| compare_option_time(left.updated_at, right.updated_at))
        .then_with(|| compare_option_time(left.created_at, right.created_at))
}

fn compare_organization_rank(
    left: &ContinuityOrganizationSource,
    right: &ContinuityOrganizationSource,
) -> Ordering {
    (
        left.character.state_updated_chapter.unwrap_or(-1),
        left.character.status_changed_chapter.unwrap_or(-1),
        left.character.updated_at,
        left.organization_updated_at,
    )
        .cmp(&(
            right.character.state_updated_chapter.unwrap_or(-1),
            right.character.status_changed_chapter.unwrap_or(-1),
            right.character.updated_at,
            right.organization_updated_at,
        ))
}

fn compare_character_career_rank(
    left: &ContinuityCharacterCareerSource,
    right: &ContinuityCharacterCareerSource,
) -> Ordering {
    (
        i32::from(left.character_career.career_type.as_deref() == Some("main")),
        left.character_career.updated_at,
        left.character_career.current_stage.unwrap_or(0),
    )
        .cmp(&(
            i32::from(right.character_career.career_type.as_deref() == Some("main")),
            right.character_career.updated_at,
            right.character_career.current_stage.unwrap_or(0),
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, IntoActiveModel, Schema};

    fn dt(day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, day)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    fn entry(
        label: &str,
        summary: Option<&str>,
        status: Option<&str>,
    ) -> ProjectContinuityLedgerEntry {
        ProjectContinuityLedgerEntry {
            label: Some(label.to_string()),
            summary: summary.map(str::to_string),
            status: status.map(str::to_string),
            target_chapter: None,
        }
    }

    #[test]
    fn story_continuity_ledger_prefers_live_character_and_relationship_state() {
        let sources = ProjectContinuityLedgerSources {
            characters: vec![
                ContinuityCharacterStateSource {
                    id: "char-old".to_string(),
                    name: "甲".to_string(),
                    status: Some("alive".to_string()),
                    current_state: Some("旧状态".to_string()),
                    state_updated_chapter: Some(1),
                    created_at: Some(dt(1)),
                    ..Default::default()
                },
                ContinuityCharacterStateSource {
                    id: "char-new".to_string(),
                    name: "乙".to_string(),
                    status: Some("injured".to_string()),
                    current_state: Some("  失去灵力   仍在追踪线索 ".to_string()),
                    state_updated_chapter: Some(9),
                    updated_at: Some(dt(9)),
                    ..Default::default()
                },
            ],
            relationships: vec![ContinuityRelationshipStateSource {
                character_from_id: "char-old".to_string(),
                character_to_id: "char-new".to_string(),
                relationship_name: Some("盟友".to_string()),
                description: Some("互相隐瞒代价".to_string()),
                status: Some("strained".to_string()),
                updated_at: Some(dt(8)),
                ..Default::default()
            }],
            analyses: vec![ContinuityAnalysisSource {
                character_states: Some(json!([
                    {
                        "character_name": "乙",
                        "state_after": "分析里重复的乙状态"
                    },
                    {
                        "character_name": "丙",
                        "state_after": "刚发现密信"
                    },
                    {
                        "character_name": "甲",
                        "relationship_changes": {
                            "丙": "临时结盟"
                        }
                    }
                ])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let ledger = build_project_continuity_ledger_from_sources(&sources, 3);

        assert_eq!(
            ledger.character_state_ledger,
            vec![
                entry("乙", Some("失去灵力 仍在追踪线索"), Some("injured")),
                entry("甲", Some("旧状态"), None),
                entry("丙", Some("刚发现密信"), None),
            ]
        );
        assert_eq!(
            ledger.relationship_state_ledger,
            vec![
                entry("甲/乙", Some("盟友; 互相隐瞒代价"), Some("strained")),
                entry("甲/丙", Some("临时结盟"), None),
            ]
        );
    }

    #[test]
    fn story_continuity_ledger_builds_foreshadow_organization_and_career_sections() {
        let character = ContinuityCharacterStateSource {
            id: "char-1".to_string(),
            name: "林河".to_string(),
            main_career_id: Some("career-main".to_string()),
            main_career_stage: Some(3),
            sub_careers_json: Some(json!([
                {"career_id": "career-sub", "stage": 2}
            ])),
            ..Default::default()
        };
        let organization_character = ContinuityCharacterStateSource {
            id: "org-char".to_string(),
            name: "白塔".to_string(),
            is_organization: true,
            status: Some("active".to_string()),
            current_state: Some("封锁港口".to_string()),
            state_updated_chapter: Some(7),
            ..Default::default()
        };
        let sources = ProjectContinuityLedgerSources {
            characters: vec![character.clone(), organization_character.clone()],
            foreshadow_memories: vec![ContinuityForeshadowMemorySource {
                title: Some("断裂的铜钥匙".to_string()),
                content: "断裂的铜钥匙藏在祭坛下方".to_string(),
                importance_score: Some(0.9),
                foreshadow_strength: Some(0.7),
                story_timeline: Some(5),
                ..Default::default()
            }],
            analyses: vec![ContinuityAnalysisSource {
                foreshadows: Some(json!([
                    {"type": "resolved", "content": "已经解决的伏笔"},
                    {"type": "planted", "content": "黑船在雾里靠岸"}
                ])),
                ..Default::default()
            }],
            organizations: vec![ContinuityOrganizationSource {
                character: organization_character,
                power_level: Some(8),
                location: Some("北港".to_string()),
                organization_updated_at: Some(dt(10)),
            }],
            careers: vec![
                ContinuityCareerSource {
                    id: "career-main".to_string(),
                    name: "剑修".to_string(),
                },
                ContinuityCareerSource {
                    id: "career-sub".to_string(),
                    name: "药师".to_string(),
                },
            ],
            character_careers: Vec::new(),
            ..Default::default()
        };

        let ledger = build_project_continuity_ledger_from_sources(&sources, 4);

        assert_eq!(
            ledger.foreshadow_state_ledger,
            vec![
                entry(
                    "断裂的铜钥匙",
                    Some("断裂的铜钥匙藏在祭坛下方"),
                    Some("planted")
                ),
                entry("黑船在雾里靠岸", None, Some("planted")),
            ]
        );
        assert_eq!(
            ledger.organization_state_ledger,
            vec![entry("白塔", Some("封锁港口; power=8"), None)]
        );
        assert_eq!(
            ledger.career_state_ledger,
            vec![
                entry("林河/剑修", Some("stage 3"), None),
                entry("林河/药师", Some("stage 2"), None),
            ]
        );
    }

    #[test]
    fn story_continuity_ledger_owner_contract_records_db_wiring_and_integration_gate() {
        let contract = build_story_continuity_ledger_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_runtime_service::story_continuity_ledger_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["db_query_wiring_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["logged_in_story_packet_smoke_passed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_service_file_deleted"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "story_continuity_ledger_service.py deleted from backend/app/services; Python reference implementation retained only under tests/test_support"
        );
        assert_eq!(
            contract["rollback_boundary"]["rollback_owner"],
            "backend/tests/test_support/story_continuity_ledger_test_support.py"
        );
    }

    #[tokio::test]
    async fn story_continuity_ledger_loads_db_backed_sources() {
        let db = setup_continuity_db().await;

        character::Entity::insert(
            character::Model {
                id: "char-1".to_string(),
                project_id: "project-1".to_string(),
                name: "林河".to_string(),
                age: None,
                gender: None,
                is_organization: false,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "injured".to_string(),
                status_changed_chapter: Some(4),
                current_state: Some("灵力受损 仍保留铜钥匙".to_string()),
                state_updated_chapter: Some(9),
                main_career_id: Some("career-main".to_string()),
                main_career_stage: Some(3),
                sub_careers: Some(r#"[{"career_id":"career-sub","stage":2}]"#.to_string()),
                avatar_url: None,
                traits: None,
                created_at: dt(1),
                updated_at: Some(dt(9)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert character");

        character::Entity::insert(
            character::Model {
                id: "char-2".to_string(),
                project_id: "project-1".to_string(),
                name: "白露".to_string(),
                age: None,
                gender: None,
                is_organization: false,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: Some("守住北港入口".to_string()),
                state_updated_chapter: Some(7),
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: dt(2),
                updated_at: Some(dt(7)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert character");

        character::Entity::insert(
            character::Model {
                id: "org-char".to_string(),
                project_id: "project-1".to_string(),
                name: "白塔".to_string(),
                age: None,
                gender: None,
                is_organization: true,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: Some("封锁港口".to_string()),
                state_updated_chapter: Some(8),
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: dt(3),
                updated_at: Some(dt(8)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert organization character");

        relationship::Entity::insert(
            relationship::Model {
                id: "rel-1".to_string(),
                project_id: "project-1".to_string(),
                character_from_id: "char-1".to_string(),
                character_to_id: "char-2".to_string(),
                relationship_type_id: None,
                relationship_name: Some("盟友".to_string()),
                intimacy_level: 6,
                status: "strained".to_string(),
                description: Some("互相隐瞒代价".to_string()),
                started_at: None,
                ended_at: None,
                source: "manual".to_string(),
                created_at: dt(4),
                updated_at: Some(dt(9)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert relationship");

        story_memory::Entity::insert(
            story_memory::Model {
                id: "memory-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: None,
                memory_type: "foreshadow".to_string(),
                title: Some("断裂的铜钥匙".to_string()),
                content: "断裂的铜钥匙藏在祭坛下方".to_string(),
                full_context: None,
                related_characters: None,
                related_locations: None,
                tags: None,
                importance_score: Some(0.9),
                story_timeline: 5,
                chapter_position: 0,
                text_length: 18,
                is_foreshadow: 1,
                foreshadow_resolved_at: None,
                foreshadow_strength: Some(0.7),
                vector_id: None,
                embedding_model: None,
                created_at: Some(dt(5)),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert memory");

        chapter::Entity::insert(
            chapter::Model {
                id: "chapter-1".to_string(),
                project_id: "project-1".to_string(),
                title: "第一章".to_string(),
                chapter_number: 11,
                content: None,
                summary: None,
                expansion_plan: None,
                status: "completed".to_string(),
                word_count: 0,
                outline_id: None,
                sub_index: 0,
                created_at: dt(6),
                updated_at: Some(dt(11)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert chapter");

        plot_analysis::Entity::insert(
            plot_analysis::Model {
                id: "analysis-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: "chapter-1".to_string(),
                plot_stage: None,
                conflict_level: None,
                conflict_types: None,
                emotional_tone: None,
                emotional_intensity: None,
                emotional_curve: None,
                hooks: None,
                hooks_count: 0,
                hooks_avg_strength: None,
                foreshadows: Some(json!([
                    {"type": "planted", "content": "黑船在雾里靠岸"}
                ])),
                foreshadows_planted: 1,
                foreshadows_resolved: 0,
                plot_points: None,
                plot_points_count: 0,
                character_states: Some(json!([
                    {"character_name": "新角色", "state_after": "拿到港口地图"},
                    {
                        "character_name": "林河",
                        "relationship_changes": {"白露": "短暂失信"}
                    }
                ])),
                scenes: None,
                pacing: None,
                overall_quality_score: None,
                pacing_score: None,
                engagement_score: None,
                coherence_score: None,
                analysis_report: None,
                suggestions: None,
                word_count: None,
                dialogue_ratio: None,
                description_ratio: None,
                created_at: Some(dt(11)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert analysis");

        organization::Entity::insert(
            organization::Model {
                id: "org-1".to_string(),
                character_id: "org-char".to_string(),
                project_id: "project-1".to_string(),
                parent_org_id: None,
                level: 2,
                power_level: 8,
                member_count: 30,
                location: Some("北港".to_string()),
                motto: None,
                color: None,
                created_at: dt(7),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert organization");

        career::Entity::insert(
            career::Model {
                id: "career-main".to_string(),
                project_id: "project-1".to_string(),
                name: "剑修".to_string(),
                career_type: "main".to_string(),
                description: None,
                category: None,
                stages: "[]".to_string(),
                max_stage: 9,
                requirements: None,
                special_abilities: None,
                worldview_rules: None,
                attribute_bonuses: None,
                source: "manual".to_string(),
                created_at: dt(8),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert career");

        character_career::Entity::insert(
            character_career::Model {
                id: "char-career-1".to_string(),
                character_id: "char-1".to_string(),
                career_id: "career-main".to_string(),
                career_type: "main".to_string(),
                current_stage: 4,
                stage_progress: Some(60),
                started_at: None,
                reached_current_stage_at: None,
                notes: Some("突破失败".to_string()),
                created_at: dt(8),
                updated_at: Some(dt(12)),
            }
            .into_active_model(),
        )
        .exec(&db)
        .await
        .expect("insert character career");

        let ledger = load_project_continuity_ledger(&db, Some("project-1"), 4)
            .await
            .expect("load ledger");

        assert_eq!(
            ledger.character_state_ledger,
            vec![
                entry("林河", Some("灵力受损 仍保留铜钥匙"), Some("injured")),
                entry("白露", Some("守住北港入口"), None),
                entry("新角色", Some("拿到港口地图"), None),
            ]
        );
        assert_eq!(
            ledger.relationship_state_ledger[0],
            entry("林河/白露", Some("盟友; 互相隐瞒代价"), Some("strained"))
        );
        assert_eq!(
            ledger.foreshadow_state_ledger,
            vec![
                entry(
                    "断裂的铜钥匙",
                    Some("断裂的铜钥匙藏在祭坛下方"),
                    Some("planted")
                ),
                entry("黑船在雾里靠岸", None, Some("planted")),
            ]
        );
        assert_eq!(
            ledger.organization_state_ledger,
            vec![entry("白塔", Some("封锁港口; power=8"), None)]
        );
        assert_eq!(
            ledger.career_state_ledger,
            vec![entry("林河/剑修", Some("stage 4; progress 60%"), None)]
        );
    }

    async fn setup_continuity_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let schema = Schema::new(DatabaseBackend::Sqlite);
        let builder = db.get_database_backend();
        for statement in [
            builder.build(&schema.create_table_from_entity(character::Entity)),
            builder.build(&schema.create_table_from_entity(relationship::Entity)),
            builder.build(&schema.create_table_from_entity(story_memory::Entity)),
            builder.build(&schema.create_table_from_entity(chapter::Entity)),
            builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
            builder.build(&schema.create_table_from_entity(organization::Entity)),
            builder.build(&schema.create_table_from_entity(career::Entity)),
            builder.build(&schema.create_table_from_entity(character_career::Entity)),
        ] {
            db.execute(statement)
                .await
                .expect("create continuity table");
        }
        db
    }
}
