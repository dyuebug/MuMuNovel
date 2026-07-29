use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::schema_owner::{
    GenerationCreativeOverrides, GenerationIntentV1, GenerationRegenerationScope,
    StoryContinuitySnapshot, StoryPacketSource, StoryPacketV1,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoryPacketFactLayer {
    pub sources: Vec<StoryPacketSource>,
    pub current_chapter_number: Option<u32>,
    pub chapter_count: Option<u32>,
    pub target_word_count: Option<u32>,
    pub story_long_term_goal: Option<String>,
    pub character_focus: Option<String>,
    pub foreshadow_payoff_plan: Option<String>,
    pub continuity: Option<StoryContinuitySnapshot>,
    pub opaque_story_facts: BTreeMap<String, Value>,
    pub compatibility_metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenerationIntentOverrides {
    pub target_word_count: Option<u32>,
    pub creative_overrides: GenerationCreativeOverrides,
    pub regeneration_scope: Option<GenerationRegenerationScope>,
    pub compatibility_metadata: BTreeMap<String, Value>,
}

pub fn merge_story_packet_layers(
    mut system_defaults: StoryPacketV1,
    authoritative_facts: StoryPacketFactLayer,
    persisted_snapshot: Option<StoryPacketFactLayer>,
) -> StoryPacketV1 {
    apply_story_packet_fact_layer(&mut system_defaults, authoritative_facts);
    if let Some(persisted_snapshot) = persisted_snapshot {
        apply_story_packet_fact_layer(&mut system_defaults, persisted_snapshot);
    }
    system_defaults
}

pub fn apply_generation_intent_overrides(
    intent: &mut GenerationIntentV1,
    overrides: GenerationIntentOverrides,
) {
    merge_positive_u32(&mut intent.target_word_count, overrides.target_word_count);
    merge_non_empty_string(
        &mut intent.creative_overrides.narrative_style,
        overrides.creative_overrides.narrative_style,
    );
    merge_non_empty_string(
        &mut intent.creative_overrides.creative_direction,
        overrides.creative_overrides.creative_direction,
    );
    merge_non_empty_string(
        &mut intent.creative_overrides.story_direction,
        overrides.creative_overrides.story_direction,
    );
    merge_non_empty_string(
        &mut intent.creative_overrides.quality_requirements,
        overrides.creative_overrides.quality_requirements,
    );
    append_unique_non_empty(
        &mut intent.creative_overrides.extra_constraints,
        overrides.creative_overrides.extra_constraints,
    );
    merge_meaningful_values(
        &mut intent.creative_overrides.opaque_overrides,
        overrides.creative_overrides.opaque_overrides,
    );

    if let Some(mut scope) = overrides.regeneration_scope {
        scope.reason = normalized_optional_string(scope.reason);
        scope.preserve_constraints = normalized_unique_strings(scope.preserve_constraints);
        intent.regeneration_scope = Some(scope);
    }
    merge_meaningful_values(
        &mut intent.compatibility_metadata,
        overrides.compatibility_metadata,
    );
}

pub fn story_packet_to_legacy_flat_value(packet: &StoryPacketV1) -> Value {
    let mut legacy = Map::new();

    if let Some(source) = packet
        .compatibility_metadata
        .get("legacy_source")
        .cloned()
        .filter(is_meaningful_value)
    {
        legacy.insert("source".to_owned(), source);
    }
    legacy.insert(
        "project_id".to_owned(),
        Value::String(packet.project_id.clone()),
    );
    if let Some(chapter_id) = packet.target.chapter_id.as_ref() {
        legacy.insert("chapter_id".to_owned(), Value::String(chapter_id.clone()));
    }
    legacy.insert(
        "current_chapter_number".to_owned(),
        packet
            .current_chapter_number
            .map_or(Value::Null, Value::from),
    );
    legacy.insert(
        "chapter_count".to_owned(),
        packet.chapter_count.map_or(Value::Null, Value::from),
    );
    legacy.insert(
        "target_word_count".to_owned(),
        packet.target_word_count.map_or(Value::Null, Value::from),
    );
    insert_optional_string(
        &mut legacy,
        "story_long_term_goal",
        packet.story_long_term_goal.as_ref(),
    );
    insert_optional_string_or_opaque(
        &mut legacy,
        "character_focus",
        packet.character_focus.as_ref(),
        packet.opaque_story_facts.get("character_focus"),
    );
    insert_optional_string_or_opaque(
        &mut legacy,
        "foreshadow_payoff_plan",
        packet.foreshadow_payoff_plan.as_ref(),
        packet.opaque_story_facts.get("foreshadow_payoff_plan"),
    );

    for (key, value) in &packet.opaque_story_facts {
        if !is_legacy_story_packet_reserved_key(key) && is_meaningful_value(value) {
            legacy.insert(key.clone(), value.clone());
        }
    }

    insert_legacy_ledger(
        &mut legacy,
        "character_state_ledger",
        &packet.continuity.character_state_ledger,
    );
    insert_legacy_ledger(
        &mut legacy,
        "relationship_state_ledger",
        &packet.continuity.relationship_state_ledger,
    );
    insert_legacy_ledger(
        &mut legacy,
        "foreshadow_state_ledger",
        &packet.continuity.foreshadow_state_ledger,
    );
    insert_legacy_ledger(
        &mut legacy,
        "organization_state_ledger",
        &packet.continuity.organization_state_ledger,
    );
    insert_legacy_ledger(
        &mut legacy,
        "career_state_ledger",
        &packet.continuity.career_state_ledger,
    );

    Value::Object(legacy)
}

pub fn generation_intent_to_legacy_value(intent: &GenerationIntentV1) -> Value {
    let mut legacy = Map::new();
    if let Some(mode) = intent
        .compatibility_metadata
        .get("legacy_mode")
        .cloned()
        .filter(is_meaningful_value)
    {
        legacy.insert("mode".to_owned(), mode);
    }
    Value::Object(legacy)
}

pub fn fill_missing_continuity(
    existing: &mut StoryContinuitySnapshot,
    fallback: StoryContinuitySnapshot,
) {
    fill_missing_ledger(
        &mut existing.character_state_ledger,
        fallback.character_state_ledger,
    );
    fill_missing_ledger(
        &mut existing.relationship_state_ledger,
        fallback.relationship_state_ledger,
    );
    fill_missing_ledger(
        &mut existing.foreshadow_state_ledger,
        fallback.foreshadow_state_ledger,
    );
    fill_missing_ledger(
        &mut existing.organization_state_ledger,
        fallback.organization_state_ledger,
    );
    fill_missing_ledger(
        &mut existing.career_state_ledger,
        fallback.career_state_ledger,
    );
}

fn apply_story_packet_fact_layer(packet: &mut StoryPacketV1, layer: StoryPacketFactLayer) {
    append_unique_sources(&mut packet.sources, layer.sources);
    merge_positive_u32(
        &mut packet.current_chapter_number,
        layer.current_chapter_number,
    );
    merge_positive_u32(&mut packet.chapter_count, layer.chapter_count);
    merge_positive_u32(&mut packet.target_word_count, layer.target_word_count);
    merge_non_empty_string(&mut packet.story_long_term_goal, layer.story_long_term_goal);
    merge_non_empty_string(&mut packet.character_focus, layer.character_focus);
    merge_non_empty_string(
        &mut packet.foreshadow_payoff_plan,
        layer.foreshadow_payoff_plan,
    );
    if let Some(continuity) = layer.continuity {
        replace_non_empty_continuity(&mut packet.continuity, continuity);
    }
    merge_meaningful_values(&mut packet.opaque_story_facts, layer.opaque_story_facts);
    merge_meaningful_values(
        &mut packet.compatibility_metadata,
        layer.compatibility_metadata,
    );
}

fn merge_non_empty_string(current: &mut Option<String>, incoming: Option<String>) {
    if let Some(incoming) = normalized_optional_string(incoming) {
        *current = Some(incoming);
    }
}

fn normalized_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_owned())
        }
    })
}

fn merge_positive_u32(current: &mut Option<u32>, incoming: Option<u32>) {
    if incoming.is_some_and(|value| value > 0) {
        *current = incoming;
    }
}

fn append_unique_sources(current: &mut Vec<StoryPacketSource>, incoming: Vec<StoryPacketSource>) {
    for source in incoming {
        if !current.contains(&source) {
            current.push(source);
        }
    }
}

fn append_unique_non_empty(current: &mut Vec<String>, incoming: Vec<String>) {
    for value in normalized_unique_strings(incoming) {
        if !current.contains(&value) {
            current.push(value);
        }
    }
}

fn normalized_unique_strings(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !normalized.iter().any(|existing| existing == value) {
            normalized.push(value.to_owned());
        }
    }
    normalized
}

fn merge_meaningful_values(
    current: &mut BTreeMap<String, Value>,
    incoming: BTreeMap<String, Value>,
) {
    for (key, value) in incoming {
        if is_meaningful_value(&value) {
            current.insert(key, value);
        }
    }
}

fn is_meaningful_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn replace_non_empty_continuity(
    current: &mut StoryContinuitySnapshot,
    incoming: StoryContinuitySnapshot,
) {
    replace_non_empty_ledger(
        &mut current.character_state_ledger,
        incoming.character_state_ledger,
    );
    replace_non_empty_ledger(
        &mut current.relationship_state_ledger,
        incoming.relationship_state_ledger,
    );
    replace_non_empty_ledger(
        &mut current.foreshadow_state_ledger,
        incoming.foreshadow_state_ledger,
    );
    replace_non_empty_ledger(
        &mut current.organization_state_ledger,
        incoming.organization_state_ledger,
    );
    replace_non_empty_ledger(
        &mut current.career_state_ledger,
        incoming.career_state_ledger,
    );
}

fn replace_non_empty_ledger<T>(current: &mut Vec<T>, incoming: Vec<T>) {
    if !incoming.is_empty() {
        *current = incoming;
    }
}

fn fill_missing_ledger<T>(current: &mut Vec<T>, fallback: Vec<T>) {
    if current.is_empty() && !fallback.is_empty() {
        *current = fallback;
    }
}

fn insert_optional_string(legacy: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        legacy.insert(key.to_owned(), Value::String(value.to_owned()));
    }
}

fn insert_optional_string_or_opaque(
    legacy: &mut Map<String, Value>,
    key: &str,
    value: Option<&String>,
    opaque_value: Option<&Value>,
) {
    if let Some(value) = value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        legacy.insert(key.to_owned(), Value::String(value.to_owned()));
    } else if let Some(value) = opaque_value.filter(|value| is_meaningful_value(value)) {
        legacy.insert(key.to_owned(), value.clone());
    }
}

fn insert_legacy_ledger(
    legacy: &mut Map<String, Value>,
    key: &str,
    entries: &[super::schema_owner::StoryLedgerEntry],
) {
    if entries.is_empty() {
        return;
    }
    legacy.insert(
        key.to_owned(),
        Value::Array(
            entries
                .iter()
                .map(|entry| entry.opaque_state.clone())
                .collect(),
        ),
    );
}

fn is_legacy_story_packet_reserved_key(key: &str) -> bool {
    matches!(
        key,
        "schema_version"
            | "source"
            | "sources"
            | "project_id"
            | "chapter_id"
            | "target"
            | "current_chapter_number"
            | "chapter_count"
            | "target_word_count"
            | "continuity"
            | "compatibility_metadata"
            | "character_state_ledger"
            | "relationship_state_ledger"
            | "foreshadow_state_ledger"
            | "organization_state_ledger"
            | "career_state_ledger"
    )
}
