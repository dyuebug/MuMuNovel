use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

pub(crate) mod request_prepare_owner;
pub(crate) mod research_payload_owner;
pub(crate) mod task_view_payload_owner;

pub(crate) use self::request_prepare_owner::{
    load_single_chapter_generation_target,
    prepare_single_chapter_generation_execution_config_from_runtime_state,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
    SingleChapterGenerationRouteRequest, SingleChapterGenerationTarget,
};
use self::research_payload_owner::build_single_chapter_research_payload_owner_contract;
#[cfg(test)]
pub(crate) use self::task_view_payload_owner::single_generation_pending_stage_code;
pub(crate) use self::task_view_payload_owner::{
    build_single_generation_runtime_payload_base,
    build_single_generation_task_view_payload_from_task_state,
    build_single_generation_task_view_payload_owner_contract,
    estimated_single_generation_task_minutes, single_generation_active_task_statuses,
};
use crate::models::chapter;
use crate::services::chapter_access_service::build_chapter_generation_access_owner_contract;
pub(crate) use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_execution_contract_service::{
    build_generation_execution_config_owner_contract,
    build_single_generation_execution_contract_owner_contract,
};
use crate::services::chapter_generation_prompt_service::{
    build_prompt_context_provider_owner_contract, build_quality_profile_owner_contract,
};
const MIN_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 500;
const MAX_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 10_000;
const MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH: usize = 1200;
const MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH: usize = 600;
const SINGLE_GENERATION_CREATIVE_MODE_VALUES: &[&str] = &[
    "balanced",
    "hook",
    "emotion",
    "suspense",
    "relationship",
    "payoff",
];
const SINGLE_GENERATION_STORY_FOCUS_VALUES: &[&str] = &[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
];
const SINGLE_GENERATION_PLOT_STAGE_VALUES: &[&str] = &["development", "climax", "ending"];
const SINGLE_GENERATION_QUALITY_PRESET_VALUES: &[&str] = &[
    "balanced",
    "plot_drive",
    "immersive",
    "emotion_drama",
    "clean_prose",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGenerationPrerequisiteCheck {
    pub(crate) can_generate: bool,
    pub(crate) error_message: String,
    pub(crate) previous_chapters: Vec<chapter::Model>,
}

pub(crate) fn build_chapter_generation_prerequisite_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_prepare_service",
        "scope": "prerequisite_owner",
        "python_source_map": [
            "backend/migrator_app/models/chapter.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_query_service.rs"
        ],
        "behavior_contract": {
            "entrypoint": "check_chapter_generation_prerequisites",
            "first_chapter": {
                "can_generate": true,
                "error_message": "",
                "previous_chapters": []
            },
            "previous_chapter_query": [
                "same_project_id",
                "chapter_number_lt_current",
                "ordered_by_chapter_number_asc"
            ],
            "incomplete_content_rule": "missing_or_trimmed_empty_content_blocks_generation",
            "error_message_template": "前置章节尚未完成: <numbers> 章",
            "route_payload_consumers": [
                "load_can_generate_payload",
                "load_single_chapter_generation_target"
            ],
            "request_normalization_helpers": [
                "normalize_optional_single_generation_request_string",
                "is_valid_optional_choice",
                "is_valid_optional_text_length",
                "normalize_single_generation_web_research_enabled"
            ],
            "request_bounds": {
                "story_creation_brief_max_chars": MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
                "quality_notes_max_chars": MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH,
                "creative_mode": SINGLE_GENERATION_CREATIVE_MODE_VALUES,
                "story_focus": SINGLE_GENERATION_STORY_FOCUS_VALUES,
                "plot_stage": SINGLE_GENERATION_PLOT_STAGE_VALUES,
                "quality_preset": SINGLE_GENERATION_QUALITY_PRESET_VALUES
            }
        },
        "validation_boundary": [
            "cargo test chapter_single_generation_prepare_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "rollback_boundary": "chapter_generation_prerequisite_query_source_map"
    })
}

pub(crate) async fn check_chapter_generation_prerequisites(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
) -> Result<ChapterGenerationPrerequisiteCheck, String> {
    if chapter_model.chapter_number == 1 {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: String::new(),
            previous_chapters: Vec::new(),
        });
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let incomplete_numbers = previous_chapters
        .iter()
        .filter(|chapter| {
            chapter
                .content
                .as_ref()
                .map(|content| content.trim().is_empty())
                .unwrap_or(true)
        })
        .map(|chapter| chapter.chapter_number.to_string())
        .collect::<Vec<_>>();

    if !incomplete_numbers.is_empty() {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: format!("前置章节尚未完成: {} 章", incomplete_numbers.join(", ")),
            previous_chapters,
        });
    }

    Ok(ChapterGenerationPrerequisiteCheck {
        can_generate: true,
        error_message: String::new(),
        previous_chapters,
    })
}

pub(crate) fn normalize_optional_single_generation_request_string(
    value: Option<String>,
) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn is_valid_optional_choice(value: Option<&str>, allowed_values: &[&str]) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| allowed_values.contains(&value))
        .unwrap_or(true)
}

pub(crate) fn is_valid_optional_text_length(value: Option<&str>, max_chars: usize) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() <= max_chars)
        .unwrap_or(true)
}

pub(crate) fn normalize_single_generation_web_research_enabled(
    enabled: Option<bool>,
    default_enabled: bool,
) -> bool {
    enabled.unwrap_or(default_enabled)
}

pub(crate) fn build_single_generation_prepare_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_prepare_service",
        "scope": "single_generation_route_request_target_prerequisite_execution_config_and_task_view_payload",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "route_request_owner": "SingleChapterGenerationRouteRequest",
            "generation_request_owner": "SingleChapterGenerationRequest",
            "target_owner": "SingleChapterGenerationTarget",
            "request_fields": [
                "style_id",
                "target_word_count",
                "model",
                "enable_analysis",
                "enable_mcp",
                "enable_web_research",
                "web_research_query",
                "narrative_perspective",
                "creative_mode",
                "story_focus",
                "plot_stage",
                "story_creation_brief",
                "quality_preset",
                "quality_notes",
                "story_repair_summary",
                "story_repair_targets",
                "story_preserve_strengths"
            ],
            "bounds": {
                "target_word_count_min": MIN_SINGLE_GENERATION_TARGET_WORD_COUNT,
                "target_word_count_max": MAX_SINGLE_GENERATION_TARGET_WORD_COUNT,
                "story_creation_brief_max_chars": MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
                "quality_notes_max_chars": MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH
            },
            "choice_fields": {
                "creative_mode": SINGLE_GENERATION_CREATIVE_MODE_VALUES,
                "story_focus": SINGLE_GENERATION_STORY_FOCUS_VALUES,
                "plot_stage": SINGLE_GENERATION_PLOT_STAGE_VALUES,
                "quality_preset": SINGLE_GENERATION_QUALITY_PRESET_VALUES
            },
            "prepare_entrypoints": [
                "load_single_chapter_generation_target",
                "prepare_single_chapter_generation_execution_config_from_runtime_state",
                "SingleChapterGenerationRequest::compat_options_with_web_research_default",
                "SingleChapterGenerationRequest::validate_request_bounds"
            ],
            "task_view_payload": [
                "build_single_generation_runtime_payload_base",
                "build_single_generation_task_view_payload_from_task_state",
                "single_generation_pending_stage_code",
                "single_generation_active_task_statuses"
            ],
            "strict_schema": {
                "deny_unknown_fields": true,
                "explicit_null_rejected_for_default_flags": [
                    "enable_analysis",
                    "enable_mcp"
                ],
                "empty_strings_trimmed_to_none": true
            }
        },
        "active_consumers": [
            "chapter_generation_routes",
            "chapter_single_generation_stream_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter-single-generation-active-gateway-smoke-rust"
        ],
        "chapter_generation_access_owner_contract": build_chapter_generation_access_owner_contract(),
        "prerequisite_owner_contract": build_chapter_generation_prerequisite_owner_contract(),
        "task_view_payload_owner_contract": build_single_generation_task_view_payload_owner_contract(),
        "execution_config_owner_contract": build_generation_execution_config_owner_contract(),
        "execution_contract_owner_contract": build_single_generation_execution_contract_owner_contract(),
        "prompt_context_provider_owner_contract": build_prompt_context_provider_owner_contract(),
        "quality_profile_owner_contract": build_quality_profile_owner_contract(),
        "research_payload_owner_contract": build_single_chapter_research_payload_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_prepare_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-single-generation-owner",
            "route_request_schema_owner": "SingleChapterGenerationRouteRequest",
            "target_prerequisite_owner": "load_single_chapter_generation_target",
            "execution_config_owner": "prepare_single_chapter_generation_execution_config_from_runtime_state",
            "task_view_payload_owner": "build_single_generation_task_view_payload_from_task_state",
            "manifest_probe_count": 6,
            "rust_manifest_probe_count": 6,
            "python_fallback_probe_count": 0,
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "single-generation prepare owner is already free of Python request-schema source maps; surviving Python exit work now lives in prerequisite/query and shared runtime contracts outside this owner",
            "status": "rust_prepare_owner_request_schema_source_map_deleted"
        },
        "rollback_boundary": {
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "source_map_policy": "single_generation_prepare_route_request_schema_source_map_deleted_and_surviving_python_exit_work_moves_to_outside_owner_contracts",
            "python_fallback_removal_ready": true,
            "rollback_files": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::research_payload_owner::build_single_chapter_research_payload_owner_contract;
    use super::{
        build_chapter_generation_prerequisite_owner_contract,
        build_single_generation_prepare_owner_contract,
        build_single_generation_runtime_payload_base,
        build_single_generation_task_view_payload_from_task_state,
        build_single_generation_task_view_payload_owner_contract,
        estimated_single_generation_task_minutes, single_generation_active_task_statuses,
        single_generation_pending_stage_code, ChapterGenerationPrerequisiteCheck,
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationCompatOptions,
        SingleChapterGenerationRequest, SingleChapterGenerationRouteRequest,
        SingleChapterGenerationTarget, MIN_SINGLE_GENERATION_TARGET_WORD_COUNT,
    };
    use crate::models::{batch_generation_task, chapter};
    use crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        build_generation_execution_config_owner_contract,
        build_single_generation_execution_contract_owner_contract,
        normalize_chapter_generation_target_word_count, BatchGenerationRequestRuntimeState,
        SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::{
        build_prompt_context_provider_owner_contract, build_quality_profile_owner_contract,
        PromptContextProviderPayload,
    };
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_runtime_restore_workflow_service::PreparedSingleChapterGenerationRestoredRuntimeLaunch;
    use crate::services::chapter_single_generation_runtime_seed_service::{
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        RestoredSingleGenerationRuntimeState,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn should_publish_single_generation_prepare_owner_contract() {
        let contract = build_single_generation_prepare_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_prepare_service"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["request_fields"]
                .as_array()
                .expect("request fields")
                .len(),
            17
        );
        assert_eq!(
            contract["behavior_contract"]["bounds"]["target_word_count_min"],
            MIN_SINGLE_GENERATION_TARGET_WORD_COUNT
        );
        assert_eq!(
            contract["behavior_contract"]["choice_fields"]["plot_stage"][1],
            "climax"
        );
        assert_eq!(
            contract["behavior_contract"]["strict_schema"]["deny_unknown_fields"],
            true
        );
        assert_eq!(
            contract["active_consumers"][4],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["prerequisite_owner_contract"]["scope"],
            "prerequisite_owner"
        );
        assert_eq!(
            contract["prerequisite_owner_contract"]["behavior_contract"]["entrypoint"],
            "check_chapter_generation_prerequisites"
        );
        assert_eq!(
            contract["chapter_generation_access_owner_contract"]["owner"],
            "chapter_access_service"
        );
        assert_eq!(
            contract["chapter_generation_access_owner_contract"]["behavior_contract"]
                ["entrypoints"][1],
            "load_accessible_chapter_for_generation"
        );
        assert_eq!(
            contract["chapter_generation_access_owner_contract"]["service_runtime_closeout_status"]
                ["owner_profiles"][1],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["owner"],
            "chapter_single_generation_prepare_service::task_view_payload_owner"
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            contract["task_view_payload_owner_contract"]["python_source_map"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["execution_config_owner_contract"]["owner"],
            build_generation_execution_config_owner_contract()["owner"]
        );
        assert_eq!(
            contract["execution_contract_owner_contract"]["owner"],
            build_single_generation_execution_contract_owner_contract()["owner"]
        );
        assert_eq!(
            contract["prompt_context_provider_owner_contract"]["owner"],
            build_prompt_context_provider_owner_contract()["owner"]
        );
        assert_eq!(
            contract["quality_profile_owner_contract"]["owner"],
            build_quality_profile_owner_contract()["owner"]
        );
        assert_eq!(
            contract["research_payload_owner_contract"]["owner"],
            "chapter_single_generation_prepare_service::research_payload_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knobs"][0],
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["route_request_schema_owner"],
            "SingleChapterGenerationRouteRequest"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["target_prerequisite_owner"],
            "load_single_chapter_generation_target"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["execution_config_owner"],
            "prepare_single_chapter_generation_execution_config_from_runtime_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["task_view_payload_owner"],
            "build_single_generation_task_view_payload_from_task_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "single-generation prepare owner is already free of Python request-schema source maps; surviving Python exit work now lives in prerequisite/query and shared runtime contracts outside this owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_prepare_owner_request_schema_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "single_generation_prepare_route_request_schema_source_map_deleted_and_surviving_python_exit_work_moves_to_outside_owner_contracts"
        );
    }

    fn prerequisite_test_chapter(chapter_number: i32, content: Option<&str>) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            chapter_number,
            title: format!("第{chapter_number}章"),
            content: content.map(str::to_string),
            summary: None,
            word_count: 0,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_publish_chapter_generation_prerequisite_owner_contract() {
        let contract = build_chapter_generation_prerequisite_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_prepare_service"
        );
        assert_eq!(contract["scope"], "prerequisite_owner");
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/chapter.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoint"],
            "check_chapter_generation_prerequisites"
        );
        assert_eq!(
            contract["behavior_contract"]["first_chapter"]["can_generate"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["previous_chapter_query"][2],
            "ordered_by_chapter_number_asc"
        );
        assert_eq!(
            contract["behavior_contract"]["error_message_template"],
            "前置章节尚未完成: <numbers> 章"
        );
        assert_eq!(
            contract["rollback_boundary"],
            "chapter_generation_prerequisite_query_source_map"
        );
    }

    #[test]
    fn should_publish_single_generation_task_view_payload_owner_contract() {
        let contract = build_single_generation_task_view_payload_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_prepare_service::task_view_payload_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_single_generation_runtime_payload_base"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "build_single_generation_task_view_payload_from_task_state"
        );
        assert_eq!(
            contract["behavior_contract"]["payload_fields"][9],
            "candidate_gateway"
        );
    }

    #[test]
    fn should_keep_single_generation_research_payload_owner_contract_nested_under_prepare_owner() {
        let contract = build_single_chapter_research_payload_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_prepare_service::research_payload_owner"
        );
        assert_eq!(contract["behavior_contract"]["asset_limit"], json!(2));
    }

    #[test]
    fn should_allow_first_chapter_generation_without_previous_chapters() {
        let result = ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: String::new(),
            previous_chapters: Vec::new(),
        };

        assert!(result.can_generate);
        assert!(result.error_message.is_empty());
        assert!(result.previous_chapters.is_empty());
    }

    #[test]
    fn should_keep_incomplete_previous_chapter_message_contract() {
        let previous_chapters = vec![
            prerequisite_test_chapter(1, Some("第一章正文")),
            prerequisite_test_chapter(2, None),
            prerequisite_test_chapter(3, Some("   ")),
        ];
        let incomplete_numbers = previous_chapters
            .iter()
            .filter(|chapter| {
                chapter
                    .content
                    .as_ref()
                    .map(|content| content.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|chapter| chapter.chapter_number.to_string())
            .collect::<Vec<_>>();

        let result = ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: format!("前置章节尚未完成: {} 章", incomplete_numbers.join(", ")),
            previous_chapters,
        };

        assert!(!result.can_generate);
        assert_eq!(result.error_message, "前置章节尚未完成: 2, 3 章");
        assert_eq!(result.previous_chapters.len(), 3);
    }

    #[test]
    fn should_normalize_single_chapter_generation_target_word_count() {
        assert_eq!(normalize_chapter_generation_target_word_count(None), 3000);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(normalize_chapter_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_keep_single_generation_task_minutes_contract() {
        assert_eq!(estimated_single_generation_task_minutes(3000, false), 2);
        assert_eq!(estimated_single_generation_task_minutes(3000, true), 3);
        assert_eq!(estimated_single_generation_task_minutes(200, false), 1);
    }

    #[test]
    fn should_keep_single_generation_active_statuses_contract() {
        assert_eq!(
            single_generation_active_task_statuses(),
            ["pending", "running"]
        );
        assert_eq!(single_generation_pending_stage_code(), "6.writing.pending");
    }

    #[test]
    fn should_build_single_generation_runtime_payload_base_from_prepare_owner() {
        let payload = build_single_generation_runtime_payload_base(
            "task-1",
            "project-1",
            Some("chapter-1"),
            "pending",
            Some(&json!({"progress": 15})),
            None,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["current_chapter_id"], "chapter-1");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["progress"], 15);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
    }

    #[test]
    fn should_project_candidate_gateway_from_single_generation_runtime_payload_base() {
        let runtime_state = json!({
            "progress": 100,
            "candidate_gateway": {
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "fallback_reason": "rust executor completed",
                "rollback_boundary": "legacy_single_generation_direct_ai",
                "rust_error": null
            }
        });

        let payload = build_single_generation_runtime_payload_base(
            "task-9",
            "project-9",
            Some("chapter-9"),
            "completed",
            Some(&runtime_state),
            None,
        );

        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            payload["candidate_gateway"]["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
    }

    #[test]
    fn should_not_project_invalid_single_generation_candidate_gateway_metadata() {
        let runtime_state = json!({
            "progress": 15,
            "candidate_gateway": "not-an-object"
        });

        let payload = build_single_generation_runtime_payload_base(
            "task-10",
            "project-10",
            Some("chapter-10"),
            "running",
            Some(&runtime_state),
            None,
        );

        assert!(payload.get("candidate_gateway").is_none());
        assert_eq!(payload["checkpoint"]["candidate_gateway"], "not-an-object");
    }

    #[test]
    fn should_build_single_generation_task_view_payload_from_prepare_owner() {
        let task = batch_generation_task::Model {
            id: "task-2".to_string(),
            project_id: "project-2".to_string(),
            user_id: "user-2".to_string(),
            start_chapter_number: 3,
            chapter_count: 1,
            chapter_ids: json!(["chapter-3"]),
            style_id: None,
            target_word_count: 2600,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-3".to_string()),
            current_chapter_number: Some(3),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        let payload = build_single_generation_task_view_payload_from_task_state(
            &task,
            Some(&json!({"phase": "generating", "progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-2");
        assert_eq!(payload["current_chapter_id"], "chapter-3");
        assert_eq!(payload["current_chapter_number"], 3);
        assert_eq!(payload["checkpoint"]["phase"], "generating");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["completed"], 0);
    }

    #[test]
    fn should_load_single_chapter_generation_target_from_request() {
        let request = SingleChapterGenerationRequest {
            style_id: None,
            target_word_count: Some(1800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        };

        assert_eq!(
            normalize_chapter_generation_target_word_count(request.target_word_count),
            1800
        );
    }

    #[test]
    fn should_reject_unknown_single_chapter_generation_route_fields_like_python_schema() {
        let error = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": 1800,
            "unexpected_field": true
        }))
        .expect_err("python ChapterGenerateRequest forbids extra fields");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn should_accept_known_single_chapter_generation_route_fields_with_strict_schema() {
        let request = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": 1800,
            "creative_mode": "hook",
            "quality_notes": "keep pacing tight"
        }))
        .expect("known python ChapterGenerateRequest fields should parse");

        assert_eq!(request.target_word_count, Some(1800));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.quality_notes.as_deref(), Some("keep pacing tight"));
    }

    #[test]
    fn should_reject_single_chapter_generation_route_null_for_non_nullable_python_default_flags() {
        for (field_name, payload) in [
            ("enable_analysis", json!({"enable_analysis": null})),
            ("enable_mcp", json!({"enable_mcp": null})),
        ] {
            let error =
                serde_json::from_value::<SingleChapterGenerationRouteRequest>(payload).unwrap_err();

            assert!(
                error.to_string().contains("invalid type: null"),
                "{field_name} should reject explicit null like Python bool defaults"
            );
        }
    }

    #[test]
    fn should_keep_single_chapter_generation_route_nullable_fields_accepting_null() {
        let request = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": null,
            "enable_web_research": null
        }))
        .expect("Python Optional fields should keep accepting explicit null");

        assert_eq!(request.target_word_count, None);
        assert_eq!(request.enable_web_research, None);
    }

    #[test]
    fn should_apply_single_chapter_generation_python_defaults_when_flags_are_missing() {
        let route_request =
            serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({}))
                .expect("missing route fields should parse");
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.enable_mcp, None);

        let request = route_request.into_generation_request();
        let compat = request.compat_options_with_web_research_default(false);

        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
    }

    #[test]
    fn should_normalize_single_chapter_generation_fields_like_python_schema() {
        let request = SingleChapterGenerationRouteRequest {
            creative_mode: Some(" hook ".to_string()),
            story_focus: Some(" advance_plot ".to_string()),
            plot_stage: Some(" development ".to_string()),
            story_creation_brief: Some(" 强化开场钩子 ".to_string()),
            quality_preset: Some(" plot_drive ".to_string()),
            quality_notes: Some(" 压缩说明段 ".to_string()),
            story_repair_summary: Some(" 修复中段节奏 ".to_string()),
            ..Default::default()
        }
        .into_generation_request();

        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            request.story_creation_brief.as_deref(),
            Some("强化开场钩子")
        );
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
        assert_eq!(request.quality_notes.as_deref(), Some("压缩说明段"));
        assert_eq!(
            request.story_repair_summary.as_deref(),
            Some("修复中段节奏")
        );
    }

    #[test]
    fn should_convert_blank_single_chapter_generation_fields_to_none() {
        let request = SingleChapterGenerationRouteRequest {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("\t".to_string()),
            plot_stage: Some("\n".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            story_repair_summary: Some("   ".to_string()),
            ..Default::default()
        }
        .into_generation_request();

        assert!(request.creative_mode.is_none());
        assert!(request.story_focus.is_none());
        assert!(request.plot_stage.is_none());
        assert!(request.story_creation_brief.is_none());
        assert!(request.quality_preset.is_none());
        assert!(request.quality_notes.is_none());
        assert!(request.story_repair_summary.is_none());
    }

    #[test]
    fn should_reject_single_chapter_generation_target_word_count_outside_python_bounds() {
        let too_low = SingleChapterGenerationRequest {
            target_word_count: Some(499),
            ..SingleChapterGenerationRequest::default()
        };
        let too_high = SingleChapterGenerationRequest {
            target_word_count: Some(10_001),
            ..SingleChapterGenerationRequest::default()
        };

        assert!(matches!(
            too_low
                .validate_request_bounds()
                .expect_err("target_word_count below python limit should fail"),
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall
        ));
        assert!(matches!(
            too_high
                .validate_request_bounds()
                .expect_err("target_word_count above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge
        ));
    }

    #[test]
    fn should_reject_single_chapter_generation_invalid_choice_fields() {
        let cases = [
            (
                SingleChapterGenerationRequest {
                    creative_mode: Some("too_fancy".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidCreativeMode,
            ),
            (
                SingleChapterGenerationRequest {
                    story_focus: Some("too_broad".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidStoryFocus,
            ),
            (
                SingleChapterGenerationRequest {
                    plot_stage: Some("middle".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidPlotStage,
            ),
            (
                SingleChapterGenerationRequest {
                    quality_preset: Some("max_quality".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidQualityPreset,
            ),
        ];

        for (request, expected_error) in cases {
            assert_eq!(
                request
                    .validate_request_bounds()
                    .expect_err("invalid generation choice should fail"),
                expected_error
            );
        }
    }

    #[test]
    fn should_reject_single_chapter_generation_text_fields_above_python_limits() {
        let long_brief = SingleChapterGenerationRequest {
            story_creation_brief: Some("a".repeat(1201)),
            ..SingleChapterGenerationRequest::default()
        };
        let long_quality_notes = SingleChapterGenerationRequest {
            quality_notes: Some("b".repeat(601)),
            ..SingleChapterGenerationRequest::default()
        };

        assert_eq!(
            long_brief
                .validate_request_bounds()
                .expect_err("story_creation_brief above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong
        );
        assert_eq!(
            long_quality_notes
                .validate_request_bounds()
                .expect_err("quality_notes above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::QualityNotesTooLong
        );
    }

    #[test]
    fn should_accept_single_chapter_generation_python_request_bounds() {
        let lower_bound_request = SingleChapterGenerationRequest {
            target_word_count: Some(500),
            ..SingleChapterGenerationRequest::default()
        };
        let upper_bound_request = SingleChapterGenerationRequest {
            target_word_count: Some(10_000),
            ..SingleChapterGenerationRequest::default()
        };
        let choice_and_text_request = SingleChapterGenerationRequest {
            target_word_count: Some(3000),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            story_creation_brief: Some("a".repeat(1200)),
            quality_notes: Some("b".repeat(600)),
            ..SingleChapterGenerationRequest::default()
        };
        let blank_choice_and_text_request = SingleChapterGenerationRequest {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("   ".to_string()),
            plot_stage: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            ..SingleChapterGenerationRequest::default()
        };

        lower_bound_request
            .validate_request_bounds()
            .expect("python lower target word count should pass");
        upper_bound_request
            .validate_request_bounds()
            .expect("python upper target word count should pass");
        choice_and_text_request
            .validate_request_bounds()
            .expect("valid python generation choices and text lengths should pass");
        blank_choice_and_text_request
            .validate_request_bounds()
            .expect("blank choices and texts normalize to None in python");
    }

    #[test]
    fn should_keep_single_chapter_generation_execution_input_contract() {
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: SingleChapterGenerationCompatOptions {
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
            },
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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

        assert_eq!(execution_input.target_word_count, 2600);
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .external_assets,
            "[]"
        );
    }

    #[test]
    fn should_keep_single_chapter_generation_target_projection_contract() {
        let chapter_model = chapter::Model {
            id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
            content: Some("content".to_string()),
            summary: Some("summary".to_string()),
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        };

        let target = SingleChapterGenerationTarget::from_model(&chapter_model);

        assert_eq!(target.project_id, "project-1");
        assert_eq!(target.chapter_id, "chapter-7");
        assert_eq!(target.chapter_number, 7);
        assert_eq!(target.title, "Seven");
    }

    #[test]
    fn should_build_single_chapter_generation_request_parts_from_owner() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: Some(7),
            target_word_count: Some(2200),
            model: Some("gpt-test".to_string()),
            enable_analysis: Some(true),
            enable_mcp: Some(true),
            enable_web_research: Some(true),
            web_research_query: Some("hero backstory".to_string()),
            narrative_perspective: Some("third_person".to_string()),
            creative_mode: Some("balanced".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("balanced".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        }
        .into_generation_request();
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-8".to_string(),
            chapter_number: 8,
            title: "Eight".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2200,
            compat_options: request.compat_options_with_web_research_default(false),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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

        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
        assert_eq!(request.enable_analysis, Some(true));
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(true));
        assert_eq!(
            request.web_research_query.as_deref(),
            Some("hero backstory")
        );
        assert_eq!(
            request.narrative_perspective.as_deref(),
            Some("third_person")
        );
        assert_eq!(request.creative_mode.as_deref(), Some("balanced"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(request.story_creation_brief.as_deref(), Some("brief"));
        assert_eq!(request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(request.story_repair_summary.as_deref(), Some("repair"));
        assert_eq!(
            request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
        assert_eq!(chapter_target.chapter_id, "chapter-8");
        assert_eq!(execution_input.target_word_count, 2200);
        assert_eq!(execution_input.compat_options.style_id(), Some(7));
        assert!(execution_input.compat_options.enable_analysis());
        assert!(execution_input.compat_options.enable_mcp());
        assert!(execution_input.compat_options.web_research_enabled());
        assert_eq!(
            execution_input.compat_options.web_research_query(),
            Some("hero backstory")
        );
        assert_eq!(
            execution_input.compat_options.narrative_perspective(),
            "third_person"
        );
        assert_eq!(execution_input.compat_options.creative_mode(), "balanced");
        assert_eq!(execution_input.compat_options.story_focus(), "advance_plot");
        assert_eq!(execution_input.compat_options.plot_stage(), "development");
        assert_eq!(
            execution_input.compat_options.story_creation_brief(),
            "brief"
        );
        assert_eq!(execution_input.compat_options.quality_preset(), "balanced");
        assert_eq!(execution_input.compat_options.quality_notes(), "notes");
        assert_eq!(
            execution_input.compat_options.story_repair_summary(),
            "repair"
        );
        assert_eq!(
            execution_input.compat_options.story_repair_targets(),
            &["target-a".to_string()]
        );
        assert_eq!(
            execution_input.compat_options.story_preserve_strengths(),
            &["strength-a".to_string()]
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .research_query,
            ""
        );
    }

    #[test]
    fn should_project_prepared_single_chapter_generation_restored_launch_owner() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-8".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2200,
                compat_options: SingleChapterGenerationCompatOptions {
                    style_id: Some(7),
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
                },
                execution_config: PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
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
        };
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            SingleChapterGenerationTarget {
                project_id: "project-1".to_string(),
                chapter_id: "chapter-8".to_string(),
                chapter_number: 8,
                title: "Eight".to_string(),
            },
            json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-test"
                }
            }),
            runtime_input,
        );

        assert_eq!(
            restored_launch.startup_snapshot_plan().runtime_state()["batch_request_runtime_state"]
                ["model_override"],
            "gpt-test"
        );

        let runtime_input = restored_launch.clone().into_runtime_launch_input();
        let (chapter_target, startup_snapshot_plan, runtime_input_again) =
            restored_launch.into_parts();

        assert!(matches!(
            runtime_input,
            SingleGenerationRuntimeLaunchInput {
                chapter_id,
                user_id,
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2200,
                    ..
                },
            } if chapter_id == "chapter-8" && user_id == "user-1"
        ));
        assert_eq!(chapter_target.chapter_number, 8);
        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-test"
        );
        assert_eq!(runtime_input_again.execution_input.target_word_count, 2200);
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_from_request_runtime_state_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "第九章".to_string(),
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                story_repair_summary: Some("沿用恢复态摘要".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..Default::default()
            },
            Some("owner-model".to_string()),
        );

        let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            "user-9",
            2800,
            &request_runtime_state,
            PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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
        );

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2800);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "沿用恢复态摘要"
        );
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string()]
        );
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .ai_config
                .provider,
            crate::ai::AIConfig::default().provider
        );
    }

    #[test]
    fn should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner(
    ) {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["request-target".to_string()],
                story_preserve_strengths: vec!["request-strength".to_string()],
                ..Default::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2400,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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
        let restored_runtime_state = RestoredSingleGenerationRuntimeState::from_quality_fragments(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-9"
            }),
            &request_runtime_state,
            ChapterQualityMetricsFragments {
                latest_quality_metrics: Some(json!({"overall_score": 84})),
                history_id: None,
                generated_at: None,
                quality_metrics_summary: Some(json!({
                    "chapter_count": 2,
                    "repair_guidance": {
                        "summary": "restored summary"
                    }
                })),
                quality_metrics_history: Some(json!([
                    {"overall_score": 80},
                    {"overall_score": 84}
                ])),
                quality_metrics_summary_state: Some(json!({"chapter_count": 2})),
            },
            None,
        );

        assert_eq!(
            restored_runtime_state.request_runtime_state(),
            &request_runtime_state
        );

        let (startup_snapshot_plan, runtime_input) = restored_runtime_state
            .into_startup_runtime_launch_parts(
                "chapter-9".to_string(),
                "user-9".to_string(),
                execution_input,
            );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["latest_quality_metrics"]["overall_score"],
            84
        );
        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "request summary"
        );
    }

    #[tokio::test]
    async fn should_prepare_single_chapter_generation_request_from_target_without_reloading_chapter(
    ) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let request = SingleChapterGenerationRequest {
            target_word_count: Some(1800),
            ..SingleChapterGenerationRequest::default()
        };
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "Nine".to_string(),
        };

        let error = PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(
            &db,
            "user-1",
            &request,
            chapter_target,
        )
        .await
        .expect_err("sqlite memory db should fail before any chapter reload path is needed");

        assert!(matches!(
            error,
            PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    #[test]
    fn should_normalize_single_chapter_generation_compat_options_from_request_owner() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: Some(9),
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("reveal_mystery".to_string()),
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: Some("immersive".to_string()),
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(false);

        assert_eq!(compat.style_id(), Some(9));
        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
        assert_eq!(compat.web_research_query(), None);
        assert_eq!(compat.creative_mode(), "hook");
        assert_eq!(compat.story_focus(), "reveal_mystery");
        assert_eq!(compat.quality_preset(), "immersive");
        assert_eq!(compat.story_repair_targets(), &[] as &[String]);
        assert_eq!(compat.story_preserve_strengths(), &[] as &[String]);
    }

    #[test]
    fn should_fallback_to_settings_default_for_single_generation_web_research() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: None,
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(true);

        assert!(compat.web_research_enabled());
    }
}
