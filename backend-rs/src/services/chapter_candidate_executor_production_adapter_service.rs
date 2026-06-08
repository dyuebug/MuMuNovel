// Rollback-aware production adapter for the Rust chapter candidate executor.
// This is the cutover boundary that route/compat code can consume without
// rebuilding Python-style provider, record, and quality closures.
#![allow(dead_code)]

use std::{future::Future, pin::Pin};

use serde_json::Value;

use crate::ai::config::AIConfig;
use crate::services::chapter_candidate_executor_runtime_adapter_service::generate_best_ranked_candidate_with_runtime_quality_adapters;
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_quality_adapter_service::{
    CandidateQualityGatePlanInput, CandidateQualityRuntimeContextBuildInput,
    CandidateStoryQualityMetricsInput, ChapterCandidateQualityAdapter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionAdapterConfig {
    pub(crate) rust_executor_enabled: bool,
    pub(crate) fallback_on_rust_error: bool,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) rollback_boundary: String,
}

impl Default for ChapterCandidateProductionAdapterConfig {
    fn default() -> Self {
        Self {
            rust_executor_enabled: false,
            fallback_on_rust_error: true,
            disabled_reason: None,
            rollback_boundary: "python_candidate_executor_fallback".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChapterCandidateProductionExecutionPath {
    RustCandidateExecutor,
    PythonFallback,
}

pub(crate) fn chapter_candidate_production_execution_path_name(
    path: ChapterCandidateProductionExecutionPath,
) -> &'static str {
    match path {
        ChapterCandidateProductionExecutionPath::RustCandidateExecutor => "rust_candidate_executor",
        ChapterCandidateProductionExecutionPath::PythonFallback => "python_fallback",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionAdapterDecision {
    pub(crate) path: ChapterCandidateProductionExecutionPath,
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateProductionFallbackContext {
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
    pub(crate) rust_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateProductionAdapterOutput {
    pub(crate) result: Value,
    pub(crate) decision: ChapterCandidateProductionAdapterDecision,
    pub(crate) fallback_applied: bool,
    pub(crate) rust_error: Option<String>,
}

pub(crate) fn resolve_chapter_candidate_production_adapter_decision(
    config: &ChapterCandidateProductionAdapterConfig,
) -> ChapterCandidateProductionAdapterDecision {
    if config.rust_executor_enabled {
        return ChapterCandidateProductionAdapterDecision {
            path: ChapterCandidateProductionExecutionPath::RustCandidateExecutor,
            reason: "rust candidate executor enabled by production adapter".to_string(),
            rollback_boundary: config.rollback_boundary.clone(),
        };
    }

    ChapterCandidateProductionAdapterDecision {
        path: ChapterCandidateProductionExecutionPath::PythonFallback,
        reason: config.disabled_reason.clone().unwrap_or_else(|| {
            "rust candidate executor disabled by production adapter".to_string()
        }),
        rollback_boundary: config.rollback_boundary.clone(),
    }
}

pub(crate) async fn execute_chapter_candidate_production_adapter<
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
    config: ChapterCandidateProductionAdapterConfig,
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
    execute_chapter_candidate_production_adapter_with_executor(
        request,
        ai_config,
        quality_adapter,
        config,
        boxed_runtime_quality_adapter_executor::<
            BuildQualityRuntimeContext,
            ComputeStoryQualityMetrics,
            ResolveQualityGatePlan,
        >,
        python_fallback_fn,
    )
    .await
}

pub(crate) async fn execute_chapter_candidate_production_adapter_with_executor<
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
    config: ChapterCandidateProductionAdapterConfig,
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
    let decision = resolve_chapter_candidate_production_adapter_decision(&config);
    if decision.path == ChapterCandidateProductionExecutionPath::PythonFallback {
        let fallback_context = ChapterCandidateProductionFallbackContext {
            reason: decision.reason.clone(),
            rollback_boundary: decision.rollback_boundary.clone(),
            rust_error: None,
        };
        let result = python_fallback_fn(request, fallback_context).await?;
        return Ok(ChapterCandidateProductionAdapterOutput {
            result,
            decision,
            fallback_applied: true,
            rust_error: None,
        });
    }

    match rust_executor_fn(request, ai_config, quality_adapter).await {
        Ok(result) => Ok(ChapterCandidateProductionAdapterOutput {
            result,
            decision,
            fallback_applied: false,
            rust_error: None,
        }),
        Err(error) if config.fallback_on_rust_error => {
            let fallback_decision = ChapterCandidateProductionAdapterDecision {
                path: ChapterCandidateProductionExecutionPath::PythonFallback,
                reason: format!(
                    "rust candidate executor failed; python fallback selected: {error}"
                ),
                rollback_boundary: config.rollback_boundary,
            };
            let fallback_context = ChapterCandidateProductionFallbackContext {
                reason: fallback_decision.reason.clone(),
                rollback_boundary: fallback_decision.rollback_boundary.clone(),
                rust_error: Some(error.clone()),
            };
            let result = python_fallback_fn(request, fallback_context).await?;
            Ok(ChapterCandidateProductionAdapterOutput {
                result,
                decision: fallback_decision,
                fallback_applied: true,
                rust_error: Some(error),
            })
        }
        Err(error) => Err(error),
    }
}

fn boxed_runtime_quality_adapter_executor<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>(
    request: &mut ChapterCandidateExecutorRequest,
    ai_config: AIConfig,
    quality_adapter: ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >,
) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send + '_>>
where
    BuildQualityRuntimeContext:
        FnMut(CandidateQualityRuntimeContextBuildInput) -> Value + Send + 'static,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value + Send + 'static,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value + Send + 'static,
{
    Box::pin(
        generate_best_ranked_candidate_with_runtime_quality_adapters(
            request,
            ai_config,
            quality_adapter,
        ),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{
        execute_chapter_candidate_production_adapter_with_executor,
        resolve_chapter_candidate_production_adapter_decision,
        ChapterCandidateProductionAdapterConfig, ChapterCandidateProductionExecutionPath,
    };
    use crate::ai::config::AIConfig;
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
    use crate::services::chapter_candidate_quality_adapter_service::{
        build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
    };

    #[test]
    fn should_resolve_python_fallback_when_rust_adapter_disabled() {
        let decision = resolve_chapter_candidate_production_adapter_decision(
            &ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("cutover flag disabled".to_string()),
                rollback_boundary: "chapters.py candidate fallback".to_string(),
            },
        );

        assert_eq!(
            decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(decision.reason, "cutover flag disabled");
        assert_eq!(decision.rollback_boundary, "chapters.py candidate fallback");
    }

    #[tokio::test]
    async fn should_execute_rust_candidate_executor_when_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "python fallback".to_string(),
            },
            |request, _ai_config, _quality_adapter| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"executor": "rust"}));
                    Ok(json!({"path": "rust", "content": "候选正文"}))
                })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect("production adapter output");

        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::RustCandidateExecutor
        );
        assert!(!output.fallback_applied);
        assert_eq!(output.result["path"], "rust");
        assert_eq!(request.runtime_state, Some(json!({"executor": "rust"})));
    }

    #[tokio::test]
    async fn should_call_python_fallback_without_rust_execution_when_disabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("smoke probe disabled rust cutover".to_string()),
                rollback_boundary: "compat service fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("rust executor should not run".to_string()) })
            },
            |request, context| {
                Box::pin(async move {
                    request.runtime_state = Some(json!({"executor": "python"}));
                    Ok(json!({
                        "path": "python",
                        "fallback_reason": context.reason,
                        "rollback_boundary": context.rollback_boundary,
                    }))
                })
            },
        )
        .await
        .expect("fallback output");

        assert!(output.fallback_applied);
        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(output.result["path"], "python");
        assert_eq!(
            output.result["fallback_reason"],
            "smoke probe disabled rust cutover"
        );
        assert_eq!(request.runtime_state, Some(json!({"executor": "python"})));
    }

    #[tokio::test]
    async fn should_fallback_to_python_when_rust_executor_fails_and_rollback_enabled() {
        let mut request = executor_request();

        let output = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "route candidate fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("provider timeout".to_string()) })
            },
            |_request, context| {
                Box::pin(async move {
                    Ok(json!({
                        "path": "python",
                        "rust_error": context.rust_error,
                        "fallback_reason": context.reason,
                    }))
                })
            },
        )
        .await
        .expect("fallback output");

        assert!(output.fallback_applied);
        assert_eq!(output.rust_error.as_deref(), Some("provider timeout"));
        assert_eq!(
            output.decision.path,
            ChapterCandidateProductionExecutionPath::PythonFallback
        );
        assert_eq!(output.result["rust_error"], "provider timeout");
        assert!(output.result["fallback_reason"]
            .as_str()
            .unwrap()
            .contains("python fallback selected"));
    }

    #[tokio::test]
    async fn should_propagate_rust_error_when_rollback_is_disabled() {
        let mut request = executor_request();

        let error = execute_chapter_candidate_production_adapter_with_executor(
            &mut request,
            AIConfig::default(),
            quality_adapter(),
            ChapterCandidateProductionAdapterConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: false,
                disabled_reason: None,
                rollback_boundary: "no fallback".to_string(),
            },
            |_request, _ai_config, _quality_adapter| {
                Box::pin(async { Err("record build failed".to_string()) })
            },
            |_request, _context| Box::pin(async { Ok(json!({"path": "python"})) }),
        )
        .await
        .expect_err("rust error should be propagated");

        assert_eq!(error, "record build failed");
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
