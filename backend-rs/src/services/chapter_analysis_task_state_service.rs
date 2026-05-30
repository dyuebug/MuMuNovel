use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use uuid::Uuid;

use crate::models::analysis_task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisTaskTimestampUpdate {
    Keep,
    Clear,
    Now,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnalysisTaskStage {
    Created,
    Running,
    Completed,
    Failed,
    AutoRecoveredAsFailed,
}

impl AnalysisTaskStage {
    fn resolve_mutation_plan(self, error_message: Option<String>) -> AnalysisTaskMutationPlan {
        match self {
            AnalysisTaskStage::Created => AnalysisTaskMutationPlan {
                status: "pending",
                progress: 0,
                error_message: None,
                created_at: AnalysisTaskTimestampUpdate::Now,
                started_at: AnalysisTaskTimestampUpdate::Clear,
                completed_at: AnalysisTaskTimestampUpdate::Clear,
            },
            AnalysisTaskStage::Running => AnalysisTaskMutationPlan {
                status: "running",
                progress: 10,
                error_message: None,
                created_at: AnalysisTaskTimestampUpdate::Keep,
                started_at: AnalysisTaskTimestampUpdate::Now,
                completed_at: AnalysisTaskTimestampUpdate::Keep,
            },
            AnalysisTaskStage::Completed => AnalysisTaskMutationPlan {
                status: "completed",
                progress: 100,
                error_message: None,
                created_at: AnalysisTaskTimestampUpdate::Keep,
                started_at: AnalysisTaskTimestampUpdate::Keep,
                completed_at: AnalysisTaskTimestampUpdate::Now,
            },
            AnalysisTaskStage::Failed | AnalysisTaskStage::AutoRecoveredAsFailed => {
                AnalysisTaskMutationPlan {
                    status: "failed",
                    progress: 0,
                    error_message,
                    created_at: AnalysisTaskTimestampUpdate::Keep,
                    started_at: AnalysisTaskTimestampUpdate::Keep,
                    completed_at: AnalysisTaskTimestampUpdate::Now,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnalysisTaskMutationPlan {
    status: &'static str,
    progress: i32,
    error_message: Option<String>,
    created_at: AnalysisTaskTimestampUpdate,
    started_at: AnalysisTaskTimestampUpdate,
    completed_at: AnalysisTaskTimestampUpdate,
}

impl AnalysisTaskMutationPlan {
    fn apply_to_active_model(self, active: &mut analysis_task::ActiveModel, now: NaiveDateTime) {
        active.status = Set(self.status.to_string());
        active.progress = Set(self.progress);
        active.error_message = Set(self.error_message);

        match self.created_at {
            AnalysisTaskTimestampUpdate::Keep => {}
            AnalysisTaskTimestampUpdate::Clear => active.created_at = Set(None),
            AnalysisTaskTimestampUpdate::Now => active.created_at = Set(Some(now)),
        }

        match self.started_at {
            AnalysisTaskTimestampUpdate::Keep => {}
            AnalysisTaskTimestampUpdate::Clear => active.started_at = Set(None),
            AnalysisTaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at {
            AnalysisTaskTimestampUpdate::Keep => {}
            AnalysisTaskTimestampUpdate::Clear => active.completed_at = Set(None),
            AnalysisTaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }
    }

    async fn persist_for_task(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        now: NaiveDateTime,
    ) -> Result<Option<analysis_task::Model>, sea_orm::DbErr> {
        let Some(existing) = analysis_task::Entity::find_by_id(task_id).one(db).await? else {
            return Ok(None);
        };

        let mut active: analysis_task::ActiveModel = existing.into();
        self.apply_to_active_model(&mut active, now);
        active.update(db).await.map(Some)
    }
}

pub(crate) fn build_analysis_task_active_model(
    chapter_id: &str,
    user_id: &str,
    project_id: &str,
    now: NaiveDateTime,
) -> analysis_task::ActiveModel {
    let plan = AnalysisTaskStage::Created.resolve_mutation_plan(None);
    analysis_task::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        chapter_id: Set(chapter_id.to_string()),
        user_id: Set(user_id.to_string()),
        project_id: Set(project_id.to_string()),
        status: Set(plan.status.to_string()),
        progress: Set(plan.progress),
        error_message: Set(plan.error_message),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
    }
}

pub(crate) async fn apply_analysis_task_state_by_id(
    db: &DatabaseConnection,
    task_id: &str,
    stage: AnalysisTaskStage,
    error_message: Option<String>,
    now: NaiveDateTime,
) -> Result<Option<analysis_task::Model>, sea_orm::DbErr> {
    stage
        .resolve_mutation_plan(error_message)
        .persist_for_task(db, task_id, now)
        .await
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;

    use super::{build_analysis_task_active_model, AnalysisTaskStage, AnalysisTaskTimestampUpdate};
    use crate::models::analysis_task;

    fn build_task(status: &str) -> analysis_task::Model {
        analysis_task::Model {
            id: "task-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            project_id: "project-1".to_string(),
            status: status.to_string(),
            progress: 0,
            error_message: Some("old error".to_string()),
            created_at: None,
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn should_resolve_analysis_task_mutation_plans() {
        let created = AnalysisTaskStage::Created.resolve_mutation_plan(None);
        assert_eq!(created.status, "pending");
        assert_eq!(created.progress, 0);
        assert!(matches!(
            created.created_at,
            AnalysisTaskTimestampUpdate::Now
        ));
        assert!(matches!(
            created.started_at,
            AnalysisTaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            created.completed_at,
            AnalysisTaskTimestampUpdate::Clear
        ));

        let running = AnalysisTaskStage::Running.resolve_mutation_plan(None);
        assert_eq!(running.status, "running");
        assert_eq!(running.progress, 10);
        assert!(matches!(
            running.started_at,
            AnalysisTaskTimestampUpdate::Now
        ));
        assert_eq!(running.error_message, None);

        let failed = AnalysisTaskStage::Failed.resolve_mutation_plan(Some("boom".to_string()));
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.progress, 0);
        assert!(matches!(
            failed.completed_at,
            AnalysisTaskTimestampUpdate::Now
        ));
        assert_eq!(failed.error_message.as_deref(), Some("boom"));

        let auto_recovered = AnalysisTaskStage::AutoRecoveredAsFailed
            .resolve_mutation_plan(Some("timeout".to_string()));
        assert_eq!(auto_recovered.status, "failed");
        assert_eq!(auto_recovered.progress, 0);
        assert_eq!(auto_recovered.error_message.as_deref(), Some("timeout"));
    }

    #[test]
    fn should_build_analysis_task_active_model_with_pending_defaults() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 10, 0)
            .expect("valid time");
        let active = build_analysis_task_active_model("chapter-9", "user-9", "project-9", now);

        assert_eq!(active.chapter_id, Set("chapter-9".to_string()));
        assert_eq!(active.user_id, Set("user-9".to_string()));
        assert_eq!(active.project_id, Set("project-9".to_string()));
        assert_eq!(active.status, Set("pending".to_string()));
        assert_eq!(active.progress, Set(0));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.created_at, Set(Some(now)));
        assert_eq!(active.started_at, Set(None));
        assert_eq!(active.completed_at, Set(None));
    }

    #[test]
    fn should_apply_analysis_task_state() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 12, 0)
            .expect("valid time");
        let mut active: analysis_task::ActiveModel = build_task("pending").into();

        AnalysisTaskStage::Completed
            .resolve_mutation_plan(None)
            .apply_to_active_model(&mut active, now);

        assert_eq!(active.status, Set("completed".to_string()));
        assert_eq!(active.progress, Set(100));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.completed_at, Set(Some(now)));
    }
}
