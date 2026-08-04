use std::fmt;

use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use crate::{
    models::{chapter_draft_attempt, plot_analysis},
    services::{
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        chapter_content_digest_service::chapter_content_digest,
        chapter_draft_source_service::extract_candidate_draft_full_content,
        chapter_generation_contract_prepare_service::build_chapter_repair_contract,
        chapter_generation_execution_contract_service::{
            build_prompt_overrides_from_compat_options, PreparedGenerationExecutionConfig,
            SingleChapterGenerationCompatOptions,
        },
        chapter_generation_runtime_service::runtime_execution_owner::load_generation_context,
        chapter_generation_runtime_service::GeneratedChapterResult,
        chapter_single_generation_result_lifecycle_service::single_generation_candidate_draft_attempt_view,
        cooperative_cancellation_service::CooperativeCancellationToken,
        novel_autopilot::failure_diagnostic::{
            NovelAutopilotFailureDiagnostic, NovelAutopilotProviderFailureHint,
        },
    },
};

pub(crate) const CHAPTER_REPAIR_RETRY_SOURCE: &str = "novel_autopilot_chapter_repair";
pub(crate) const CHAPTER_REPAIR_RETRY_STATE: &str = "retry";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterRepairCandidate {
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) chapter_status: String,
    pub(crate) content_digest: String,
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_message: Option<String>,
    pub(crate) analysis_id: String,
    pub(crate) source_content_digest: String,
    repair_base_content: String,
    repair_base_word_count: i32,
}

impl ChapterRepairCandidate {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_retry_draft_attempt(
        &self,
        project_id: &str,
        step_id: &str,
        run_id: &str,
        run_epoch: i64,
        step_attempt: i32,
        quality_decision: &str,
    ) -> chapter_draft_attempt::Model {
        let result = GeneratedChapterResult {
            chapter_id: self.chapter_id.clone(),
            chapter_number: self.chapter_number,
            content: self.content.clone(),
            word_count: self.word_count,
            chapter_status: self.chapter_status.clone(),
            attempt_state: CHAPTER_REPAIR_RETRY_STATE.to_string(),
            quality_metrics: self.quality_metrics.clone(),
            quality_gate_action: self.quality_gate_action.clone(),
            quality_gate_message: self.quality_gate_message.clone(),
            ..Default::default()
        };
        let mut view = single_generation_candidate_draft_attempt_view(
            &result,
            &self.repair_base_content,
            self.repair_base_word_count,
        );
        view.quality_gate_decision = Some(quality_decision.to_string());
        view.repair_payload
            .insert("run_id".to_string(), Value::String(run_id.to_string()));
        view.repair_payload
            .insert("run_epoch".to_string(), json!(run_epoch));
        view.repair_payload.insert(
            "source_content_digest".to_string(),
            Value::String(self.source_content_digest.clone()),
        );
        view.repair_payload.insert(
            "analysis_id".to_string(),
            Value::String(self.analysis_id.clone()),
        );
        view.repair_payload.insert(
            "candidate_content_digest".to_string(),
            Value::String(self.content_digest.clone()),
        );
        view.repair_payload
            .insert("step_attempt".to_string(), json!(step_attempt));
        if let Some(message) = self
            .quality_gate_message
            .as_deref()
            .map(str::trim)
            .filter(|message| !message.is_empty())
        {
            view.repair_payload.insert(
                "quality_gate_message".to_string(),
                Value::String(message.chars().take(1000).collect()),
            );
        }

        chapter_draft_attempt::Model {
            id: step_id.to_string(),
            project_id: project_id.to_string(),
            chapter_id: Some(self.chapter_id.clone()),
            batch_task_id: None,
            source: CHAPTER_REPAIR_RETRY_SOURCE.to_string(),
            attempt_state: CHAPTER_REPAIR_RETRY_STATE.to_string(),
            quality_gate_action: view.quality_gate_action,
            quality_gate_decision: view.quality_gate_decision,
            word_count: view.word_count,
            summary_preview: view.summary_preview,
            content_preview: view.content_preview,
            quality_metrics: view.quality_metrics,
            repair_payload: Some(Value::Object(view.repair_payload)),
            created_at: Some(Utc::now().naive_utc()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterRepairRetryBaseline {
    content: String,
    word_count: i32,
    quality_metrics: Option<Value>,
    quality_gate_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChapterRepairGenerationError {
    Cancelled,
    InvalidInput(&'static str),
    Context(String),
    AnalysisNotFound,
    Generation {
        message: String,
        provider_hint: Option<NovelAutopilotProviderFailureHint>,
    },
    InvalidResult(&'static str),
}

impl ChapterRepairGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Context(_) => "context_error",
            Self::AnalysisNotFound => "analysis_not_found",
            Self::Generation { .. } => "generation_error",
            Self::InvalidResult(_) => "invalid_result",
        }
    }

    pub(crate) fn failure_diagnostic(&self) -> NovelAutopilotFailureDiagnostic {
        match self {
            Self::Cancelled => NovelAutopilotFailureDiagnostic::context_invalid("cancelled"),
            Self::InvalidInput(_) => {
                NovelAutopilotFailureDiagnostic::context_invalid("invalid_input")
            }
            Self::Context(_) | Self::AnalysisNotFound => {
                NovelAutopilotFailureDiagnostic::context_invalid(self.code())
            }
            Self::Generation {
                message,
                provider_hint,
            } => {
                if message_indicates_response_invalid(message) {
                    NovelAutopilotFailureDiagnostic::response_invalid_with_hint(
                        "generation_error",
                        provider_hint.clone(),
                    )
                } else {
                    NovelAutopilotFailureDiagnostic::provider_failure(
                        "generation_error",
                        provider_hint.clone(),
                        Some(message),
                    )
                }
            }
            Self::InvalidResult(_) => {
                NovelAutopilotFailureDiagnostic::response_invalid("invalid_result")
            }
        }
    }
}

impl fmt::Display for ChapterRepairGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("chapter repair was cancelled"),
            Self::InvalidInput(field) => write!(formatter, "invalid chapter repair input: {field}"),
            Self::Context(_) => formatter.write_str("failed to load chapter repair context"),
            Self::AnalysisNotFound => formatter.write_str("current chapter analysis was not found"),
            Self::Generation { .. } => formatter.write_str("chapter repair generation failed"),
            Self::InvalidResult(field) => {
                write!(formatter, "invalid chapter repair result: {field}")
            }
        }
    }
}

impl std::error::Error for ChapterRepairGenerationError {}

pub(crate) async fn generate_chapter_repair_candidate_for_autopilot(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    run_id: &str,
    run_epoch: i64,
    execution_config: PreparedGenerationExecutionConfig,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<ChapterRepairCandidate, ChapterRepairGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    if user_id.trim().is_empty() {
        return Err(ChapterRepairGenerationError::InvalidInput("user_id"));
    }
    if chapter_id.trim().is_empty() {
        return Err(ChapterRepairGenerationError::InvalidInput("chapter_id"));
    }

    if run_id.trim().is_empty() {
        return Err(ChapterRepairGenerationError::InvalidInput("run_id"));
    }
    let mut context = load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(|error| ChapterRepairGenerationError::Context(error.into_runtime_message()))?;
    ensure_not_cancelled(cancellation_token)?;

    let original_content = context
        .chapter_model
        .content
        .as_deref()
        .filter(|content| !content.trim().is_empty())
        .ok_or(ChapterRepairGenerationError::InvalidInput(
            "chapter_content",
        ))?
        .to_string();
    // Digest 必须基于数据库中的原始 UTF-8 正文字节，trim 只用于空正文判定。
    let source_content_digest = chapter_content_digest(&original_content);
    let target_word_count = context.chapter_model.word_count.max(1);
    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
        .filter(plot_analysis::Column::SourceContentDigest.eq(source_content_digest.clone()))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| ChapterRepairGenerationError::Context(error.to_string()))?
        .ok_or(ChapterRepairGenerationError::AnalysisNotFound)?;

    let retry_baseline = load_latest_scoped_retry_baseline(
        db,
        &context.chapter_model.project_id,
        chapter_id,
        run_id,
        run_epoch,
        &source_content_digest,
        &analysis.id,
    )
    .await
    .map_err(|error| ChapterRepairGenerationError::Context(error.to_string()))?;
    let (repair_base_content, repair_base_word_count) = retry_baseline
        .as_ref()
        .map(|baseline| (baseline.content.clone(), baseline.word_count))
        .unwrap_or_else(|| (original_content.clone(), context.chapter_model.word_count));
    if retry_baseline.is_some() {
        context.chapter_model.content = Some(repair_base_content.clone());
        context.chapter_model.word_count = repair_base_word_count;
    }

    let compat_options = build_repair_compat_options(&analysis, retry_baseline.as_ref());
    let overrides = build_prompt_overrides_from_compat_options(&compat_options);
    let mut story_packet = context.story_packet.clone();
    story_packet.target_word_count = u32::try_from(target_word_count).ok();
    let repair_contract = build_chapter_repair_contract(story_packet)
        .map_err(|error| ChapterRepairGenerationError::Context(error.to_string()))?;
    let PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload,
        role_policy_context,
    } = execution_config;
    let provider_hint = Some(NovelAutopilotProviderFailureHint::from_ai_config(
        &ai_config,
    ));
    let generated = context
        .generate_candidate_only_with_contract_and_guidance(
            ai_config,
            target_word_count,
            provider_payload,
            &overrides,
            additional_guidance,
            gateway_config,
            repair_contract,
            role_policy_context,
        )
        .await
        .map_err(|error| ChapterRepairGenerationError::Generation {
            message: error,
            provider_hint,
        })?;
    ensure_not_cancelled(cancellation_token)?;

    if generated.chapter_id != chapter_id {
        return Err(ChapterRepairGenerationError::InvalidResult("chapter_id"));
    }
    if generated.chapter_number != context.chapter_model.chapter_number {
        return Err(ChapterRepairGenerationError::InvalidResult(
            "chapter_number",
        ));
    }
    if generated.content.trim().is_empty() {
        return Err(ChapterRepairGenerationError::InvalidResult("content"));
    }
    if generated.word_count <= 0 {
        return Err(ChapterRepairGenerationError::InvalidResult("word_count"));
    }
    let content_digest = chapter_content_digest(&generated.content);
    if content_digest == chapter_content_digest(&repair_base_content) {
        return Err(ChapterRepairGenerationError::InvalidResult(
            "content_unchanged",
        ));
    }

    Ok(ChapterRepairCandidate {
        chapter_id: generated.chapter_id,
        chapter_number: generated.chapter_number,
        content: generated.content,
        word_count: generated.word_count,
        chapter_status: generated.chapter_status,
        content_digest,
        quality_metrics: generated.quality_metrics,
        quality_gate_action: generated.quality_gate_action,
        quality_gate_message: generated.quality_gate_message,
        analysis_id: analysis.id,
        source_content_digest,
        repair_base_content,
        repair_base_word_count,
    })
}

fn build_repair_compat_options(
    analysis: &plot_analysis::Model,
    retry_baseline: Option<&ChapterRepairRetryBaseline>,
) -> SingleChapterGenerationCompatOptions {
    let mut repair_targets = retry_baseline
        .map(latest_quality_feedback_targets)
        .unwrap_or_default();
    repair_targets.extend(
        analysis
            .suggestions
            .as_ref()
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .take(8)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
    repair_targets = deduplicate_non_empty(repair_targets, 8);
    let repair_summary = repair_targets
        .first()
        .cloned()
        .or_else(|| analysis.analysis_report.clone());
    let mut story_preserve_strengths = Vec::new();
    if analysis.pacing_score.is_some_and(|score| score >= 8.5) {
        story_preserve_strengths.push("节奏稳定".to_string());
    }
    if analysis.engagement_score.is_some_and(|score| score >= 8.5) {
        story_preserve_strengths.push("追读牵引".to_string());
    }
    if analysis.coherence_score.is_some_and(|score| score >= 8.5) {
        story_preserve_strengths.push("逻辑连贯".to_string());
    }

    SingleChapterGenerationCompatOptions {
        story_repair_summary: repair_summary,
        story_repair_targets: repair_targets,
        story_preserve_strengths,
        ..Default::default()
    }
}

async fn load_latest_scoped_retry_baseline(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    run_id: &str,
    run_epoch: i64,
    source_content_digest: &str,
    analysis_id: &str,
) -> Result<Option<ChapterRepairRetryBaseline>, sea_orm::DbErr> {
    let attempts = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ProjectId.eq(project_id))
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .filter(chapter_draft_attempt::Column::Source.eq(CHAPTER_REPAIR_RETRY_SOURCE))
        .filter(chapter_draft_attempt::Column::AttemptState.eq(CHAPTER_REPAIR_RETRY_STATE))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .all(db)
        .await?;

    let Some(attempt) = attempts.into_iter().find(|attempt| {
        retry_attempt_matches_scope(
            attempt,
            run_id,
            run_epoch,
            source_content_digest,
            analysis_id,
        )
    }) else {
        return Ok(None);
    };
    let (content, content_complete) = extract_candidate_draft_full_content(&attempt);
    let stored_digest = attempt
        .repair_payload
        .as_ref()
        .and_then(|payload| payload.get("candidate_content_digest"))
        .and_then(Value::as_str);
    if !content_complete
        || content.trim().is_empty()
        || stored_digest != Some(chapter_content_digest(&content).as_str())
    {
        tracing::warn!(
            event = "novel_book_autopilot_chapter_repair_retry_candidate_invalid",
            chapter_id,
            run_id,
            run_epoch,
            attempt_id = %attempt.id,
            "scoped chapter repair retry candidate is incomplete or corrupted; using accepted chapter"
        );
        return Ok(None);
    }
    let quality_gate_message = attempt
        .repair_payload
        .as_ref()
        .and_then(|payload| payload.get("quality_gate_message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);
    Ok(Some(ChapterRepairRetryBaseline {
        word_count: i32::try_from(content.chars().count()).unwrap_or(i32::MAX),
        content,
        quality_metrics: attempt.quality_metrics,
        quality_gate_message,
    }))
}

fn retry_attempt_matches_scope(
    attempt: &chapter_draft_attempt::Model,
    run_id: &str,
    run_epoch: i64,
    source_content_digest: &str,
    analysis_id: &str,
) -> bool {
    let Some(payload) = attempt.repair_payload.as_ref() else {
        return false;
    };
    payload.get("run_id").and_then(Value::as_str) == Some(run_id)
        && payload.get("run_epoch").and_then(Value::as_i64) == Some(run_epoch)
        && payload.get("source_content_digest").and_then(Value::as_str)
            == Some(source_content_digest)
        && payload.get("analysis_id").and_then(Value::as_str) == Some(analysis_id)
}

fn latest_quality_feedback_targets(baseline: &ChapterRepairRetryBaseline) -> Vec<String> {
    let mut targets = Vec::new();
    if let Some(message) = baseline.quality_gate_message.as_deref() {
        targets.push(message.to_string());
    }
    let failed_metrics = baseline
        .quality_metrics
        .as_ref()
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|gate| gate.get("failed_metrics"))
        .or_else(|| {
            baseline
                .quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("failed_metrics"))
        })
        .and_then(Value::as_array);
    if let Some(failed_metrics) = failed_metrics {
        for metric in failed_metrics {
            if let Some(text) = metric.as_str() {
                targets.push(format!("修复未达标质量项：{}", text.trim()));
            } else if let Some(name) = metric
                .get("metric")
                .or_else(|| metric.get("name"))
                .or_else(|| metric.get("key"))
                .and_then(Value::as_str)
            {
                targets.push(format!("修复未达标质量项：{}", name.trim()));
            }
        }
    }
    if let Some(guidance) = baseline
        .quality_metrics
        .as_ref()
        .and_then(|metrics| metrics.get("repair_guidance"))
    {
        for field in ["repair_targets", "focus_areas", "suggestions"] {
            if let Some(items) = guidance.get(field).and_then(Value::as_array) {
                targets.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
            }
        }
    }
    deduplicate_non_empty(targets, 6)
}

fn deduplicate_non_empty(items: Vec<String>, limit: usize) -> Vec<String> {
    let mut normalized = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() || normalized.iter().any(|existing| existing == item) {
            continue;
        }
        normalized.push(item.chars().take(500).collect::<String>());
        if normalized.len() >= limit {
            break;
        }
    }
    normalized
}

fn message_indicates_response_invalid(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("json")
        || normalized.contains("parse")
        || normalized.contains("解析")
        || normalized.contains("candidate")
        || normalized.contains("result")
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), ChapterRepairGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterRepairGenerationError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DbBackend, IntoActiveModel, Schema,
    };
    use serde_json::json;

    use crate::{
        models::chapter_draft_attempt,
        services::{
            chapter_content_digest_service::chapter_content_digest,
            chapter_draft_source_service::extract_candidate_draft_full_content,
        },
    };

    use super::{
        ensure_not_cancelled, latest_quality_feedback_targets, load_latest_scoped_retry_baseline,
        retry_attempt_matches_scope, ChapterRepairCandidate, ChapterRepairGenerationError,
        ChapterRepairRetryBaseline,
    };

    const PROJECT_ID: &str = "repair-project";
    const CHAPTER_ID: &str = "repair-chapter";
    const RUN_ID: &str = "repair-run";
    const ANALYSIS_ID: &str = "repair-analysis";
    const RUN_EPOCH: i64 = 2;
    const ACCEPTED_CONTENT: &str = "已接受章节正文";

    fn test_time(second: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 26)
            .expect("valid date")
            .and_hms_opt(12, 0, second)
            .expect("valid time")
    }

    fn candidate(content: &str) -> ChapterRepairCandidate {
        ChapterRepairCandidate {
            chapter_id: CHAPTER_ID.to_string(),
            chapter_number: 2,
            content: content.to_string(),
            word_count: i32::try_from(content.chars().count()).expect("word count fits i32"),
            chapter_status: "completed".to_string(),
            content_digest: chapter_content_digest(content),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{"metric": "outline_alignment_rate"}]
                },
                "repair_guidance": {
                    "repair_targets": ["补足转折因果"]
                }
            })),
            quality_gate_action: Some("retry".to_string()),
            quality_gate_message: Some("冲突升级仍不充分".to_string()),
            analysis_id: ANALYSIS_ID.to_string(),
            source_content_digest: chapter_content_digest(ACCEPTED_CONTENT),
            repair_base_content: ACCEPTED_CONTENT.to_string(),
            repair_base_word_count: i32::try_from(ACCEPTED_CONTENT.chars().count()).unwrap(),
        }
    }

    async fn setup_draft_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)))
            .await
            .expect("create draft attempt table");
        db
    }

    async fn insert_attempt(
        db: &sea_orm::DatabaseConnection,
        attempt: chapter_draft_attempt::Model,
    ) {
        attempt
            .into_active_model()
            .insert(db)
            .await
            .expect("insert retry draft attempt");
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(ChapterRepairGenerationError::Cancelled.code(), "cancelled");
        assert_eq!(
            ChapterRepairGenerationError::AnalysisNotFound.code(),
            "analysis_not_found"
        );
    }

    #[test]
    fn generation_error_diagnostic_maps_rate_limit_without_leaking_raw_message() {
        use crate::services::novel_autopilot::failure_diagnostic::{
            NovelAutopilotFailureDomain, NovelAutopilotProviderFailureHint,
        };

        let error = ChapterRepairGenerationError::Generation {
            message: "HTTP 429 Too Many Requests api_key=secret prompt=完整正文".to_string(),
            provider_hint: Some(NovelAutopilotProviderFailureHint {
                provider: Some("openai".to_string()),
                model: Some("gpt-5.1".to_string()),
                http_status: None,
            }),
        };
        let diagnostic = error.failure_diagnostic();
        let serialized = serde_json::to_string(&diagnostic.to_value()).expect("serialize");

        assert_eq!(
            diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterRepair),
            "chapter_repair_provider_rate_limited"
        );
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("完整正文"));
    }

    #[test]
    fn cancellation_guard_allows_missing_token() {
        assert_eq!(ensure_not_cancelled(None), Ok(()));
    }

    #[test]
    fn retry_draft_contains_complete_scoped_candidate_and_quality_feedback() {
        let candidate = candidate("第一轮返修候选正文");
        let draft = candidate.build_retry_draft_attempt(
            PROJECT_ID,
            "repair-step-1",
            RUN_ID,
            RUN_EPOCH,
            1,
            "retry",
        );

        assert_eq!(draft.source, "novel_autopilot_chapter_repair");
        assert_eq!(draft.attempt_state, "retry");
        assert_eq!(draft.batch_task_id, None);
        assert!(retry_attempt_matches_scope(
            &draft,
            RUN_ID,
            RUN_EPOCH,
            &chapter_content_digest(ACCEPTED_CONTENT),
            ANALYSIS_ID,
        ));
        assert_eq!(
            extract_candidate_draft_full_content(&draft),
            ("第一轮返修候选正文".to_string(), true)
        );
        assert_eq!(
            draft
                .repair_payload
                .as_ref()
                .and_then(|payload| payload.get("quality_gate_message")),
            Some(&json!("冲突升级仍不充分"))
        );
    }

    #[test]
    fn latest_quality_feedback_is_prioritized_and_deduplicated() {
        let baseline = ChapterRepairRetryBaseline {
            content: "候选".to_string(),
            word_count: 2,
            quality_metrics: candidate("候选").quality_metrics,
            quality_gate_message: Some("冲突升级仍不充分".to_string()),
        };

        let targets = latest_quality_feedback_targets(&baseline);

        assert_eq!(targets[0], "冲突升级仍不充分");
        assert!(targets
            .iter()
            .any(|target| target.contains("outline_alignment_rate")));
        assert!(targets.iter().any(|target| target == "补足转折因果"));
    }

    #[tokio::test]
    async fn latest_scoped_retry_skips_newer_attempt_from_another_run() {
        let db = setup_draft_db().await;
        let mut scoped = candidate("同一运行的候选").build_retry_draft_attempt(
            PROJECT_ID,
            "repair-step-scoped",
            RUN_ID,
            RUN_EPOCH,
            1,
            "retry",
        );
        scoped.created_at = Some(test_time(1));
        insert_attempt(&db, scoped).await;

        let mut other_run = candidate("其他运行的更新候选").build_retry_draft_attempt(
            PROJECT_ID,
            "repair-step-other-run",
            "other-run",
            RUN_EPOCH,
            1,
            "retry",
        );
        other_run.created_at = Some(test_time(2));
        insert_attempt(&db, other_run).await;

        let loaded = load_latest_scoped_retry_baseline(
            &db,
            PROJECT_ID,
            CHAPTER_ID,
            RUN_ID,
            RUN_EPOCH,
            &chapter_content_digest(ACCEPTED_CONTENT),
            ANALYSIS_ID,
        )
        .await
        .expect("load scoped retry")
        .expect("scoped retry exists");

        assert_eq!(loaded.content, "同一运行的候选");
    }

    #[tokio::test]
    async fn corrupted_latest_scoped_retry_falls_back_instead_of_using_older_candidate() {
        let db = setup_draft_db().await;
        let mut older = candidate("较早的有效候选").build_retry_draft_attempt(
            PROJECT_ID,
            "repair-step-older",
            RUN_ID,
            RUN_EPOCH,
            1,
            "retry",
        );
        older.created_at = Some(test_time(1));
        insert_attempt(&db, older).await;

        let mut corrupted = candidate("最新候选").build_retry_draft_attempt(
            PROJECT_ID,
            "repair-step-corrupt",
            RUN_ID,
            RUN_EPOCH,
            2,
            "retry",
        );
        corrupted.created_at = Some(test_time(2));
        corrupted
            .repair_payload
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("repair payload object")
            .insert(
                "candidate_content_digest".to_string(),
                json!("sha256:corrupted"),
            );
        insert_attempt(&db, corrupted).await;

        let loaded = load_latest_scoped_retry_baseline(
            &db,
            PROJECT_ID,
            CHAPTER_ID,
            RUN_ID,
            RUN_EPOCH,
            &chapter_content_digest(ACCEPTED_CONTENT),
            ANALYSIS_ID,
        )
        .await
        .expect("load scoped retry");

        assert_eq!(loaded, None);
    }
}
