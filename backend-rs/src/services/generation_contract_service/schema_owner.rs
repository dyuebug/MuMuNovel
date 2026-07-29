use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GENERATION_CONTRACT_SCHEMA_VERSION: &str = "generation-contract/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationIntentKind {
    OutlineGenerate,
    OutlineExpand,
    ChapterGenerate,
    BatchChapterGenerate,
    ChapterRegenerate,
    ChapterPartialRegenerate,
    ChapterReview,
    ChapterRepair,
    BookPolish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationTargetKind {
    Outline,
    Chapter,
    ChapterBatch,
    ChapterSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSelection {
    pub start_index: usize,
    pub end_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationTarget {
    pub kind: GenerationTargetKind,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapter_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<GenerationSelection>,
}

impl GenerationTarget {
    pub fn outline(project_id: impl Into<String>, outline_id: Option<String>) -> Self {
        Self {
            kind: GenerationTargetKind::Outline,
            project_id: project_id.into(),
            outline_id,
            chapter_id: None,
            chapter_ids: Vec::new(),
            selection: None,
        }
    }

    pub fn chapter(project_id: impl Into<String>, chapter_id: impl Into<String>) -> Self {
        Self {
            kind: GenerationTargetKind::Chapter,
            project_id: project_id.into(),
            outline_id: None,
            chapter_id: Some(chapter_id.into()),
            chapter_ids: Vec::new(),
            selection: None,
        }
    }

    pub fn chapter_batch(project_id: impl Into<String>, chapter_ids: Vec<String>) -> Self {
        Self {
            kind: GenerationTargetKind::ChapterBatch,
            project_id: project_id.into(),
            outline_id: None,
            chapter_id: None,
            chapter_ids,
            selection: None,
        }
    }

    pub fn chapter_selection(
        project_id: impl Into<String>,
        chapter_id: impl Into<String>,
        selection: GenerationSelection,
    ) -> Self {
        Self {
            kind: GenerationTargetKind::ChapterSelection,
            project_id: project_id.into(),
            outline_id: None,
            chapter_id: Some(chapter_id.into()),
            chapter_ids: Vec::new(),
            selection: Some(selection),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryPacketSourceKind {
    SystemDefaults,
    AuthoritativeDatabase,
    RuntimeSnapshot,
    GenerationHistory,
    LegacyRequestAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryPacketSource {
    pub kind: StoryPacketSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryLedgerEntry {
    pub entity_type: String,
    pub entity_id: String,
    pub opaque_state: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryContinuitySnapshot {
    #[serde(default)]
    pub character_state_ledger: Vec<StoryLedgerEntry>,
    #[serde(default)]
    pub relationship_state_ledger: Vec<StoryLedgerEntry>,
    #[serde(default)]
    pub foreshadow_state_ledger: Vec<StoryLedgerEntry>,
    #[serde(default)]
    pub organization_state_ledger: Vec<StoryLedgerEntry>,
    #[serde(default)]
    pub career_state_ledger: Vec<StoryLedgerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryPacketV1 {
    pub schema_version: String,
    pub project_id: String,
    pub target: GenerationTarget,
    #[serde(default)]
    pub sources: Vec<StoryPacketSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_chapter_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_word_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_long_term_goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreshadow_payoff_plan: Option<String>,
    #[serde(default)]
    pub continuity: StoryContinuitySnapshot,
    #[serde(default)]
    pub opaque_story_facts: BTreeMap<String, Value>,
    #[serde(default)]
    pub compatibility_metadata: BTreeMap<String, Value>,
}

impl StoryPacketV1 {
    pub fn new(project_id: impl Into<String>, target: GenerationTarget) -> Self {
        Self {
            schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
            project_id: project_id.into(),
            target,
            sources: Vec::new(),
            current_chapter_number: None,
            chapter_count: None,
            target_word_count: None,
            story_long_term_goal: None,
            character_focus: None,
            foreshadow_payoff_plan: None,
            continuity: StoryContinuitySnapshot::default(),
            opaque_story_facts: BTreeMap::new(),
            compatibility_metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationCreativeOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creative_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_requirements: Option<String>,
    #[serde(default)]
    pub extra_constraints: Vec<String>,
    #[serde(default)]
    pub opaque_overrides: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationRegenerationScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<GenerationSelection>,
    #[serde(default)]
    pub preserve_constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationIntentV1 {
    pub schema_version: String,
    pub kind: GenerationIntentKind,
    pub target: GenerationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_word_count: Option<u32>,
    #[serde(default)]
    pub creative_overrides: GenerationCreativeOverrides,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regeneration_scope: Option<GenerationRegenerationScope>,
    #[serde(default)]
    pub compatibility_metadata: BTreeMap<String, Value>,
}

impl GenerationIntentV1 {
    pub fn new(kind: GenerationIntentKind, target: GenerationTarget) -> Self {
        Self {
            schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
            kind,
            target,
            target_word_count: None,
            creative_overrides: GenerationCreativeOverrides::default(),
            regeneration_scope: None,
            compatibility_metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationContractSnapshotV1 {
    pub schema_version: String,
    pub story_packet: StoryPacketV1,
    pub generation_intent: GenerationIntentV1,
    pub input_digest: String,
}
