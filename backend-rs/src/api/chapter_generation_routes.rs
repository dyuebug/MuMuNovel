use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Json, Sse},
    routing::post,
    Router,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::api::chapters_error_mapper::map_single_chapter_generation_request_error;
use crate::config::AppConfig;
use crate::services::auth::Claims;
use crate::services::chapter_candidate_route_gateway_service::{
    build_chapter_candidate_route_gateway_config_from_app_config,
    ChapterCandidateRouteGatewayConfig,
};
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationRouteRequest;
use crate::services::chapter_single_generation_runtime_restore_workflow_service::SingleGenerationBackgroundWriteWorkflowEntry;
use crate::services::chapter_single_generation_stream_workflow_service::create_owned_single_generation_stream;
use crate::utils::sse::default_sse_keep_alive;

const GENERATE_STREAM_ROUTE: &str = "/chapters/{chapter_id}/generate-stream";
const GENERATE_BACKGROUND_ROUTE: &str = "/chapters/{chapter_id}/generate-background";

pub(crate) fn build_chapter_single_generation_route_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_routes",
        "scope": "single_generation_stream_and_background_route_group",
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
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "route_contract": {
            "stream": GENERATE_STREAM_ROUTE,
            "background": GENERATE_BACKGROUND_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "generate_chapter_content_stream",
                "generate_chapter_content_background"
            ],
            "workflow_consumers": [
                "create_owned_single_generation_stream",
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
            ],
            "error_mapping": [
                "map_single_chapter_generation_error",
                "map_single_chapter_generation_request_error"
            ],
            "gateway_config": [
                "build_single_generation_route_gateway_config",
                "stream route consumes AppConfig candidate gateway config",
                "background route consumes AppConfig candidate gateway config"
            ],
            "response_modes": [
                "SSE stream with default keep-alive",
                "JSON background task payload"
            ]
        },
        "active_consumers": [
            "router::chapters_routes",
            "deploy/strangler-gateway-probes.json",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "readiness_evidence": [
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-single-generation-fixture-import-project-business-rust",
            "chapter-single-generation-fixture-list-chapter-business-rust",
            "chapter-single-generation-configure-mock-openai-business-rust",
            "chapter-single-generation-stream-business-rust",
            "chapter-single-generation-background-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-single-generation-owner",
            "business_probes": [
                "chapter-single-generation-active-gateway-smoke-rust",
                "chapter-single-generation-fixture-import-project-business-rust",
                "chapter-single-generation-fixture-list-chapter-business-rust",
                "chapter-single-generation-configure-mock-openai-business-rust",
                "chapter-single-generation-stream-business-rust",
                "chapter-single-generation-background-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-single-generation-owner",
            "readiness_probe_count": 6,
            "business_probe_count": 3,
            "auth_guard_probe_count": 0,
            "fixture_probe_count": 3,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "aggregate source-map has been repointed to the Rust owner chain; final physical deletion still requires a separate same-round approval and rollback policy",
        "migration_policy": "Single chapter generation business smoke is covered by phase5-single-generation-owner; the aggregate Python route/compat source-map has been repointed while the stream and wiring shells remain frozen as rollback/source-map material, and final physical deletion still requires a separate same-round approval.",
        "validation_boundary": [
            "cargo test api::chapter_generation_routes",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-single-generation-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "deployment_owner": "deploy/nginx/mumunovel.conf",
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "python_route_files_status": "source_map_only_for_single_generation_active_traffic",
            "python_bootstrap_status": "lazy_imported_and_registered_for_explicit_gateway_rollback_only",
            "source_map_freeze_status": "frozen_source_map_rollback_only",
            "source_map_physical_closeout_action": "repoint_aggregate_and_freeze_stream_shells",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "explicit delete approval for the repointed aggregate source-map shell",
                "stream and wiring source-map shells remain frozen until a dedicated delete round"
            ],
            "rollback_files": [
                "backend/app/api/chapter_generation_routes.py",
                "backend/app/api/chapters.py",
                "backend/app/services/chapter_generation/stream/entry_service.py",
                "backend/app/services/chapter_generation/stream/service.py",
                "backend/app/services/chapter_generation/stream/wiring_service.py"
            ]
        }
    })
}

fn build_single_generation_route_gateway_config(
    config: &AppConfig,
) -> ChapterCandidateRouteGatewayConfig {
    build_chapter_candidate_route_gateway_config_from_app_config(config)
}

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<AppConfig>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    body: Option<Json<SingleChapterGenerationRouteRequest>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let gateway_config = build_single_generation_route_gateway_config(&config);
    let result = SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload(
        &db,
        &chapter_id,
        &claims.sub,
        body.map(|Json(payload)| payload).unwrap_or_default(),
        gateway_config,
        Utc::now().naive_utc(),
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Json(result))
}

async fn generate_chapter_content_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<AppConfig>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    body: Option<Json<SingleChapterGenerationRouteRequest>>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream = create_owned_single_generation_stream(
        db.clone(),
        claims.sub.clone(),
        chapter_id,
        body.map(|Json(payload)| payload).unwrap_or_default(),
        build_single_generation_route_gateway_config(&config),
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(GENERATE_STREAM_ROUTE, post(generate_chapter_content_stream))
        .route(
            GENERATE_BACKGROUND_ROUTE,
            post(generate_chapter_content_background),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_route_gateway_config, map_single_chapter_generation_request_error,
        SingleChapterGenerationRouteRequest, GENERATE_BACKGROUND_ROUTE, GENERATE_STREAM_ROUTE,
    };
    use crate::config::{AppConfig, AppRuntimeMode};
    use crate::services::chapter_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_single_generation_prepare_service::PrepareSingleChapterGenerationRequestError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn should_publish_single_generation_route_owner_contract() {
        let contract = super::build_chapter_single_generation_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_generation_routes");
        assert_eq!(
            contract["scope"],
            "single_generation_stream_and_background_route_group"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/chapter_generation_routes.py"
        );
        assert_eq!(
            contract["python_source_map"][2],
            "backend/app/services/chapter_generation/route_wiring_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_generation_routes.rs"
        );
        assert_eq!(contract["route_contract"]["stream"], GENERATE_STREAM_ROUTE);
        assert_eq!(
            contract["route_contract"]["background"],
            GENERATE_BACKGROUND_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][0],
            "generate_chapter_content_stream"
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][1],
            "generate_chapter_content_background"
        );
        assert_eq!(
            contract["behavior_contract"]["workflow_consumers"][1],
            "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["gateway_config"][0],
            "build_single_generation_route_gateway_config"
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter_single_generation_active_gateway_smoke_service"
        );
        assert_eq!(contract["readiness_evidence"].as_array().unwrap().len(), 6);
        assert_eq!(
            contract["readiness_evidence"][5],
            "chapter-single-generation-background-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("single-generation business probes should be present")
                .len(),
            6
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            6
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 3);
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            0
        );
        assert_eq!(contract["business_smoke_status"]["fixture_probe_count"], 3);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "aggregate source-map has been repointed to the Rust owner chain; final physical deletion still requires a separate same-round approval and rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-single-generation-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(false)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "lazy_imported_and_registered_for_explicit_gateway_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "frozen_source_map_rollback_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "repoint_aggregate_and_freeze_stream_shells"
        );
    }

    #[test]
    fn should_keep_whole_single_generation_route_file_owned_by_rust() {
        assert_eq!(
            [GENERATE_STREAM_ROUTE, GENERATE_BACKGROUND_ROUTE],
            [
                "/chapters/{chapter_id}/generate-stream",
                "/chapters/{chapter_id}/generate-background",
            ],
        );
    }

    #[test]
    fn should_build_single_generation_route_gateway_config_from_app_config() {
        let mut config = app_config();
        config.chapter_candidate_rust_executor_enabled = true;
        config.chapter_candidate_rust_executor_fallback_on_error = false;
        config.chapter_candidate_rust_executor_disabled_reason =
            "  operator enabled Rust candidate route  ".to_string();
        config.chapter_candidate_rust_executor_rollback_boundary =
            "  legacy_single_generation_direct_ai  ".to_string();

        let gateway_config = build_single_generation_route_gateway_config(&config);

        assert!(gateway_config.rust_executor_enabled);
        assert!(!gateway_config.fallback_on_rust_error);
        assert_eq!(
            gateway_config.disabled_reason.as_deref(),
            Some("operator enabled Rust candidate route")
        );
        assert_eq!(
            gateway_config.rollback_boundary,
            "legacy_single_generation_direct_ai"
        );
    }

    #[test]
    fn should_keep_single_chapter_generation_route_payload_contract() {
        let route_request = SingleChapterGenerationRouteRequest {
            style_id: Some(7),
            target_word_count: Some(1800),
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
        };
        assert_eq!(route_request.style_id, Some(7));
        assert_eq!(route_request.target_word_count, Some(1800));
        assert_eq!(route_request.model.as_deref(), Some("gpt-test"));
        assert_eq!(route_request.enable_analysis, Some(true));
        assert_eq!(route_request.enable_mcp, Some(true));
        assert_eq!(route_request.enable_web_research, Some(true));
        assert_eq!(
            route_request.web_research_query.as_deref(),
            Some("hero backstory")
        );
        assert_eq!(
            route_request.narrative_perspective.as_deref(),
            Some("third_person")
        );
        assert_eq!(route_request.creative_mode.as_deref(), Some("balanced"));
        assert_eq!(route_request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(route_request.plot_stage.as_deref(), Some("development"));
        assert_eq!(route_request.story_creation_brief.as_deref(), Some("brief"));
        assert_eq!(route_request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(route_request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(
            route_request.story_repair_summary.as_deref(),
            Some("repair")
        );
        assert_eq!(
            route_request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            route_request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
    }

    #[test]
    fn should_accept_empty_single_chapter_generation_route_payload() {
        let request = SingleChapterGenerationRouteRequest::default();

        assert_eq!(request.style_id, None);
        assert_eq!(request.target_word_count, None);
        assert_eq!(request.model, None);
        assert_eq!(request.enable_analysis, None);
        assert_eq!(request.enable_mcp, None);
        assert_eq!(request.enable_web_research, None);
    }

    #[test]
    fn single_generation_background_not_found_or_access_denied_remains_404() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Chapter not found or access denied" })
        );
    }

    #[test]
    fn single_generation_background_config_error_maps_to_bad_request() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Config("model missing".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "model missing" }));
    }

    #[test]
    fn single_generation_background_prerequisites_blocked_maps_to_bad_request() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(
                "前置章节尚未完成: 2 章".to_string(),
            ),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "前置章节尚未完成: 2 章" }));
    }

    #[test]
    fn single_generation_target_word_count_bounds_match_python_contract() {
        let lower_response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall,
        );
        let upper_response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge,
        );

        assert_eq!(lower_response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            lower_response.1 .0,
            json!({ "detail": "target_word_count must be greater than or equal to 500" })
        );
        assert_eq!(upper_response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            upper_response.1 .0,
            json!({ "detail": "target_word_count must be less than or equal to 10000" })
        );
    }

    #[test]
    fn single_generation_generation_choice_errors_remain_bad_request() {
        let cases = [
            (
                PrepareSingleChapterGenerationRequestError::InvalidCreativeMode,
                "creative_mode is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidStoryFocus,
                "story_focus is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidPlotStage,
                "plot_stage is invalid",
            ),
            (
                PrepareSingleChapterGenerationRequestError::InvalidQualityPreset,
                "quality_preset is invalid",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_single_chapter_generation_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn single_generation_generation_text_length_errors_remain_bad_request() {
        let cases = [
            (
                PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong,
                "story_creation_brief must be at most 1200 characters",
            ),
            (
                PrepareSingleChapterGenerationRequestError::QualityNotesTooLong,
                "quality_notes must be at most 600 characters",
            ),
        ];

        for (error, expected_detail) in cases {
            let response = map_single_chapter_generation_request_error(error);

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }
    }

    #[test]
    fn single_generation_background_not_found_remains_404() {
        let response = map_single_chapter_generation_request_error(
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Chapter not found" }));
    }

    fn app_config() -> AppConfig {
        AppConfig {
            app_host: "127.0.0.1".to_string(),
            app_port: 8001,
            app_name: "MuMuNovel".to_string(),
            app_version: "0.1.0-rs".to_string(),
            database_url: "sqlite::memory:".to_string(),
            database_pool_size: 50,
            enable_startup_schema_sync: false,
            log_level: "info".to_string(),
            debug: true,
            runtime_mode: AppRuntimeMode::Development,
            cors_origins: "*".to_string(),
            jwt_secret: "secret".to_string(),
            static_dir: "../backend/static".to_string(),
            local_auth_enabled: true,
            local_auth_username: String::new(),
            local_auth_password: String::new(),
            local_auth_display_name: "本地管理员".to_string(),
            linuxdo_client_id: String::new(),
            linuxdo_client_secret: String::new(),
            linuxdo_redirect_uri: String::new(),
            frontend_url: "http://localhost".to_string(),
            session_expire_minutes: 120,
            session_refresh_threshold_minutes: 30,
            chapter_candidate_rust_executor_enabled: false,
            chapter_candidate_rust_executor_fallback_on_error: true,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: "python_candidate_executor_fallback"
                .to_string(),
        }
    }
}
