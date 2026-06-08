use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::{Map, Value};
use uuid::Uuid;

#[cfg(test)]
use super::chapter_single_generation_runtime_restore_service::build_single_generation_runtime_launch_input_from_request_runtime_state;
#[cfg(test)]
use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;

use super::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use super::chapter_single_generation_prepare_service::load_single_chapter_generation_target;
#[cfg(test)]
use super::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
use super::chapter_single_generation_prepare_service::{
    build_single_generation_task_view_payload_from_task_state,
    estimated_single_generation_task_minutes, single_generation_active_task_statuses,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
    SingleChapterGenerationRouteRequest,
};
use super::chapter_single_generation_runtime_restore_service::{
    PreparedSingleChapterGenerationRestoredRuntimeLaunch,
    PreparedSingleGenerationBackgroundLaunchParts,
};
use super::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLifecyclePlan;
use crate::models::{batch_generation_snapshot, batch_generation_task};
#[cfg(test)]
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationExecutionInput;
use crate::services::chapter_generation_quality_runtime_context_service::{
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_request_runtime_state_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_snapshot_service::load_chapter_generation_snapshot_map;
use crate::services::chapter_generation_task_recovery_service::recover_generation_task_if_needed;

#[derive(Debug, Clone, Default, PartialEq)]
struct SingleGenerationQualityStatusContext {
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
            serde_json::json!(self.latest_quality_metrics),
        );
        payload.insert(
            "quality_metrics_history".to_string(),
            serde_json::json!(self.quality_metrics_history),
        );
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            serde_json::json!(self.quality_metrics_summary_state),
        );
        payload.insert(
            "quality_metrics_summary".to_string(),
            serde_json::json!(self.quality_metrics_summary),
        );
        payload.insert(
            "quality_history_context".to_string(),
            serde_json::json!(self.quality_history_context),
        );
        payload.insert(
            "active_story_repair_payload".to_string(),
            serde_json::json!(self.active_story_repair_payload),
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
struct SingleGenerationExistingBackgroundTaskReadState {
    task: batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    quality_status_context: SingleGenerationQualityStatusContext,
}

impl SingleGenerationExistingBackgroundTaskReadState {
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

    fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    fn workflow_runtime_state(&self) -> Option<&Value> {
        self.workflow_runtime_state.as_ref()
    }

    fn quality_status_context(&self) -> &SingleGenerationQualityStatusContext {
        &self.quality_status_context
    }
}

async fn load_owned_single_generation_existing_background_task_payload(
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

async fn load_active_single_generation_existing_background_task_read_states(
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

fn build_single_generation_existing_background_task_payload(
    read_state: SingleGenerationExistingBackgroundTaskReadState,
) -> Value {
    let task = read_state.task();
    let workflow_runtime_state = read_state.workflow_runtime_state();
    let quality_status_context = read_state.quality_status_context();

    let mut payload =
        build_single_generation_task_view_payload_from_task_state(task, workflow_runtime_state);
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
#[derive(Debug, Clone)]
pub(crate) enum SingleGenerationBackgroundWriteWorkflowEntry {
    ExistingTaskPayload(Value),
    Launch(PreparedSingleGenerationBackgroundLaunchParts),
}

impl SingleGenerationBackgroundWriteWorkflowEntry {
    pub(crate) async fn start_from_route_payload(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        route_request: SingleChapterGenerationRouteRequest,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        Self::start(
            db,
            chapter_id,
            user_id,
            route_request.into_generation_request(),
            candidate_gateway_config,
            now,
        )
        .await
    }

    async fn start(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: SingleChapterGenerationRequest,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        Self::prepare(db, chapter_id, user_id, request)
            .await?
            .persist_and_dispatch(db, candidate_gateway_config, now)
            .await
    }

    async fn prepare(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: SingleChapterGenerationRequest,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        let chapter_target = load_single_chapter_generation_target(db, chapter_id, user_id).await?;
        if let Some(existing_task_payload) =
            load_owned_single_generation_existing_background_task_payload(
                db,
                chapter_id,
                &chapter_target.project_id,
                user_id,
            )
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Internal)?
        {
            return Ok(Self::ExistingTaskPayload(existing_task_payload));
        }

        let task_id = Uuid::new_v4().to_string();
        let launch_parts =
            PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_background_launch_parts_from_target(
                db,
                user_id,
                &request,
                chapter_target,
                task_id,
            )
            .await?;

        Ok(Self::Launch(launch_parts))
    }

    async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        match self {
            Self::ExistingTaskPayload(payload) => Ok(payload),
            Self::Launch(launch_parts) => {
                persist_owned_single_generation_background_launch(
                    db,
                    candidate_gateway_config,
                    now,
                    launch_parts,
                )
                .await
            }
        }
    }

    #[cfg(test)]
    fn from_existing_task_payload(payload: Value) -> Self {
        Self::ExistingTaskPayload(payload)
    }

    #[cfg(test)]
    fn from_prepared_request(
        task_id: String,
        user_id: &str,
        chapter_target: SingleChapterGenerationTarget,
        execution_input: SingleChapterGenerationExecutionInput,
        request_runtime_state: BatchGenerationRequestRuntimeState,
        runtime_state_payload: Value,
    ) -> Self {
        Self::Launch(build_background_launch_parts_from_prepared_request(
            task_id,
            user_id,
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        ))
    }
}

async fn persist_owned_single_generation_background_launch(
    db: &DatabaseConnection,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    now: chrono::NaiveDateTime,
    launch_parts: PreparedSingleGenerationBackgroundLaunchParts,
) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
    let PreparedSingleGenerationBackgroundLaunchParts {
        task_seed,
        startup_snapshot_plan,
        response_payload,
        runtime_input,
    } = launch_parts;
    let task_id = task_seed.id.clone();
    let task: crate::models::batch_generation_task::ActiveModel = task_seed.into_active_model(now);

    task.insert(db)
        .await
        .map_err(|error| PrepareSingleChapterGenerationRequestError::Internal(error.to_string()))?;
    startup_snapshot_plan
        .persist(db, &task_id)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
    SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config(
        task_id,
        runtime_input,
        candidate_gateway_config,
    )
    .spawn(db.clone());

    Ok(response_payload)
}

#[cfg(test)]
fn build_background_launch_parts_from_restored_launch(
    task_id: String,
    restored_launch: PreparedSingleChapterGenerationRestoredRuntimeLaunch,
) -> PreparedSingleGenerationBackgroundLaunchParts {
    restored_launch.into_background_launch_parts(task_id)
}

#[cfg(test)]
fn build_background_launch_parts_from_prepared_request(
    task_id: String,
    user_id: &str,
    chapter_target: SingleChapterGenerationTarget,
    execution_input: SingleChapterGenerationExecutionInput,
    request_runtime_state: BatchGenerationRequestRuntimeState,
    runtime_state_payload: Value,
) -> PreparedSingleGenerationBackgroundLaunchParts {
    let target_word_count = execution_input.target_word_count;
    let execution_config = execution_input.execution_config.clone();
    let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
        &chapter_target,
        user_id,
        target_word_count,
        &request_runtime_state,
        execution_config,
    );
    let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
        chapter_target,
        runtime_state_payload,
        runtime_input,
    );

    build_background_launch_parts_from_restored_launch(task_id, restored_launch)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_background_launch_parts_from_prepared_request,
        build_background_launch_parts_from_restored_launch,
        build_single_generation_existing_background_task_payload,
        single_generation_existing_background_task_contains_chapter,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch,
        SingleGenerationBackgroundWriteWorkflowEntry,
        SingleGenerationExistingBackgroundTaskReadState, SingleGenerationQualityStatusContext,
    };
    use crate::ai::AIConfig;
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        }
    }

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
    fn should_keep_single_generation_background_workflow_existing_payload_owner_contract() {
        let entry =
            SingleGenerationBackgroundWriteWorkflowEntry::from_existing_task_payload(json!({
                "task_id": "task-11",
                "chapter_id": "chapter-11",
                "status": "running",
                "message": "已有后台生成任务正在执行"
            }));

        match entry {
            SingleGenerationBackgroundWriteWorkflowEntry::ExistingTaskPayload(payload) => {
                assert_eq!(payload["task_id"], "task-11");
                assert_eq!(payload["chapter_id"], "chapter-11");
                assert_eq!(payload["status"], "running");
                assert_eq!(payload["message"], "已有后台生成任务正在执行");
            }
            SingleGenerationBackgroundWriteWorkflowEntry::Launch(_) => {
                panic!("expected existing task payload branch")
            }
        }
    }

    #[test]
    fn should_build_single_generation_quality_status_context_from_snapshot_and_runtime_state() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 91})),
            quality_metrics_history: Some(json!([
                {"overall_score": 84},
                {"overall_score": 91}
            ])),
            quality_metrics_summary: Some(json!({
                "quality_gate": {"decision": "pass"},
                "chapter_count": 2
            })),
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        };
        let runtime_state = json!({
            "quality_metrics_summary_state": {
                "scope": "chapter",
                "chapter_count": 2
            },
            "active_story_repair_payload": {
                "summary": "沿用修复建议"
            }
        });

        let context = SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            Some(&snapshot),
            Some(&runtime_state),
        );

        assert_eq!(
            context.latest_quality_metrics,
            Some(json!({"overall_score": 91}))
        );
        assert_eq!(
            context.quality_metrics_summary_state,
            Some(json!({"scope": "chapter", "chapter_count": 2}))
        );
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"summary": "沿用修复建议"}))
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
                .and_then(Value::as_i64),
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
    fn should_preserve_richer_quality_runtime_contract_on_existing_single_generation_background_payload(
    ) {
        let payload = build_single_generation_existing_background_task_payload(
            SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
                batch_generation_task::Model {
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
                Some(&batch_generation_snapshot::Model {
                    id: "snapshot-7".to_string(),
                    batch_task_id: "task-7".to_string(),
                    latest_quality_metrics: Some(json!({"overall_score": 84})),
                    quality_metrics_history: Some(json!([
                        {"overall_score": 80},
                        {"overall_score": 84}
                    ])),
                    quality_metrics_summary: Some(json!({
                        "quality_gate": {"decision": "manual_review"},
                        "chapter_count": 1
                    })),
                    workflow_runtime_state: Some(json!({
                        "active_story_repair_payload": {
                            "summary": "沿用修复建议",
                            "scope": "chapter"
                        },
                        "quality_metrics_summary_state": {
                            "scope": "chapter",
                            "chapter_count": 1
                        }
                    })),
                    created_at: None,
                    updated_at: None,
                }),
            ),
        );

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
    }

    #[test]
    fn should_keep_single_generation_existing_background_payload_owner_contract() {
        let payload = build_single_generation_existing_background_task_payload(
            SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
                batch_generation_task::Model {
                    id: "task-8".to_string(),
                    project_id: "project-1".to_string(),
                    user_id: "user-1".to_string(),
                    start_chapter_number: 8,
                    chapter_count: 1,
                    chapter_ids: json!(["chapter-8"]),
                    style_id: None,
                    target_word_count: 2800,
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
                None,
            ),
        );

        assert_eq!(payload["task_id"], "task-8");
        assert_eq!(payload["chapter_id"], "chapter-8");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["message"], "已有后台生成任务正在执行");
        assert_eq!(payload["estimated_time_minutes"], 1);
        assert!(payload["latest_quality_metrics"].is_null());
    }

    #[test]
    fn should_build_single_generation_existing_background_task_payload_from_single_owner() {
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

    #[test]
    fn should_keep_single_generation_background_workflow_launch_owner_contract() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-12".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第十二章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 3200,
            compat_options: empty_compat_options(),
            execution_config:
                crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            execution_input.compat_options.clone(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "quality_metrics_summary": {"chapter_count": 1}
        });
        let entry = SingleGenerationBackgroundWriteWorkflowEntry::from_prepared_request(
            "task-12".to_string(),
            "user-1",
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        );

        match entry {
            SingleGenerationBackgroundWriteWorkflowEntry::Launch(launch) => {
                let response_payload = launch.response_payload.clone();

                assert_eq!(response_payload["task_id"], "task-12");
                assert_eq!(response_payload["chapter_id"], "chapter-12");
                assert_eq!(response_payload["estimated_time_minutes"], 3);
                assert_eq!(
                    launch.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                        ["chapter_count"],
                    1
                );
            }
            SingleGenerationBackgroundWriteWorkflowEntry::ExistingTaskPayload(_) => {
                panic!("expected launch branch")
            }
        }
    }

    #[test]
    fn should_keep_background_launch_owner_contract_from_restored_launch() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-31".to_string(),
            project_id: "project-31".to_string(),
            chapter_number: 31,
            title: "第三十一章".to_string(),
        };
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            chapter_target,
            json!({
                "quality_metrics_summary": {"chapter_count": 1},
                "active_story_repair_payload": {"summary": "沿用修复建议"}
            }),
            SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-31".to_string(),
                user_id: "user-31".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 3600,
                    compat_options: empty_compat_options(),
                    execution_config:
                        crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                            ai_config: AIConfig::default(),
                            provider_payload: PromptContextProviderPayload {
                                recent_chapters_context: String::new(),
                                previous_chapter_summary: String::new(),
                                chapter_careers: "[]".to_string(),
                                characters_info: "[]".to_string(),
                                foreshadow_reminders: "[]".to_string(),
                                relevant_memories: "[]".to_string(),
                                research_query: String::new(),
                                research_assets: "[]".to_string(),
                                external_assets: "[]".to_string(),
                                reference_assets: "[]".to_string(),
                                mcp_references: String::new(),
                            },
                        },
                },
            },
        );

        let launch_parts = build_background_launch_parts_from_restored_launch(
            "task-31".to_string(),
            restored_launch,
        );

        assert_eq!(launch_parts.task_seed.id, "task-31");
        assert_eq!(launch_parts.task_seed.project_id, "project-31");
        assert_eq!(launch_parts.task_seed.user_id, "user-31");
        assert_eq!(launch_parts.task_seed.target_word_count, 3600);
        assert_eq!(launch_parts.response_payload["task_id"], "task-31");
        assert_eq!(launch_parts.response_payload["chapter_id"], "chapter-31");
        assert_eq!(launch_parts.response_payload["status"], "pending");
        assert_eq!(
            launch_parts.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["chapter_count"],
            1
        );
        assert_eq!(
            launch_parts
                .startup_snapshot_plan
                .active_story_repair_payload(),
            Some(json!({"summary": "沿用修复建议"}))
        );
    }

    #[test]
    fn should_build_background_launch_parts_from_prepared_request_owner_path() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-32".to_string(),
            project_id: "project-32".to_string(),
            chapter_number: 32,
            title: "第三十二章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2800,
            compat_options: empty_compat_options(),
            execution_config:
                crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            execution_input.compat_options.clone(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "quality_metrics_summary": {"chapter_count": 1},
            "active_story_repair_payload": {
                "summary": "继承历史修复建议",
                "scope": "chapter"
            }
        });

        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-32".to_string(),
            "user-32",
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        );

        assert_eq!(launch_parts.task_seed.id, "task-32");
        assert_eq!(launch_parts.task_seed.project_id, "project-32");
        assert_eq!(launch_parts.runtime_input.chapter_id, "chapter-32");
        assert_eq!(launch_parts.runtime_input.user_id, "user-32");
        assert_eq!(launch_parts.response_payload["estimated_time_minutes"], 2);
        assert_eq!(
            launch_parts.startup_snapshot_plan.runtime_state()["chapter_id"],
            "chapter-32"
        );
        assert_eq!(
            launch_parts
                .startup_snapshot_plan
                .active_story_repair_payload(),
            Some(json!({
                "summary": "继承历史修复建议",
                "scope": "chapter"
            }))
        );
    }
}
