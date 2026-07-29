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
use crate::services::generation_contract_service::{
    merge_generation_contract_history_summary, GenerationContractSnapshotV1,
};
use crate::services::generation_execution_audit_service::{
    merge_generation_execution_audit, GenerationExecutionAuditV1,
};

pub(crate) fn build_chapter_generation_history_persistence_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_history_persistence_service",
        "scope": "generated_history_model_construction_single_generation_persistence_transaction_and_candidate_draft_attempt_insert",
        "python_source_map": [
            "backend/migrator_app/models/generation_history.py",
            "backend/migrator_app/models/chapter_draft_attempt.py"
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
        "source_map_closeout_status": {
            "default_python_module_consumers": [
                "backend/tests/test_support/database_test_support.py"
            ],
            "dedicated_python_regression_surfaces": [
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_stream_routes.py"
            ],
            "shared_test_support_consumers": [
                "backend/tests/test_support/chapter_generation_history_test_support.py",
                "backend/tests/test_support/batch_generation_retry_test_adapter.py"
            ],
            "physical_python_closeout_completed": true,
            "shared_schema_hold_status": {
                "generation_history_model": "shared_python_database_metadata_and_regression_reference",
                "chapter_draft_attempt_model": "shared_python_database_metadata_and_regression_reference",
                "default_python_module_consumers": [
                    "backend/tests/test_support/database_test_support.py"
                ],
                "physical_closeout_ready": false
            },
            "remaining_cutover_gate": "generation_history_and_chapter_draft_attempt_python_model_files_remain_shared_metadata_and_regression_references_only",
            "status": "rust_chapter_generation_history_persistence_owner_shared_model_source_maps_only"
        },
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo test chapter_generation_history_persistence_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_history_persistence_owner_is_rust_only_surviving_python_generation_history_and_chapter_draft_attempt_closeout_is_limited_to_shared_metadata_registration_and_regression_reference",
            "rollback_files": [
                "backend/migrator_app/models/generation_history.py",
                "backend/migrator_app/models/chapter_draft_attempt.py"
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

fn build_generated_history_payload_base(
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

pub(crate) fn build_generated_history_payload(
    result: &GeneratedChapterResult,
    created_at: NaiveDateTime,
) -> Value {
    build_generated_history_payload_base(result, created_at)
}

pub(crate) fn build_generated_history_payload_with_contract(
    result: &GeneratedChapterResult,
    generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
    created_at: NaiveDateTime,
) -> Result<Value, String> {
    build_generated_history_payload_with_contract_and_audit(
        result,
        generation_contract_snapshot,
        None,
        created_at,
    )
}

pub(crate) fn build_generated_history_payload_with_contract_and_audit(
    result: &GeneratedChapterResult,
    generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
    generation_execution_audit: Option<&GenerationExecutionAuditV1>,
    created_at: NaiveDateTime,
) -> Result<Value, String> {
    let mut payload = build_generated_history_payload_base(result, created_at);
    if let Some(snapshot) = generation_contract_snapshot {
        merge_generation_contract_history_summary(&mut payload, snapshot)
            .map_err(|error| error.to_string())?;
    }
    if let Some(audit) = generation_execution_audit {
        merge_generation_execution_audit(&mut payload, audit).map_err(|error| error.to_string())?;
    }
    Ok(payload)
}

fn build_generated_history_active_model(
    chapter_model: &chapter::Model,
    prompt: String,
    result: &GeneratedChapterResult,
    generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
    generation_execution_audit: Option<&GenerationExecutionAuditV1>,
    created_at: NaiveDateTime,
) -> Result<generation_history::ActiveModel, String> {
    let generated_content = build_generated_history_payload_with_contract_and_audit(
        result,
        generation_contract_snapshot,
        generation_execution_audit,
        created_at,
    )?;
    Ok(generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(Some(chapter_model.id.clone())),
        prompt: Set(Some(prompt)),
        generated_content: Set(Some(generated_content.to_string())),
        model: Set(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(created_at)),
    })
}

pub(crate) async fn persist_single_generation_generated_result(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    prompt: String,
    result: GeneratedChapterResult,
) -> Result<GeneratedChapterResult, String> {
    persist_single_generation_generated_result_with_contract(
        db,
        chapter_model,
        prompt,
        result,
        None,
    )
    .await
}

pub(crate) async fn persist_single_generation_generated_result_with_contract(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    prompt: String,
    result: GeneratedChapterResult,
    generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
) -> Result<GeneratedChapterResult, String> {
    persist_single_generation_generated_result_with_contract_and_audit(
        db,
        chapter_model,
        prompt,
        result,
        generation_contract_snapshot,
        None,
    )
    .await
}

pub(crate) async fn persist_single_generation_generated_result_with_contract_and_audit(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    prompt: String,
    mut result: GeneratedChapterResult,
    generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
    generation_execution_audit: Option<&GenerationExecutionAuditV1>,
) -> Result<GeneratedChapterResult, String> {
    let now = Utc::now().naive_utc();
    let txn = db.begin().await.map_err(|error| error.to_string())?;
    let previous_word_count = chapter_model.word_count.max(0);
    let should_persist_content = result.content_applied || result.provisional_draft_saved;

    let history = build_generated_history_active_model(
        chapter_model,
        prompt,
        &result,
        generation_contract_snapshot,
        generation_execution_audit,
        now,
    )?;
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_chapter_generation_history_persistence_owner_contract;

    #[test]
    fn should_publish_history_persistence_owner_contract_with_shared_metadata_only_python_hold() {
        let contract = build_chapter_generation_history_persistence_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_history_persistence_service"
        );
        assert_eq!(
            contract["python_source_map"],
            json!([
                "backend/migrator_app/models/generation_history.py",
                "backend/migrator_app/models/chapter_draft_attempt.py"
            ])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["default_python_module_consumers"],
            json!(["backend/tests/test_support/database_test_support.py"])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_test_support_consumers"],
            json!([
                "backend/tests/test_support/chapter_generation_history_test_support.py",
                "backend/tests/test_support/batch_generation_retry_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["physical_python_closeout_completed"],
            json!(true)
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["generation_history_model"],
            "shared_python_database_metadata_and_regression_reference"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["chapter_draft_attempt_model"],
            "shared_python_database_metadata_and_regression_reference"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["remaining_cutover_gate"],
            "generation_history_and_chapter_draft_attempt_python_model_files_remain_shared_metadata_and_regression_references_only"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["status"],
            "rust_chapter_generation_history_persistence_owner_shared_model_source_maps_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_history_persistence_owner_is_rust_only_surviving_python_generation_history_and_chapter_draft_attempt_closeout_is_limited_to_shared_metadata_registration_and_regression_reference"
        );
    }
}
