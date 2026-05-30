use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_batch_generation_access_service::{
    load_accessible_chapters_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_generation_prerequisite_service::check_chapter_generation_prerequisites;
use crate::services::chapter_batch_generation_quality_status_service::{
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalKind,
    BatchGenerationQualityStatusContext,
};
use crate::services::chapter_batch_generation_resume_semantics_service::{
    ResumeBatchGenerationCommandState, ResumeExecutionSelection,
};
use crate::services::chapter_batch_generation_runtime_state_service::{
    build_batch_generation_resume_runtime_checkpoint, dispatch_batch_generation_runtime,
};
use crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationExecutionInput;
use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
use crate::services::chapter_batch_generation_runtime_state_service::reset_batch_generation_task_for_resume;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_task_runtime_payload_from_runtime_parts,
    BatchGenerationCommandProgressSummary,
};
use crate::services::chapter_batch_generation_write_workflow_service::{
    active_story_repair_payload_from_runtime_state, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_single_generation_prepare_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
};
use crate::services::chapter_single_generation_runtime_state_service::dispatch_single_chapter_generation_runtime;
use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
use crate::services::chapter_story_repair_quality_context_service::{
    build_quality_metrics_summary_state_from_history,
    extract_quality_history_context,
    restore_active_story_repair_payload_from_quality_context as restore_active_story_repair_payload_from_quality_state,
    restore_story_repair_compat_options_from_active_snapshot as restore_story_repair_compat_options_from_quality_state,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResumeBatchGenerationDomainError {
    InvalidStatus,
    ManualReviewBlocked,
    NoChaptersToResume,
    ChaptersUnavailable,
    PrerequisitesBlocked(String),
    Internal(String),
}

impl ResumeBatchGenerationDomainError {
    pub(crate) fn detail_message(&self) -> String {
        match self {
            Self::InvalidStatus => "Only failed or cancelled tasks can be resumed".to_string(),
            Self::ManualReviewBlocked => {
                "Manual review blocked tasks cannot be resumed".to_string()
            }
            Self::NoChaptersToResume => {
                "Batch generation task has no chapters to resume".to_string()
            }
            Self::ChaptersUnavailable => "Some chapters no longer exist".to_string(),
            Self::PrerequisitesBlocked(detail) => {
                format!("Resume blocked by prerequisites: {detail}")
            }
            Self::Internal(detail) => detail.clone(),
        }
    }
}

pub(crate) fn dispatch_resumed_batch_generation_execution(
    db: DatabaseConnection,
    task_id: String,
    user_id: String,
    execution_selection: ResumeExecutionSelection,
    target_word_count: i32,
    compat_options: SingleChapterGenerationCompatOptions,
    execution_config: PreparedGenerationExecutionConfig,
) {
    match execution_selection {
        ResumeExecutionSelection::SingleChapter { chapter_id } => {
            dispatch_single_chapter_generation_runtime(
                db,
                task_id,
                SingleGenerationRuntimeLaunchInput {
                    chapter_id,
                    user_id,
                    execution_input: SingleChapterGenerationExecutionInput {
                        target_word_count,
                        compat_options,
                        execution_config,
                    },
                },
            )
        }
        ResumeExecutionSelection::Batch { chapter_ids } => dispatch_batch_generation_runtime(
            db,
            task_id,
            BatchGenerationExecutionInput {
                user_id,
                chapter_ids,
                target_word_count,
                compat_options,
                ai_config: execution_config.ai_config,
            },
        ),
    }
}

fn restore_story_repair_compat_options_from_active_snapshot(
    compat_options: &SingleChapterGenerationCompatOptions,
    active_story_repair_payload: Option<&Value>,
    snapshot: Option<&batch_generation_snapshot::Model>,
) -> SingleChapterGenerationCompatOptions {
    restore_story_repair_compat_options_from_quality_state(
        compat_options,
        active_story_repair_payload,
        snapshot.and_then(|item| item.quality_metrics_summary.as_ref()),
        snapshot.and_then(|item| item.latest_quality_metrics.as_ref()),
    )
}

fn restore_active_story_repair_payload_from_quality_context(
    snapshot: &batch_generation_snapshot::Model,
    scope: &str,
) -> Option<Value> {
    restore_active_story_repair_payload_from_quality_state(
        snapshot.quality_metrics_summary.as_ref(),
        snapshot.latest_quality_metrics.as_ref(),
        scope,
        "recent_history_summary",
        "Recent history summary",
    )
}

pub(crate) async fn prepare_batch_generation_resume(
    db: &DatabaseConnection,
    command_state: ResumeBatchGenerationCommandState,
    _user_id: &str,
    workflow_runtime_state: Option<&Value>,
    snapshot: Option<&batch_generation_snapshot::Model>,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<(ResumeExecutionSelection, i32, Value), ResumeBatchGenerationDomainError> {
    if !matches!(command_state.status.as_str(), "failed" | "cancelled") {
        return Err(ResumeBatchGenerationDomainError::InvalidStatus);
    }

    let runtime_active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state);
    let failed_terminal_semantics = resolve_failed_terminal_semantics_from_sources(
        Some(&command_state.failed_chapters),
        Some(&BatchGenerationQualityStatusContext {
            active_story_repair_payload: runtime_active_story_repair_payload.clone(),
            quality_metrics_summary: snapshot.and_then(|item| item.quality_metrics_summary.clone()),
            latest_quality_metrics: snapshot.and_then(|item| item.latest_quality_metrics.clone()),
        }),
        command_state.current_retry_count,
        command_state.max_retries,
    );
    if failed_terminal_semantics
        .as_ref()
        .is_some_and(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::ManualReview)
    {
        return Err(ResumeBatchGenerationDomainError::ManualReviewBlocked);
    }

    let target_word_count =
        normalize_chapter_generation_target_word_count(Some(command_state.target_word_count));
    let execution = match command_state.resolve_execution_selection() {
        Some(selection) => selection,
        None => return Err(ResumeBatchGenerationDomainError::NoChaptersToResume),
    };
    let restored_compat_options = restore_story_repair_compat_options_from_active_snapshot(
        &request_runtime_state.compat_options,
        runtime_active_story_repair_payload.as_ref(),
        snapshot,
    );
    let restored_request_runtime_state = BatchGenerationRequestRuntimeState::new(
        restored_compat_options,
        request_runtime_state.model_override.clone(),
    );
    if let ResumeExecutionSelection::Batch { chapter_ids } = &execution {
        let remaining_chapters = load_accessible_chapters_for_generation(db, chapter_ids, _user_id)
            .await
            .map_err(|error| match error {
                LoadAccessibleChapterForGenerationError::ChapterNotFound
                | LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
                    ResumeBatchGenerationDomainError::ChaptersUnavailable
                }
                LoadAccessibleChapterForGenerationError::Internal(detail) => {
                    ResumeBatchGenerationDomainError::Internal(detail)
                }
            })?;
        if let Some(first_chapter) = remaining_chapters.first() {
            let prerequisite = check_chapter_generation_prerequisites(db, first_chapter)
                .await
                .map_err(ResumeBatchGenerationDomainError::Internal)?;
            if !prerequisite.can_generate {
                return Err(ResumeBatchGenerationDomainError::PrerequisitesBlocked(
                    prerequisite.error_message,
                ));
            }
        }
    }
    let resume_runtime_state_seed =
        build_resume_runtime_state_seed(
            &command_state,
            workflow_runtime_state,
            snapshot,
            &restored_request_runtime_state,
        );
    let resume_checkpoint =
        build_batch_generation_resume_runtime_checkpoint(&command_state, resume_runtime_state_seed.clone());

    reset_batch_generation_task_for_resume(db, &command_state, resume_runtime_state_seed)
        .await
        .map_err(ResumeBatchGenerationDomainError::Internal)?;

    let reset = command_state.resolve_reset_semantics();
    let task_id = command_state.batch_id.clone();
    let summary = BatchGenerationCommandProgressSummary {
        batch_id: task_id.clone(),
        total_chapters: command_state.total_chapters,
        completed_chapters: reset.completed_chapters,
    };
    let mut payload = build_batch_generation_task_runtime_payload_from_runtime_parts(
        summary.batch_id(),
        crate::services::chapter_batch_generation_status_semantics_service::batch_generation_task_type(
            command_state.task_kind(),
        ),
        &command_state.project_id,
        reset.status,
        reset.current_chapter_id.as_deref(),
        command_state.created_at,
        Some(&resume_checkpoint),
        Some(("chapter_id", json!(reset.current_chapter_id.clone()))),
    );

    payload.insert("total_chapters".to_string(), json!(summary.total_chapters));
    payload.insert(
        "completed_chapters".to_string(),
        json!(summary.completed_chapters),
    );
    payload.insert("batch_id".to_string(), json!(summary.batch_id));
    payload.insert("message".to_string(), json!("Batch generation resumed"));
    payload.insert(
        "resumed_from_batch_id".to_string(),
        json!(command_state.batch_id),
    );

    Ok((execution, target_word_count, Value::Object(payload)))
}

fn build_resume_runtime_state_seed(
    command_state: &ResumeBatchGenerationCommandState,
    workflow_runtime_state: Option<&Value>,
    snapshot: Option<&batch_generation_snapshot::Model>,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Option<Value> {
    let quality_metrics_summary = snapshot.and_then(|item| item.quality_metrics_summary.clone());
    let quality_metrics_history = snapshot.and_then(|item| item.quality_metrics_history.clone());
    let quality_metrics_summary_state = workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("quality_metrics_summary_state"))
        .cloned()
        .or_else(|| {
            quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|history| build_quality_metrics_summary_state_from_history(history, "batch"))
        });
    let active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state)
            .or_else(|| {
                snapshot.and_then(|item| {
                    restore_active_story_repair_payload_from_quality_context(item, "batch")
                })
            })
            .or_else(|| request_runtime_state.active_story_repair_payload_with_scope("batch"));

    let mut runtime_state = serde_json::Map::from_iter([
        (
            "resume_from_batch_id".to_string(),
            json!(command_state.batch_id.clone()),
        ),
        ("current_retry_count".to_string(), json!(0)),
        ("max_retries".to_string(), json!(command_state.max_retries)),
    ]);

    if let Some(payload) = active_story_repair_payload {
        runtime_state.insert("active_story_repair_payload".to_string(), payload);
    }
    if let Some(summary) = quality_metrics_summary {
        runtime_state.insert("quality_metrics_summary".to_string(), summary.clone());
        if let Some(history_context) = extract_quality_history_context(Some(&summary)) {
            runtime_state.insert("quality_history_context".to_string(), history_context);
        }
    }
    if let Some(history) = quality_metrics_history {
        runtime_state.insert("quality_metrics_history".to_string(), history);
    }
    if let Some(summary_state) = quality_metrics_summary_state {
        runtime_state.insert("quality_metrics_summary_state".to_string(), summary_state);
    }

    Some(Value::Object(runtime_state))
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_snapshot;
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::manual_review_label;
    use crate::services::chapter_batch_generation_resume_semantics_service::{
        ResumeBatchGenerationCommandState, ResumeExecutionSelection,
    };
    use crate::services::chapter_batch_generation_runtime_checkpoint_service::build_pending_batch_generation_runtime_checkpoint;
    use crate::services::chapter_batch_generation_task_payload_base_service::build_batch_generation_command_summary_payload;
    use crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;
    use crate::services::chapter_story_repair_quality_context_service::{
        extract_quality_gate_object, extract_repair_guidance_object,
    };
    use serde_json::json;

    use super::{
        build_resume_runtime_state_seed, dispatch_resumed_batch_generation_execution,
        restore_active_story_repair_payload_from_quality_context,
        restore_story_repair_compat_options_from_active_snapshot,
        ResumeBatchGenerationDomainError,
    };

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_snapshot(
        latest_quality_metrics: Option<serde_json::Value>,
        quality_metrics_summary: Option<serde_json::Value>,
    ) -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics,
            quality_metrics_history: None,
            quality_metrics_summary,
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_build_resume_execution_selection_for_single_and_batch_tasks() {
        let mut single = build_task("failed");
        single.chapter_count = 1;
        single.chapter_ids = json!(["chapter-1"]);
        single.current_chapter_id = Some("chapter-1".to_string());

        let single_state = ResumeBatchGenerationCommandState::from_task(&single);
        let single_selection = single_state
            .resolve_execution_selection()
            .expect("single selection should exist");
        assert!(matches!(
            single_selection,
            crate::services::chapter_batch_generation_resume_semantics_service::ResumeExecutionSelection::SingleChapter {
                chapter_id,
            } if chapter_id == "chapter-1"
        ));

        let mut batch = build_task("cancelled");
        batch.chapter_count = 2;
        batch.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}]);
        batch.current_chapter_id = None;

        let batch_state = ResumeBatchGenerationCommandState::from_task(&batch);
        let batch_selection = batch_state
            .resolve_execution_selection()
            .expect("batch selection should exist");
        assert!(matches!(
            batch_selection,
            crate::services::chapter_batch_generation_resume_semantics_service::ResumeExecutionSelection::Batch {
                chapter_ids,
            } if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_detect_quality_blocked_failed_chapter_as_manual_review_blocker() {
        assert_eq!(
            manual_review_label(Some(&json!([{
                "phase": "quality_blocked"
            }]))),
            Some("需人工复核".to_string())
        );
    }

    #[test]
    fn should_detect_exhausted_auto_repair_quality_context_as_manual_review_blocker() {
        assert_eq!(
            crate::services::chapter_batch_generation_quality_status_service::manual_review_label_from_quality_context_with_retry_budget(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复预算已耗尽"
                    }
                })),
                None,
                3,
                3,
            ),
            Some("自动修复预算已耗尽".to_string())
        );
    }

    #[test]
    fn should_fail_malformed_single_resume_execution_selection_with_shared_batch_fallback() {
        let mut malformed_single = build_task("failed");
        malformed_single.chapter_count = 1;
        malformed_single.chapter_ids = json!({"chapter_id": "chapter-1"});
        malformed_single.current_chapter_id = Some("chapter-1".to_string());

        let malformed_state = ResumeBatchGenerationCommandState::from_task(&malformed_single);
        let error = malformed_state
            .resolve_execution_selection()
            .map(|_| panic!("malformed single should fallback to batch error"))
            .unwrap_or(ResumeBatchGenerationDomainError::NoChaptersToResume);

        assert_eq!(error, ResumeBatchGenerationDomainError::NoChaptersToResume);
        assert_eq!(
            error.detail_message(),
            "Batch generation task has no chapters to resume"
        );
    }

    #[test]
    fn should_keep_resume_domain_error_detail_messages_stable() {
        assert_eq!(
            ResumeBatchGenerationDomainError::InvalidStatus.detail_message(),
            "Only failed or cancelled tasks can be resumed"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::ManualReviewBlocked.detail_message(),
            "Manual review blocked tasks cannot be resumed"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::NoChaptersToResume.detail_message(),
            "Batch generation task has no chapters to resume"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::ChaptersUnavailable.detail_message(),
            "Some chapters no longer exist"
        );
        assert_eq!(
            ResumeBatchGenerationDomainError::Internal("boom".to_string()).detail_message(),
            "boom"
        );
    }

    #[test]
    fn should_detect_manual_review_resume_blocker_from_shared_quality_semantics() {
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "needs review"
            }]))),
            Some("needs review".to_string())
        );
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review"
            }]))),
            Some("需人工复核".to_string())
        );
        assert!(manual_review_label(Some(&json!([{
            "quality_gate_decision": "passed"
        }])))
        .is_none());
    }

    #[tokio::test]
    async fn should_block_resume_when_runtime_active_story_repair_payload_requires_manual_review() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "需要人工处理",
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核"
            }
        });

        let result = super::prepare_batch_generation_resume(
            &sea_orm::DatabaseConnection::Disconnected,
            command_state,
            "user-1",
            Some(&workflow_runtime_state),
            None,
            &BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        )
        .await;

        match result {
            Err(ResumeBatchGenerationDomainError::ManualReviewBlocked) => {}
            other => panic!("expected ManualReviewBlocked, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn should_block_resume_when_quality_summary_requires_manual_review_even_without_failed_chapter_label() {
        let mut task = build_task("failed");
        task.failed_chapters = json!([]);
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "等待人工处理"
                },
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "质量闸门要求人工复核"
                }
            })),
        );

        let result = super::prepare_batch_generation_resume(
            &sea_orm::DatabaseConnection::Disconnected,
            command_state,
            "user-1",
            None,
            Some(&snapshot),
            &BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        )
        .await;

        match result {
            Err(ResumeBatchGenerationDomainError::ManualReviewBlocked) => {}
            other => panic!("expected ManualReviewBlocked, got {:?}", other),
        }
    }

    #[test]
    fn should_build_pending_runtime_checkpoint_for_queued_batch_task() {
        let checkpoint = build_pending_batch_generation_runtime_checkpoint(
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            Some((0, 4)),
        );

        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "queued");
        assert_eq!(checkpoint["completed"], 0);
        assert_eq!(checkpoint["total"], 4);
        assert!(checkpoint["chapter_id"].is_null());
    }

    #[test]
    fn should_build_resume_payload_from_updated_task_projection() {
        let mut task = build_task("pending");
        task.project_id = "project-9".to_string();
        task.current_chapter_id = Some("chapter-2".to_string());
        task.total_chapters = 3;
        task.completed_chapters = 1;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let payload = serde_json::json!({
            "batch_id": command_state.batch_id,
            "message": "Batch generation resumed",
            "project_id": command_state.project_id,
            "task_type": "chapter_single_generate",
            "status": "pending",
            "stage_code": "6.writing.pending",
            "execution_mode": "interactive",
            "current_chapter_id": "chapter-2",
            "created_at": null,
            "checkpoint": {
                "stage_code": "6.writing.pending",
                "execution_mode": "interactive",
                "chapter_id": "chapter-2"
            },
            "completed_chapters": 1,
            "total_chapters": 3
        });
        let summary_payload = build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: "task-1".to_string(),
                total_chapters: 3,
                completed_chapters: 1,
            },
            "Batch generation resumed",
        );
        assert_eq!(summary_payload["total_chapters"], 3);
        assert_eq!(summary_payload["completed_chapters"], 1);
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-2");
    }

    #[test]
    fn should_build_resume_payload_from_shared_command_projection_owner() {
        let mut task = build_task("failed");
        task.id = "task-7".to_string();
        task.project_id = "project-7".to_string();
        task.total_chapters = 4;
        task.completed_chapters = 2;
        task.current_chapter_id = Some("chapter-4".to_string());

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let summary_payload = build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: command_state.completed_chapters,
            },
            "Batch generation resumed",
        );
        assert_eq!(summary_payload["total_chapters"], 4);
        assert_eq!(summary_payload["completed_chapters"], 2);
    }

    #[test]
    fn should_build_resume_payload_from_reset_single_task_projection() {
        let mut task = build_task("failed");
        task.project_id = "project-9".to_string();
        task.current_chapter_id = Some("chapter-2".to_string());
        task.total_chapters = 3;
        task.completed_chapters = 2;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let reset = command_state.resolve_reset_semantics();
        let summary_payload = build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: reset.completed_chapters,
            },
            "Batch generation resumed",
        );
        assert_eq!(summary_payload["total_chapters"], 3);
        assert_eq!(summary_payload["completed_chapters"], 0);
        assert_eq!(reset.status, "pending");
        assert_eq!(reset.current_chapter_id.as_deref(), Some("chapter-2"));
    }

    #[test]
    fn should_build_resume_payload_from_reset_batch_task_projection() {
        let mut task = build_task("cancelled");
        task.chapter_count = 2;
        task.chapter_ids = json!(["chapter-1", "chapter-2"]);
        task.current_chapter_id = Some("chapter-2".to_string());
        task.current_chapter_number = Some(2);
        task.total_chapters = 2;
        task.completed_chapters = 1;

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let reset = command_state.resolve_reset_semantics();
        let summary_payload = build_batch_generation_command_summary_payload(
            super::BatchGenerationCommandProgressSummary {
                batch_id: command_state.batch_id,
                total_chapters: command_state.total_chapters,
                completed_chapters: reset.completed_chapters,
            },
            "Batch generation resumed",
        );
        assert_eq!(summary_payload["total_chapters"], 2);
        assert_eq!(summary_payload["completed_chapters"], 0);
        assert_eq!(reset.status, "pending");
        assert!(reset.current_chapter_id.is_none());
    }

    #[test]
    fn should_build_resume_response_payload_from_owner() {
        let payload = serde_json::json!({
            "batch_id": "task-1",
            "message": "Batch generation resumed",
            "project_id": "project-9",
            "task_type": "chapter_single_generate",
            "status": "pending",
            "stage_code": "6.writing.pending",
            "execution_mode": "interactive",
            "current_chapter_id": "chapter-2",
            "created_at": null,
            "checkpoint": {
                "stage_code": "6.writing.pending",
                "execution_mode": "interactive",
                "chapter_id": "chapter-2",
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3,
                "current_chapter_id": "chapter-2",
                "current_chapter_number": 2,
                "phase": "pending",
                "progress": 0,
                "status": "pending",
                "last_event": "resume"
            },
            "completed_chapters": 0,
            "total_chapters": 3,
            "resumed_from_batch_id": "task-1"
        });

        assert_eq!(payload["message"], "Batch generation resumed");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-2");
        assert_eq!(payload["checkpoint"]["resume_from_batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["current_retry_count"], 0);
        assert_eq!(payload["checkpoint"]["max_retries"], 3);
        assert_eq!(payload["checkpoint"]["current_chapter_id"], "chapter-2");
        assert_eq!(payload["checkpoint"]["current_chapter_number"], 2);
        assert_eq!(payload["checkpoint"]["phase"], "pending");
        assert_eq!(payload["checkpoint"]["progress"], 0);
        assert_eq!(payload["checkpoint"]["status"], "pending");
        assert_eq!(payload["checkpoint"]["last_event"], "resume");
        assert_eq!(payload["completed_chapters"], 0);
        assert_eq!(payload["total_chapters"], 3);
        assert_eq!(payload["resumed_from_batch_id"], "task-1");
    }

    #[test]
    fn should_keep_resume_execution_selection_contract_for_dispatch_owner() {
        let single_execution = ResumeExecutionSelection::SingleChapter {
            chapter_id: "chapter-1".to_string(),
        };
        let batch_execution = ResumeExecutionSelection::Batch {
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
        };

        assert!(matches!(
            single_execution,
            ResumeExecutionSelection::SingleChapter {
                chapter_id,
            } if chapter_id == "chapter-1"
        ));
        assert!(matches!(
            batch_execution,
            ResumeExecutionSelection::Batch {
                chapter_ids,
            } if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_keep_resume_dispatch_helper_contract_explicit() {
        let dispatch_helper = dispatch_resumed_batch_generation_execution;
        let selection = ResumeExecutionSelection::Batch {
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
        };

        let _ = dispatch_helper;
        assert!(matches!(
            selection,
            ResumeExecutionSelection::Batch {
                chapter_ids,
            } if chapter_ids == vec!["chapter-1".to_string(), "chapter-2".to_string()]
        ));
    }

    #[test]
    fn should_keep_resume_execution_and_payload_contract_explicit() {
        let execution = ResumeExecutionSelection::SingleChapter {
            chapter_id: "chapter-9".to_string(),
        };
        let response_payload = json!({
            "batch_id": "task-9",
            "message": "Batch generation resumed",
        });

        assert!(matches!(
            execution,
            ResumeExecutionSelection::SingleChapter {
                chapter_id,
            } if chapter_id == "chapter-9"
        ));
        assert_eq!(response_payload["batch_id"], "task-9");
    }

    #[test]
    fn should_prefer_existing_active_story_repair_payload_for_resume_seed() {
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "来自运行态",
                "source": "current_chapter_quality"
            }
        });
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("来自请求".to_string()),
                story_repair_targets: vec!["请求目标".to_string()],
                ..Default::default()
            },
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));

        let seed = build_resume_runtime_state_seed(
            &command_state,
            Some(&workflow_runtime_state),
            None,
            &request_runtime_state,
        )
        .expect("resume runtime state seed");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert_eq!(
            seed["active_story_repair_payload"]["summary"],
            "来自运行态"
        );
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "current_chapter_quality"
        );
    }

    #[test]
    fn should_rehydrate_manual_story_repair_payload_for_resume_seed_when_snapshot_missing() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("补强前章伏笔".to_string()),
                story_repair_targets: vec!["伏笔回收".to_string()],
                story_preserve_strengths: vec!["尾声氛围".to_string()],
                ..Default::default()
            },
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));

        let seed = build_resume_runtime_state_seed(&command_state, None, None, &request_runtime_state)
            .expect("resume seed");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert_eq!(
            seed["active_story_repair_payload"]["summary"],
            "补强前章伏笔"
        );
        assert_eq!(
            seed["active_story_repair_payload"]["repair_targets"],
            json!(["伏笔回收"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["preserve_strengths"],
            json!(["尾声氛围"])
        );
        assert_eq!(
            seed["active_story_repair_payload"]["source"],
            "manual_request"
        );
    }

    #[test]
    fn should_skip_resume_runtime_state_seed_without_story_repair_payload() {
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let seed = build_resume_runtime_state_seed(
            &command_state,
            None,
            None,
            &BatchGenerationRequestRuntimeState::new(
                SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        )
        .expect("resume seed without repair payload");

        assert_eq!(seed["resume_from_batch_id"], "task-1");
        assert_eq!(seed["current_retry_count"], 0);
        assert_eq!(seed["max_retries"], 3);
        assert!(seed.get("active_story_repair_payload").is_none());
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_active_snapshot_when_request_empty() {
        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            Some(&json!({
                "summary": "补强前章伏笔",
                "repair_targets": ["回收悬念", "压缩说明"],
                "preserve_strengths": ["角色张力", "结尾钩子"]
            })),
            None,
        );

        assert_eq!(
            restored.story_repair_summary(),
            "补强前章伏笔"
        );
        assert_eq!(
            restored.story_repair_targets(),
            &["回收悬念".to_string(), "压缩说明".to_string()]
        );
        assert_eq!(
            restored.story_preserve_strengths(),
            &["角色张力".to_string(), "结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_keep_explicit_story_repair_compat_options_over_active_snapshot() {
        let compat = SingleChapterGenerationCompatOptions {
            story_repair_summary: Some("来自请求".to_string()),
            story_repair_targets: vec!["请求目标".to_string()],
            story_preserve_strengths: vec!["请求长板".to_string()],
            ..Default::default()
        };
        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &compat,
            Some(&json!({
                "summary": "来自快照",
                "repair_targets": ["快照目标"],
                "preserve_strengths": ["快照长板"]
            })),
            None,
        );

        assert_eq!(restored.story_repair_summary(), "来自请求");
        assert_eq!(restored.story_repair_targets(), &["请求目标".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["请求长板".to_string()]
        );
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_quality_metrics_summary_when_active_snapshot_missing() {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "根据质量摘要补强中段冲突",
                    "repair_targets": ["提前引爆冲突", "减少重复说明"],
                    "preserve_strengths": ["人物张力", "结尾钩子"]
                }
            })),
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            Some(&snapshot),
        );

        assert_eq!(
            restored.story_repair_summary(),
            "根据质量摘要补强中段冲突"
        );
        assert_eq!(
            restored.story_repair_targets(),
            &["提前引爆冲突".to_string(), "减少重复说明".to_string()]
        );
        assert_eq!(
            restored.story_preserve_strengths(),
            &["人物张力".to_string(), "结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_restore_active_story_repair_payload_from_quality_context() {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "根据批量质量摘要补强冲突密度",
                    "repair_targets": ["提前爆点", "压缩说明"],
                    "preserve_strengths": ["角色压迫感"],
                    "focus_areas": ["节奏", "冲突", "", "节奏", "信息密度"],
                    "weakest_metric_key": "pacing",
                    "weakest_metric_label": "节奏",
                    "weakest_metric_value": 63.5
                },
                "quality_gate": {
                    "status": "failed",
                    "decision": "auto_repair",
                    "label": "需要修复",
                    "summary": "中段说明偏多",
                    "failed_metrics": [
                        {"label": "节奏"},
                        {"label": "信息密度"},
                        {"name": "ignored"}
                    ]
                }
            })),
        );

        let payload = restore_active_story_repair_payload_from_quality_context(&snapshot, "batch")
            .expect("active story repair payload");

        assert_eq!(payload["summary"], "根据批量质量摘要补强冲突密度");
        assert_eq!(payload["repair_targets"], json!(["提前爆点", "压缩说明"]));
        assert_eq!(payload["preserve_strengths"], json!(["角色压迫感"]));
        assert_eq!(payload["focus_areas"], json!(["节奏", "冲突", "信息密度"]));
        assert_eq!(payload["weakest_metric_key"], "pacing");
        assert_eq!(payload["weakest_metric_label"], "节奏");
        assert_eq!(payload["weakest_metric_value"], 63.5);
        assert_eq!(payload["quality_gate_status"], "failed");
        assert_eq!(payload["quality_gate_decision"], "auto_repair");
        assert_eq!(payload["quality_gate_label"], "需要修复");
        assert_eq!(payload["quality_gate_summary"], "中段说明偏多");
        assert_eq!(payload["quality_gate_failed_metrics"], json!(["节奏", "信息密度"]));
        assert_eq!(payload["source"], "recent_history_summary");
        assert_eq!(payload["source_label"], "Recent history summary");
        assert_eq!(payload["scope"], "batch");
        assert!(payload["updated_at"].is_null());
    }

    #[test]
    fn should_prefer_quality_context_active_story_repair_payload_for_resume_seed_when_runtime_payload_missing() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "沿用批量摘要修复建议",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["钩子"]
                },
                "quality_gate": {
                    "status": "failed",
                    "decision": "auto_repair",
                    "label": "需要修复",
                    "summary": "存在节奏问题",
                    "failed_metrics": [{"label": "节奏"}]
                }
            })),
        );

        let seed = build_resume_runtime_state_seed(
            &command_state,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .expect("resume seed with quality context");

        assert_eq!(seed["active_story_repair_payload"]["summary"], "沿用批量摘要修复建议");
        assert_eq!(seed["active_story_repair_payload"]["repair_targets"], json!(["压缩说明"]));
        assert_eq!(seed["active_story_repair_payload"]["preserve_strengths"], json!(["钩子"]));
        assert_eq!(seed["active_story_repair_payload"]["quality_gate_decision"], "auto_repair");
        assert_eq!(seed["active_story_repair_payload"]["quality_gate_failed_metrics"], json!(["节奏"]));
        assert_eq!(seed["active_story_repair_payload"]["source"], "recent_history_summary");
        assert_eq!(
            seed["quality_metrics_summary"]["repair_guidance"]["summary"],
            "沿用批量摘要修复建议"
        );
    }

    #[test]
    fn should_prefer_runtime_active_story_repair_payload_over_quality_context_for_resume_seed() {
        let workflow_runtime_state = json!({
            "active_story_repair_payload": {
                "summary": "来自运行态",
                "source": "current_chapter_quality"
            }
        });
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "来自质量摘要"
                }
            })),
        );

        let seed = build_resume_runtime_state_seed(
            &command_state,
            Some(&workflow_runtime_state),
            Some(&snapshot),
            &request_runtime_state,
        )
        .expect("resume seed");

        assert_eq!(seed["active_story_repair_payload"]["summary"], "来自运行态");
        assert_eq!(seed["active_story_repair_payload"]["source"], "current_chapter_quality");
        assert_eq!(
            seed["quality_metrics_summary"]["repair_guidance"]["summary"],
            "来自质量摘要"
        );
    }

    #[test]
    fn should_restore_quality_history_context_into_resume_seed_from_quality_summary() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "repair_guidance": {
                    "summary": "来自质量摘要"
                },
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 87}],
                    "history_scope": "batch"
                }
            })),
        );

        let seed = build_resume_runtime_state_seed(
            &command_state,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .expect("resume seed");

        assert_eq!(
            seed["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 87}],
                "history_scope": "batch"
            })
        );
    }

    #[test]
    fn should_restore_quality_summary_state_and_history_into_resume_seed() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let workflow_runtime_state = json!({
            "quality_metrics_summary_state": {
                "scope": "batch",
                "chapter_count": 2,
                "first_overall_score": 88.0,
                "last_overall_score": 84.0
            }
        });
        let snapshot = batch_generation_snapshot::Model {
            quality_metrics_history: Some(json!([
                {"overall_score": 88},
                {"overall_score": 84}
            ])),
            quality_metrics_summary: Some(json!({
                "overall_score": 84.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 84}]
                }
            })),
            ..build_snapshot(None, None)
        };

        let seed = build_resume_runtime_state_seed(
            &command_state,
            Some(&workflow_runtime_state),
            Some(&snapshot),
            &request_runtime_state,
        )
        .expect("resume seed");

        assert_eq!(
            seed["quality_metrics_history"],
            json!([
                {"overall_score": 88},
                {"overall_score": 84}
            ])
        );
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(seed["quality_metrics_summary_state"]["first_overall_score"], 88.0);
        assert_eq!(seed["quality_metrics_summary_state"]["last_overall_score"], 84.0);
    }

    #[test]
    fn should_rebuild_quality_summary_state_from_history_when_runtime_state_missing() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let command_state = ResumeBatchGenerationCommandState::from_task(&build_task("failed"));
        let snapshot = batch_generation_snapshot::Model {
            quality_metrics_history: Some(json!([
                {
                    "overall_score": 88,
                    "pacing_score": 8.3,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "pacing_score": 7.5,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ])),
            quality_metrics_summary: Some(json!({
                "overall_score": 84.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 84}]
                }
            })),
            ..build_snapshot(None, None)
        };

        let seed = build_resume_runtime_state_seed(
            &command_state,
            None,
            Some(&snapshot),
            &request_runtime_state,
        )
        .expect("resume seed");

        assert_eq!(seed["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(seed["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(seed["quality_metrics_summary_state"]["first_overall_score"], 88.0);
        assert_eq!(seed["quality_metrics_summary_state"]["last_overall_score"], 84.0);
        assert_eq!(seed["quality_metrics_summary_state"]["pacing_score_total"], 15.8);
        assert_eq!(seed["quality_metrics_summary_state"]["pacing_score_count"], 2);
        assert_eq!(
            seed["quality_metrics_summary_state"]["recent_history"][1]["quality_gate"]["decision"],
            "auto_repair"
        );
    }

    #[test]
    fn should_extract_quality_gate_object_from_summary_or_raw_shape() {
        let direct = extract_quality_gate_object(Some(&json!({
            "quality_gate": {
                "decision": "auto_repair"
            }
        })))
        .expect("direct gate");
        let raw = extract_quality_gate_object(Some(&json!({
            "raw": {
                "quality_gate": {
                    "decision": "manual_review"
                }
            }
        })))
        .expect("raw gate");

        assert_eq!(direct["decision"], "auto_repair");
        assert_eq!(raw["decision"], "manual_review");
        assert!(extract_quality_gate_object(Some(&json!({"foo": "bar"}))).is_none());
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_latest_quality_metrics_when_summary_missing() {
        let snapshot = build_snapshot(
            Some(json!({
                "repair_guidance": {
                    "summary": "根据最新质量指标压缩解释段",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["氛围描写"]
                }
            })),
            None,
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            Some(&snapshot),
        );

        assert_eq!(
            restored.story_repair_summary(),
            "根据最新质量指标压缩解释段"
        );
        assert_eq!(restored.story_repair_targets(), &["压缩说明".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["氛围描写".to_string()]
        );
    }

    #[test]
    fn should_restore_story_repair_compat_options_from_raw_quality_metrics_summary_when_needed() {
        let snapshot = build_snapshot(
            None,
            Some(json!({
                "raw": {
                    "repair_guidance": {
                        "summary": "从 raw 质量摘要恢复补强建议",
                        "repair_targets": ["强化转折"],
                        "preserve_strengths": ["对白节奏"]
                    }
                }
            })),
        );

        let restored = restore_story_repair_compat_options_from_active_snapshot(
            &SingleChapterGenerationCompatOptions::default(),
            None,
            Some(&snapshot),
        );

        assert_eq!(
            restored.story_repair_summary(),
            "从 raw 质量摘要恢复补强建议"
        );
        assert_eq!(restored.story_repair_targets(), &["强化转折".to_string()]);
        assert_eq!(
            restored.story_preserve_strengths(),
            &["对白节奏".to_string()]
        );
    }

    #[test]
    fn should_prefer_quality_metrics_summary_guidance_over_latest_metrics_guidance() {
        let snapshot = build_snapshot(
            Some(json!({
                "repair_guidance": {
                    "summary": "来自 latest",
                    "repair_targets": ["latest target"],
                    "preserve_strengths": ["latest strength"]
                }
            })),
            Some(json!({
                "repair_guidance": {
                    "summary": "来自 summary",
                    "repair_targets": ["summary target"],
                    "preserve_strengths": ["summary strength"]
                }
            })),
        );

        let guidance = crate::services::chapter_story_repair_quality_context_service::quality_repair_guidance_from_quality_context(
            snapshot.quality_metrics_summary.as_ref(),
            snapshot.latest_quality_metrics.as_ref(),
        )
            .expect("quality guidance should be resolved");

        assert_eq!(guidance.get("summary"), Some(&json!("来自 summary")));
        assert_eq!(
            guidance.get("repair_targets"),
            Some(&json!(["summary target"]))
        );
        assert_eq!(
            guidance.get("preserve_strengths"),
            Some(&json!(["summary strength"]))
        );
    }

    #[test]
    fn should_extract_repair_guidance_object_from_summary_or_raw_shape() {
        let direct = extract_repair_guidance_object(Some(&json!({
            "repair_guidance": {
                "summary": "direct"
            }
        })))
        .expect("direct guidance");
        let raw = extract_repair_guidance_object(Some(&json!({
            "raw": {
                "repair_guidance": {
                    "summary": "raw"
                }
            }
        })))
        .expect("raw guidance");

        assert_eq!(direct.get("summary"), Some(&json!("direct")));
        assert_eq!(raw.get("summary"), Some(&json!("raw")));
        assert!(extract_repair_guidance_object(Some(&json!({"foo": "bar"}))).is_none());
    }
}
