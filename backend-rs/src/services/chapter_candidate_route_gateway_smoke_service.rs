// Deployment-smoke owner for the chapter candidate route gateway.
// It proves the gateway can be consumed without repointing the active route yet.
#![allow(dead_code)]

use serde_json::{json, Map, Value};

use crate::ai::config::AIConfig;
use crate::services::chapter_candidate_executor_production_adapter_service::chapter_candidate_production_execution_path_name;
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_quality_adapter_service::{
    build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_route_gateway_service::{
    execute_chapter_candidate_route_gateway_with_executor, ChapterCandidateRouteGatewayConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRouteGatewaySmokeProbe {
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) route_group: &'static str,
    pub(crate) config: ChapterCandidateRouteGatewayConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateRouteGatewaySmokeResult {
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) route_group: String,
    pub(crate) ok: bool,
    pub(crate) execution_path: String,
    pub(crate) fallback_applied: bool,
    pub(crate) reason: String,
    pub(crate) rollback_boundary: String,
    pub(crate) rust_error: Option<String>,
    pub(crate) result: Value,
    pub(crate) runtime_state: Option<Value>,
}

pub(crate) fn build_default_chapter_candidate_route_gateway_smoke_probes(
) -> Vec<ChapterCandidateRouteGatewaySmokeProbe> {
    vec![
        ChapterCandidateRouteGatewaySmokeProbe {
            name: "chapter-candidate-route-gateway-rust-owner",
            owner: "rust",
            route_group: "chapters",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: true,
                fallback_on_rust_error: true,
                disabled_reason: None,
                rollback_boundary: "python_candidate_executor_fallback".to_string(),
            },
        },
        ChapterCandidateRouteGatewaySmokeProbe {
            name: "chapter-candidate-route-gateway-python-fallback",
            owner: "python-fallback",
            route_group: "chapters",
            config: ChapterCandidateRouteGatewayConfig {
                rust_executor_enabled: false,
                fallback_on_rust_error: true,
                disabled_reason: Some("smoke probe forces python fallback".to_string()),
                rollback_boundary: "python_candidate_executor_fallback".to_string(),
            },
        },
    ]
}

pub(crate) async fn run_chapter_candidate_route_gateway_smoke_suite(
) -> Result<Vec<ChapterCandidateRouteGatewaySmokeResult>, String> {
    let mut results = Vec::new();

    for probe in build_default_chapter_candidate_route_gateway_smoke_probes() {
        results.push(run_chapter_candidate_route_gateway_smoke_probe(probe).await?);
    }

    Ok(results)
}

pub(crate) async fn run_chapter_candidate_route_gateway_smoke_probe(
    probe: ChapterCandidateRouteGatewaySmokeProbe,
) -> Result<ChapterCandidateRouteGatewaySmokeResult, String> {
    let mut request = smoke_executor_request(probe.name);
    let rust_probe_name = probe.name.to_string();
    let python_probe_name = probe.name.to_string();

    let output = execute_chapter_candidate_route_gateway_with_executor(
        &mut request,
        AIConfig::default(),
        smoke_quality_adapter(),
        probe.config,
        move |request, _ai_config, _quality_adapter| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "gateway_smoke": "rust",
                    "probe": rust_probe_name,
                }));
                Ok(json!({
                    "path": "rust",
                    "probe": rust_probe_name,
                    "gateway_consumed": true,
                }))
            })
        },
        move |request, context| {
            Box::pin(async move {
                request.runtime_state = Some(json!({
                    "gateway_smoke": "python-fallback",
                    "probe": python_probe_name,
                }));
                Ok(json!({
                    "path": "python-fallback",
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

    Ok(ChapterCandidateRouteGatewaySmokeResult {
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
        result: output.result,
        runtime_state: request.runtime_state,
    })
}

fn smoke_executor_request(probe_name: &str) -> ChapterCandidateExecutorRequest {
    ChapterCandidateExecutorRequest {
        base_generate_kwargs: Map::from_iter([
            ("prompt".to_string(), json!("SMOKE_PROMPT")),
            ("probe".to_string(), json!(probe_name)),
        ]),
        target_word_count: 1200,
        source: "chapter".to_string(),
        generation_label: "candidate".to_string(),
        max_candidates: 1,
        runtime_state: None,
    }
}

fn smoke_quality_adapter(
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
            generation_intent: json!({"mode": "smoke"}),
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

#[cfg(test)]
mod tests {
    use super::{
        build_default_chapter_candidate_route_gateway_smoke_probes,
        run_chapter_candidate_route_gateway_smoke_probe,
        run_chapter_candidate_route_gateway_smoke_suite,
    };

    #[test]
    fn should_build_default_smoke_probes_for_rust_and_python_fallback() {
        let probes = build_default_chapter_candidate_route_gateway_smoke_probes();

        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].name, "chapter-candidate-route-gateway-rust-owner");
        assert_eq!(probes[0].owner, "rust");
        assert!(probes[0].config.rust_executor_enabled);
        assert_eq!(
            probes[1].name,
            "chapter-candidate-route-gateway-python-fallback"
        );
        assert_eq!(probes[1].owner, "python-fallback");
        assert!(!probes[1].config.rust_executor_enabled);
    }

    #[tokio::test]
    async fn should_run_smoke_suite_through_rust_and_python_fallback_paths() {
        let results = run_chapter_candidate_route_gateway_smoke_suite()
            .await
            .expect("smoke results");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].execution_path, "rust_candidate_executor");
        assert!(!results[0].fallback_applied);
        assert_eq!(results[0].result["path"], "rust");
        assert_eq!(
            results[0].runtime_state.as_ref().unwrap()["gateway_smoke"],
            "rust"
        );

        assert_eq!(results[1].execution_path, "python_fallback");
        assert!(results[1].fallback_applied);
        assert_eq!(results[1].result["path"], "python-fallback");
        assert_eq!(
            results[1].result["fallback_reason"],
            "smoke probe forces python fallback"
        );
        assert_eq!(
            results[1].runtime_state.as_ref().unwrap()["gateway_smoke"],
            "python-fallback"
        );
    }

    #[tokio::test]
    async fn should_keep_probe_metadata_in_smoke_result() {
        let probe = build_default_chapter_candidate_route_gateway_smoke_probes()
            .into_iter()
            .next()
            .expect("rust probe");

        let result = run_chapter_candidate_route_gateway_smoke_probe(probe)
            .await
            .expect("smoke result");

        assert!(result.ok);
        assert_eq!(result.owner, "rust");
        assert_eq!(result.route_group, "chapters");
        assert_eq!(
            result.rollback_boundary,
            "python_candidate_executor_fallback"
        );
        assert_eq!(result.result["gateway_consumed"], true);
    }
}
