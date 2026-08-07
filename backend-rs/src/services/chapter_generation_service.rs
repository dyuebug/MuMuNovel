use std::fmt;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;

use crate::{
    models::{chapter_draft_attempt, novel_autopilot_step_run},
    services::{
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        chapter_content_digest_service::chapter_content_digest,
        chapter_draft_source_service::extract_candidate_draft_full_content,
        chapter_generation_execution_contract_service::{
            build_prompt_overrides_from_compat_options, PreparedGenerationExecutionConfig,
            SingleChapterGenerationCompatOptions,
        },
        chapter_generation_runtime_service::runtime_execution_owner::{
            load_generation_context, ChapterGenerationRuntimeContext,
        },
        cooperative_cancellation_service::CooperativeCancellationToken,
    },
};

pub(crate) const CHAPTER_GENERATE_RETRY_SOURCE: &str = "novel_autopilot_chapter_generate";
pub(crate) const CHAPTER_GENERATE_RETRY_STATE: &str = "retry";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterGenerationRetryScope {
    pub(crate) run_id: String,
    pub(crate) run_epoch: i64,
    pub(crate) current_step_attempt: i32,
    pub(crate) step_key: String,
    pub(crate) source_chapter_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterGenerationRetryBaseline {
    content: String,
    word_count: i32,
    quality_diagnostic: Option<Value>,
    quality_gate_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGeneratedDraft {
    pub chapter_id: String,
    pub chapter_number: i32,
    pub title: String,
    pub content: String,
    pub word_count: i32,
    pub chapter_status: String,
    pub quality_metrics: Option<Value>,
    pub quality_gate_action: Option<String>,
    pub quality_gate_message: Option<String>,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChapterGenerationError {
    Cancelled,
    InvalidInput(&'static str),
    Context(String),
    Generation(String),
    InvalidResult(&'static str),
}

impl ChapterGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::InvalidInput(_) => "invalid_input",
            Self::Context(_) => "context_error",
            Self::Generation(_) => "generation_error",
            Self::InvalidResult(_) => "invalid_result",
        }
    }
}

impl fmt::Display for ChapterGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("chapter generation was cancelled"),
            Self::InvalidInput(field) => {
                write!(formatter, "invalid chapter generation input: {field}")
            }
            Self::Context(_) => formatter.write_str("failed to load chapter generation context"),
            Self::Generation(_) => formatter.write_str("chapter candidate generation failed"),
            Self::InvalidResult(field) => {
                write!(formatter, "invalid generated chapter result: {field}")
            }
        }
    }
}

impl std::error::Error for ChapterGenerationError {}

pub(crate) async fn generate_chapter_candidate_for_autopilot(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    compat_options: &SingleChapterGenerationCompatOptions,
    retry_scope: &ChapterGenerationRetryScope,
    execution_config: PreparedGenerationExecutionConfig,
    additional_guidance: Option<&str>,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<ChapterGeneratedDraft, ChapterGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    if user_id.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("user_id"));
    }
    if chapter_id.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("chapter_id"));
    }
    if target_word_count <= 0 {
        return Err(ChapterGenerationError::InvalidInput("target_word_count"));
    }

    if retry_scope.run_id.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("run_id"));
    }
    if retry_scope.current_step_attempt <= 0 {
        return Err(ChapterGenerationError::InvalidInput("step_attempt"));
    }
    if retry_scope.step_key.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput("step_key"));
    }
    if retry_scope.source_chapter_snapshot_digest.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidInput(
            "source_chapter_snapshot_digest",
        ));
    }

    let mut context = load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(|error| ChapterGenerationError::Context(error.into_runtime_message()))?;
    ensure_not_cancelled(cancellation_token)?;

    let retry_baseline = load_scoped_retry_baseline(
        db,
        &context.chapter_model.project_id,
        chapter_id,
        retry_scope,
    )
    .await
    .map_err(ChapterGenerationError::Context)?;
    let effective_compat_options = if let Some(baseline) = retry_baseline.as_ref() {
        apply_retry_baseline(&mut context, baseline)?;
        compat_options_with_retry_feedback(compat_options, baseline)
    } else {
        compat_options.clone()
    };

    let overrides = build_prompt_overrides_from_compat_options(&effective_compat_options);
    let PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload,
        role_policy_context,
    } = execution_config;
    let generated = context
        .generate_candidate_only_with_guidance(
            ai_config,
            target_word_count,
            provider_payload,
            &overrides,
            additional_guidance,
            gateway_config,
            role_policy_context,
        )
        .await
        .map_err(ChapterGenerationError::Generation)?;
    ensure_not_cancelled(cancellation_token)?;

    if generated.chapter_id != context.chapter_model.id {
        return Err(ChapterGenerationError::InvalidResult("chapter_id"));
    }
    if generated.chapter_number != context.chapter_model.chapter_number {
        return Err(ChapterGenerationError::InvalidResult("chapter_number"));
    }
    if generated.content.trim().is_empty() {
        return Err(ChapterGenerationError::InvalidResult("content"));
    }
    if generated.word_count <= 0 {
        return Err(ChapterGenerationError::InvalidResult("word_count"));
    }

    let content_digest = chapter_content_digest(&generated.content);
    Ok(ChapterGeneratedDraft {
        chapter_id: generated.chapter_id,
        chapter_number: generated.chapter_number,
        title: generated.title,
        content: generated.content,
        word_count: generated.word_count,
        chapter_status: generated.chapter_status,
        quality_metrics: generated.quality_metrics,
        quality_gate_action: generated.quality_gate_action,
        quality_gate_message: generated.quality_gate_message,
        content_digest,
    })
}

async fn load_scoped_retry_baseline(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    scope: &ChapterGenerationRetryScope,
) -> Result<Option<ChapterGenerationRetryBaseline>, String> {
    if scope.current_step_attempt <= 1 {
        return Ok(None);
    }
    let expected_attempt = scope.current_step_attempt - 1;
    let previous_step = novel_autopilot_step_run::Entity::find()
        .filter(novel_autopilot_step_run::Column::RunId.eq(&scope.run_id))
        .filter(novel_autopilot_step_run::Column::RunEpoch.eq(scope.run_epoch))
        .filter(novel_autopilot_step_run::Column::StepKey.eq(&scope.step_key))
        .filter(novel_autopilot_step_run::Column::Attempt.eq(expected_attempt))
        .order_by_desc(novel_autopilot_step_run::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|_| "failed to load previous chapter generation step".to_string())?;
    let previous_was_quality_retry = previous_step
        .as_ref()
        .and_then(|step| step.error_code.as_deref())
        .is_some_and(|code| {
            matches!(
                code,
                "chapter_quality_retry" | "chapter_quality_auto_repair"
            )
        });
    let attempts = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ProjectId.eq(project_id))
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .filter(chapter_draft_attempt::Column::Source.eq(CHAPTER_GENERATE_RETRY_SOURCE))
        .filter(chapter_draft_attempt::Column::AttemptState.eq(CHAPTER_GENERATE_RETRY_STATE))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|_| "failed to load chapter generation retry evidence".to_string())?;

    let Some(attempt) = attempts
        .into_iter()
        .find(|attempt| retry_attempt_matches_run_attempt(attempt, scope, expected_attempt))
    else {
        return if previous_was_quality_retry {
            Err("chapter generation quality retry evidence is missing".to_string())
        } else {
            Ok(None)
        };
    };
    if !previous_was_quality_retry {
        return Err("chapter generation retry evidence has no quality retry owner".to_string());
    }
    let stored_snapshot_digest = attempt
        .repair_payload
        .as_ref()
        .and_then(|payload| payload.get("source_chapter_snapshot_digest"))
        .and_then(Value::as_str);
    if stored_snapshot_digest != Some(scope.source_chapter_snapshot_digest.as_str()) {
        return Err("chapter generation retry evidence scope is stale".to_string());
    }
    validate_retry_baseline(&attempt).map(Some)
}

fn retry_attempt_matches_run_attempt(
    attempt: &chapter_draft_attempt::Model,
    scope: &ChapterGenerationRetryScope,
    expected_attempt: i32,
) -> bool {
    let Some(payload) = attempt.repair_payload.as_ref() else {
        return false;
    };
    payload.get("run_id").and_then(Value::as_str) == Some(scope.run_id.as_str())
        && payload.get("run_epoch").and_then(Value::as_i64) == Some(scope.run_epoch)
        && payload.get("step_attempt").and_then(Value::as_i64) == Some(i64::from(expected_attempt))
}

fn validate_retry_baseline(
    attempt: &chapter_draft_attempt::Model,
) -> Result<ChapterGenerationRetryBaseline, String> {
    let (content, content_complete) = extract_candidate_draft_full_content(attempt);
    let payload = attempt
        .repair_payload
        .as_ref()
        .ok_or_else(|| "chapter generation retry evidence payload is missing".to_string())?;
    let stored_digest = payload
        .get("candidate_content_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let actual_word_count = i32::try_from(content.chars().count()).unwrap_or(i32::MAX);
    if !content_complete
        || content.trim().is_empty()
        || stored_digest != chapter_content_digest(&content)
        || attempt.word_count <= 0
        || attempt.word_count != actual_word_count
    {
        return Err("chapter generation retry evidence is incomplete or corrupted".to_string());
    }
    Ok(ChapterGenerationRetryBaseline {
        content,
        word_count: actual_word_count,
        quality_diagnostic: attempt.quality_metrics.clone(),
        quality_gate_message: payload
            .get("quality_gate_message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .map(str::to_string),
    })
}

fn apply_retry_baseline(
    context: &mut ChapterGenerationRuntimeContext,
    baseline: &ChapterGenerationRetryBaseline,
) -> Result<(), ChapterGenerationError> {
    context.chapter_model.content = Some(baseline.content.clone());
    context.chapter_model.word_count = baseline.word_count;
    let chapter_fact = context
        .story_packet
        .opaque_story_facts
        .get_mut("chapter")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            ChapterGenerationError::Context("chapter story fact is missing".to_string())
        })?;
    chapter_fact.insert(
        "content".to_string(),
        Value::String(baseline.content.clone()),
    );
    chapter_fact.insert("word_count".to_string(), Value::from(baseline.word_count));
    Ok(())
}

fn compat_options_with_retry_feedback(
    base: &SingleChapterGenerationCompatOptions,
    baseline: &ChapterGenerationRetryBaseline,
) -> SingleChapterGenerationCompatOptions {
    let mut options = base.clone();
    let mut targets = Vec::new();
    if let Some(message) = baseline.quality_gate_message.as_deref() {
        targets.push(message.to_string());
    }
    if let Some(diagnostic) = baseline.quality_diagnostic.as_ref() {
        for key in ["repair_targets", "focus_areas"] {
            targets.extend(
                diagnostic
                    .get(key)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string),
            );
        }
        targets.extend(
            diagnostic
                .get("failed_metrics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|metric| {
                    metric
                        .get("label")
                        .or_else(|| metric.get("key"))
                        .and_then(Value::as_str)
                })
                .map(|metric| format!("修复未达标质量项：{}", metric.trim())),
        );
    }
    targets.extend(options.story_repair_targets);
    let mut deduplicated = Vec::new();
    for target in targets {
        let target = target.trim();
        if !target.is_empty() && !deduplicated.iter().any(|item| item == target) {
            deduplicated.push(target.chars().take(160).collect::<String>());
        }
        if deduplicated.len() >= 8 {
            break;
        }
    }
    options.story_repair_summary = deduplicated
        .first()
        .cloned()
        .or(options.story_repair_summary);
    options.story_repair_targets = deduplicated;
    options
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), ChapterGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterGenerationError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate};
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
    };
    use serde_json::json;

    use crate::{
        models::{chapter, chapter_draft_attempt, novel_autopilot_step_run, project},
        services::{
            chapter_content_digest_service::chapter_content_digest,
            chapter_generation_execution_contract_service::{
                build_prompt_overrides_from_compat_options, SingleChapterGenerationCompatOptions,
            },
            chapter_generation_prompt_service::build_previous_chapter_prompt_context,
            chapter_generation_runtime_service::runtime_execution_owner::ChapterGenerationRuntimeContext,
            generation_contract_service::{GenerationTarget, StoryPacketV1},
        },
    };

    use super::{
        apply_retry_baseline, compat_options_with_retry_feedback, ensure_not_cancelled,
        load_scoped_retry_baseline, ChapterGenerationError, ChapterGenerationRetryBaseline,
        ChapterGenerationRetryScope, CHAPTER_GENERATE_RETRY_SOURCE, CHAPTER_GENERATE_RETRY_STATE,
    };

    const PROJECT_ID: &str = "project-generate-retry";
    const CHAPTER_ID: &str = "chapter-generate-retry";

    fn test_time() -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 8, 7)
            .expect("valid date")
            .and_hms_opt(10, 0, 0)
            .expect("valid time")
    }

    async fn setup_retry_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect retry evidence database");
        let builder = DbBackend::Sqlite;
        for statement in [
            builder.build(
                &Schema::new(builder).create_table_from_entity(chapter_draft_attempt::Entity),
            ),
            builder.build(
                &Schema::new(builder).create_table_from_entity(novel_autopilot_step_run::Entity),
            ),
        ] {
            db.execute(statement)
                .await
                .expect("create retry evidence test table");
        }
        db
    }

    fn retry_scope(current_step_attempt: i32) -> ChapterGenerationRetryScope {
        ChapterGenerationRetryScope {
            run_id: "run-1".to_string(),
            run_epoch: 3,
            current_step_attempt,
            step_key: "chapter:1:generate".to_string(),
            source_chapter_snapshot_digest: "sha256:source-snapshot".to_string(),
        }
    }

    async fn insert_previous_step(db: &DatabaseConnection, attempt: i32, error_code: &str) {
        let now = test_time();
        novel_autopilot_step_run::ActiveModel {
            id: Set(format!("previous-step-{attempt}")),
            run_id: Set("run-1".to_string()),
            step_key: Set("chapter:1:generate".to_string()),
            step_type: Set("chapter_generate".to_string()),
            phase: Set("chapter_loop".to_string()),
            chapter_id: Set(Some(CHAPTER_ID.to_string())),
            chapter_number: Set(Some(1)),
            attempt: Set(attempt),
            run_epoch: Set(3),
            status: Set("failed".to_string()),
            background_task_id: Set(Some(format!("task-{attempt}"))),
            input_digest: Set(format!("input-{attempt}")),
            result_digest: Set(None),
            quality_decision: Set(Some("retry".to_string())),
            error_code: Set(Some(error_code.to_string())),
            started_at: Set(Some(now)),
            completed_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert previous generation step");
    }

    fn retry_attempt(
        id: &str,
        step_attempt: i32,
        content: &str,
        created_at: chrono::NaiveDateTime,
    ) -> chapter_draft_attempt::ActiveModel {
        let digest = chapter_content_digest(content);
        chapter_draft_attempt::ActiveModel {
            id: Set(id.to_string()),
            project_id: Set(PROJECT_ID.to_string()),
            chapter_id: Set(Some(CHAPTER_ID.to_string())),
            batch_task_id: Set(None),
            source: Set(CHAPTER_GENERATE_RETRY_SOURCE.to_string()),
            attempt_state: Set(CHAPTER_GENERATE_RETRY_STATE.to_string()),
            quality_gate_action: Set(Some("auto_repair".to_string())),
            quality_gate_decision: Set(Some("auto_repair".to_string())),
            word_count: Set(i32::try_from(content.chars().count()).expect("word count fits")),
            summary_preview: Set(None),
            content_preview: Set(None),
            quality_metrics: Set(Some(json!({
                "failed_metrics": [{"key": "pacing", "label": "节奏"}],
                "repair_targets": ["压缩说明段"]
            }))),
            repair_payload: Set(Some(json!({
                "run_id": "run-1",
                "run_epoch": 3,
                "step_attempt": step_attempt,
                "source_chapter_snapshot_digest": "sha256:source-snapshot",
                "candidate_full_content": content,
                "candidate_content_digest": digest,
                "content_complete": true,
                "quality_gate_message": "继续修复节奏"
            }))),
            created_at: Set(Some(created_at)),
        }
    }

    #[test]
    fn cancellation_guard_allows_missing_token() {
        assert_eq!(ensure_not_cancelled(None), Ok(()));
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(ChapterGenerationError::Cancelled.code(), "cancelled");
        assert_eq!(
            ChapterGenerationError::InvalidResult("content").code(),
            "invalid_result"
        );
    }

    #[tokio::test]
    async fn provider_retry_without_quality_evidence_allows_fresh_generation() {
        let db = setup_retry_db().await;
        insert_previous_step(&db, 1, "chapter_generation_error").await;

        let baseline = load_scoped_retry_baseline(&db, PROJECT_ID, CHAPTER_ID, &retry_scope(2))
            .await
            .expect("missing quality evidence is not corruption");

        assert!(baseline.is_none());
    }

    #[tokio::test]
    async fn quality_retry_consumes_only_immediately_previous_attempt() {
        let db = setup_retry_db().await;
        insert_previous_step(&db, 2, "chapter_quality_auto_repair").await;
        retry_attempt("attempt-1", 1, "第一版候选", test_time())
            .insert(&db)
            .await
            .expect("insert attempt 1");
        retry_attempt(
            "attempt-2",
            2,
            "第二版候选",
            test_time() + Duration::seconds(1),
        )
        .insert(&db)
        .await
        .expect("insert attempt 2");

        let baseline = load_scoped_retry_baseline(&db, PROJECT_ID, CHAPTER_ID, &retry_scope(3))
            .await
            .expect("load immediately previous attempt")
            .expect("attempt 2 baseline exists");

        assert_eq!(baseline.content, "第二版候选");
    }

    #[tokio::test]
    async fn corrupted_latest_scoped_quality_evidence_fails_closed_without_fallback() {
        let db = setup_retry_db().await;
        insert_previous_step(&db, 1, "chapter_quality_auto_repair").await;
        retry_attempt("valid-older", 1, "较早但有效的候选", test_time())
            .insert(&db)
            .await
            .expect("insert valid evidence");
        let mut corrupted = retry_attempt(
            "corrupted-latest",
            1,
            "最新候选",
            test_time() + Duration::seconds(1),
        );
        corrupted.word_count = Set(999);
        corrupted
            .insert(&db)
            .await
            .expect("insert corrupted evidence");

        let error = load_scoped_retry_baseline(&db, PROJECT_ID, CHAPTER_ID, &retry_scope(2))
            .await
            .expect_err("latest scoped corruption must fail closed");

        assert_eq!(
            error,
            "chapter generation retry evidence is incomplete or corrupted"
        );
    }

    #[tokio::test]
    async fn quality_retry_with_changed_source_snapshot_fails_closed() {
        let db = setup_retry_db().await;
        insert_previous_step(&db, 1, "chapter_quality_retry").await;
        retry_attempt("attempt-1", 1, "第一版候选", test_time())
            .insert(&db)
            .await
            .expect("insert attempt 1");
        let mut scope = retry_scope(2);
        scope.source_chapter_snapshot_digest = "sha256:manually-edited-snapshot".to_string();

        let error = load_scoped_retry_baseline(&db, PROJECT_ID, CHAPTER_ID, &scope)
            .await
            .expect_err("changed source snapshot must reject prior quality evidence");

        assert_eq!(error, "chapter generation retry evidence scope is stale");
    }

    #[test]
    fn retry_baseline_reaches_story_packet_and_prompt_overrides() {
        let now = test_time();
        let chapter_model = chapter::Model {
            id: CHAPTER_ID.to_string(),
            project_id: PROJECT_ID.to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: Some("数据库中的旧正文".to_string()),
            summary: None,
            word_count: 8,
            status: "pending".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: now,
            updated_at: Some(now),
        };
        let project_model = project::Model {
            id: PROJECT_ID.to_string(),
            user_id: "owner-1".to_string(),
            title: "测试小说".to_string(),
            description: None,
            theme: None,
            genre: None,
            target_words: 100_000,
            current_words: 0,
            status: "writing".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 4,
            outline_mode: "one-to-one".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(1),
            narrative_perspective: None,
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: now,
            updated_at: Some(now),
        };
        let mut story_packet = StoryPacketV1::new(
            PROJECT_ID,
            GenerationTarget::chapter(PROJECT_ID, CHAPTER_ID),
        );
        story_packet.opaque_story_facts.insert(
            "chapter".to_string(),
            json!({"content": "数据库中的旧正文", "word_count": 8}),
        );
        let mut context = ChapterGenerationRuntimeContext {
            chapter_model,
            project_model,
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet,
        };
        let baseline = ChapterGenerationRetryBaseline {
            content: "质量重试候选正文".to_string(),
            word_count: 8,
            quality_diagnostic: Some(json!({
                "failed_metrics": [{"key": "pacing", "label": "节奏"}],
                "repair_targets": ["压缩说明段"]
            })),
            quality_gate_message: Some("继续修复节奏".to_string()),
        };

        apply_retry_baseline(&mut context, &baseline).expect("apply retry baseline");
        let compat = compat_options_with_retry_feedback(
            &SingleChapterGenerationCompatOptions::default(),
            &baseline,
        );
        let overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            context.story_packet.opaque_story_facts["chapter"]["content"],
            "质量重试候选正文"
        );
        assert_eq!(
            context.chapter_model.content.as_deref(),
            Some("质量重试候选正文")
        );
        assert_eq!(
            overrides.story_repair_summary.as_deref(),
            Some("继续修复节奏")
        );
        assert!(overrides
            .story_repair_targets
            .iter()
            .any(|target| target == "压缩说明段"));
    }
}
