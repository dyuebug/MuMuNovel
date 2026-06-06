use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_generation_snapshot_query_service::load_chapter_generation_snapshot_map;
use crate::services::chapter_generation_task_recovery_service::recover_generation_task_if_needed;

use super::chapter_single_generation_prepare_service::{
    build_single_generation_task_view_payload_from_task_state,
    estimated_single_generation_task_minutes, single_generation_active_task_statuses,
};
use super::chapter_single_generation_quality_status_service::SingleGenerationQualityStatusContext;

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationExistingBackgroundTaskContext {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) workflow_runtime_state: Option<Value>,
    pub(crate) quality_status_context: SingleGenerationQualityStatusContext,
}

impl SingleGenerationExistingBackgroundTaskContext {
    fn from_task_and_snapshot(
        task: batch_generation_task::Model,
        snapshot: Option<&batch_generation_snapshot::Model>,
    ) -> Self {
        let workflow_runtime_state = snapshot.and_then(|item| item.workflow_runtime_state.clone());
        let quality_status_context =
            SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                snapshot,
                workflow_runtime_state.as_ref(),
            );

        Self {
            task,
            workflow_runtime_state,
            quality_status_context,
        }
    }
}

async fn load_active_single_generation_background_tasks(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
) -> Result<Vec<batch_generation_task::Model>, String> {
    batch_generation_task::Entity::find()
        .filter(batch_generation_task::Column::UserId.eq(user_id))
        .filter(batch_generation_task::Column::ProjectId.eq(project_id))
        .filter(
            batch_generation_task::Column::Status.is_in(single_generation_active_task_statuses()),
        )
        .order_by_desc(batch_generation_task::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn load_owned_single_generation_existing_background_task_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    project_id: &str,
    user_id: &str,
) -> Result<Option<Value>, String> {
    let tasks = load_active_single_generation_background_tasks(db, user_id, project_id).await?;
    let task_contexts =
        load_active_single_generation_existing_background_task_contexts(db, tasks).await?;

    Ok(task_contexts
        .into_iter()
        .find(|context| {
            single_generation_existing_background_task_contains_chapter(&context.task, chapter_id)
        })
        .map(into_single_generation_existing_background_task_payload))
}

pub(crate) fn into_single_generation_existing_background_task_payload(
    context: SingleGenerationExistingBackgroundTaskContext,
) -> Value {
    let SingleGenerationExistingBackgroundTaskContext {
        task,
        workflow_runtime_state,
        quality_status_context,
    } = context;

    let mut payload = build_single_generation_task_view_payload_from_task_state(
        &task,
        workflow_runtime_state.as_ref(),
    );
    quality_status_context.insert_into_payload(&mut payload);
    payload.insert("task_id".to_string(), serde_json::json!(task.id.clone()));
    payload.insert(
        "chapter_id".to_string(),
        serde_json::json!(task.current_chapter_id.clone()),
    );
    payload.insert("status".to_string(), serde_json::json!(task.status.clone()));
    payload.insert(
        "message".to_string(),
        serde_json::json!("已有后台生成任务正在执行"),
    );
    payload.insert(
        "estimated_time_minutes".to_string(),
        serde_json::json!(estimated_single_generation_task_minutes(
            task.target_word_count,
            task.enable_analysis,
        )),
    );

    Value::Object(payload)
}

fn single_generation_existing_background_task_contains_chapter(
    task: &batch_generation_task::Model,
    chapter_id: &str,
) -> bool {
    task.chapter_ids
        .as_array()
        .into_iter()
        .flatten()
        .any(|item| {
            item.as_str() == Some(chapter_id)
                || item.get("id").and_then(Value::as_str) == Some(chapter_id)
        })
}

async fn load_active_single_generation_existing_background_task_contexts(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<SingleGenerationExistingBackgroundTaskContext>, String> {
    let mut active_tasks = Vec::with_capacity(tasks.len());

    for task in tasks {
        let (task, _) = recover_generation_task_if_needed(db, task).await?;
        if !single_generation_active_task_statuses().contains(&task.status.as_str()) {
            continue;
        }

        active_tasks.push(task);
    }

    let task_ids: Vec<String> = active_tasks.iter().map(|task| task.id.clone()).collect();
    let mut snapshots_by_task_id = load_chapter_generation_snapshot_map(db, &task_ids).await?;

    Ok(active_tasks
        .into_iter()
        .map(|task| {
            let snapshot = snapshots_by_task_id.remove(&task.id);
            SingleGenerationExistingBackgroundTaskContext::from_task_and_snapshot(
                task,
                snapshot.as_ref(),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::models::{batch_generation_snapshot, batch_generation_task};

    use super::{
        into_single_generation_existing_background_task_payload,
        SingleGenerationExistingBackgroundTaskContext,
    };
    use crate::services::chapter_single_generation_quality_status_service::SingleGenerationQualityStatusContext;

    #[test]
    fn should_preserve_richer_quality_runtime_contract_on_existing_single_generation_background_payload(
    ) {
        let payload = into_single_generation_existing_background_task_payload(
            SingleGenerationExistingBackgroundTaskContext {
                task: batch_generation_task::Model {
                    id: "task-7".to_string(),
                    project_id: "project-1".to_string(),
                    user_id: "user-1".to_string(),
                    start_chapter_number: 7,
                    chapter_count: 1,
                    chapter_ids: json!(["chapter-7"]),
                    style_id: None,
                    target_word_count: 4500,
                    enable_analysis: true,
                    status: "running".to_string(),
                    total_chapters: 1,
                    completed_chapters: 0,
                    failed_chapters: json!([]),
                    current_chapter_id: Some("chapter-7".to_string()),
                    current_chapter_number: Some(7),
                    current_retry_count: 0,
                    max_retries: 3,
                    created_at: None,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                },
                workflow_runtime_state: Some(json!({
                    "progress": 42,
                    "phase": "generating"
                })),
                quality_status_context: SingleGenerationQualityStatusContext {
                    latest_quality_metrics: Some(json!({"overall_score": 84})),
                    quality_metrics_history: Some(json!([
                        {"overall_score": 80},
                        {"overall_score": 84}
                    ])),
                    quality_metrics_summary_state: Some(json!({
                        "scope": "chapter",
                        "chapter_count": 2
                    })),
                    quality_metrics_summary: Some(json!({
                        "overall_score": 84,
                        "chapter_count": 2
                    })),
                    quality_history_context: Some(json!({
                        "scope": "chapter",
                        "recent_metrics": [{"overall_score": 84}]
                    })),
                    active_story_repair_payload: Some(json!({
                        "summary": "沿用修复建议",
                        "scope": "chapter"
                    })),
                },
            },
        );

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["message"], "已有后台生成任务正在执行");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 80);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_history_context"]["scope"], "chapter");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
    }

    #[test]
    fn should_keep_single_generation_existing_background_payload_read_context_owner_contract() {
        let payload = into_single_generation_existing_background_task_payload(
            SingleGenerationExistingBackgroundTaskContext {
                task: batch_generation_task::Model {
                    id: "task-8".to_string(),
                    project_id: "project-1".to_string(),
                    user_id: "user-1".to_string(),
                    start_chapter_number: 8,
                    chapter_count: 1,
                    chapter_ids: json!(["chapter-8"]),
                    style_id: None,
                    target_word_count: 3000,
                    enable_analysis: false,
                    status: "pending".to_string(),
                    total_chapters: 1,
                    completed_chapters: 0,
                    failed_chapters: json!([]),
                    current_chapter_id: Some("chapter-8".to_string()),
                    current_chapter_number: Some(8),
                    current_retry_count: 0,
                    max_retries: 3,
                    created_at: None,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                },
                workflow_runtime_state: Some(json!({
                    "progress": 15,
                    "phase": "queued"
                })),
                quality_status_context: SingleGenerationQualityStatusContext::default(),
            },
        );

        assert_eq!(payload["task_id"], "task-8");
        assert_eq!(payload["chapter_id"], "chapter-8");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["checkpoint"]["progress"], 15);
    }

    #[test]
    fn should_build_single_generation_existing_background_task_payload_from_single_owner() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        };
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = into_single_generation_existing_background_task_payload(
            SingleGenerationExistingBackgroundTaskContext {
                task: batch_generation_task::Model {
                    id: "task-1".to_string(),
                    project_id: "project-1".to_string(),
                    user_id: "user-1".to_string(),
                    start_chapter_number: 1,
                    chapter_count: 2,
                    chapter_ids: json!(["chapter-1", "chapter-2"]),
                    style_id: None,
                    target_word_count: 3000,
                    enable_analysis: false,
                    status: "running".to_string(),
                    total_chapters: 2,
                    completed_chapters: 1,
                    failed_chapters: json!([]),
                    current_chapter_id: Some("chapter-2".to_string()),
                    current_chapter_number: Some(2),
                    current_retry_count: 0,
                    max_retries: 3,
                    created_at: None,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                },
                workflow_runtime_state: Some(workflow_runtime_state),
                quality_status_context,
            },
        );

        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["chapter_id"], "chapter-2");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["message"], "已有后台生成任务正在执行");
        assert_eq!(payload["estimated_time_minutes"], 2);
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }
}
