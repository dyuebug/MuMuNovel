use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::services::generation_contract_service::GenerationIntentKind;

pub const ROLE_MODEL_POLICY_SCHEMA_VERSION: &str = "role-model-policy/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationRole {
    Planner,
    Writer,
    Reviewer,
}

impl GenerationRole {
    pub fn from_intent(intent_kind: GenerationIntentKind) -> Self {
        match intent_kind {
            GenerationIntentKind::OutlineGenerate | GenerationIntentKind::OutlineExpand => {
                Self::Planner
            }
            GenerationIntentKind::ChapterGenerate
            | GenerationIntentKind::BatchChapterGenerate
            | GenerationIntentKind::ChapterRegenerate
            | GenerationIntentKind::ChapterPartialRegenerate
            | GenerationIntentKind::ChapterRepair
            | GenerationIntentKind::BookPolish => Self::Writer,
            GenerationIntentKind::ChapterReview => Self::Reviewer,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleModelSelectionV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleModelPolicyV1 {
    pub schema_version: String,
    #[serde(default)]
    pub roles: BTreeMap<GenerationRole, RoleModelSelectionV1>,
}

impl Default for RoleModelPolicyV1 {
    fn default() -> Self {
        Self {
            schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_owned(),
            roles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionSource {
    RouteOverride,
    RoleOverride,
    GlobalSettings,
    ProviderDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRoleModelPolicyV1 {
    pub role: GenerationRole,
    pub policy_schema_version: String,
    pub policy_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<String>,
    pub resolved_provider: String,
    pub resolved_model: String,
    pub provider_source: ModelSelectionSource,
    pub model_source: ModelSelectionSource,
}
