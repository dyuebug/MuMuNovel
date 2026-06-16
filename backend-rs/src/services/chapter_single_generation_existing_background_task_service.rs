use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Map, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_read_context_service::recover_generation_task_if_needed;
use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    build_generation_quality_runtime_owner_contract,
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, load_chapter_generation_snapshot_map,
};
use crate::services::chapter_single_generation_prepare_service::{
    build_single_generation_prepare_owner_contract,
    build_single_generation_task_view_payload_from_task_state,
    estimated_single_generation_task_minutes, single_generation_active_task_statuses,
};
use crate::services::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;

pub(crate) fn build_single_generation_existing_background_task_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_existing_background_task_service",
        "scope": "existing_background_task_query_read_state_quality_projection_and_response_payload",
        "python_source_map": [
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/chapter_generation/route_wiring_service.py",
            "backend/app/services/chapter_generation/stream/entry_service.py",
            "backend/app/services/chapter_generation/stream/service.py",
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/chapter_generation/stream/wiring_service.py",
            "backend/app/services/compat/chapter_generation_route_compat_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_existing_background_task_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "load_owned_single_generation_existing_background_task_payload",
                "load_active_single_generation_background_tasks",
                "load_active_single_generation_existing_background_task_read_states",
                "build_single_generation_existing_background_task_payload"
            ],
            "recovery_and_snapshot_contract": [
                "recover_generation_task_if_needed",
                "load_chapter_generation_snapshot_map",
                "SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot"
            ],
            "response_payload_fields": [
                "task_id",
                "chapter_id",
                "status",
                "message",
                "estimated_time_minutes",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload"
            ],
            "chapter_membership_contract": [
                "single_generation_existing_background_task_contains_chapter",
                "string chapter id",
                "object chapter id"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_generation_routes::generate_chapter_background",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "prepare_owner_contract": build_single_generation_prepare_owner_contract(),
        "runtime_state_owner_contract": build_single_generation_runtime_state_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_existing_background_task_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SingleGenerationQualityStatusContext {
    latest_quality_metrics: Option<Value>,
    quality_metrics_history: Option<Value>,
    quality_metrics_summary_state: Option<Value>,
    quality_metrics_summary: Option<Value>,
    quality_history_context: Option<Value>,
    active_story_repair_payload: Option<Value>,
}

impl SingleGenerationQualityStatusContext {
    fn from_snapshot_and_runtime_state(
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state);
        let quality_runtime_context =
            resolve_generation_quality_runtime_context_from_persisted_sources(
                "chapter",
                snapshot.and_then(|item| item.latest_quality_metrics.as_ref()),
                snapshot.and_then(|item| item.quality_metrics_history.as_ref()),
                workflow_runtime_state
                    .and_then(Value::as_object)
                    .and_then(|state| state.get("quality_metrics_summary_state")),
                snapshot.and_then(|item| item.quality_metrics_summary.as_ref()),
            );

        Self::from_runtime_quality_context_and_active_payload(
            &quality_runtime_context,
            active_story_repair_payload.as_ref(),
        )
    }

    fn insert_into_payload(&self, payload: &mut Map<String, Value>) {
        payload.insert(
            "latest_quality_metrics".to_string(),
            json!(self.latest_quality_metrics),
        );
        payload.insert(
            "quality_metrics_history".to_string(),
            json!(self.quality_metrics_history),
        );
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            json!(self.quality_metrics_summary_state),
        );
        payload.insert(
            "quality_metrics_summary".to_string(),
            json!(self.quality_metrics_summary),
        );
        payload.insert(
            "quality_history_context".to_string(),
            json!(self.quality_history_context),
        );
        payload.insert(
            "active_story_repair_payload".to_string(),
            json!(self.active_story_repair_payload),
        );
    }

    fn from_runtime_quality_context_and_active_payload(
        quality_runtime_context: &GenerationQualityRuntimeContext,
        active_story_repair_payload: Option<&Value>,
    ) -> Self {
        Self {
            latest_quality_metrics: quality_runtime_context.latest_quality_metrics.clone(),
            quality_metrics_history: quality_runtime_context.quality_metrics_history.clone(),
            quality_metrics_summary_state: quality_runtime_context
                .quality_metrics_summary_state
                .clone(),
            quality_metrics_summary: quality_runtime_context.quality_metrics_summary.clone(),
            quality_history_context: quality_runtime_context.quality_history_context.clone(),
            active_story_repair_payload: active_story_repair_payload.cloned(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationExistingBackgroundTaskReadState {
    task: batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    quality_status_context: SingleGenerationQualityStatusContext,
}

impl SingleGenerationExistingBackgroundTaskReadState {
    pub(crate) fn from_task_and_snapshot(
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

    pub(crate) fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    pub(crate) fn workflow_runtime_state(&self) -> Option<&Value> {
        self.workflow_runtime_state.as_ref()
    }

    pub(crate) fn quality_status_context(&self) -> &SingleGenerationQualityStatusContext {
        &self.quality_status_context
    }
}

pub(crate) async fn load_owned_single_generation_existing_background_task_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    project_id: &str,
    user_id: &str,
) -> Result<Option<Value>, String> {
    let tasks = load_active_single_generation_background_tasks(db, user_id, project_id).await?;
    let read_states =
        load_active_single_generation_existing_background_task_read_states(db, tasks).await?;

    Ok(read_states
        .into_iter()
        .find(|read_state| {
            single_generation_existing_background_task_contains_chapter(
                read_state.task(),
                chapter_id,
            )
        })
        .map(build_single_generation_existing_background_task_payload))
}

pub(crate) async fn load_active_single_generation_background_tasks(
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

pub(crate) fn single_generation_existing_background_task_contains_chapter(
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

pub(crate) async fn load_active_single_generation_existing_background_task_read_states(
    db: &DatabaseConnection,
    tasks: Vec<batch_generation_task::Model>,
) -> Result<Vec<SingleGenerationExistingBackgroundTaskReadState>, String> {
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
            SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
                task,
                snapshot.as_ref(),
            )
        })
        .collect())
}

pub(crate) fn build_single_generation_existing_background_task_payload(
    read_state: SingleGenerationExistingBackgroundTaskReadState,
) -> Value {
    let task = read_state.task();
    let workflow_runtime_state = read_state.workflow_runtime_state();
    let quality_status_context = read_state.quality_status_context();

    build_single_generation_existing_background_response_payload(
        task,
        workflow_runtime_state,
        quality_status_context,
    )
}

pub(crate) fn build_single_generation_existing_background_response_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
    quality_status_context: &SingleGenerationQualityStatusContext,
) -> Value {
    let mut payload =
        build_single_generation_task_view_payload_from_task_state(task, workflow_runtime_state);
    quality_status_context.insert_into_payload(&mut payload);
    insert_single_generation_existing_background_response_fields(&mut payload, task);

    Value::Object(payload)
}

fn insert_single_generation_existing_background_response_fields(
    payload: &mut Map<String, Value>,
    task: &batch_generation_task::Model,
) {
    payload.insert("task_id".to_string(), json!(task.id.clone()));
    payload.insert(
        "chapter_id".to_string(),
        json!(task.current_chapter_id.clone()),
    );
    payload.insert("status".to_string(), json!(task.status.clone()));
    payload.insert("message".to_string(), json!("已有后台生成任务正在执行"));
    payload.insert(
        "estimated_time_minutes".to_string(),
        json!(estimated_single_generation_task_minutes(
            task.target_word_count,
            task.enable_analysis,
        )),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_existing_background_task_owner_contract,
        build_single_generation_existing_background_task_payload,
        single_generation_existing_background_task_contains_chapter,
        SingleGenerationExistingBackgroundTaskReadState,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
    use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;
    use crate::services::chapter_single_generation_prepare_service::build_single_generation_prepare_owner_contract;
    use crate::services::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;
    use serde_json::json;

    fn build_existing_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", {"id": "chapter-2"}]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: true,
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
        }
    }

    #[test]
    fn should_publish_single_generation_existing_background_task_owner_contract() {
        let contract = build_single_generation_existing_background_task_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_existing_background_task_service"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "load_owned_single_generation_existing_background_task_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_fields"][10],
            "active_story_repair_payload"
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            build_single_generation_prepare_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            build_single_generation_runtime_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            build_chapter_generation_snapshot_owner_contract()["owner"]
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            build_generation_quality_runtime_owner_contract()["owner"]
        );
    }

    #[test]
    fn should_match_single_generation_existing_background_task_for_string_or_object_chapter_ids() {
        let task = build_existing_task();

        assert!(single_generation_existing_background_task_contains_chapter(
            &task,
            "chapter-1"
        ));
        assert!(single_generation_existing_background_task_contains_chapter(
            &task,
            "chapter-2"
        ));
        assert!(!single_generation_existing_background_task_contains_chapter(&task, "chapter-9"));
    }

    #[test]
    fn should_build_single_generation_existing_background_read_state_from_task_and_snapshot() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 91})),
            quality_metrics_history: Some(json!([{"overall_score": 91}])),
            quality_metrics_summary: Some(json!({"chapter_count": 1})),
            workflow_runtime_state: Some(json!({
                "progress": 55,
                "active_story_repair_payload": {
                    "summary": "沿用修复建议"
                }
            })),
            created_at: None,
            updated_at: None,
        };

        let read_state = SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
            build_existing_task(),
            Some(&snapshot),
        );

        assert_eq!(read_state.task().id, "task-1");
        assert_eq!(
            read_state
                .workflow_runtime_state()
                .and_then(|state| state.get("progress"))
                .and_then(serde_json::Value::as_i64),
            Some(55)
        );
        assert_eq!(
            read_state
                .quality_status_context()
                .latest_quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("overall_score")),
            Some(&json!(91))
        );
    }

    #[test]
    fn should_build_single_generation_existing_background_task_payload() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 92})),
            quality_metrics_history: Some(json!([{"overall_score": 88}, {"overall_score": 92}])),
            quality_metrics_summary: Some(json!({"chapter_count": 2})),
            workflow_runtime_state: Some(json!({
                "progress": 77,
                "active_story_repair_payload": {
                    "summary": "保持冲突升级"
                }
            })),
            created_at: None,
            updated_at: None,
        };
        let read_state = SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
            batch_generation_task::Model {
                id: "task-1".to_string(),
                project_id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                start_chapter_number: 2,
                chapter_count: 1,
                chapter_ids: json!(["chapter-2"]),
                style_id: None,
                target_word_count: 3200,
                enable_analysis: true,
                status: "running".to_string(),
                total_chapters: 1,
                completed_chapters: 0,
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
            Some(&snapshot),
        );
        let payload = build_single_generation_existing_background_task_payload(read_state);

        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["chapter_id"], "chapter-2");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["message"], "已有后台生成任务正在执行");
        assert_eq!(payload["estimated_time_minutes"], 3);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 92);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 92);
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "保持冲突升级"
        );
    }
}
