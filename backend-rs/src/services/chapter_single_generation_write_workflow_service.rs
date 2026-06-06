use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

#[cfg(test)]
use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;

use super::chapter_single_generation_existing_background_query_service::load_owned_single_generation_existing_background_task_payload;
use super::chapter_single_generation_prepare_service::{
    build_single_chapter_generation_request_from_route_payload,
    load_single_chapter_generation_target, PrepareSingleChapterGenerationRequestError,
    PreparedSingleChapterGenerationRestoredRuntimeLaunch,
    PreparedSingleGenerationBackgroundLaunchParts, SingleChapterGenerationRequest,
    SingleChapterGenerationRouteRequest,
};
#[cfg(test)]
use super::chapter_single_generation_prepare_service::{
    build_single_generation_runtime_launch_input, SingleChapterGenerationExecutionInput,
};

#[cfg(test)]
use super::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
#[cfg(test)]
use super::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

#[derive(Debug, Clone)]
enum SingleGenerationBackgroundWorkflowEntry {
    ExistingTaskPayload(Value),
    Launch(PreparedSingleGenerationBackgroundLaunchParts),
}

impl SingleGenerationBackgroundWorkflowEntry {
    async fn start(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: SingleChapterGenerationRequest,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        Self::prepare(db, chapter_id, user_id, request)
            .await?
            .persist_and_dispatch(db, now)
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
        let launch_parts = PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_background_launch_parts_from_target(
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
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        match self {
            Self::ExistingTaskPayload(payload) => Ok(payload),
            Self::Launch(launch_parts) => launch_parts.persist_and_dispatch(db, now).await,
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
    let runtime_input = build_single_generation_runtime_launch_input(
        chapter_target.chapter_id.clone(),
        user_id.to_string(),
        execution_input,
        &request_runtime_state,
        &runtime_state_payload,
    );
    let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
        chapter_target,
        runtime_state_payload,
        runtime_input,
    );

    build_background_launch_parts_from_restored_launch(task_id, restored_launch)
}

pub(crate) async fn start_owned_single_generation_background_write_workflow(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: SingleChapterGenerationRequest,
) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
    SingleGenerationBackgroundWorkflowEntry::start(
        db,
        chapter_id,
        user_id,
        request,
        Utc::now().naive_utc(),
    )
    .await
}

pub(crate) async fn start_owned_single_generation_background_write_workflow_from_route_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    route_request: SingleChapterGenerationRouteRequest,
) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
    start_owned_single_generation_background_write_workflow(
        db,
        chapter_id,
        user_id,
        build_single_chapter_generation_request_from_route_payload(route_request),
    )
    .await
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        build_background_launch_parts_from_prepared_request,
        SingleGenerationBackgroundWorkflowEntry, SingleGenerationRuntimeLaunchInput,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_request_runtime_state_service::{
        batch_generation_request_runtime_state_payload,
        parse_batch_generation_request_runtime_state, BatchGenerationRequestRuntimeState,
    };
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_prepare_service::{
        build_single_generation_runtime_launch_input,
        build_single_generation_runtime_state_payload_from_parts,
        build_single_generation_runtime_state_payload_from_sources,
        resolve_single_generation_runtime_compat_options_from_seed,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch, RestoredSingleGenerationRuntimeState,
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
        SingleChapterGenerationTarget, SingleGenerationRuntimeSeedSource,
    };
    use crate::services::chapter_single_generation_snapshot_service::SingleGenerationStartupSnapshotPlan;
    use crate::services::chapter_story_repair_quality_context_service::aggregate_story_repair_quality_summaries;

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

    #[test]
    fn should_build_single_generation_background_launch_persistence_plan_from_prepared_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: empty_compat_options(),
            execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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

        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 5, 0)
            .expect("valid time");
        let task_id = Uuid::new_v4().to_string();
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            execution_input.compat_options.clone(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "quality_metrics_summary": {"chapter_count": 1}
        });
        let launch_parts = build_background_launch_parts_from_prepared_request(
            task_id.clone(),
            "user-1",
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        );
        let checkpoint = launch_parts.startup_snapshot_plan.runtime_state().clone();
        let response_payload = launch_parts.response_payload.clone();
        let task = launch_parts.task_seed.clone().into_active_model(now);

        assert_eq!(task_id.len(), 36);
        assert_eq!(task.total_chapters, Set(1));
        assert_eq!(task.current_chapter_id, Set(Some("chapter-7".to_string())));
        assert_eq!(launch_parts.runtime_input.user_id, "user-1");
        assert_eq!(
            launch_parts.runtime_input.execution_input.target_word_count,
            2600
        );
        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["quality_metrics_summary"]["chapter_count"], 1);
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
    }

    #[test]
    fn should_keep_single_generation_background_persistence_plan_runtime_input_owner_contract() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-7".to_string(),
            "user-1",
            chapter_target,
            SingleChapterGenerationExecutionInput {
                target_word_count: 2600,
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
            BatchGenerationRequestRuntimeState::new(empty_compat_options(), None),
            json!({}),
        );
        let super::PreparedSingleGenerationBackgroundLaunchParts {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = launch_parts;

        assert_eq!(task_seed.id, "task-7");
        assert_eq!(task_seed.current_chapter_id.as_deref(), Some("chapter-7"));
        assert_eq!(
            startup_snapshot_plan.runtime_state()["chapter_id"],
            "chapter-7"
        );
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(runtime_input.execution_input.target_word_count, 2600);
    }

    #[test]
    fn should_keep_single_generation_background_persistence_plan_restored_launch_owner_contract() {
        let launch_parts = super::build_background_launch_parts_from_restored_launch(
            "task-9".to_string(),
            PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
                SingleChapterGenerationTarget {
                    chapter_id: "chapter-9".to_string(),
                    project_id: "project-1".to_string(),
                    chapter_number: 9,
                    title: "第九章".to_string(),
                },
                json!({
                    "chapter_id": "chapter-9",
                    "quality_metrics_summary": {
                        "chapter_count": 2
                    }
                }),
                SingleGenerationRuntimeLaunchInput {
                    chapter_id: "chapter-9".to_string(),
                    user_id: "user-9".to_string(),
                    execution_input: SingleChapterGenerationExecutionInput {
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
                    },
                },
            ),
        );
        let super::PreparedSingleGenerationBackgroundLaunchParts {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = launch_parts;

        assert_eq!(task_seed.id, "task-9");
        assert_eq!(task_seed.current_chapter_id.as_deref(), Some("chapter-9"));
        assert_eq!(
            startup_snapshot_plan.runtime_state()["chapter_id"],
            "chapter-9"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(response_payload["chapter_id"], "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 3200);
    }

    #[test]
    fn should_project_single_generation_background_persistence_plan_from_restored_launch_owner() {
        let launch_parts = super::build_background_launch_parts_from_restored_launch(
            "task-11".to_string(),
            PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
                SingleChapterGenerationTarget {
                    chapter_id: "chapter-11".to_string(),
                    project_id: "project-1".to_string(),
                    chapter_number: 11,
                    title: "第十一章".to_string(),
                },
                json!({
                    "chapter_id": "chapter-11",
                    "active_story_repair_payload": {
                        "summary": "沿用修复建议",
                        "scope": "chapter"
                    },
                    "quality_metrics_summary": {
                        "chapter_count": 3
                    }
                }),
                SingleGenerationRuntimeLaunchInput {
                    chapter_id: "chapter-11".to_string(),
                    user_id: "user-11".to_string(),
                    execution_input: SingleChapterGenerationExecutionInput {
                        target_word_count: 3800,
                        compat_options: SingleChapterGenerationCompatOptions {
                            enable_analysis: true,
                            ..empty_compat_options()
                        },
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
            ),
        );

        assert_eq!(launch_parts.task_seed.id, "task-11");
        assert_eq!(
            launch_parts.task_seed.current_chapter_id.as_deref(),
            Some("chapter-11")
        );
        assert_eq!(
            launch_parts.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["chapter_count"],
            3
        );
        assert_eq!(launch_parts.response_payload["chapter_id"], "chapter-11");
        assert_eq!(launch_parts.response_payload["estimated_time_minutes"], 3);
        assert_eq!(
            launch_parts.response_payload["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
        assert_eq!(launch_parts.runtime_input.user_id, "user-11");
    }

    #[test]
    fn should_keep_single_generation_background_active_model_defaults() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 5, 0)
            .expect("valid time");

        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-7".to_string(),
            "user-1",
            chapter_target,
            SingleChapterGenerationExecutionInput {
                target_word_count: 2600,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
            BatchGenerationRequestRuntimeState::new(
                empty_compat_options(),
                None,
            ),
            json!({}),
        );
        let active = launch_parts.task_seed.into_active_model(now);

        assert_eq!(active.id, Set("task-7".to_string()));
        assert_eq!(active.total_chapters, Set(1));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-7".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(7)));
        assert_eq!(active.enable_analysis, Set(false));
    }

    #[test]
    fn should_build_single_generation_background_response_payload_from_runtime_seed() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-7".to_string(),
            "user-1",
            chapter_target,
            SingleChapterGenerationExecutionInput {
                target_word_count: 4500,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..empty_compat_options()
                },
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
            BatchGenerationRequestRuntimeState::new(empty_compat_options(), None),
            json!({
                "active_story_repair_payload": {
                    "summary": "沿用修复建议",
                    "repair_targets": ["压缩说明"],
                    "scope": "chapter"
                }
            }),
        );
        let payload = launch_parts.response_payload;

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明"])
        );
    }

    /*
    #[test]
    fn should_preserve_richer_quality_runtime_contract_on_single_generation_background_create_payload(
    ) {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "绗竷绔?.to_string(),
        };
        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-7".to_string(),
            "user-1",
            chapter_target,
            SingleChapterGenerationExecutionInput {
                target_word_count: 4500,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..empty_compat_options()
                },
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
            BatchGenerationRequestRuntimeState::new(empty_compat_options(), None),
            json!({
                "active_story_repair_payload": {
                    "summary": "娌跨敤淇寤鸿",
                    "repair_targets": ["鍘嬬缉璇存槑"],
                    "scope": "chapter"
                },
                "latest_quality_metrics": {
                    "overall_score": 91,
                    "quality_gate": {
                        "decision": "pass"
                    }
                },
                "quality_metrics_history": [
                    {
                        "overall_score": 84,
                        "quality_gate": {
                            "decision": "manual_review"
                        }
                    },
                    {
                        "overall_score": 91,
                        "quality_gate": {
                            "decision": "pass"
                        }
                    }
                ],
                "quality_metrics_summary": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_metrics_summary_state": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_history_context": {
                    "scope": "chapter",
                    "history_scope": "chapter",
                    "recent_metrics": [
                        {
                            "overall_score": 91
                        }
                    ]
                }
            }),
        );
        let payload = launch_parts.response_payload;

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["batch_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "娌跨敤淇寤鸿"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(
            payload["quality_history_context"]["history_scope"],
            "chapter"
        );
    }

    */

    #[test]
    fn should_preserve_richer_quality_runtime_contract_on_single_generation_background_create_payload_safe(
    ) {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };
        let launch_parts = build_background_launch_parts_from_prepared_request(
            "task-7".to_string(),
            "user-1",
            chapter_target,
            SingleChapterGenerationExecutionInput {
                target_word_count: 4500,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..empty_compat_options()
                },
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
            BatchGenerationRequestRuntimeState::new(empty_compat_options(), None),
            json!({
                "active_story_repair_payload": {
                    "summary": "repair summary",
                    "repair_targets": ["repair target"],
                    "scope": "chapter"
                },
                "latest_quality_metrics": {
                    "overall_score": 91,
                    "quality_gate": {
                        "decision": "pass"
                    }
                },
                "quality_metrics_history": [
                    {
                        "overall_score": 84,
                        "quality_gate": {
                            "decision": "manual_review"
                        }
                    },
                    {
                        "overall_score": 91,
                        "quality_gate": {
                            "decision": "pass"
                        }
                    }
                ],
                "quality_metrics_summary": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_metrics_summary_state": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_history_context": {
                    "scope": "chapter",
                    "history_scope": "chapter",
                    "recent_metrics": [
                        {
                            "overall_score": 91
                        }
                    ]
                }
            }),
        );
        let payload = launch_parts.response_payload;

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["batch_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-7");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "repair summary"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["repair target"])
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(
            payload["quality_history_context"]["history_scope"],
            "chapter"
        );
    }

    #[test]
    fn should_keep_single_generation_background_workflow_existing_payload_owner_contract() {
        let entry = SingleGenerationBackgroundWorkflowEntry::from_existing_task_payload(json!({
            "task_id": "task-11",
            "chapter_id": "chapter-11",
            "status": "running",
            "message": "已有后台生成任务正在执行"
        }));

        match entry {
            SingleGenerationBackgroundWorkflowEntry::ExistingTaskPayload(payload) => {
                assert_eq!(payload["task_id"], "task-11");
                assert_eq!(payload["chapter_id"], "chapter-11");
                assert_eq!(payload["status"], "running");
                assert_eq!(payload["message"], "已有后台生成任务正在执行");
            }
            SingleGenerationBackgroundWorkflowEntry::Launch(_) => {
                panic!("expected existing task payload branch")
            }
        }
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
        let entry = SingleGenerationBackgroundWorkflowEntry::from_prepared_request(
            "task-12".to_string(),
            "user-1",
            chapter_target,
            execution_input,
            request_runtime_state,
            runtime_state_payload,
        );

        match entry {
            SingleGenerationBackgroundWorkflowEntry::Launch(launch) => {
                let response_payload = launch.response_payload.clone();

                assert_eq!(response_payload["chapter_id"], "chapter-12");
                assert_eq!(response_payload["estimated_time_minutes"], 3);
                assert_eq!(
                    launch.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                        ["chapter_count"],
                    1
                );
            }
            SingleGenerationBackgroundWorkflowEntry::ExistingTaskPayload(_) => {
                panic!("expected launch branch")
            }
        }
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_from_restored_seed() {
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: SingleChapterGenerationCompatOptions {
                enable_analysis: false,
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["request-target".to_string()],
                story_preserve_strengths: vec!["request-strength".to_string()],
                ..empty_compat_options()
            },
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
            "active_story_repair_payload": {
                "scope": "chapter",
                "summary": "restored summary",
                "targets": ["restored-target"],
                "preserve_strengths": ["restored-strength"]
            },
            "quality_metrics_summary": {
                "manual_review_label": "continuity"
            }
        });

        let runtime_input = build_single_generation_runtime_launch_input(
            "chapter-7".to_string(),
            "user-1".to_string(),
            execution_input,
            &request_runtime_state,
            &runtime_state_payload,
        );

        assert_eq!(runtime_input.chapter_id, "chapter-7");
        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(runtime_input.execution_input.target_word_count, 2600);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary
                .as_deref(),
            Some("request summary")
        );
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_targets,
            vec!["request-target".to_string()]
        );
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_preserve_strengths,
            vec!["request-strength".to_string()]
        );
    }

    #[test]
    fn should_convert_restored_single_generation_launch_into_runtime_input() {
        let launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            SingleChapterGenerationTarget {
                chapter_id: "chapter-9".to_string(),
                project_id: "project-1".to_string(),
                chapter_number: 9,
                title: "第九章".to_string(),
            },
            json!({
                "quality_metrics_summary": {"chapter_count": 2}
            }),
            SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-9".to_string(),
                user_id: "user-77".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2400,
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

        let runtime_input = launch.into_runtime_launch_input();

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-77");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
    }

    #[test]
    fn should_materialize_single_generation_startup_snapshot_inside_restored_launch_owner() {
        let launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            SingleChapterGenerationTarget {
                chapter_id: "chapter-9".to_string(),
                project_id: "project-1".to_string(),
                chapter_number: 9,
                title: "第九章".to_string(),
            },
            json!({
                "chapter_id": "chapter-9",
                "quality_metrics_summary": {
                    "chapter_count": 2
                },
                "active_story_repair_payload": {
                    "summary": "沿用修复建议",
                    "scope": "chapter"
                }
            }),
            SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-9".to_string(),
                user_id: "user-77".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2400,
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

        let startup_snapshot_plan = launch.startup_snapshot_plan();

        assert_eq!(
            startup_snapshot_plan.runtime_state()["chapter_id"],
            "chapter-9"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            startup_snapshot_plan.active_story_repair_payload(),
            Some(json!({
                "summary": "沿用修复建议",
                "scope": "chapter"
            }))
        );
    }

    #[test]
    fn should_merge_single_generation_runtime_state_into_pending_checkpoint_for_resume() {
        let checkpoint = serde_json::json!({
            "phase": "pending",
            "status": "pending",
            "chapter_id": "chapter-7",
            "current_chapter_id": "chapter-7"
        });
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                style_id: Some(7),
                enable_analysis: true,
                enable_mcp: false,
                web_research_enabled: true,
                web_research_query: Some("旧都城的废墟".to_string()),
                narrative_perspective: Some("第一人称".to_string()),
                creative_mode: Some("balanced".to_string()),
                story_focus: Some("character".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("角色在遗迹中寻找真相".to_string()),
                quality_preset: Some("strict".to_string()),
                quality_notes: Some("强化悬念".to_string()),
                story_repair_summary: Some("补强上一章伏笔回收".to_string()),
                story_repair_targets: vec!["伏笔".to_string()],
                story_preserve_strengths: vec!["氛围".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );
        let checkpoint = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            checkpoint,
            batch_generation_request_runtime_state_payload(&runtime_state),
        )
        .runtime_state()
        .clone();

        let seeded_state = parse_batch_generation_request_runtime_state(Some(&checkpoint));
        assert_eq!(seeded_state, runtime_state);
        assert_eq!(
            checkpoint["active_story_repair_payload"]["summary"],
            "补强上一章伏笔回收"
        );
        assert_eq!(
            checkpoint["active_story_repair_payload"]["repair_targets"],
            json!(["伏笔"])
        );
    }

    #[test]
    fn should_restore_current_chapter_quality_seed_owner_for_single_generation() {
        let restored_runtime_state = RestoredSingleGenerationRuntimeState::from_quality_fragments(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-7"
            }),
            &BatchGenerationRequestRuntimeState::new(empty_compat_options(), None),
            ChapterQualityMetricsFragments {
                latest_quality_metrics: Some(json!({"overall_score": 81})),
                history_id: Some("history-1".to_string()),
                generated_at: Some("2026-05-31T01:00:00".to_string()),
                quality_metrics_summary: Some(json!({"raw": {"overall_score": 81}})),
                quality_metrics_history: Some(
                    json!([{"overall_score": 79}, {"overall_score": 81}]),
                ),
                quality_metrics_summary_state: Some(json!({"chapter_count": 2})),
            },
            Some(json!({"quality_gate": {"label": "最近历史摘要"}})),
        );

        assert_eq!(
            restored_runtime_state.seed_source(),
            SingleGenerationRuntimeSeedSource::CurrentChapterQuality
        );
        assert_eq!(
            restored_runtime_state.runtime_state_payload()["latest_quality_metrics"]
                ["overall_score"],
            81
        );
        assert_eq!(
            restored_runtime_state.runtime_state_payload()["quality_metrics_history"],
            json!([{"overall_score": 79}, {"overall_score": 81}])
        );
        assert_eq!(
            restored_runtime_state.runtime_state_payload()["quality_metrics_summary_state"]
                ["chapter_count"],
            2
        );
    }

    #[test]
    fn should_restore_recent_history_summary_seed_owner_for_single_generation() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(empty_compat_options(), None);
        let restored_runtime_state = RestoredSingleGenerationRuntimeState::from_quality_fragments(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-7"
            }),
            &runtime_state,
            ChapterQualityMetricsFragments {
                latest_quality_metrics: None,
                history_id: None,
                generated_at: None,
                quality_metrics_summary: None,
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
            },
            Some(json!({
                "repair_guidance": {
                    "summary": "沿用最近历史修复建议",
                    "repair_targets": ["压缩说明"],
                    "preserve_strengths": ["人物张力"],
                    "focus_areas": ["节奏"]
                },
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "最近历史摘要"
                }
            })),
        );
        let payload = restored_runtime_state.runtime_state_payload();

        assert_eq!(
            restored_runtime_state.seed_source(),
            SingleGenerationRuntimeSeedSource::RecentHistorySummary
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
    }

    #[test]
    fn should_seed_single_generation_runtime_state_from_current_chapter_quality_only() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            Some("gpt-4.1".to_string()),
        );
        let quality_metrics_summary = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "压缩当前章节解释段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["悬念氛围"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章说明偏多",
                "failed_metrics": [{"label": "节奏"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 84}],
                "history_scope": "chapter"
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "pacing_score": 7.6,
            "repair_guidance": {
                "summary": "压缩当前章节解释段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["悬念氛围"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章说明偏多",
                "failed_metrics": [{"label": "节奏"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 84}],
                "history_scope": "chapter"
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_metrics_summary),
            Some(&latest_quality_metrics),
            None,
            None,
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "压缩当前章节解释段"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "current_chapter_quality"
        );
        assert_eq!(
            payload["quality_history_context"]["history_scope"],
            "chapter"
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"][0]["overall_score"],
            84
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"][0]["quality_gate"]["decision"],
            "repair"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
    }

    #[test]
    fn should_merge_manual_and_current_chapter_quality_into_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工长板".to_string()],
                ..empty_compat_options()
            },
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "当前质量摘要",
                "repair_targets": ["共同目标", "质量目标"],
                "preserve_strengths": ["质量长板"],
                "focus_areas": ["节奏", "冲突"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章节奏不稳",
                "failed_metrics": [{"label": "节奏"}]
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 82,
            "repair_guidance": {
                "summary": "当前质量摘要",
                "repair_targets": ["共同目标", "质量目标"],
                "preserve_strengths": ["质量长板"],
                "focus_areas": ["节奏", "冲突"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章节奏不稳",
                "failed_metrics": [{"label": "节奏"}]
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_metrics_summary),
            Some(&latest_quality_metrics),
            None,
            None,
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "手工摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "质量目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工长板", "质量长板"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_current_chapter_quality"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source_label"],
            "Manual + current chapter quality"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 82);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 82);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
    }

    #[test]
    fn should_seed_single_generation_runtime_state_from_recent_history_summary_when_current_quality_missing(
    ) {
        let runtime_state = BatchGenerationRequestRuntimeState::new(empty_compat_options(), None);
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "沿用前序章节修复建议",
                "repair_targets": ["压缩说明", "前置冲突"],
                "preserve_strengths": ["人物张力"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "前序章节存在节奏问题",
                "failed_metrics": [{"label": "节奏"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 81}],
                "history_scope": "chapter"
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            None,
            None,
            "recent_history_summary",
            "Recent history summary",
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用前序章节修复建议"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source_label"],
            "Recent history summary"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 81}],
                "history_scope": "chapter"
            })
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
    }

    #[test]
    fn should_preserve_existing_single_generation_quality_history_when_seeding_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            Some("gpt-4.1".to_string()),
        );
        let quality_metrics_summary = json!({
            "overall_score": 81,
            "repair_guidance": {
                "summary": "压缩当前章节解释段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["悬念氛围"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复"
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 81}],
                "history_scope": "chapter"
            }
        });
        let existing_quality_metrics_history = json!([
            {
                "overall_score": 86,
                "quality_gate": {"decision": "passed"},
                "repair_guidance": {"summary": "保持节奏"}
            },
            {
                "overall_score": 81,
                "quality_gate": {"decision": "repair"},
                "repair_guidance": {"summary": "压缩当前章节解释段"}
            }
        ]);
        let existing_quality_metrics_summary_state = json!({
            "scope": "chapter",
            "chapter_count": 2,
            "first_overall_score": 86.0,
            "last_overall_score": 81.0,
            "recent_history": [
                {
                    "overall_score": 86,
                    "quality_gate": {"decision": "passed"}
                },
                {
                    "overall_score": 81,
                    "quality_gate": {"decision": "repair"}
                }
            ]
        });
        let latest_quality_metrics = json!({
            "overall_score": 81,
            "repair_guidance": {
                "summary": "压缩当前章节解释段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["悬念氛围"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复"
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_metrics_summary),
            Some(&latest_quality_metrics),
            Some(&existing_quality_metrics_history),
            Some(&existing_quality_metrics_summary_state),
        );

        assert_eq!(
            payload["quality_metrics_history"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 86);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            86.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            81.0
        );
    }

    #[test]
    fn should_aggregate_recent_history_summaries_before_seeding_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(empty_compat_options(), None);
        let first_summary = json!({
            "overall_score": 85,
            "repair_guidance": {
                "summary": "优先压缩当前说明段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["人物张力"],
                "focus_areas": ["pacing", "conflict"]
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let second_summary = json!({
            "overall_score": 80,
            "repair_guidance": {
                "summary": "补角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["对白辨识度"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });
        let aggregated =
            aggregate_story_repair_quality_summaries(&[first_summary, second_summary], "chapter")
                .expect("aggregated chapter summary");

        let payload = build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&aggregated),
            None,
            None,
            None,
            "recent_history_summary",
            "Recent history summary",
        );
        let compat =
            resolve_single_generation_runtime_compat_options_from_seed(&runtime_state, &payload);

        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            payload["quality_metrics_history"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 80);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 85);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 85);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            80.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            85.0
        );
        assert_eq!(compat.story_repair_summary(), "优先压缩当前说明段");
        assert_eq!(
            compat.story_repair_targets(),
            &[
                "压缩说明".to_string(),
                "提前冲突".to_string(),
                "强化动机".to_string()
            ]
        );
    }

    #[test]
    fn should_merge_manual_and_recent_history_summary_into_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工长板".to_string()],
                ..empty_compat_options()
            },
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "前序章节质量摘要",
                "repair_targets": ["共同目标", "历史目标"],
                "preserve_strengths": ["历史长板"],
                "focus_areas": ["节奏", "信息密度"]
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            None,
            None,
            "recent_history_summary",
            "Recent history summary",
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "手工摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工长板", "历史长板"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_recent_history_summary"
        );
    }

    #[test]
    fn should_restore_single_generation_runtime_compat_options_from_seeded_story_repair_payload() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(empty_compat_options(), None);
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "沿用单章历史修复建议",
                "repair_targets": ["压缩说明", "补强冲突"],
                "preserve_strengths": ["人物张力"]
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            None,
            None,
            "recent_history_summary",
            "Recent history summary",
        );
        let compat =
            resolve_single_generation_runtime_compat_options_from_seed(&runtime_state, &payload);

        assert_eq!(compat.story_repair_summary(), "沿用单章历史修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "补强冲突".to_string()]
        );
        assert_eq!(compat.story_preserve_strengths(), &["人物张力".to_string()]);
    }

    #[test]
    fn should_restore_single_generation_runtime_compat_options_from_history_only_quality_runtime_context(
    ) {
        let runtime_state = BatchGenerationRequestRuntimeState::new(empty_compat_options(), None);
        let payload = json!({
            "quality_metrics_history": [
                {
                    "overall_score": 81,
                    "repair_guidance": {
                        "summary": "沿用历史质量修复建议",
                        "repair_targets": ["压缩说明", "补强冲突"],
                        "preserve_strengths": ["人物张力"]
                    }
                }
            ]
        });

        let compat =
            resolve_single_generation_runtime_compat_options_from_seed(&runtime_state, &payload);

        assert_eq!(compat.story_repair_summary(), "沿用历史质量修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "补强冲突".to_string()]
        );
        assert_eq!(compat.story_preserve_strengths(), &["人物张力".to_string()]);
    }
}
