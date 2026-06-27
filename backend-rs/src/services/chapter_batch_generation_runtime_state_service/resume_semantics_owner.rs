use chrono::NaiveDateTime;
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    batch_generation_task_kind, BatchGenerationTaskKind,
};

use super::build_batch_generation_runtime_checkpoint_for_stage;
use super::BatchGenerationSnapshotStage;

pub(crate) fn build_batch_generation_resume_semantics_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::resume_semantics_projection",
        "scope": "resume_command_state_runtime_position_reset_projection_and_execution_selection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_semantics_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_restore_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "resume_state_entrypoints": [
                "ResumeBatchGenerationCommandState::from_task",
                "ResumeBatchGenerationCommandState::task_kind",
                "ResumeBatchGenerationCommandState::resolve_runtime_semantics",
                "ResumeBatchGenerationCommandState::resolve_reset_semantics",
                "ResumeBatchGenerationCommandState::resolve_execution_selection"
            ],
            "resume_projection_types": [
                "ResumeRuntimeSemantics",
                "ResumeResetSemantics",
                "ResumeExecutionSelection",
                "ResolveResumeExecutionSelectionError"
            ],
            "checkpoint_entrypoints": [
                "ResumeResetSemantics::build_resume_checkpoint",
                "ResumeResetSemantics::build_resume_checkpoint_with_seed"
            ],
            "batch_selection_rules": [
                "single_chapter task resumes from current_chapter_id",
                "batch task resumes from current_chapter_id if present",
                "batch task falls back to completed_chapters index when current chapter is missing",
                "malformed batch chapter_ids yields no resumable chapters",
                "resume index beyond remaining chapter_ids yields no chapters left to resume"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::resume_restore_owner",
            "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_resume_semantics_owner_is_rust_only_and_surviving_resume_route_surfaces_are_tracked_by_external_command_contracts",
            "runtime_state_keys": [
                "current_chapter_id",
                "current_chapter_number",
                "completed_chapters",
                "failed_chapters",
                "current_retry_count",
                "chapter_ids",
                "chapter_count"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_resume_route_smoke"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResumeBatchGenerationCommandState {
    pub(crate) batch_id: String,
    pub(crate) project_id: String,
    pub(crate) status: String,
    pub(crate) chapter_count: i32,
    pub(crate) chapter_ids: Value,
    pub(crate) target_word_count: i32,
    pub(crate) total_chapters: i32,
    pub(crate) completed_chapters: i32,
    pub(crate) failed_chapters: Value,
    pub(crate) current_retry_count: i32,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) max_retries: i32,
    pub(crate) created_at: Option<NaiveDateTime>,
}

impl ResumeBatchGenerationCommandState {
    pub(crate) fn from_task(task: &batch_generation_task::Model) -> Self {
        Self {
            batch_id: task.id.clone(),
            project_id: task.project_id.clone(),
            status: task.status.clone(),
            chapter_count: task.chapter_count,
            chapter_ids: task.chapter_ids.clone(),
            target_word_count: task.target_word_count,
            total_chapters: task.total_chapters,
            completed_chapters: task.completed_chapters,
            failed_chapters: task.failed_chapters.clone(),
            current_retry_count: task.current_retry_count,
            current_chapter_id: task.current_chapter_id.clone(),
            current_chapter_number: task.current_chapter_number,
            max_retries: task.max_retries,
            created_at: task.created_at,
        }
    }

    pub(crate) fn task_kind(&self) -> BatchGenerationTaskKind {
        batch_generation_task_kind(self.chapter_count, &self.chapter_ids)
    }

    pub(crate) fn resolve_runtime_semantics(&self) -> ResumeRuntimeSemantics {
        match self.task_kind() {
            BatchGenerationTaskKind::SingleChapter => ResumeRuntimeSemantics {
                current_chapter_id: self.current_chapter_id.clone(),
                current_chapter_number: self.current_chapter_number,
                include_progress_totals: false,
            },
            BatchGenerationTaskKind::Batch => ResumeRuntimeSemantics {
                current_chapter_id: None,
                current_chapter_number: None,
                include_progress_totals: true,
            },
        }
    }

    pub(crate) fn resolve_reset_semantics(&self) -> ResumeResetSemantics {
        let runtime = self.resolve_runtime_semantics();
        ResumeResetSemantics {
            status: "pending",
            current_chapter_id: runtime.current_chapter_id,
            current_chapter_number: runtime.current_chapter_number,
            include_progress_totals: runtime.include_progress_totals,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
        }
    }

    pub(crate) fn resolve_execution_selection(
        &self,
    ) -> Result<ResumeExecutionSelection, ResolveResumeExecutionSelectionError> {
        match self.task_kind() {
            BatchGenerationTaskKind::SingleChapter => self
                .current_chapter_id
                .clone()
                .map(|chapter_id| ResumeExecutionSelection::SingleChapter { chapter_id })
                .ok_or(ResolveResumeExecutionSelectionError::NoResumableChaptersFound),
            BatchGenerationTaskKind::Batch => self
                .resolve_remaining_batch_chapter_ids()
                .map(|chapter_ids| ResumeExecutionSelection::Batch { chapter_ids }),
        }
    }

    fn resolve_remaining_batch_chapter_ids(
        &self,
    ) -> Result<Vec<String>, ResolveResumeExecutionSelectionError> {
        let chapter_ids = self.parse_batch_chapter_ids();
        if chapter_ids.is_empty() {
            return Err(ResolveResumeExecutionSelectionError::NoResumableChaptersFound);
        }

        let resume_start_index = self
            .current_chapter_id
            .as_deref()
            .and_then(|current_chapter_id| {
                chapter_ids
                    .iter()
                    .position(|chapter_id| chapter_id == current_chapter_id)
            })
            .unwrap_or_else(|| self.completed_chapters.max(0) as usize);

        if resume_start_index >= chapter_ids.len() {
            return Err(ResolveResumeExecutionSelectionError::NoChaptersLeftToResume);
        }

        Ok(chapter_ids[resume_start_index..].to_vec())
    }

    fn parse_batch_chapter_ids(&self) -> Vec<String> {
        self.chapter_ids
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| {
                item.as_str().map(str::to_string).or_else(|| {
                    item.get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResumeRuntimeSemantics {
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) include_progress_totals: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResumeResetSemantics {
    pub(crate) status: &'static str,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) include_progress_totals: bool,
    pub(crate) completed_chapters: i32,
    pub(crate) failed_chapters: Value,
    pub(crate) current_retry_count: i32,
}

impl ResumeResetSemantics {
    pub(crate) fn build_resume_checkpoint(&self, total_chapters: i32) -> Value {
        build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Resumed {
                include_progress_totals: self.include_progress_totals,
            },
            self.current_chapter_id.as_deref(),
            self.current_chapter_number,
            self.completed_chapters,
            total_chapters,
        )
    }

    pub(crate) fn build_resume_checkpoint_with_seed(
        &self,
        total_chapters: i32,
        runtime_state_seed: Option<Value>,
    ) -> Value {
        let mut checkpoint = self.build_resume_checkpoint(total_chapters);
        if let (Some(checkpoint_object), Some(Value::Object(seed_object))) =
            (checkpoint.as_object_mut(), runtime_state_seed)
        {
            checkpoint_object.extend(seed_object);
        }
        checkpoint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResumeExecutionSelection {
    SingleChapter { chapter_id: String },
    Batch { chapter_ids: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveResumeExecutionSelectionError {
    NoResumableChaptersFound,
    NoChaptersLeftToResume,
}

#[cfg(test)]
mod resume_semantics_contract_tests {
    use serde_json::json;

    use super::{
        ResolveResumeExecutionSelectionError, ResumeBatchGenerationCommandState,
        ResumeExecutionSelection, ResumeResetSemantics,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationTaskKind;

    fn task(
        chapter_count: i32,
        chapter_ids: serde_json::Value,
        current_chapter_id: Option<&str>,
        current_chapter_number: Option<i32>,
    ) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count,
            chapter_ids,
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "failed".to_string(),
            total_chapters: chapter_count,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: current_chapter_id.map(ToString::to_string),
            current_chapter_number,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_resolve_resume_runtime_position_for_single_task_only() {
        let single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!(["chapter-1"]),
            Some("chapter-1"),
            Some(1),
        ));
        let batch = ResumeBatchGenerationCommandState::from_task(&task(
            2,
            json!(["chapter-1", "chapter-2"]),
            Some("chapter-1"),
            Some(1),
        ));
        let malformed_single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!({"chapter_id": "chapter-1"}),
            Some("chapter-1"),
            Some(1),
        ));

        let single_semantics = single.resolve_runtime_semantics();
        assert_eq!(
            (
                single_semantics.current_chapter_id,
                single_semantics.current_chapter_number,
            ),
            (Some("chapter-1".to_string()), Some(1))
        );

        let batch_semantics = batch.resolve_runtime_semantics();
        assert_eq!(
            (
                batch_semantics.current_chapter_id,
                batch_semantics.current_chapter_number,
            ),
            (None, None)
        );

        let malformed_semantics = malformed_single.resolve_runtime_semantics();
        assert_eq!(
            (
                malformed_semantics.current_chapter_id,
                malformed_semantics.current_chapter_number,
            ),
            (None, None)
        );
    }

    #[test]
    fn should_resolve_resume_runtime_semantics_for_single_batch_and_malformed_single_tasks() {
        let single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!(["chapter-1"]),
            Some("chapter-1"),
            Some(1),
        ));
        let batch = ResumeBatchGenerationCommandState::from_task(&task(
            2,
            json!(["chapter-1", "chapter-2"]),
            Some("chapter-1"),
            Some(1),
        ));
        let malformed_single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!({"chapter_id": "chapter-1"}),
            Some("chapter-1"),
            Some(1),
        ));

        let single_semantics = single.resolve_runtime_semantics();
        assert_eq!(
            single_semantics.current_chapter_id.as_deref(),
            Some("chapter-1")
        );
        assert_eq!(single_semantics.current_chapter_number, Some(1));
        assert!(!single_semantics.include_progress_totals);

        let batch_semantics = batch.resolve_runtime_semantics();
        assert!(batch_semantics.current_chapter_id.is_none());
        assert!(batch_semantics.current_chapter_number.is_none());
        assert!(batch_semantics.include_progress_totals);

        let malformed_semantics = malformed_single.resolve_runtime_semantics();
        assert!(malformed_semantics.current_chapter_id.is_none());
        assert!(malformed_semantics.current_chapter_number.is_none());
        assert!(malformed_semantics.include_progress_totals);
    }

    #[test]
    fn should_resolve_resume_reset_semantics_for_single_batch_and_malformed_single_tasks() {
        let single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!(["chapter-1"]),
            Some("chapter-1"),
            Some(1),
        ));
        let batch = ResumeBatchGenerationCommandState::from_task(&task(
            2,
            json!(["chapter-1", "chapter-2"]),
            Some("chapter-1"),
            Some(1),
        ));
        let malformed_single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!({"chapter_id": "chapter-1"}),
            Some("chapter-1"),
            Some(1),
        ));

        let single_reset = single.resolve_reset_semantics();
        assert_eq!(single_reset.status, "pending");
        assert_eq!(
            single_reset.current_chapter_id.as_deref(),
            Some("chapter-1")
        );
        assert_eq!(single_reset.current_chapter_number, Some(1));
        assert!(!single_reset.include_progress_totals);
        assert_eq!(single_reset.completed_chapters, 0);
        assert_eq!(single_reset.failed_chapters, json!([]));
        assert_eq!(single_reset.current_retry_count, 0);

        let batch_reset = batch.resolve_reset_semantics();
        assert_eq!(batch_reset.status, "pending");
        assert!(batch_reset.current_chapter_id.is_none());
        assert!(batch_reset.current_chapter_number.is_none());
        assert!(batch_reset.include_progress_totals);
        assert_eq!(batch_reset.completed_chapters, 0);
        assert_eq!(batch_reset.failed_chapters, json!([]));
        assert_eq!(batch_reset.current_retry_count, 0);

        let malformed_reset = malformed_single.resolve_reset_semantics();
        assert_eq!(malformed_reset.status, "pending");
        assert!(malformed_reset.current_chapter_id.is_none());
        assert!(malformed_reset.current_chapter_number.is_none());
        assert!(malformed_reset.include_progress_totals);
        assert_eq!(malformed_reset.completed_chapters, 0);
        assert_eq!(malformed_reset.failed_chapters, json!([]));
        assert_eq!(malformed_reset.current_retry_count, 0);
    }

    #[test]
    fn should_build_resume_checkpoint_from_reset_semantics() {
        let single_checkpoint = ResumeResetSemantics {
            status: "pending",
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(3),
            include_progress_totals: false,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
        }
        .build_resume_checkpoint(5);
        assert_eq!(single_checkpoint["phase"], "pending");
        assert_eq!(single_checkpoint["status"], "pending");
        assert_eq!(single_checkpoint["last_event"], "resume");
        assert_eq!(
            single_checkpoint["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert_eq!(single_checkpoint["chapter_id"], "chapter-1");
        assert_eq!(single_checkpoint["current_chapter_id"], "chapter-1");
        assert_eq!(single_checkpoint["current_chapter_number"], 3);
        assert!(single_checkpoint.get("completed").is_none());
        assert!(single_checkpoint.get("total").is_none());

        let batch_checkpoint = ResumeResetSemantics {
            status: "pending",
            current_chapter_id: None,
            current_chapter_number: None,
            include_progress_totals: true,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
        }
        .build_resume_checkpoint(4);
        assert_eq!(batch_checkpoint["phase"], "pending");
        assert_eq!(batch_checkpoint["status"], "pending");
        assert_eq!(batch_checkpoint["last_event"], "resume");
        assert_eq!(batch_checkpoint["completed"], 0);
        assert_eq!(batch_checkpoint["total"], 4);
        assert!(batch_checkpoint["chapter_id"].is_null());
        assert!(batch_checkpoint["current_chapter_id"].is_null());
        assert!(batch_checkpoint["current_chapter_number"].is_null());
    }

    #[test]
    fn should_build_resume_checkpoint_with_seed_from_reset_semantics_owner() {
        let checkpoint = ResumeResetSemantics {
            status: "pending",
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            include_progress_totals: false,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_retry_count: 0,
        }
        .build_resume_checkpoint_with_seed(
            5,
            Some(json!({
                "resume_from_batch_id": "task-1",
                "current_retry_count": 0,
                "max_retries": 3
            })),
        );

        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "resume");
        assert_eq!(checkpoint["current_chapter_id"], "chapter-2");
        assert_eq!(checkpoint["current_chapter_number"], 2);
        assert_eq!(checkpoint["resume_from_batch_id"], "task-1");
        assert_eq!(checkpoint["current_retry_count"], 0);
        assert_eq!(checkpoint["max_retries"], 3);
    }

    #[test]
    fn should_resolve_resume_execution_selection_for_resumable_targets_only() {
        let single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!(["chapter-1"]),
            Some("chapter-1"),
            Some(1),
        ));
        let batch = ResumeBatchGenerationCommandState::from_task(&task(
            2,
            json!(["chapter-1", {"id": "chapter-2"}, {"name": "ignored"}]),
            Some("chapter-1"),
            Some(1),
        ));
        let malformed_single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!({"chapter_id": "chapter-1"}),
            Some("chapter-1"),
            Some(1),
        ));

        assert_eq!(
            single.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::SingleChapter {
                chapter_id: "chapter-1".to_string(),
            })
        );
        assert_eq!(
            batch.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
            })
        );
        assert_eq!(
            malformed_single.resolve_execution_selection(),
            Err(ResolveResumeExecutionSelectionError::NoResumableChaptersFound)
        );
    }

    #[test]
    fn should_resume_batch_execution_from_current_chapter_position() {
        let mut task_model = task(
            4,
            json!(["chapter-1", "chapter-2", "chapter-3", "chapter-4"]),
            Some("chapter-3"),
            Some(3),
        );
        task_model.completed_chapters = 2;

        let batch = ResumeBatchGenerationCommandState::from_task(&task_model);

        assert_eq!(
            batch.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-3".to_string(), "chapter-4".to_string()],
            })
        );
    }

    #[test]
    fn should_resume_batch_execution_from_completed_chapter_count_when_position_missing() {
        let mut task_model = task(
            4,
            json!(["chapter-1", "chapter-2", "chapter-3", "chapter-4"]),
            None,
            None,
        );
        task_model.completed_chapters = 2;

        let batch = ResumeBatchGenerationCommandState::from_task(&task_model);

        assert_eq!(
            batch.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-3".to_string(), "chapter-4".to_string()],
            })
        );
    }

    #[test]
    fn should_fail_resume_execution_selection_when_no_batch_chapters_left() {
        let mut task_model = task(2, json!(["chapter-1", "chapter-2"]), None, None);
        task_model.completed_chapters = 2;

        let batch = ResumeBatchGenerationCommandState::from_task(&task_model);

        assert_eq!(
            batch.resolve_execution_selection(),
            Err(ResolveResumeExecutionSelectionError::NoChaptersLeftToResume)
        );
    }

    #[test]
    fn should_resolve_shared_resume_task_boundaries_for_single_and_batch_tasks() {
        let single = ResumeBatchGenerationCommandState::from_task(&task(
            1,
            json!(["chapter-1"]),
            Some("chapter-1"),
            Some(1),
        ));
        let batch = ResumeBatchGenerationCommandState::from_task(&task(
            2,
            json!(["chapter-1", {"id": "chapter-2"}]),
            Some("chapter-1"),
            Some(1),
        ));

        assert_eq!(
            single.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::SingleChapter {
                chapter_id: "chapter-1".to_string(),
            })
        );
        assert_eq!(
            single.resolve_runtime_semantics(),
            super::ResumeRuntimeSemantics {
                current_chapter_id: Some("chapter-1".to_string()),
                current_chapter_number: Some(1),
                include_progress_totals: false,
            }
        );

        assert_eq!(
            batch.resolve_execution_selection(),
            Ok(ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
            })
        );
        assert_eq!(
            batch.resolve_runtime_semantics(),
            super::ResumeRuntimeSemantics {
                current_chapter_id: None,
                current_chapter_number: None,
                include_progress_totals: true,
            }
        );
    }

    #[test]
    fn should_build_resume_command_state_from_task_projection() {
        let mut task = task(
            2,
            json!(["chapter-1", "chapter-2"]),
            Some("chapter-2"),
            Some(2),
        );
        task.id = "task-9".to_string();
        task.project_id = "project-9".to_string();
        task.total_chapters = 2;
        task.completed_chapters = 1;

        let state = ResumeBatchGenerationCommandState::from_task(&task);

        assert_eq!(state.batch_id, "task-9");
        assert_eq!(state.project_id, "project-9");
        assert_eq!(state.completed_chapters, 1);
        assert_eq!(state.total_chapters, 2);
        assert_eq!(state.task_kind(), BatchGenerationTaskKind::Batch);
    }
}
