use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

pub const NOVEL_AUTOPILOT_SCHEMA_VERSION: &str = "novel-autopilot/v1";
pub const DEFAULT_GATE_INTERVAL: u32 = 5;
pub const DEFAULT_MAX_CHAPTERS: u32 = 200;
pub const DEFAULT_MAX_TOKENS: u64 = 4_000_000;
pub const DEFAULT_MAX_RUNTIME_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const DEFAULT_MAX_STEP_ATTEMPTS: u32 = 3;
pub const DEFAULT_MAX_CONSECUTIVE_PROVIDER_FAILURES: u32 = 5;
pub const DEFAULT_MAX_CONSECUTIVE_QUALITY_FAILURES: u32 = 5;

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $value:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant)),+,
                    _ => Err(format!("unsupported {}: {value}", stringify!($name))),
                }
            }
        }
    };
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotRunStatus {
        Queued => "queued",
        Running => "running",
        Paused => "paused",
        WaitingHuman => "waiting_human",
        Completed => "completed",
        Failed => "failed",
        Cancelled => "cancelled",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NovelAutopilotFailureCounterKind {
    Provider,
    Quality,
    None,
}

impl NovelAutopilotRunStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub const fn is_active(self) -> bool {
        !self.is_terminal()
    }

    pub const fn can_schedule(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    pub const fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Queued, Self::Running)
                | (Self::Queued, Self::WaitingHuman)
                | (Self::Queued, Self::Cancelled)
                | (Self::Running, Self::Paused)
                | (Self::Running, Self::WaitingHuman)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Paused, Self::Queued)
                | (Self::Paused, Self::Cancelled)
                | (Self::WaitingHuman, Self::Queued)
                | (Self::WaitingHuman, Self::Cancelled)
                | (Self::WaitingHuman, Self::Failed)
        )
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotPhase {
        Validate => "validate",
        Foundation => "foundation",
        WorldBuilding => "world_building",
        CareerDesign => "career_design",
        CharacterDesign => "character_design",
        OrganizationDesign => "organization_design",
        Outline => "outline",
        ChapterLoop => "chapter_loop",
        BookReview => "book_review",
        BookPolish => "book_polish",
        Export => "export",
        Completed => "completed",
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotExecutionScope {
        PlanningOnly => "planning_only",
        NextNChapters => "next_n_chapters",
        ContinueFromCurrent => "continue_from_current",
        CompleteBook => "complete_book",
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotHumanGateMode {
        FullyAutomatic => "fully_automatic",
        HighRiskOnly => "high_risk_only",
        EveryNChapters => "every_n_chapters",
        EveryVolume => "every_volume",
        EveryChapter => "every_chapter",
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotStepStatus {
        Queued => "queued",
        Running => "running",
        Completed => "completed",
        Skipped => "skipped",
        Failed => "failed",
        Cancelled => "cancelled",
        Stale => "stale",
    }
}

impl NovelAutopilotStepStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Skipped | Self::Failed | Self::Cancelled | Self::Stale
        )
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotStepType {
        Validate => "validate",
        Foundation => "foundation",
        WorldBuilding => "world_building",
        CareerDesign => "career_design",
        CharacterDesign => "character_design",
        OrganizationDesign => "organization_design",
        Outline => "outline",
        OutlineExpand => "outline_expand",
        ChapterGenerate => "chapter_generate",
        ChapterAnalyze => "chapter_analyze",
        ChapterRepair => "chapter_repair",
        BookReview => "book_review",
        BookPolish => "book_polish",
        Export => "export",
    }
}

string_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum NovelAutopilotQualityDecision {
        Accept => "accept",
        AutoRepair => "auto_repair",
        Retry => "retry",
        ManualReview => "manual_review",
        Reject => "reject",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NovelAutopilotRunConfig {
    pub execution_scope: NovelAutopilotExecutionScope,
    pub human_gate_mode: NovelAutopilotHumanGateMode,
    pub gate_interval: u32,
    pub next_chapter_count: Option<u32>,
    pub max_chapters: u32,
    pub max_tokens: u64,
    pub max_estimated_cost: Option<f64>,
    pub max_runtime_seconds: u64,
    pub max_step_attempts: u32,
    pub max_consecutive_provider_failures: u32,
    pub max_consecutive_quality_failures: u32,
    pub regenerate_existing: bool,
    pub run_book_review: bool,
    pub run_book_polish: bool,
    pub export_format: String,
}

impl Default for NovelAutopilotRunConfig {
    fn default() -> Self {
        Self {
            execution_scope: NovelAutopilotExecutionScope::CompleteBook,
            human_gate_mode: NovelAutopilotHumanGateMode::HighRiskOnly,
            gate_interval: DEFAULT_GATE_INTERVAL,
            next_chapter_count: None,
            max_chapters: DEFAULT_MAX_CHAPTERS,
            max_tokens: DEFAULT_MAX_TOKENS,
            max_estimated_cost: None,
            max_runtime_seconds: DEFAULT_MAX_RUNTIME_SECONDS,
            max_step_attempts: DEFAULT_MAX_STEP_ATTEMPTS,
            max_consecutive_provider_failures: DEFAULT_MAX_CONSECUTIVE_PROVIDER_FAILURES,
            max_consecutive_quality_failures: DEFAULT_MAX_CONSECUTIVE_QUALITY_FAILURES,
            regenerate_existing: false,
            run_book_review: true,
            run_book_polish: true,
            export_format: "txt".to_string(),
        }
    }
}

/// Private durable payload stored in `novel_autopilot_runs.config_snapshot`.
///
/// This envelope is never serialized into public API responses, task results, or invocation
/// audits. `decode` remains compatible with the legacy direct `NovelAutopilotRunConfig` shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NovelAutopilotPrivateSnapshot {
    pub config: NovelAutopilotRunConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<String>,
}

impl NovelAutopilotPrivateSnapshot {
    pub(crate) fn new(config: NovelAutopilotRunConfig) -> Self {
        Self {
            config,
            guidance: None,
        }
    }

    pub(crate) fn decode(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value.clone()).or_else(|_| {
            serde_json::from_value::<NovelAutopilotRunConfig>(value.clone()).map(Self::new)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovelAutopilotConfigValidationError {
    pub field: &'static str,
    pub code: &'static str,
}

impl fmt::Display for NovelAutopilotConfigValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.code)
    }
}

impl std::error::Error for NovelAutopilotConfigValidationError {}

impl NovelAutopilotRunConfig {
    pub fn validate(&self) -> Result<(), NovelAutopilotConfigValidationError> {
        if self.max_chapters == 0 {
            return Err(invalid("max_chapters", "must_be_positive"));
        }
        if self.max_tokens == 0 {
            return Err(invalid("max_tokens", "must_be_positive"));
        }
        if self.max_runtime_seconds == 0 {
            return Err(invalid("max_runtime_seconds", "must_be_positive"));
        }
        if self.max_step_attempts == 0 {
            return Err(invalid("max_step_attempts", "must_be_positive"));
        }
        if self.max_consecutive_provider_failures == 0 {
            return Err(invalid(
                "max_consecutive_provider_failures",
                "must_be_positive",
            ));
        }
        if self.max_consecutive_quality_failures == 0 {
            return Err(invalid(
                "max_consecutive_quality_failures",
                "must_be_positive",
            ));
        }
        if self.human_gate_mode == NovelAutopilotHumanGateMode::EveryVolume {
            return Err(invalid("human_gate_mode", "not_supported"));
        }
        if matches!(
            self.human_gate_mode,
            NovelAutopilotHumanGateMode::EveryNChapters
        ) && self.gate_interval == 0
        {
            return Err(invalid("gate_interval", "must_be_positive"));
        }
        if matches!(
            self.execution_scope,
            NovelAutopilotExecutionScope::NextNChapters
        ) {
            match self.next_chapter_count {
                Some(value) if value > 0 && value <= self.max_chapters => {}
                Some(_) => {
                    return Err(invalid(
                        "next_chapter_count",
                        "must_be_between_one_and_max_chapters",
                    ));
                }
                None => return Err(invalid("next_chapter_count", "is_required")),
            }
        } else if self.next_chapter_count.is_some() {
            return Err(invalid(
                "next_chapter_count",
                "only_allowed_for_next_n_chapters",
            ));
        }
        if let Some(value) = self.max_estimated_cost {
            if !value.is_finite() || value <= 0.0 {
                return Err(invalid("max_estimated_cost", "must_be_positive_finite"));
            }
        }
        if self.regenerate_existing {
            return Err(invalid("regenerate_existing", "not_supported"));
        }
        if self.export_format != "txt" {
            return Err(invalid("export_format", "unsupported"));
        }
        Ok(())
    }
}

const fn invalid(field: &'static str, code: &'static str) -> NovelAutopilotConfigValidationError {
    NovelAutopilotConfigValidationError { field, code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_rejects_unsupported_every_volume_gate() {
        let config = NovelAutopilotRunConfig {
            human_gate_mode: NovelAutopilotHumanGateMode::EveryVolume,
            ..NovelAutopilotRunConfig::default()
        };

        let error = config
            .validate()
            .expect_err("volume gate requires a durable volume boundary owner");
        assert_eq!(error.field, "human_gate_mode");
        assert_eq!(error.code, "not_supported");
    }

    #[test]
    fn config_rejects_unimplemented_existing_chapter_regeneration() {
        let config = NovelAutopilotRunConfig {
            regenerate_existing: true,
            ..NovelAutopilotRunConfig::default()
        };

        let error = config
            .validate()
            .expect_err("existing chapter regeneration is not implemented");
        assert_eq!(error.field, "regenerate_existing");
        assert_eq!(error.code, "not_supported");
    }

    #[test]
    fn config_only_accepts_export_format_supported_by_export_service() {
        NovelAutopilotRunConfig::default()
            .validate()
            .expect("default txt export must remain supported");

        for export_format in ["markdown", "docx"] {
            let config = NovelAutopilotRunConfig {
                export_format: export_format.to_string(),
                ..NovelAutopilotRunConfig::default()
            };
            let error = config
                .validate()
                .expect_err("unsupported export format must fail before execution");
            assert_eq!(error.field, "export_format");
            assert_eq!(error.code, "unsupported");
        }
    }

    #[test]
    fn private_snapshot_decodes_legacy_config_shape() {
        let config = NovelAutopilotRunConfig::default();
        let value = serde_json::to_value(&config).expect("legacy config should serialize");

        let decoded = NovelAutopilotPrivateSnapshot::decode(&value)
            .expect("legacy config should remain decodable");

        assert_eq!(decoded.config, config);
        assert!(decoded.guidance.is_none());
    }

    #[test]
    fn private_snapshot_round_trip_preserves_guidance() {
        let snapshot = NovelAutopilotPrivateSnapshot {
            config: NovelAutopilotRunConfig::default(),
            guidance: Some("后续加强人物冲突".to_string()),
        };
        let value = serde_json::to_value(&snapshot).expect("snapshot should serialize");

        let decoded =
            NovelAutopilotPrivateSnapshot::decode(&value).expect("snapshot envelope should decode");

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn private_snapshot_new_envelope_without_guidance_decodes() {
        let snapshot = NovelAutopilotPrivateSnapshot::new(NovelAutopilotRunConfig::default());
        let value = serde_json::to_value(&snapshot).expect("snapshot should serialize");

        let decoded =
            NovelAutopilotPrivateSnapshot::decode(&value).expect("snapshot envelope should decode");

        assert_eq!(decoded.config, snapshot.config);
        assert!(decoded.guidance.is_none());
    }

    #[test]
    fn private_snapshot_rejects_unknown_shape() {
        let error = NovelAutopilotPrivateSnapshot::decode(&json!({
            "unexpected": true,
        }))
        .expect_err("unknown private snapshot shape must be rejected");

        assert!(error.is_data() || error.is_syntax());
    }
}
