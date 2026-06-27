use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::models::plot_analysis;
use crate::services::chapter_analysis_runtime_service::{
    analyze_generated_chapter_follow_up, prepare_chapter_analysis_execution,
};
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

use super::{
    build_current_chapter_latest_quality_metrics_from_plot_analysis,
    build_current_chapter_quality_summary_from_plot_analysis, load_chapter_generation_snapshot,
    upsert_batch_generation_runtime_snapshot, BatchGenerationPersistedRuntimeContext,
    BatchGenerationRuntimeSession,
};

pub(crate) fn build_batch_generation_follow_up_analysis_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::follow_up_analysis_runtime_projection",
        "scope": "follow_up_analysis_attempt_started_completed_retry_and_stop_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/follow_up_analysis_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs"
        ],
        "behavior_contract": {
            "attempt_entrypoints": [
                "BatchGenerationFollowUpAnalysisPlan::from_generated_result",
                "BatchGenerationFollowUpAnalysisPlan::execute",
                "BatchGenerationAnalysisAttemptPlan::from_generated_result",
                "BatchGenerationAnalysisAttemptPlan::execute",
                "BatchGenerationAnalysisAttemptPlan::resolve_result"
            ],
            "persistence_entrypoints": [
                "BatchGenerationAnalysisStartedPersistencePlan::from_generated_result",
                "BatchGenerationAnalysisStartedPersistencePlan::persist",
                "BatchGenerationAnalysisCompletionPersistencePlan::from_generated_result",
                "BatchGenerationAnalysisCompletionPersistencePlan::persist"
            ],
            "routing_entrypoints": [
                "BatchGenerationAnalysisRoutingPlan::from_analysis_error_message",
                "BatchGenerationAnalysisRoutingPlan::persist_and_resolve",
                "should_stop_batch_generation_analysis_without_retry",
                "format_analysis_error_message"
            ],
            "analysis_gateways": [
                "prepare_chapter_analysis_execution",
                "analyze_generated_chapter_follow_up"
            ],
            "analysis_runtime_fields": [
                "analysis_task_message",
                "analysis_task_progress",
                "analysis_last_error",
                "analysis_retry_count",
                "analysis_max_retries",
                "last_event",
                "last_message",
                "phase",
                "progress"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service"
        ],
        "analysis_runtime_owner_contract": crate::services::chapter_analysis_runtime_service::build_chapter_analysis_runtime_owner_contract(),
        "snapshot_persistence_owner_contract": crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract(),
        "quality_runtime_owner_contract": crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_follow_up_analysis_owner_is_rust_only_and_surviving_analysis_runtime_surfaces_are_tracked_by_external_analysis_contracts",
            "runtime_state_keys": [
                "analysis_task_message",
                "analysis_task_progress",
                "analysis_last_error",
                "analysis_retry_count",
                "analysis_max_retries"
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationAnalysisCompletionPersistencePlan {
    pub(crate) current_quality_snapshot: Option<Value>,
    pub(crate) completed_snapshot: Value,
}

impl BatchGenerationAnalysisCompletionPersistencePlan {
    async fn build_current_quality_snapshot(
        db: &DatabaseConnection,
        batch_task_id: &str,
        chapter_id: &str,
    ) -> Option<Value> {
        let latest_analysis = plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
            .order_by_desc(plot_analysis::Column::CreatedAt)
            .one(db)
            .await
            .ok()
            .flatten()?;
        let quality_summary =
            build_current_chapter_quality_summary_from_plot_analysis(&latest_analysis)?;
        let latest_quality_metrics =
            build_current_chapter_latest_quality_metrics_from_plot_analysis(&latest_analysis);
        let persisted_runtime_context = load_chapter_generation_snapshot(db, batch_task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default();

        Some(
            persisted_runtime_context.build_current_chapter_quality_runtime_snapshot(
                &quality_summary,
                latest_quality_metrics.as_ref(),
            ),
        )
    }

    pub(crate) async fn from_generated_result(
        db: &DatabaseConnection,
        batch_task_id: &str,
        generated: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            current_quality_snapshot: Self::build_current_quality_snapshot(
                db,
                batch_task_id,
                &generated.chapter_id,
            )
            .await,
            completed_snapshot: build_batch_generation_analysis_completed_snapshot(
                generated,
                analysis_retry_count,
            ),
        }
    }

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
    ) -> Option<Value> {
        if let Some(current_quality_snapshot) = self.current_quality_snapshot.as_ref() {
            let _ = upsert_batch_generation_runtime_snapshot(
                db,
                batch_task_id,
                current_quality_snapshot.clone(),
            )
            .await;
        }
        let _ =
            upsert_batch_generation_runtime_snapshot(db, batch_task_id, self.completed_snapshot)
                .await;

        self.current_quality_snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationAnalysisStartedPersistencePlan {
    pub(crate) started_snapshot: Value,
}

impl BatchGenerationAnalysisStartedPersistencePlan {
    pub(crate) fn from_generated_result(
        analysis_task_id: Option<&str>,
        generated: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            started_snapshot: build_batch_generation_analysis_started_snapshot(
                analysis_task_id,
                generated,
                analysis_retry_count,
            ),
        }
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection, batch_task_id: &str) {
        let _ = upsert_batch_generation_runtime_snapshot(db, batch_task_id, self.started_snapshot)
            .await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationAnalysisRoutingPlan {
    Retry {
        retry_snapshot: Value,
        next_retry_count: i32,
        wait_seconds: u64,
    },
    Stop {
        error_message: String,
    },
}

impl BatchGenerationAnalysisRoutingPlan {
    pub(crate) fn from_analysis_error_message(
        chapter_number: i32,
        error_message: String,
        analysis_retry_count: i32,
    ) -> Self {
        if should_stop_batch_generation_analysis_without_retry(&error_message) {
            return Self::Stop { error_message };
        }

        if analysis_retry_count < 2 {
            let next_retry_count = analysis_retry_count + 1;
            let wait_seconds = 2_i32.pow(next_retry_count as u32).min(10) as u64;
            return Self::Retry {
                retry_snapshot: json!({
                    "last_event": "analysis_retry",
                    "last_message": format!("第 {} 章分析失败，准备重试", chapter_number),
                    "progress": 85,
                    "phase": "parsing",
                    "analysis_task_message": format!("第 {} 章分析失败，准备重试", chapter_number),
                    "analysis_task_progress": 85,
                    "analysis_last_error": error_message,
                    "analysis_retry_count": next_retry_count,
                    "analysis_max_retries": 3,
                }),
                next_retry_count,
                wait_seconds,
            };
        }

        Self::Stop { error_message }
    }

    pub(crate) async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
    ) -> Result<(), String> {
        match self {
            BatchGenerationAnalysisRoutingPlan::Retry {
                retry_snapshot,
                wait_seconds,
                ..
            } => {
                let _ = upsert_batch_generation_runtime_snapshot(db, batch_task_id, retry_snapshot)
                    .await;
                sleep(Duration::from_secs(wait_seconds)).await;
                Ok(())
            }
            BatchGenerationAnalysisRoutingPlan::Stop { error_message } => Err(error_message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationAnalysisAttemptResolution {
    Completed(Option<Value>),
    Retry,
}

pub(crate) fn should_stop_batch_generation_analysis_without_retry(error_message: &str) -> bool {
    matches!(
        error_message,
        "章节不存在或内容为空"
            | "章节或项目已删除，无法继续分析"
            | "Chapter or project was deleted before analysis"
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationAnalysisAttemptPlan {
    pub(crate) generated_result: GeneratedChapterResult,
    pub(crate) analysis_retry_count: i32,
}

impl BatchGenerationAnalysisAttemptPlan {
    pub(crate) fn from_generated_result(
        generated_result: &GeneratedChapterResult,
        analysis_retry_count: i32,
    ) -> Self {
        Self {
            generated_result: generated_result.clone(),
            analysis_retry_count,
        }
    }

    async fn persist_started(
        &self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        analysis_task_id: Option<&str>,
    ) {
        BatchGenerationAnalysisStartedPersistencePlan::from_generated_result(
            analysis_task_id,
            &self.generated_result,
            self.analysis_retry_count,
        )
        .persist(db, batch_task_id)
        .await;
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        session: &BatchGenerationRuntimeSession,
    ) -> Result<BatchGenerationAnalysisAttemptResolution, String> {
        let prepared_analysis = prepare_chapter_analysis_execution(
            db,
            &self.generated_result.chapter_id,
            &session.user_id,
        )
        .await
        .ok();
        let analysis_task_id = prepared_analysis.as_ref().map(|item| item.task_id());
        self.persist_started(db, batch_task_id, analysis_task_id)
            .await;

        let generated_result = self.generated_result;
        let analysis_retry_count = self.analysis_retry_count;

        let result = if let Some(prepared_analysis) = prepared_analysis {
            prepared_analysis.execute(db, &session.user_id).await
        } else {
            analyze_generated_chapter_follow_up(db, &session.user_id, &generated_result)
                .await
                .map_err(|error| format_analysis_error_message(&error))
        };

        Self::resolve_result(
            db,
            batch_task_id,
            &generated_result,
            analysis_retry_count,
            result,
        )
        .await
    }

    pub(crate) async fn resolve_result(
        db: &DatabaseConnection,
        batch_task_id: &str,
        generated_result: &GeneratedChapterResult,
        analysis_retry_count: i32,
        result: Result<Value, String>,
    ) -> Result<BatchGenerationAnalysisAttemptResolution, String> {
        match result {
            Ok(_) => {
                let completion_plan =
                    BatchGenerationAnalysisCompletionPersistencePlan::from_generated_result(
                        db,
                        batch_task_id,
                        generated_result,
                        analysis_retry_count,
                    )
                    .await;
                Ok(BatchGenerationAnalysisAttemptResolution::Completed(
                    completion_plan.persist(db, batch_task_id).await,
                ))
            }
            Err(error_message) => {
                match BatchGenerationAnalysisRoutingPlan::from_analysis_error_message(
                    generated_result.chapter_number,
                    error_message,
                    analysis_retry_count,
                )
                .persist_and_resolve(db, batch_task_id)
                .await
                {
                    Ok(()) => Ok(BatchGenerationAnalysisAttemptResolution::Retry),
                    Err(error_message) => Err(error_message),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationFollowUpAnalysisPlan {
    pub(crate) generated_result: GeneratedChapterResult,
}

impl BatchGenerationFollowUpAnalysisPlan {
    pub(crate) fn from_generated_result(generated_result: &GeneratedChapterResult) -> Self {
        Self {
            generated_result: generated_result.clone(),
        }
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        batch_task_id: &str,
        session: &BatchGenerationRuntimeSession,
    ) -> Result<Option<Value>, String> {
        if !session.compat_options.enable_analysis() {
            return Ok(None);
        }

        for analysis_retry_count in 0..3 {
            match BatchGenerationAnalysisAttemptPlan::from_generated_result(
                &self.generated_result,
                analysis_retry_count,
            )
            .execute(db, batch_task_id, session)
            .await?
            {
                BatchGenerationAnalysisAttemptResolution::Completed(current_quality_snapshot) => {
                    return Ok(current_quality_snapshot);
                }
                BatchGenerationAnalysisAttemptResolution::Retry => {
                    continue;
                }
            }
        }

        Err("章节分析失败".to_string())
    }
}

pub(crate) fn build_batch_generation_analysis_completed_snapshot(
    generated: &GeneratedChapterResult,
    analysis_retry_count: i32,
) -> Value {
    json!({
        "last_event": "analysis_completed",
        "last_message": format!("第 {} 章分析完成", generated.chapter_number),
        "progress": 100,
        "analysis_task_message": format!("第 {} 章分析完成", generated.chapter_number),
        "analysis_task_progress": 100,
        "analysis_last_error": Value::Null,
        "analysis_retry_count": analysis_retry_count,
        "analysis_max_retries": 3,
    })
}

pub(crate) fn build_batch_generation_analysis_started_snapshot(
    analysis_task_id: Option<&str>,
    generated: &GeneratedChapterResult,
    analysis_retry_count: i32,
) -> Value {
    json!({
        "last_event": "analysis_started",
        "last_message": "正在分析章节",
        "progress": 85,
        "phase": "parsing",
        "analysis_task_id": analysis_task_id,
        "analysis_task_message": format!("第 {} 章分析任务已启动", generated.chapter_number),
        "analysis_task_progress": 85,
        "analysis_started_chapter_id": generated.chapter_id,
        "analysis_started_chapter_number": generated.chapter_number,
        "analysis_started_at": chrono::Utc::now().to_rfc3339(),
        "analysis_retry_count": analysis_retry_count,
        "analysis_max_retries": 3,
    })
}

pub(crate) fn format_analysis_error_message(
    error: &crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError,
) -> String {
    match error {
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ChapterEmpty => {
            "章节不存在或内容为空".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::ProjectMissing => {
            "章节或项目已删除，无法继续分析".to_string()
        }
        crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError::Internal(message) => {
            message.clone()
        }
    }
}
