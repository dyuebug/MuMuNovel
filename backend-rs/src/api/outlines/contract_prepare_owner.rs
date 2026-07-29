use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::models::{outline, project};
use crate::services::generation_contract_service::{
    apply_generation_intent_overrides, build_generation_contract_snapshot, GenerationContractError,
    GenerationContractSnapshotV1, GenerationCreativeOverrides, GenerationIntentKind,
    GenerationIntentOverrides, GenerationIntentV1, GenerationTarget, StoryPacketSource,
    StoryPacketSourceKind, StoryPacketV1,
};
use crate::services::wizard_service::build_project_long_term_goal;

const LEGACY_MODE_NEW: &str = "outline_generate_new";
const LEGACY_MODE_CONTINUE: &str = "outline_generate_continue";
const LEGACY_MODE_EXPAND: &str = "outline_expand_single";
const LEGACY_MODE_BATCH_EXPAND: &str = "outline_expand_batch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutlineGenerateContractMode {
    New,
    Continue,
}

impl OutlineGenerateContractMode {
    fn legacy_mode(self) -> &'static str {
        match self {
            Self::New => LEGACY_MODE_NEW,
            Self::Continue => LEGACY_MODE_CONTINUE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OutlineGenerateContractInput<'a> {
    pub chapter_count: usize,
    pub target_words: Option<i32>,
    pub narrative_perspective: Option<&'a str>,
    pub requirements: Option<&'a str>,
    pub creative_mode: Option<&'a str>,
    pub story_focus: Option<&'a str>,
    pub plot_stage: Option<&'a str>,
    pub story_creation_brief: Option<&'a str>,
    pub quality_preset: Option<&'a str>,
    pub quality_notes: Option<&'a str>,
    pub compact_mode: Option<bool>,
    pub story_direction: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedOutlineGenerateParameters {
    pub chapter_count: usize,
    pub target_words: i32,
    pub narrative_perspective: Option<String>,
    pub requirements: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub compact_mode: bool,
    pub story_direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedOutlineGenerateContract {
    pub snapshot: GenerationContractSnapshotV1,
    pub resolved: ResolvedOutlineGenerateParameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutlineExpandContractMode {
    Single,
    Batch,
}

impl OutlineExpandContractMode {
    fn legacy_mode(self) -> &'static str {
        match self {
            Self::Single => LEGACY_MODE_EXPAND,
            Self::Batch => LEGACY_MODE_BATCH_EXPAND,
        }
    }

    fn chapter_count_key(self) -> &'static str {
        match self {
            Self::Single => "target_chapter_count",
            Self::Batch => "chapters_per_outline",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OutlineExpandContractInput<'a> {
    pub mode: OutlineExpandContractMode,
    pub target_chapter_count: usize,
    pub expansion_strategy: &'a str,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedOutlineExpandParameters {
    pub target_chapter_count: usize,
    pub expansion_strategy: String,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedOutlineExpandContract {
    pub snapshot: GenerationContractSnapshotV1,
    pub resolved: ResolvedOutlineExpandParameters,
}

pub(super) fn prepare_outline_generate_contract(
    project_model: &project::Model,
    mode: OutlineGenerateContractMode,
    input: OutlineGenerateContractInput<'_>,
) -> Result<PreparedOutlineGenerateContract, GenerationContractError> {
    let target = GenerationTarget::outline(project_model.id.clone(), None);
    let mut intent = GenerationIntentV1::new(GenerationIntentKind::OutlineGenerate, target.clone());

    apply_generation_intent_overrides(&mut intent, generate_system_defaults(mode));
    apply_generation_intent_overrides(&mut intent, generate_project_defaults(project_model));
    apply_generation_intent_overrides(&mut intent, generate_request_overrides(mode, input));

    let resolved = resolve_generate_parameters(project_model, &intent);
    let story_packet = build_project_story_packet(
        project_model,
        target,
        mode.legacy_mode(),
        resolved.story_creation_brief.as_deref(),
        generate_long_term_goal_target_words(project_model, mode, &resolved),
        None,
    );
    let snapshot = build_generation_contract_snapshot(story_packet, intent)?;

    Ok(PreparedOutlineGenerateContract { snapshot, resolved })
}

pub(super) fn prepare_outline_expand_contract(
    project_model: &project::Model,
    outline_model: &outline::Model,
    input: OutlineExpandContractInput<'_>,
) -> Result<PreparedOutlineExpandContract, GenerationContractError> {
    let target =
        GenerationTarget::outline(project_model.id.clone(), Some(outline_model.id.clone()));
    let mut intent = GenerationIntentV1::new(GenerationIntentKind::OutlineExpand, target.clone());

    let mut system_defaults = GenerationIntentOverrides::default();
    system_defaults.creative_overrides.creative_direction = Some("balanced".to_owned());
    system_defaults
        .creative_overrides
        .opaque_overrides
        .insert("batch_size".to_owned(), json!(5));
    apply_generation_intent_overrides(&mut intent, system_defaults);

    let mut request_overrides = GenerationIntentOverrides::default();
    request_overrides.creative_overrides.creative_direction =
        normalized_owned(Some(input.expansion_strategy));
    request_overrides
        .creative_overrides
        .opaque_overrides
        .insert(
            input.mode.chapter_count_key().to_owned(),
            json!(input.target_chapter_count),
        );
    request_overrides
        .creative_overrides
        .opaque_overrides
        .insert(
            "auto_create_chapters".to_owned(),
            json!(input.auto_create_chapters),
        );
    request_overrides
        .creative_overrides
        .opaque_overrides
        .insert(
            "enable_scene_analysis".to_owned(),
            json!(input.enable_scene_analysis),
        );
    request_overrides
        .creative_overrides
        .opaque_overrides
        .insert("batch_size".to_owned(), json!(input.batch_size));
    request_overrides.compatibility_metadata.insert(
        "legacy_mode".to_owned(),
        Value::String(input.mode.legacy_mode().to_owned()),
    );
    apply_generation_intent_overrides(&mut intent, request_overrides);

    let resolved = resolve_expand_parameters(&intent, input.mode);
    let story_packet = build_project_story_packet(
        project_model,
        target,
        input.mode.legacy_mode(),
        project_model.default_story_creation_brief.as_deref(),
        positive_u32(project_model.target_words).and_then(|value| i32::try_from(value).ok()),
        Some(outline_model),
    );
    let snapshot = build_generation_contract_snapshot(story_packet, intent)?;

    Ok(PreparedOutlineExpandContract { snapshot, resolved })
}

fn generate_system_defaults(mode: OutlineGenerateContractMode) -> GenerationIntentOverrides {
    let mut defaults = GenerationIntentOverrides::default();
    defaults
        .creative_overrides
        .opaque_overrides
        .insert("compact_mode".to_owned(), Value::Bool(true));

    if mode == OutlineGenerateContractMode::Continue {
        defaults.creative_overrides.narrative_style = Some("第三人称".to_owned());
        defaults.creative_overrides.story_direction = Some("自然延续".to_owned());
        defaults.creative_overrides.opaque_overrides.insert(
            "plot_stage".to_owned(),
            Value::String("development".to_owned()),
        );
    }

    defaults
}

fn generate_project_defaults(project_model: &project::Model) -> GenerationIntentOverrides {
    let mut defaults = GenerationIntentOverrides {
        target_word_count: positive_u32(project_model.target_words),
        creative_overrides: GenerationCreativeOverrides {
            narrative_style: normalized_owned(project_model.narrative_perspective.as_deref()),
            creative_direction: normalized_owned(project_model.default_creative_mode.as_deref()),
            story_direction: None,
            quality_requirements: normalized_owned(project_model.default_quality_notes.as_deref()),
            extra_constraints: Vec::new(),
            opaque_overrides: BTreeMap::new(),
        },
        regeneration_scope: None,
        compatibility_metadata: BTreeMap::new(),
    };

    insert_optional_string(
        &mut defaults.creative_overrides.opaque_overrides,
        "story_focus",
        project_model.default_story_focus.as_deref(),
    );
    insert_optional_string(
        &mut defaults.creative_overrides.opaque_overrides,
        "plot_stage",
        project_model.default_plot_stage.as_deref(),
    );
    insert_optional_string(
        &mut defaults.creative_overrides.opaque_overrides,
        "story_creation_brief",
        project_model.default_story_creation_brief.as_deref(),
    );
    insert_optional_string(
        &mut defaults.creative_overrides.opaque_overrides,
        "quality_preset",
        project_model.default_quality_preset.as_deref(),
    );
    if let Some(chapter_count) = project_model.chapter_count.and_then(positive_u32) {
        defaults
            .creative_overrides
            .opaque_overrides
            .insert("chapter_count".to_owned(), json!(chapter_count));
    }

    defaults
}

fn generate_request_overrides(
    mode: OutlineGenerateContractMode,
    input: OutlineGenerateContractInput<'_>,
) -> GenerationIntentOverrides {
    let effective_chapter_count = match mode {
        OutlineGenerateContractMode::New => input.chapter_count.clamp(1, 10),
        OutlineGenerateContractMode::Continue => input.chapter_count,
    };
    let mut overrides = GenerationIntentOverrides {
        target_word_count: input.target_words.and_then(positive_u32),
        creative_overrides: GenerationCreativeOverrides {
            narrative_style: normalized_owned(input.narrative_perspective),
            creative_direction: normalized_owned(input.creative_mode),
            story_direction: normalized_owned(input.story_direction),
            quality_requirements: normalized_owned(input.quality_notes),
            extra_constraints: Vec::new(),
            opaque_overrides: BTreeMap::new(),
        },
        regeneration_scope: None,
        compatibility_metadata: BTreeMap::new(),
    };

    overrides
        .creative_overrides
        .opaque_overrides
        .insert("chapter_count".to_owned(), json!(effective_chapter_count));
    insert_optional_string(
        &mut overrides.creative_overrides.opaque_overrides,
        "requirements",
        input.requirements,
    );
    insert_optional_string(
        &mut overrides.creative_overrides.opaque_overrides,
        "story_focus",
        input.story_focus,
    );
    insert_optional_string(
        &mut overrides.creative_overrides.opaque_overrides,
        "plot_stage",
        input.plot_stage,
    );
    insert_optional_string(
        &mut overrides.creative_overrides.opaque_overrides,
        "story_creation_brief",
        input.story_creation_brief,
    );
    insert_optional_string(
        &mut overrides.creative_overrides.opaque_overrides,
        "quality_preset",
        input.quality_preset,
    );
    if let Some(compact_mode) = input.compact_mode {
        overrides
            .creative_overrides
            .opaque_overrides
            .insert("compact_mode".to_owned(), Value::Bool(compact_mode));
    }
    overrides.compatibility_metadata.insert(
        "legacy_mode".to_owned(),
        Value::String(mode.legacy_mode().to_owned()),
    );

    overrides
}

fn resolve_generate_parameters(
    project_model: &project::Model,
    intent: &GenerationIntentV1,
) -> ResolvedOutlineGenerateParameters {
    ResolvedOutlineGenerateParameters {
        chapter_count: opaque_usize(intent, "chapter_count").unwrap_or(1),
        target_words: intent
            .target_word_count
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(project_model.target_words),
        narrative_perspective: intent.creative_overrides.narrative_style.clone(),
        requirements: opaque_string(intent, "requirements"),
        creative_mode: intent.creative_overrides.creative_direction.clone(),
        story_focus: opaque_string(intent, "story_focus"),
        plot_stage: opaque_string(intent, "plot_stage"),
        story_creation_brief: opaque_string(intent, "story_creation_brief"),
        quality_preset: opaque_string(intent, "quality_preset"),
        quality_notes: intent.creative_overrides.quality_requirements.clone(),
        compact_mode: opaque_bool(intent, "compact_mode").unwrap_or(true),
        story_direction: intent.creative_overrides.story_direction.clone(),
    }
}

fn resolve_expand_parameters(
    intent: &GenerationIntentV1,
    mode: OutlineExpandContractMode,
) -> ResolvedOutlineExpandParameters {
    ResolvedOutlineExpandParameters {
        target_chapter_count: opaque_usize(intent, mode.chapter_count_key()).unwrap_or_default(),
        expansion_strategy: intent
            .creative_overrides
            .creative_direction
            .clone()
            .unwrap_or_else(|| "balanced".to_owned()),
        auto_create_chapters: opaque_bool(intent, "auto_create_chapters").unwrap_or(false),
        enable_scene_analysis: opaque_bool(intent, "enable_scene_analysis").unwrap_or(false),
        batch_size: opaque_usize(intent, "batch_size").unwrap_or(5),
    }
}

fn build_project_story_packet(
    project_model: &project::Model,
    target: GenerationTarget,
    legacy_mode: &str,
    story_creation_brief: Option<&str>,
    long_term_goal_target_words: Option<i32>,
    outline_model: Option<&outline::Model>,
) -> StoryPacketV1 {
    let mut packet = StoryPacketV1::new(project_model.id.clone(), target);
    packet.sources = vec![
        StoryPacketSource {
            kind: StoryPacketSourceKind::SystemDefaults,
            reference: Some("outline_contract_defaults".to_owned()),
        },
        StoryPacketSource {
            kind: StoryPacketSourceKind::AuthoritativeDatabase,
            reference: Some(format!("project:{}", project_model.id)),
        },
        StoryPacketSource {
            kind: StoryPacketSourceKind::LegacyRequestAdapter,
            reference: Some(legacy_mode.to_owned()),
        },
    ];
    packet.chapter_count = project_model.chapter_count.and_then(positive_u32);
    packet.target_word_count = positive_u32(project_model.target_words);
    packet.story_long_term_goal = build_project_long_term_goal(
        project_model.theme.as_deref(),
        project_model.description.as_deref(),
        story_creation_brief,
        project_model
            .chapter_count
            .and_then(|value| usize::try_from(value).ok()),
        long_term_goal_target_words.and_then(|value| usize::try_from(value).ok()),
    );
    packet.compatibility_metadata.insert(
        "legacy_mode".to_owned(),
        Value::String(legacy_mode.to_owned()),
    );

    insert_required_string(
        &mut packet.opaque_story_facts,
        "project_title",
        &project_model.title,
    );
    insert_optional_string(
        &mut packet.opaque_story_facts,
        "project_description",
        project_model.description.as_deref(),
    );
    insert_optional_string(
        &mut packet.opaque_story_facts,
        "theme",
        project_model.theme.as_deref(),
    );
    insert_optional_string(
        &mut packet.opaque_story_facts,
        "genre",
        project_model.genre.as_deref(),
    );
    insert_required_string(
        &mut packet.opaque_story_facts,
        "outline_mode",
        &project_model.outline_mode,
    );

    let mut world_context = BTreeMap::new();
    insert_optional_string(
        &mut world_context,
        "time_period",
        project_model.world_time_period.as_deref(),
    );
    insert_optional_string(
        &mut world_context,
        "location",
        project_model.world_location.as_deref(),
    );
    insert_optional_string(
        &mut world_context,
        "atmosphere",
        project_model.world_atmosphere.as_deref(),
    );
    insert_optional_string(
        &mut world_context,
        "rules",
        project_model.world_rules.as_deref(),
    );
    if !world_context.is_empty() {
        packet
            .opaque_story_facts
            .insert("world_context".to_owned(), json!(world_context));
    }

    if let Some(outline_model) = outline_model {
        packet.sources.push(StoryPacketSource {
            kind: StoryPacketSourceKind::AuthoritativeDatabase,
            reference: Some(format!("outline:{}", outline_model.id)),
        });
        packet.opaque_story_facts.insert(
            "outline".to_owned(),
            json!({
                "id": outline_model.id,
                "title": outline_model.title,
                "content": outline_model.content,
                "structure": outline_model.structure,
                "order_index": outline_model.order_index,
            }),
        );
    }

    packet
}

fn generate_long_term_goal_target_words(
    project_model: &project::Model,
    mode: OutlineGenerateContractMode,
    resolved: &ResolvedOutlineGenerateParameters,
) -> Option<i32> {
    match mode {
        OutlineGenerateContractMode::New => Some(resolved.target_words),
        OutlineGenerateContractMode::Continue => Some(project_model.target_words),
    }
}

fn insert_required_string(map: &mut BTreeMap<String, Value>, key: &str, value: &str) {
    if let Some(value) = normalized_owned(Some(value)) {
        map.insert(key.to_owned(), Value::String(value));
    }
}

fn insert_optional_string(map: &mut BTreeMap<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = normalized_owned(value) {
        map.insert(key.to_owned(), Value::String(value));
    }
}

fn normalized_owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn positive_u32<T>(value: T) -> Option<u32>
where
    i64: From<T>,
{
    let value = i64::from(value);
    u32::try_from(value).ok().filter(|value| *value > 0)
}

fn opaque_value<'a>(intent: &'a GenerationIntentV1, key: &str) -> Option<&'a Value> {
    intent.creative_overrides.opaque_overrides.get(key)
}

fn opaque_string(intent: &GenerationIntentV1, key: &str) -> Option<String> {
    opaque_value(intent, key)
        .and_then(Value::as_str)
        .and_then(|value| normalized_owned(Some(value)))
}

fn opaque_usize(intent: &GenerationIntentV1, key: &str) -> Option<usize> {
    opaque_value(intent, key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn opaque_bool(intent: &GenerationIntentV1, key: &str) -> Option<bool> {
    opaque_value(intent, key).and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::services::generation_contract_service::{
        GenerationIntentKind, GenerationTargetKind,
    };

    fn project_model() -> project::Model {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 14)
            .expect("date")
            .and_hms_opt(8, 0, 0)
            .expect("time");
        project::Model {
            id: "project-1".to_owned(),
            user_id: "user-1".to_owned(),
            title: "契约测试小说".to_owned(),
            description: Some("项目描述".to_owned()),
            theme: Some("选择与代价".to_owned()),
            genre: Some("幻想".to_owned()),
            target_words: 120_000,
            current_words: 0,
            status: "outline".to_owned(),
            wizard_status: "completed".to_owned(),
            wizard_step: 5,
            outline_mode: "one-to-many".to_owned(),
            world_time_period: Some("架空近代".to_owned()),
            world_location: Some("雾港".to_owned()),
            world_atmosphere: Some("紧张".to_owned()),
            world_rules: Some("魔法需要支付记忆".to_owned()),
            chapter_count: Some(40),
            narrative_perspective: Some("第三人称限知".to_owned()),
            character_count: 4,
            default_creative_mode: Some("稳健推进".to_owned()),
            default_story_focus: Some("角色成长".to_owned()),
            default_plot_stage: Some("development".to_owned()),
            default_story_creation_brief: Some("保持悬疑感".to_owned()),
            default_quality_preset: Some("balanced".to_owned()),
            default_quality_notes: Some("避免重复解释".to_owned()),
            created_at,
            updated_at: None,
        }
    }

    fn outline_model(id: &str, title: &str, order_index: i32) -> outline::Model {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 14)
            .expect("date")
            .and_hms_opt(9, 0, 0)
            .expect("time");
        outline::Model {
            id: id.to_owned(),
            project_id: "project-1".to_owned(),
            title: title.to_owned(),
            content: Some(format!("{title}的剧情内容")),
            structure: Some("起承转合".to_owned()),
            order_index: Some(order_index),
            created_at,
            updated_at: None,
        }
    }

    fn generate_input<'a>() -> OutlineGenerateContractInput<'a> {
        OutlineGenerateContractInput {
            chapter_count: 6,
            target_words: Some(180_000),
            narrative_perspective: Some("  "),
            requirements: Some("强化开篇冲突"),
            creative_mode: Some("高张力推进"),
            story_focus: Some("主角抉择"),
            plot_stage: Some("climax"),
            story_creation_brief: Some("围绕失忆真相展开"),
            quality_preset: Some("strict"),
            quality_notes: Some("减少说明性旁白"),
            compact_mode: Some(false),
            story_direction: None,
        }
    }

    #[test]
    fn new_outline_contract_applies_project_defaults_then_non_empty_request_overrides() {
        let project = project_model();
        let prepared = prepare_outline_generate_contract(
            &project,
            OutlineGenerateContractMode::New,
            generate_input(),
        )
        .expect("prepare new outline contract");

        assert_eq!(
            prepared.snapshot.generation_intent.kind,
            GenerationIntentKind::OutlineGenerate
        );
        assert_eq!(
            prepared.snapshot.story_packet.target.kind,
            GenerationTargetKind::Outline
        );
        assert_eq!(prepared.snapshot.story_packet.target.outline_id, None);
        assert_eq!(prepared.resolved.chapter_count, 6);
        assert_eq!(prepared.resolved.target_words, 180_000);
        assert_eq!(
            prepared.resolved.narrative_perspective.as_deref(),
            Some("第三人称限知")
        );
        assert_eq!(
            prepared.resolved.creative_mode.as_deref(),
            Some("高张力推进")
        );
        assert_eq!(prepared.resolved.story_focus.as_deref(), Some("主角抉择"));
        assert_eq!(prepared.resolved.plot_stage.as_deref(), Some("climax"));
        assert_eq!(
            prepared.resolved.quality_notes.as_deref(),
            Some("减少说明性旁白")
        );
        assert!(!prepared.resolved.compact_mode);

        let serialized = serde_json::to_string(&prepared.snapshot).expect("serialize snapshot");
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("provider"));
        assert!(!serialized.contains("model"));
    }

    #[test]
    fn continue_outline_contract_keeps_defaults_when_request_strings_are_empty() {
        let project = project_model();
        let prepared = prepare_outline_generate_contract(
            &project,
            OutlineGenerateContractMode::Continue,
            OutlineGenerateContractInput {
                chapter_count: 3,
                target_words: None,
                narrative_perspective: Some(" "),
                requirements: None,
                creative_mode: Some(""),
                story_focus: Some(" "),
                plot_stage: Some(""),
                story_creation_brief: None,
                quality_preset: Some(""),
                quality_notes: Some(" "),
                compact_mode: None,
                story_direction: Some(""),
            },
        )
        .expect("prepare continue outline contract");

        assert_eq!(prepared.snapshot.generation_intent.target.outline_id, None);
        assert_eq!(
            prepared.resolved.narrative_perspective.as_deref(),
            Some("第三人称限知")
        );
        assert_eq!(prepared.resolved.creative_mode.as_deref(), Some("稳健推进"));
        assert_eq!(prepared.resolved.story_focus.as_deref(), Some("角色成长"));
        assert_eq!(prepared.resolved.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            prepared.resolved.story_creation_brief.as_deref(),
            Some("保持悬疑感")
        );
        assert_eq!(
            prepared.resolved.quality_preset.as_deref(),
            Some("balanced")
        );
        assert_eq!(
            prepared.resolved.quality_notes.as_deref(),
            Some("避免重复解释")
        );
        assert_eq!(
            prepared.resolved.story_direction.as_deref(),
            Some("自然延续")
        );
        assert!(prepared.resolved.compact_mode);
    }

    #[test]
    fn meaningful_generate_override_changes_digest() {
        let project = project_model();
        let baseline = prepare_outline_generate_contract(
            &project,
            OutlineGenerateContractMode::New,
            generate_input(),
        )
        .expect("baseline");
        let mut changed_input = generate_input();
        changed_input.narrative_perspective = Some("第一人称");
        let changed = prepare_outline_generate_contract(
            &project,
            OutlineGenerateContractMode::New,
            changed_input,
        )
        .expect("changed");

        assert_ne!(
            baseline.snapshot.input_digest,
            changed.snapshot.input_digest
        );
    }

    #[test]
    fn single_expand_contract_targets_actual_outline_and_projects_runtime_parameters() {
        let project = project_model();
        let outline = outline_model("outline-1", "迷雾来信", 1);
        let prepared = prepare_outline_expand_contract(
            &project,
            &outline,
            OutlineExpandContractInput {
                mode: OutlineExpandContractMode::Single,
                target_chapter_count: 8,
                expansion_strategy: "character_driven",
                auto_create_chapters: true,
                enable_scene_analysis: true,
                batch_size: 4,
            },
        )
        .expect("prepare single expand contract");

        assert_eq!(
            prepared.snapshot.generation_intent.kind,
            GenerationIntentKind::OutlineExpand
        );
        assert_eq!(
            prepared.snapshot.story_packet.target.kind,
            GenerationTargetKind::Outline
        );
        assert_eq!(
            prepared.snapshot.story_packet.target.outline_id.as_deref(),
            Some("outline-1")
        );
        assert_eq!(prepared.resolved.target_chapter_count, 8);
        assert_eq!(prepared.resolved.expansion_strategy, "character_driven");
        assert!(prepared.resolved.auto_create_chapters);
        assert!(prepared.resolved.enable_scene_analysis);
        assert_eq!(prepared.resolved.batch_size, 4);
        assert_eq!(
            prepared.snapshot.story_packet.opaque_story_facts["outline"]["title"],
            json!("迷雾来信")
        );
    }

    #[test]
    fn batch_expand_builds_independent_target_and_digest_for_each_outline() {
        let project = project_model();
        let first = prepare_outline_expand_contract(
            &project,
            &outline_model("outline-1", "第一幕", 1),
            OutlineExpandContractInput {
                mode: OutlineExpandContractMode::Batch,
                target_chapter_count: 5,
                expansion_strategy: "balanced",
                auto_create_chapters: false,
                enable_scene_analysis: true,
                batch_size: 5,
            },
        )
        .expect("first batch contract");
        let second = prepare_outline_expand_contract(
            &project,
            &outline_model("outline-2", "第二幕", 2),
            OutlineExpandContractInput {
                mode: OutlineExpandContractMode::Batch,
                target_chapter_count: 5,
                expansion_strategy: "balanced",
                auto_create_chapters: false,
                enable_scene_analysis: true,
                batch_size: 5,
            },
        )
        .expect("second batch contract");

        assert_eq!(
            first
                .snapshot
                .generation_intent
                .target
                .outline_id
                .as_deref(),
            Some("outline-1")
        );
        assert_eq!(
            second
                .snapshot
                .generation_intent
                .target
                .outline_id
                .as_deref(),
            Some("outline-2")
        );
        assert_ne!(first.snapshot.input_digest, second.snapshot.input_digest);
        assert_eq!(first.resolved.target_chapter_count, 5);
        assert_eq!(first.resolved.batch_size, 5);
    }
}
