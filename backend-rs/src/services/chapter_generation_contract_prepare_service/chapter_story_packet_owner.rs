use std::collections::{BTreeMap, HashMap};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Map, Value};

use crate::models::{chapter, character, foreshadow, project};
use crate::services::generation_contract_service::{
    build_generation_contract_snapshot, fill_missing_continuity, merge_story_packet_layers,
    GenerationContractSnapshotV1, GenerationIntentKind, GenerationIntentV1, GenerationTarget,
    StoryContinuitySnapshot, StoryLedgerEntry, StoryPacketFactLayer, StoryPacketSource,
    StoryPacketSourceKind, StoryPacketV1,
};
use crate::services::wizard_service::build_project_long_term_goal;

const PROJECT_FACT_KEY: &str = "project";
const CHAPTER_FACT_KEY: &str = "chapter";
const CHARACTERS_FACT_KEY: &str = "characters";
const UNRESOLVED_FORESHADOWS_FACT_KEY: &str = "unresolved_foreshadows";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterAnalysisPromptContext {
    pub(crate) chapter_number: String,
    pub(crate) title: String,
    pub(crate) word_count: String,
    pub(crate) content: String,
    pub(crate) existing_foreshadows: String,
    pub(crate) characters_info: String,
}

impl ChapterAnalysisPromptContext {
    pub(crate) fn into_prompt_params(self) -> HashMap<String, String> {
        HashMap::from([
            ("chapter_number".to_owned(), self.chapter_number),
            ("title".to_owned(), self.title),
            ("word_count".to_owned(), self.word_count),
            ("content".to_owned(), self.content),
            ("existing_foreshadows".to_owned(), self.existing_foreshadows),
            ("characters_info".to_owned(), self.characters_info),
        ])
    }
}

pub(crate) fn build_chapter_story_packet_contract(
    project_model: &project::Model,
    chapter_model: &chapter::Model,
    previous_story_runtime_snapshot: Option<&Value>,
    continuity: Option<StoryContinuitySnapshot>,
) -> StoryPacketV1 {
    let target = GenerationTarget::chapter(project_model.id.clone(), chapter_model.id.clone());
    let mut system_defaults = StoryPacketV1::new(project_model.id.clone(), target);
    system_defaults.sources.push(StoryPacketSource {
        kind: StoryPacketSourceKind::SystemDefaults,
        reference: None,
    });

    let authoritative_facts = StoryPacketFactLayer {
        sources: vec![StoryPacketSource {
            kind: StoryPacketSourceKind::AuthoritativeDatabase,
            reference: Some(format!(
                "project:{}/chapter:{}",
                project_model.id, chapter_model.id
            )),
        }],
        current_chapter_number: positive_u32(chapter_model.chapter_number),
        chapter_count: project_model.chapter_count.and_then(positive_u32),
        target_word_count: positive_u32(project_model.target_words),
        story_long_term_goal: build_project_long_term_goal(
            project_model.theme.as_deref(),
            project_model.description.as_deref(),
            project_model.default_story_creation_brief.as_deref(),
            project_model
                .chapter_count
                .and_then(|value| usize::try_from(value).ok()),
            usize::try_from(project_model.target_words).ok(),
        ),
        opaque_story_facts: BTreeMap::from([
            (
                PROJECT_FACT_KEY.to_owned(),
                json!({
                    "id": project_model.id,
                    "title": project_model.title,
                    "world_rules": project_model.world_rules,
                }),
            ),
            (
                CHAPTER_FACT_KEY.to_owned(),
                json!({
                    "id": chapter_model.id,
                    "project_id": chapter_model.project_id,
                    "chapter_number": chapter_model.chapter_number,
                    "title": chapter_model.title,
                    "word_count": chapter_model.word_count.max(0),
                    "content": chapter_model.content.clone().unwrap_or_default(),
                }),
            ),
        ]),
        compatibility_metadata: BTreeMap::from([(
            "legacy_source".to_owned(),
            json!("single_generation_active_route"),
        )]),
        ..StoryPacketFactLayer::default()
    };
    let persisted_snapshot = previous_story_runtime_snapshot
        .and_then(Value::as_object)
        .map(build_legacy_story_packet_fact_layer);
    let mut packet =
        merge_story_packet_layers(system_defaults, authoritative_facts, persisted_snapshot);

    if let Some(continuity) = continuity {
        fill_missing_continuity(&mut packet.continuity, continuity);
    }
    packet
}

pub(crate) async fn prepare_chapter_analysis_story_packet(
    db: &DatabaseConnection,
    project_model: &project::Model,
    chapter_model: &chapter::Model,
) -> Result<StoryPacketV1, String> {
    let unresolved_foreshadows = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(&project_model.id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .order_by_desc(foreshadow::Column::CreatedAt)
        .limit(50)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_asc(character::Column::Name)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut packet = build_chapter_story_packet_contract(project_model, chapter_model, None, None);
    packet.opaque_story_facts.insert(
        CHARACTERS_FACT_KEY.to_owned(),
        Value::Array(
            characters
                .into_iter()
                .map(|item| {
                    json!({
                        "id": item.id,
                        "name": item.name,
                        "role_type": item.role_type,
                        "status": item.status,
                    })
                })
                .collect(),
        ),
    );
    packet.opaque_story_facts.insert(
        UNRESOLVED_FORESHADOWS_FACT_KEY.to_owned(),
        Value::Array(
            unresolved_foreshadows
                .into_iter()
                .map(|item| {
                    json!({
                        "id": item.id,
                        "title": item.title,
                        "plant_chapter_number": item.plant_chapter_number,
                        "content": item.content,
                    })
                })
                .collect(),
        ),
    );
    Ok(packet)
}

pub(crate) fn project_story_packet_to_analysis_prompt_context(
    packet: &StoryPacketV1,
) -> Result<ChapterAnalysisPromptContext, String> {
    let _project = required_fact_object(packet, PROJECT_FACT_KEY)?;
    let chapter = required_fact_object(packet, CHAPTER_FACT_KEY)?;
    let characters = optional_fact_array(packet, CHARACTERS_FACT_KEY)?;
    let unresolved_foreshadows = optional_fact_array(packet, UNRESOLVED_FORESHADOWS_FACT_KEY)?;

    Ok(ChapterAnalysisPromptContext {
        chapter_number: required_number_text(chapter, "chapter_number")?,
        title: string_field(chapter, "title"),
        word_count: non_negative_number_text(chapter, "word_count")?,
        content: string_field(chapter, "content"),
        existing_foreshadows: format_foreshadows(unresolved_foreshadows),
        characters_info: format_characters(characters),
    })
}

pub(crate) fn build_chapter_review_contract(
    packet: StoryPacketV1,
) -> Result<GenerationContractSnapshotV1, String> {
    build_chapter_intent_contract(packet, GenerationIntentKind::ChapterReview)
}

pub(crate) fn build_chapter_repair_contract(
    packet: StoryPacketV1,
) -> Result<GenerationContractSnapshotV1, String> {
    build_chapter_intent_contract(packet, GenerationIntentKind::ChapterRepair)
}

fn build_chapter_intent_contract(
    packet: StoryPacketV1,
    intent_kind: GenerationIntentKind,
) -> Result<GenerationContractSnapshotV1, String> {
    let intent = GenerationIntentV1::new(intent_kind, packet.target.clone());
    build_generation_contract_snapshot(packet, intent).map_err(|error| error.to_string())
}

fn required_fact_object<'a>(
    packet: &'a StoryPacketV1,
    key: &str,
) -> Result<&'a Map<String, Value>, String> {
    packet
        .opaque_story_facts
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Story Packet missing object fact: {key}"))
}

fn optional_fact_array<'a>(packet: &'a StoryPacketV1, key: &str) -> Result<&'a [Value], String> {
    match packet.opaque_story_facts.get(key) {
        None => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(format!("Story Packet fact must be an array: {key}")),
    }
}

fn required_number_text(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .ok_or_else(|| format!("Story Packet field must be an integer: {key}"))
}

fn non_negative_number_text(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value.max(0).to_string())
        .ok_or_else(|| format!("Story Packet field must be an integer: {key}"))
}

fn string_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn format_characters(characters: &[Value]) -> String {
    if characters.is_empty() {
        return "[]".to_owned();
    }
    characters
        .iter()
        .map(|item| {
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
            let role_type = item
                .get("role_type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("未设定");
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("- {name}（身份：{role_type}；状态：{status}）")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_foreshadows(foreshadows: &[Value]) -> String {
    if foreshadows.is_empty() {
        return "[]".to_owned();
    }
    foreshadows
        .iter()
        .map(|item| {
            let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let plant_chapter_number = item
                .get("plant_chapter_number")
                .and_then(Value::as_i64)
                .map(|number| number.to_string())
                .unwrap_or_else(|| "未知".to_owned());
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replace('\n', " ");
            format!("- [ID: {id}] 标题：{title}；埋入章节：{plant_chapter_number}；内容：{content}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_legacy_story_packet_fact_layer(snapshot: &Map<String, Value>) -> StoryPacketFactLayer {
    let mut opaque_story_facts = BTreeMap::new();
    let story_long_term_goal =
        legacy_string_or_opaque(snapshot, "story_long_term_goal", &mut opaque_story_facts);
    let character_focus =
        legacy_string_or_opaque(snapshot, "character_focus", &mut opaque_story_facts);
    let foreshadow_payoff_plan =
        legacy_string_or_opaque(snapshot, "foreshadow_payoff_plan", &mut opaque_story_facts);
    let continuity = StoryContinuitySnapshot {
        character_state_ledger: legacy_story_ledger(
            snapshot.get("character_state_ledger"),
            "character",
        ),
        relationship_state_ledger: legacy_story_ledger(
            snapshot.get("relationship_state_ledger"),
            "relationship",
        ),
        foreshadow_state_ledger: legacy_story_ledger(
            snapshot.get("foreshadow_state_ledger"),
            "foreshadow",
        ),
        organization_state_ledger: legacy_story_ledger(
            snapshot.get("organization_state_ledger"),
            "organization",
        ),
        career_state_ledger: legacy_story_ledger(snapshot.get("career_state_ledger"), "career"),
    };
    let continuity = continuity_has_entries(&continuity).then_some(continuity);

    StoryPacketFactLayer {
        sources: vec![StoryPacketSource {
            kind: StoryPacketSourceKind::RuntimeSnapshot,
            reference: Some("previous_generation_history_runtime_snapshot".to_owned()),
        }],
        story_long_term_goal,
        character_focus,
        foreshadow_payoff_plan,
        continuity,
        opaque_story_facts,
        ..StoryPacketFactLayer::default()
    }
}

fn legacy_string_or_opaque(
    snapshot: &Map<String, Value>,
    key: &str,
    opaque_story_facts: &mut BTreeMap<String, Value>,
) -> Option<String> {
    let value = snapshot.get(key)?;
    if let Some(value) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_owned());
    }
    if meaningful_legacy_value(value) {
        opaque_story_facts.insert(key.to_owned(), value.clone());
    }
    None
}

fn legacy_story_ledger(value: Option<&Value>, entity_type: &str) -> Vec<StoryLedgerEntry> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, value)| meaningful_legacy_value(value))
        .map(|(index, value)| StoryLedgerEntry {
            entity_type: entity_type.to_owned(),
            entity_id: value
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{entity_type}:{index}")),
            opaque_state: value.clone(),
        })
        .collect()
}

fn continuity_has_entries(continuity: &StoryContinuitySnapshot) -> bool {
    !continuity.character_state_ledger.is_empty()
        || !continuity.relationship_state_ledger.is_empty()
        || !continuity.foreshadow_state_ledger.is_empty()
        || !continuity.organization_state_ledger.is_empty()
        || !continuity.career_state_ledger.is_empty()
}

fn meaningful_legacy_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn positive_u32(value: i32) -> Option<u32> {
    u32::try_from(value).ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_chapter_repair_contract, build_chapter_review_contract,
        project_story_packet_to_analysis_prompt_context,
    };
    use crate::services::generation_contract_service::{GenerationTarget, StoryPacketV1};

    fn analysis_packet() -> StoryPacketV1 {
        let mut packet = StoryPacketV1::new(
            "project-1",
            GenerationTarget::chapter("project-1", "chapter-1"),
        );
        packet.opaque_story_facts.insert(
            "project".to_owned(),
            json!({"id": "project-1", "title": "项目", "world_rules": "规则"}),
        );
        packet.opaque_story_facts.insert(
            "chapter".to_owned(),
            json!({
                "id": "chapter-1",
                "project_id": "project-1",
                "chapter_number": 7,
                "title": "第七章",
                "word_count": 1234,
                "content": "章节正文",
            }),
        );
        packet.opaque_story_facts.insert(
            "characters".to_owned(),
            json!([
                {"id": "c-1", "name": "沈砚", "role_type": null, "status": "active"},
                {"id": "c-2", "name": "闻舟", "role_type": "对手", "status": "hidden"}
            ]),
        );
        packet.opaque_story_facts.insert(
            "unresolved_foreshadows".to_owned(),
            json!([{
                "id": "f-1",
                "title": "旧约定",
                "plant_chapter_number": null,
                "content": "第一行\n第二行"
            }]),
        );
        packet
    }

    #[test]
    fn should_project_analysis_prompt_context_without_changing_legacy_format() {
        let context = project_story_packet_to_analysis_prompt_context(&analysis_packet())
            .expect("project analysis context");

        assert_eq!(context.chapter_number, "7");
        assert_eq!(context.title, "第七章");
        assert_eq!(context.word_count, "1234");
        assert_eq!(context.content, "章节正文");
        assert_eq!(
            context.characters_info,
            "- 沈砚（身份：未设定；状态：active）\n- 闻舟（身份：对手；状态：hidden）"
        );
        assert_eq!(
            context.existing_foreshadows,
            "- [ID: f-1] 标题：旧约定；埋入章节：未知；内容：第一行 第二行"
        );
    }

    #[test]
    fn should_use_same_packet_for_review_and_repair_with_distinct_digest() {
        let packet = analysis_packet();
        let review = build_chapter_review_contract(packet.clone()).expect("review contract");
        let repair = build_chapter_repair_contract(packet.clone()).expect("repair contract");

        assert_eq!(review.story_packet, packet);
        assert_eq!(repair.story_packet, review.story_packet);
        assert_ne!(review.generation_intent.kind, repair.generation_intent.kind);
        assert_ne!(review.input_digest, repair.input_digest);
    }
}
