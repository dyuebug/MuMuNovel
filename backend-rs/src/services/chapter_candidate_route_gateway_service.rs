// Route/deployment gateway owner for the chapter candidate executor cutover.
// It maps deployment config into the rollback-aware production adapter, so
// routes do not need to rebuild cutover and fallback decisions locally.
#![allow(dead_code)]

use std::{future::Future, pin::Pin};

use serde_json::Value;

use crate::ai::config::AIConfig;
use crate::config::AppConfig;
use crate::services::chapter_candidate_executor_production_adapter_service::{
    execute_chapter_candidate_production_adapter,
    execute_chapter_candidate_production_adapter_with_executor,
    ChapterCandidateProductionAdapterConfig, ChapterCandidateProductionAdapterOutput,
    ChapterCandidateProductionFallbackContext,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_quality_adapter_service::{
    CandidateQualityGatePlanInput, CandidateQualityRuntimeContextBuildInput,
    CandidateStoryQualityMetricsInput, ChapterCandidateQualityAdapter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRouteGatewayConfig {
    pub(crate) rust_executor_enabled: bool,
    pub(crate) fallback_on_rust_error: bool,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) rollback_boundary: String,
}

pub(crate) fn build_chapter_candidate_route_gateway_config_from_app_config(
    config: &AppConfig,
) -> ChapterCandidateRouteGatewayConfig {
    ChapterCandidateRouteGatewayConfig {
        rust_executor_enabled: config.chapter_candidate_rust_executor_enabled,
        fallback_on_rust_error: config.chapter_candidate_rust_executor_fallback_on_error,
        disabled_reason: non_empty_string(&config.chapter_candidate_rust_executor_disabled_reason),
        rollback_boundary: non_empty_string(
            &config.chapter_candidate_rust_executor_rollback_boundary,
        )
        .unwrap_or_else(|| "python_candidate_executor_fallback".to_string()),
    }
}

pub(crate) fn build_chapter_candidate_production_adapter_config_from_route_gateway(
    config: &ChapterCandidateRouteGatewayConfig,
) -> ChapterCandidateProductionAdapterConfig {
    ChapterCandidateProductionAdapterConfig {
        rust_executor_enabled: config.rust_executor_enabled,
        fallback_on_rust_error: config.fallback_on_rust_error,
        disabled_reason: config.disabled_reason.clone(),
        rollback_boundary: config.rollback_boundary.clone(),
    }
}

pub(crate) async fn execute_chapter_candidate_route_gateway<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    execute_chapter_candidate_production_adapter(
        request,
        ai_config,
        quality_adapter,
        build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config),
        python_fallback_fn,
    )
    .await
}

pub(crate) async fn execute_chapter_candidate_route_gateway_with_executor<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
    RustExecutor,
    PythonFallback,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    rust_executor_fn: RustExecutor,
    python_fallback_fn: PythonFallback,
) -> Result<ChapterCandidateProductionAdapterOutput, String>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
    RustExecutor: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        AIConfig,
        ChapterCandidateQualityAdapter<
            BuildQualityRuntimeContext,
            ComputeStoryQualityMetrics,
            ResolveQualityGatePlan,
        >,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
    PythonFallback: for<'request> FnOnce(
        &'request mut ChapterCandidateExecutorRequest,
        ChapterCandidateProductionFallbackContext,
    ) -> Pin<
        Box<dyn Future<Output = Result<Value, String>> + Send + 'request>,
    >,
{
    execute_chapter_candidate_production_adapter_with_executor(
        request,
        ai_config,
        quality_adapter,
        build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config),
        rust_executor_fn,
        python_fallback_fn,
    )
    .await
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_production_adapter_config_from_route_gateway,
        build_chapter_candidate_route_gateway_config_from_app_config,
        execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
    };
    use crate::ai::config::AIConfig;
    use crate::config::{AppConfig, AppRuntimeMode};
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
    use crate::services::chapter_candidate_quality_adapter_service::{
        build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
    };

    #[test]
    fn should_build_gateway_config_from_app_config_cutover_flags() {
        let mut app_config = app_config();
        app_config.chapter_candidate_rust_executor_enabled = true;
        app_config.chapter_candidate_rust_executor_fallback_on_error = false;
        app_config.chapter_candidate_rust_executor_disabled_reason =
            "  smoke probe owns fallback  ".to_string();
        app_config.chapter_candidate_rust_executor_rollback_boundary =
            "  chapters.py candidate fallback  ".to_string();

        let gateway_config =
            build_chapter_candidate_route_gateway_config_from_app_config(&app_config);

        assert!(gateway_config.rust_executor_enabled);
        assert!(!gateway_config.fallback_on_rust_error);
        assert_eq!(
            gateway_config.disabled_reason.as_deref(),
            Some("smoke probe owns fallback")
        );
        assert_eq!(
            gateway_config.rollback_boundary,
            "chapters.py candidate fallback"
        );
    }

    #[test]
    fn should_default_blank_gateway_reason_and_boundary() {
        let mut app_config = app_config();
        app_config.chapter_candidate_rust_executor_disabled_reason = " ".to_string();
        app_config.chapter_candidate_rust_executor_rollback_boundary = "\n".to_string();

        let gateway_config =
            build_chapter_candidate_route_gateway_config_from_app_config(&app_config);
        let production_config =
            build_chapter_candidate_production_adapter_config_from_route_gateway(&gateway_config);

        assert!(gateway_config.disabled_reason.is_none());
        assert_eq!(
            gateway_config.rollback_boundary,
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            production_config.rollback_boundary,
            "python_candidate_executor_fallback"
        );
    }

    #[tokio::test]
    async fn should_execute_rust_path_through_route_gateway_when_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_route_gateway_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "route gateway fallback".to_string(),
            },
            |request, _ai_config, _quality_adapter| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"gateway": "rust"}));
                    Ok(json!({"path": "rust"}))
                })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect("route gateway output");

        assert!(!output.fallback_applied);
        assert_eq!(output.result["path"], "rust");
        assert_eq!(request.runtime_state, Some(json!({"gateway": "rust"})));
    }

    #[tokio::test]
    async fn should_execute_python_fallback_through_route_gateway_when_disabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_route_gateway_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("gateway cutover disabled".to_string()),
                rollback_boundary: "route gateway fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("rust executor should not run".to_string()) })
            },
            |_request, context| {
                Box::pin(async move {
                    Ok(json!({
                        "path": "python",
                        "reason": context.reason,
                        "rollback_boundary": context.rollback_boundary,
                    }))
                })
            },
        )
        .await
        .expect("route gateway fallback output");

        assert!(output.fallback_applied);
        assert_eq!(output.result["path"], "python");
        assert_eq!(output.result["reason"], "gateway cutover disabled");
        assert_eq!(output.result["rollback_boundary"], "route gateway fallback");
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
            chapter_candidate_rust_executor_enabled: true,
            chapter_candidate_rust_executor_fallback_on_error: true,
            chapter_candidate_rust_executor_disabled_reason: String::new(),
            chapter_candidate_rust_executor_rollback_boundary: "python_candidate_executor_fallback"
                .to_string(),
        }
    }

    fn executor_request() -> ChapterCandidateExecutorRequest {
        ChapterCandidateExecutorRequest {
            base_generate_kwargs: Map::from_iter([("prompt".to_string(), json!("PROMPT"))]),
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates: 1,
            runtime_state: None,
        }
    }

    fn quality_adapter(
    ) -> crate::services::chapter_candidate_quality_adapter_service::ChapterCandidateQualityAdapter<
        impl FnMut(
            crate::services::chapter_candidate_quality_adapter_service::CandidateQualityRuntimeContextBuildInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_quality_adapter_service::CandidateStoryQualityMetricsInput,
        ) -> Value,
        impl FnMut(
            crate::services::chapter_candidate_quality_adapter_service::CandidateQualityGatePlanInput,
        ) -> Value,
    >{
        build_chapter_candidate_quality_adapter(
            ChapterCandidateQualityAdapterContext {
                story_packet: json!({"packet": true}),
                project: json!({"world_rules": "rules"}),
                chapter: json!({"id": "chapter-1"}),
                chapter_context: json!({"chapter_outline": "outline"}),
                target_word_count: 1200,
                generation_intent: json!({"mode": "draft"}),
                retry_count: 0,
                max_retries: 1,
                current_story_repair_payload: None,
                scope: "chapter".to_string(),
                log_prefix: "Chapter".to_string(),
            },
            |_input| json!({"runtime": "context"}),
            |_input| json!({"overall_score": 90.0}),
            |_input| json!({"action": "continue"}),
        )
    }
}
