// Active-route smoke owner for single-chapter generation gateway cutover.
// It exercises the same request/content gateway boundary as the production
// single-generation route, but uses fake executors so no provider call occurs.
#![allow(dead_code)]

use serde_json::{json, Value};

use crate::ai::config::AIConfig;
use crate::services::chapter_candidate_executor_production_adapter_service::chapter_candidate_production_execution_path_name;
use crate::services::chapter_candidate_quality_adapter_service::{
    build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_route_gateway_service::{
    execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
};
use crate::services::chapter_generation_runtime_service::{
    build_single_generation_candidate_executor_request, single_generation_candidate_gateway_content,
};

const ACTIVE_SINGLE_GENERATION_ROUTE_GROUP: &str = "chapter_single_generation";
const ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY: &str = "legacy_single_generation_direct_ai";
const ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 2400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterSingleGenerationActiveGatewaySmokeProbe {
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) route_group: &'static str,
    pub(crate) prompt: &'static str,
    pub(crate) config: ChapterCandidateRouteGatewayConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterSingleGenerationActiveGatewaySmokeResult {
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) route_group: String,
    pub(crate) ok: bool,
    pub(crate) execution_path: String,
    pub(crate) fallback_applied: bool,
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
    pub(crate) rust_error: Option<String>,
    pub(crate) content: String,
    pub(crate) result: Value,
    pub(crate) runtime_state: Option<Value>,
}

pub(crate) fn build_default_chapter_single_generation_active_gateway_smoke_probes(
) -> Vec<ChapterSingleGenerationActiveGatewaySmokeProbe> {
    vec![
        ChapterSingleGenerationActiveGatewaySmokeProbe {
            name: "chapter-single-generation-active-gateway-rust-owner",
            owner: "rust",
            route_group: ACTIVE_SINGLE_GENERATION_ROUTE_GROUP,
            prompt: "ACTIVE_SINGLE_GENERATION_PROMPT",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY.to_string(),
            },
        },
        ChapterSingleGenerationActiveGatewaySmokeProbe {
            name: "chapter-single-generation-active-gateway-direct-fallback",
            owner: "rust-direct-fallback",
            route_group: ACTIVE_SINGLE_GENERATION_ROUTE_GROUP,
            prompt: "ACTIVE_SINGLE_GENERATION_FALLBACK_PROMPT",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some(
                    "single generation active gateway smoke keeps direct fallback".to_string(),
                ),
                rollback_boundary: ACTIVE_SINGLE_GENERATION_ROLLBACK_BOUNDARY.to_string(),
            },
        },
    ]
}

pub(crate) async fn run_chapter_single_generation_active_gateway_smoke_suite(
) -> Result<Vec<ChapterSingleGenerationActiveGatewaySmokeResult>, String> {
    let mut results = Vec::new();

    for probe in build_default_chapter_single_generation_active_gateway_smoke_probes() {
        results.push(run_chapter_single_generation_active_gateway_smoke_probe(probe).await?);
    }

    Ok(results)
}

pub(crate) async fn run_chapter_single_generation_active_gateway_smoke_probe(
    probe: ChapterSingleGenerationActiveGatewaySmokeProbe,
) -> Result<ChapterSingleGenerationActiveGatewaySmokeResult, String> {
    let mut ai_config = AIConfig::default();
    ai_config.temperature = 0.72;
    ai_config.max_tokens = 4096;

    let mut request = build_single_generation_candidate_executor_request(
        probe.prompt,
        ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT,
        &ai_config,
    );
    let rust_probe_name = probe.name.to_string();
    let python_probe_name = probe.name.to_string();

    let output = execute_chapter_candidate_route_gateway_with_executor(
        &mut request,
        ai_config,
        smoke_quality_adapter(probe.name),
        probe.config,
        move |request, _ai_config, _quality_adapter| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "active_single_generation_gateway": "rust",
                    "probe": rust_probe_name,
                    "generation_label": request.generation_label,
                    "source": request.source,
                }));
                Ok(json!({
                    "full_content": "Rust 候选章节正文。",
                    "generation_path": "single_generation_rust_candidate_gateway",
                    "probe": rust_probe_name,
                    "gateway_consumed": true,
                }))
            })
        },
        move |request, context| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "active_single_generation_gateway": "direct-fallback",
                    "probe": python_probe_name,
                    "generation_label": request.generation_label,
                    "source": request.source,
                }));
                Ok(json!({
                    "content": "直接生成回退章节正文。",
                    "generation_path": "direct_generation_fallback",
                    "probe": python_probe_name,
                    "gateway_consumed": true,
                    "fallback_reason": context.reason,
                    "rollback_boundary": context.rollback_boundary,
                    "rust_error": context.rust_error,
                }))
            })
        },
    )
    .await?;

    let content = single_generation_candidate_gateway_content(&output.result)?;

    Ok(ChapterSingleGenerationActiveGatewaySmokeResult {
        name: probe.name.to_string(),
        owner: probe.owner.to_string(),
        route_group: probe.route_group.to_string(),
        ok: true,
        execution_path: chapter_candidate_production_execution_path_name(output.decision.path)
            .to_string(),
        fallback_applied: output.fallback_applied,
        reason: output.decision.reason,
        rollback_boundary: output.decision.rollback_boundary,
        rust_error: output.rust_error,
        content,
        result: output.result,
        runtime_state: request.runtime_state,
    })
}

fn smoke_quality_adapter(
    probe_name: &str,
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
            story_packet: json!({"packet": "active_single_generation_smoke"}),
            project: json!({"world_rules": "rules"}),
            chapter: json!({"id": "chapter-1", "title": "第一章"}),
            chapter_context: json!({"chapter_outline": "outline"}),
            target_word_count: i64::from(ACTIVE_SINGLE_GENERATION_TARGET_WORD_COUNT),
            generation_intent: json!({
                "mode": "single_generation_active_route_smoke",
                "probe": probe_name,
            }),
            retry_count: 0,
            max_retries: 1,
            current_story_repair_payload: None,
            scope: "chapter".to_string(),
            log_prefix: "SingleGenerationActiveGatewaySmoke".to_string(),
        },
        |_input| json!({"runtime": "context"}),
        |input| {
            json!({
                "overall_score": 90.0,
                "word_count": input.content.chars().count(),
            })
        },
        |_input| {
            json!({
                "action": "continue",
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass",
                    "allow_save": true,
                }
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_default_chapter_single_generation_active_gateway_smoke_probes,
        run_chapter_single_generation_active_gateway_smoke_probe,
        run_chapter_single_generation_active_gateway_smoke_suite,
    };

    #[test]
    fn should_build_active_gateway_smoke_probes_for_enabled_and_direct_fallback_paths() {
        let probes = build_default_chapter_single_generation_active_gateway_smoke_probes();

        assert_eq!(probes.len(), 2);
        assert_eq!(
            probes[0].name,
            "chapter-single-generation-active-gateway-rust-owner"
        );
        assert!(probes[0].config.rust_executor_enabled);
        assert_eq!(
            probes[1].name,
            "chapter-single-generation-active-gateway-direct-fallback"
        );
        assert!(!probes[1].config.rust_executor_enabled);
        assert_eq!(probes[0].route_group, "chapter_single_generation");
    }

    #[tokio::test]
    async fn should_run_active_gateway_smoke_through_enabled_and_direct_fallback_paths() {
        let results = run_chapter_single_generation_active_gateway_smoke_suite()
            .await
            .expect("active gateway smoke results");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].execution_path, "rust_candidate_executor");
        assert!(!results[0].fallback_applied);
        assert_eq!(results[0].content, "Rust 候选章节正文。");
        assert_eq!(
            results[0].result["generation_path"],
            "single_generation_rust_candidate_gateway"
        );
        assert_eq!(
            results[0].runtime_state.as_ref().unwrap()["generation_label"],
            "single_generation_candidate"
        );

        assert_eq!(results[1].execution_path, "python_fallback");
        assert!(results[1].fallback_applied);
        assert_eq!(results[1].content, "直接生成回退章节正文。");
        assert_eq!(
            results[1].result["generation_path"],
            "direct_generation_fallback"
        );
        assert_eq!(
            results[1].result["fallback_reason"],
            "single generation active gateway smoke keeps direct fallback"
        );
    }

    #[tokio::test]
    async fn should_keep_active_gateway_probe_metadata_and_runtime_state() {
        let probe = build_default_chapter_single_generation_active_gateway_smoke_probes()
            .into_iter()
            .next()
            .expect("rust active gateway probe");

        let result = run_chapter_single_generation_active_gateway_smoke_probe(probe)
            .await
            .expect("active gateway smoke result");

        assert!(result.ok);
        assert_eq!(result.owner, "rust");
        assert_eq!(result.route_group, "chapter_single_generation");
        assert_eq!(
            result.rollback_boundary,
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(result.result["gateway_consumed"], true);
        assert_eq!(result.runtime_state.as_ref().unwrap()["source"], "chapter");
    }
}
