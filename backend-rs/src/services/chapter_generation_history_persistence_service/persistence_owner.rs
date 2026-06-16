use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_generation_history_payload_service::build_generated_chapter_history_payload_with_quality_metrics;
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_single_generation_result_lifecycle_service::{
    single_generation_candidate_draft_lifecycle_view, SingleGenerationCandidateDraftLifecycleView,
    CHAPTER_GENERATION_HISTORY_MODEL,
};

pub(crate) fn build_chapter_generation_history_persistence_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_history_persistence_service",
        "scope": "generated_history_model_construction_single_generation_persistence_transaction_and_candidate_draft_attempt_insert",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/models/generation_history.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_history_persistence_service.rs",
            "backend-rs/src/services/chapter_generation_history_persistence_service/persistence_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_generation_history_payload_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_generated_history_active_model",
                "build_single_generation_candidate_draft_attempt_active_model",
                "persist_single_generation_candidate_draft_attempt",
                "persist_single_generation_generated_result"
            ],
            "persistence_steps": [
                "chapter_update_when_content_applied_or_provisional_draft_saved",
                "candidate_draft_attempt_insert_when_content_not_applied",
                "generation_history_insert",
                "single_transaction_commit"
            ],
            "history_model": CHAPTER_GENERATION_HISTORY_MODEL
        },
        "active_consumers": [
            "chapter_generation_runtime_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo test chapter_generation_history_persistence_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_generation_finalize_and_history_shells_as_source_map_until_explicit_freeze_delete_round",
            "rollback_files": [
                "backend/app/services/chapter_generation/stream/finalize_service.py",
                "backend/app/services/chapter_generation/stream/candidate_service.py",
                "backend/app/models/generation_history.py"
            ]
        }
    })
}

pub(crate) fn build_single_generation_candidate_draft_attempt_active_model(
    draft_attempt: &chapter_draft_attempt::Model,
) -> chapter_draft_attempt::ActiveModel {
    chapter_draft_attempt::ActiveModel {
        id: Set(draft_attempt.id.clone()),
        project_id: Set(draft_attempt.project_id.clone()),
        chapter_id: Set(draft_attempt.chapter_id.clone()),
        batch_task_id: Set(draft_attempt.batch_task_id.clone()),
        source: Set(draft_attempt.source.clone()),
        attempt_state: Set(draft_attempt.attempt_state.clone()),
        quality_gate_action: Set(draft_attempt.quality_gate_action.clone()),
        quality_gate_decision: Set(draft_attempt.quality_gate_decision.clone()),
        word_count: Set(draft_attempt.word_count),
        summary_preview: Set(draft_attempt.summary_preview.clone()),
        content_preview: Set(draft_attempt.content_preview.clone()),
        quality_metrics: Set(draft_attempt.quality_metrics.clone()),
        repair_payload: Set(draft_attempt.repair_payload.clone()),
        created_at: Set(draft_attempt.created_at),
    }
}

pub(crate) async fn persist_single_generation_candidate_draft_attempt(
    db: &DatabaseConnection,
    draft_lifecycle_view: SingleGenerationCandidateDraftLifecycleView,
) -> Result<Value, String> {
    let draft_attempt = draft_lifecycle_view.draft_attempt;
    let draft_summary = draft_lifecycle_view.candidate_draft_payload;

    build_single_generation_candidate_draft_attempt_active_model(&draft_attempt)
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(draft_summary)
}

pub(crate) fn build_generated_history_payload(
    result: &GeneratedChapterResult,
    created_at: NaiveDateTime,
) -> Value {
    build_generated_chapter_history_payload_with_quality_metrics(
        &result.content,
        result.quality_metrics.as_ref(),
        result.candidate_gateway_metadata.as_ref(),
        result.content_applied,
        Some(&result.attempt_state),
        created_at,
    )
}

pub(crate) fn build_generated_history_active_model(
    chapter_model: &chapter::Model,
    prompt: String,
    result: &GeneratedChapterResult,
    created_at: NaiveDateTime,
) -> generation_history::ActiveModel {
    generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(Some(chapter_model.id.clone())),
        prompt: Set(Some(prompt)),
        generated_content: Set(Some(
            build_generated_history_payload(result, created_at).to_string(),
        )),
        model: Set(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(created_at)),
    }
}

pub(crate) async fn persist_single_generation_generated_result(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    prompt: String,
    mut result: GeneratedChapterResult,
) -> Result<GeneratedChapterResult, String> {
    let now = Utc::now().naive_utc();
    let txn = db.begin().await.map_err(|error| error.to_string())?;
    let previous_word_count = chapter_model.word_count.max(0);
    let should_persist_content = result.content_applied || result.provisional_draft_saved;

    let history = build_generated_history_active_model(chapter_model, prompt, &result, now);
    if should_persist_content {
        let mut active: chapter::ActiveModel = chapter_model.clone().into();
        active.content = Set(Some(result.content.clone()));
        active.word_count = Set(result.word_count);
        active.status = Set(result.chapter_status.clone());
        active.updated_at = Set(Some(now));
        active
            .update(&txn)
            .await
            .map_err(|error| error.to_string())?;
        result.saved_word_count = result.word_count;
    } else {
        result.saved_word_count = previous_word_count;
    }

    if !result.content_applied {
        let draft_lifecycle_view = single_generation_candidate_draft_lifecycle_view(
            chapter_model,
            &result,
            chapter_model.content.as_deref().unwrap_or_default(),
            previous_word_count,
        );
        let draft_attempt = draft_lifecycle_view.draft_attempt;
        build_single_generation_candidate_draft_attempt_active_model(&draft_attempt)
            .insert(&txn)
            .await
            .map_err(|error| error.to_string())?;
        result.candidate_draft = Some(draft_lifecycle_view.candidate_draft_payload);
    }

    history
        .insert(&txn)
        .await
        .map_err(|error| error.to_string())?;

    txn.commit().await.map_err(|error| error.to_string())?;

    Ok(result)
}
