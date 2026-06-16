use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_draft_view_payload_service::build_candidate_draft_payload;
use crate::services::chapter_generation_history_payload_service::build_generated_chapter_history_payload_with_quality_metrics;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::resolve_active_story_repair_payload_with_quality_fallback;
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

pub(crate) const CHAPTER_GENERATION_HISTORY_MODEL: &str = "chapter_generation_v1";

pub(crate) fn build_single_generation_result_lifecycle_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_result_lifecycle_service",
        "scope": "single_generation_generated_result_quality_draft_and_history_lifecycle",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_result_lifecycle_service.rs",
            "backend-rs/src/services/chapter_single_generation_result_lifecycle_service/lifecycle_owner.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "update_latest_generated_chapter_history_quality_metrics",
                "generated_result_quality_view",
                "generated_result_lifecycle_view",
                "build_single_generation_followup_draft_result",
                "build_single_generation_candidate_draft_attempt",
                "single_generation_candidate_draft_lifecycle_view",
                "persisted_history_payload_view"
            ],
            "quality_gate_actions": [
                "continue",
                "retry",
                "manual_review"
            ],
            "draft_lifecycle_fields": [
                "content_applied",
                "provisional_draft_saved",
                "attempt_state",
                "chapter_status"
            ],
            "history_payload_fields": [
                "content_applied",
                "attempt_state",
                "candidate_gateway"
            ]
        },
        "active_consumers": [
            "chapter_generation_runtime_service",
            "chapter_single_generation_stream_workflow_service",
            "chapter_analysis_runtime_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo test chapter_single_generation_stream_workflow_service",
            "cargo test chapter_analysis_runtime_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_single_generation_candidate_finalize_and_history_shells_as_source_map_until_explicit_freeze_delete_round",
            "rollback_files": [
                "backend/app/services/chapter_generation/stream/candidate_service.py",
                "backend/app/services/chapter_generation/stream/finalize_service.py",
                "backend/app/services/manual_chapter_analysis_execution_service.py"
            ]
        }
    })
}

pub(crate) fn resolve_generated_history_attempt_state(
    content_applied: bool,
    attempt_state: Option<&str>,
) -> String {
    let trimmed = attempt_state.unwrap_or_default().trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    if content_applied {
        "applied".to_string()
    } else {
        "candidate".to_string()
    }
}

fn normalized_non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedResultQualityView {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_decision: Option<String>,
    pub(crate) quality_gate_message: Option<String>,
}

pub(crate) fn generated_result_quality_view(candidate: &Value) -> GeneratedResultQualityView {
    let quality_metrics = candidate
        .get("quality_metrics")
        .filter(|payload| payload.is_object())
        .cloned();
    let quality_gate_action =
        generated_result_quality_gate_action(candidate, quality_metrics.as_ref());
    let quality_gate_decision = generated_result_quality_gate_decision(quality_metrics.as_ref());
    let quality_gate_message =
        generated_result_quality_gate_message(candidate, quality_metrics.as_ref());

    GeneratedResultQualityView {
        quality_metrics,
        quality_gate_action,
        quality_gate_decision,
        quality_gate_message,
    }
}

fn generated_result_quality_gate_action(
    candidate: &Value,
    quality_metrics: Option<&Value>,
) -> Option<String> {
    let action = candidate
        .get("quality_gate_action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            candidate
                .get("quality_gate_plan")
                .and_then(|payload| payload.get("action"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    if action.is_some() {
        return action;
    }

    match candidate
        .get("quality_gate_plan")
        .and_then(|payload| payload.get("quality_gate"))
        .and_then(|payload| payload.get("decision"))
        .and_then(Value::as_str)
        .map(str::trim)
        .or_else(|| {
            quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(|payload| payload.get("decision"))
                .and_then(Value::as_str)
                .map(str::trim)
        }) {
        Some("passed") | Some("continue") | Some("allow_save") => Some("continue".to_string()),
        Some("auto_repair") | Some("repair") | Some("retry") => Some("retry".to_string()),
        Some("manual_review") => Some("manual_review".to_string()),
        Some(other) if !other.is_empty() => Some(other.to_string()),
        _ => Some("continue".to_string()),
    }
}

fn generated_result_quality_gate_decision(quality_metrics: Option<&Value>) -> Option<String> {
    quality_metrics
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|payload| payload.get("decision"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn generated_result_quality_gate_message(
    candidate: &Value,
    quality_metrics: Option<&Value>,
) -> Option<String> {
    normalized_non_empty_string(
        candidate
            .get("quality_gate_message")
            .and_then(Value::as_str),
    )
    .or_else(|| {
        normalized_non_empty_string(
            quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(|payload| {
                    payload
                        .get("summary")
                        .or_else(|| payload.get("label"))
                        .and_then(Value::as_str)
                }),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedResultLifecycleView {
    pub(crate) content_applied: bool,
    pub(crate) provisional_draft_saved: bool,
    pub(crate) attempt_state: String,
    pub(crate) chapter_status: String,
}

pub(crate) fn generated_result_lifecycle_view(
    current_chapter_status: &str,
    quality_gate_action: Option<&str>,
    blocked_attempt_state_fallback: &str,
) -> GeneratedResultLifecycleView {
    let quality_gate_action = normalized_non_empty_string(quality_gate_action);
    let content_applied = matches!(quality_gate_action.as_deref(), None | Some("continue"));
    let provisional_draft_saved = matches!(quality_gate_action.as_deref(), Some("retry"));
    let attempt_state = if content_applied {
        "applied".to_string()
    } else {
        quality_gate_action
            .clone()
            .unwrap_or_else(|| blocked_attempt_state_fallback.to_string())
    };
    let chapter_status = if content_applied {
        "completed".to_string()
    } else if provisional_draft_saved {
        "draft".to_string()
    } else {
        current_chapter_status.to_string()
    };

    GeneratedResultLifecycleView {
        content_applied,
        provisional_draft_saved,
        attempt_state,
        chapter_status,
    }
}

pub(crate) fn apply_generated_result_quality_view(
    result: &mut GeneratedChapterResult,
    quality_view: &GeneratedResultQualityView,
) {
    result.quality_metrics = quality_view.quality_metrics.clone();
    result.quality_gate_action = quality_view.quality_gate_action.clone();
    result.quality_gate_message = quality_view.quality_gate_message.clone();
}

pub(crate) fn apply_generated_result_lifecycle_view(
    result: &mut GeneratedChapterResult,
    lifecycle_view: &GeneratedResultLifecycleView,
) {
    result.content_applied = lifecycle_view.content_applied;
    result.provisional_draft_saved = lifecycle_view.provisional_draft_saved;
    result.attempt_state = lifecycle_view.attempt_state.clone();
    result.chapter_status = lifecycle_view.chapter_status.clone();
}

pub(crate) fn build_single_generation_followup_draft_result(
    result: &GeneratedChapterResult,
    chapter_status_fallback: &str,
    blocked_attempt_state_fallback: &str,
    quality_gate_action: Option<&str>,
    quality_gate_message: Option<&str>,
    quality_metrics: Option<&Value>,
) -> GeneratedChapterResult {
    let lifecycle_view = generated_result_lifecycle_view(
        chapter_status_fallback,
        quality_gate_action,
        blocked_attempt_state_fallback,
    );
    let metrics = quality_metrics
        .cloned()
        .or_else(|| result.quality_metrics.clone());
    let quality_view = GeneratedResultQualityView {
        quality_gate_decision: generated_result_quality_gate_decision(metrics.as_ref()),
        quality_metrics: metrics,
        quality_gate_action: normalized_non_empty_string(quality_gate_action),
        quality_gate_message: normalized_non_empty_string(quality_gate_message)
            .or_else(|| result.quality_gate_message.clone()),
    };

    let mut draft_result = result.clone();
    apply_generated_result_quality_view(&mut draft_result, &quality_view);
    apply_generated_result_lifecycle_view(&mut draft_result, &lifecycle_view);
    draft_result
}

pub(crate) fn build_single_generation_candidate_draft_attempt(
    chapter_model: &chapter::Model,
    result: &GeneratedChapterResult,
    previous_content: &str,
    previous_word_count: i32,
) -> chapter_draft_attempt::Model {
    let draft_attempt_view = single_generation_candidate_draft_attempt_view(
        result,
        previous_content,
        previous_word_count,
    );

    chapter_draft_attempt::Model {
        id: Uuid::new_v4().to_string(),
        project_id: chapter_model.project_id.clone(),
        chapter_id: Some(chapter_model.id.clone()),
        batch_task_id: None,
        source: "chapter".to_string(),
        attempt_state: result.attempt_state.clone(),
        quality_gate_action: draft_attempt_view.quality_gate_action,
        quality_gate_decision: draft_attempt_view.quality_gate_decision,
        word_count: draft_attempt_view.word_count,
        summary_preview: draft_attempt_view.summary_preview,
        content_preview: draft_attempt_view.content_preview,
        quality_metrics: draft_attempt_view.quality_metrics,
        repair_payload: Some(Value::Object(draft_attempt_view.repair_payload)),
        created_at: Some(Utc::now().naive_utc()),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationCandidateDraftLifecycleView {
    pub(crate) draft_attempt: chapter_draft_attempt::Model,
    pub(crate) candidate_draft_payload: Value,
}

pub(crate) fn single_generation_candidate_draft_lifecycle_view(
    chapter_model: &chapter::Model,
    result: &GeneratedChapterResult,
    previous_content: &str,
    previous_word_count: i32,
) -> SingleGenerationCandidateDraftLifecycleView {
    let draft_attempt = build_single_generation_candidate_draft_attempt(
        chapter_model,
        result,
        previous_content,
        previous_word_count,
    );
    let candidate_draft_payload =
        build_candidate_draft_payload(&draft_attempt, chapter_model.updated_at, false);

    SingleGenerationCandidateDraftLifecycleView {
        draft_attempt,
        candidate_draft_payload,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationCandidateDraftAttemptView {
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_decision: Option<String>,
    pub(crate) word_count: i32,
    pub(crate) summary_preview: Option<String>,
    pub(crate) content_preview: Option<String>,
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) repair_payload: serde_json::Map<String, Value>,
}

pub(crate) fn single_generation_candidate_draft_attempt_view(
    result: &GeneratedChapterResult,
    previous_content: &str,
    previous_word_count: i32,
) -> SingleGenerationCandidateDraftAttemptView {
    let quality_view = GeneratedResultQualityView {
        quality_metrics: result.quality_metrics.clone(),
        quality_gate_action: result.quality_gate_action.clone(),
        quality_gate_decision: generated_result_quality_gate_decision(
            result.quality_metrics.as_ref(),
        ),
        quality_gate_message: result.quality_gate_message.clone(),
    };
    let mut repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        None,
        quality_view.quality_metrics.as_ref(),
        quality_view.quality_metrics.as_ref(),
        "chapter",
        "plot_analysis",
        "Plot analysis",
    )
    .and_then(|payload| payload.as_object().cloned())
    .unwrap_or_default();
    repair_payload.insert(
        "previous_content".to_string(),
        json!(previous_content.trim()),
    );
    repair_payload.insert(
        "previous_word_count".to_string(),
        json!(previous_word_count.max(0)),
    );
    repair_payload.insert(
        "candidate_full_content".to_string(),
        json!(result.content.clone()),
    );
    repair_payload.insert("content_complete".to_string(), json!(true));

    SingleGenerationCandidateDraftAttemptView {
        quality_gate_action: quality_view.quality_gate_action,
        quality_gate_decision: quality_view.quality_gate_decision,
        word_count: result.word_count.max(0),
        summary_preview: Some(result.content.chars().take(220).collect::<String>()),
        content_preview: Some(result.content.chars().take(4000).collect::<String>()),
        quality_metrics: quality_view.quality_metrics,
        repair_payload,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistedHistoryPayloadView {
    pub(crate) content_applied: bool,
    pub(crate) attempt_state: Option<String>,
    pub(crate) candidate_gateway_metadata: Option<Value>,
}

pub(crate) fn persisted_history_payload_view(
    generated_content: Option<&str>,
) -> PersistedHistoryPayloadView {
    let default_attempt_state = Some("generated_from_runtime".to_string());
    let Some(generated_content) = generated_content else {
        return PersistedHistoryPayloadView {
            content_applied: true,
            attempt_state: default_attempt_state,
            candidate_gateway_metadata: None,
        };
    };
    let Ok(payload) = serde_json::from_str::<Value>(generated_content) else {
        return PersistedHistoryPayloadView {
            content_applied: true,
            attempt_state: default_attempt_state,
            candidate_gateway_metadata: None,
        };
    };

    let content_applied = payload
        .get("content_applied")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let attempt_state = payload
        .get("attempt_state")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let candidate_gateway_metadata = payload
        .get("candidate_gateway")
        .filter(|metadata| metadata.is_object())
        .cloned();

    PersistedHistoryPayloadView {
        content_applied,
        attempt_state,
        candidate_gateway_metadata,
    }
}

pub(crate) async fn update_latest_generated_chapter_history_quality_metrics(
    db: &DatabaseConnection,
    chapter_id: &str,
    content: &str,
    quality_metrics: &Value,
) -> Result<(), String> {
    let Some(history_model) = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .filter(
            generation_history::Column::Model
                .eq(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
        )
        .order_by_desc(generation_history::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };

    let persisted_history_view =
        persisted_history_payload_view(history_model.generated_content.as_deref());
    let mut active: generation_history::ActiveModel = history_model.into();
    let created_at = active
        .created_at
        .clone()
        .take()
        .flatten()
        .unwrap_or_else(|| Utc::now().naive_utc());
    active.generated_content = Set(Some(
        build_generated_chapter_history_payload_with_quality_metrics(
            content,
            Some(quality_metrics),
            persisted_history_view.candidate_gateway_metadata.as_ref(),
            persisted_history_view.content_applied,
            persisted_history_view.attempt_state.as_deref(),
            created_at,
        )
        .to_string(),
    ));
    active.update(db).await.map_err(|error| error.to_string())?;
    Ok(())
}
