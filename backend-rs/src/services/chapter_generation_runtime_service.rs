#[cfg(test)]
pub(crate) use self::candidate_runtime_owner::build_single_generation_direct_fallback_candidate_payload;
pub(crate) use self::candidate_runtime_owner::{
    build_single_generation_candidate_executor_request,
    build_single_generation_candidate_gateway_metadata,
    execute_single_generation_candidate_runtime, single_generation_candidate_gateway_content,
};
use self::context_compaction_owner::build_generation_context_compaction_owner_contract;
use self::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
pub(crate) use self::runtime_execution_owner::{
    build_single_generation_runtime_execution_owner_contract,
    generate_and_persist_chapter_content_with_candidate_route_gateway,
};
pub(crate) use self::single_generation_candidate_quality_owner::build_chapter_single_generation_candidate_quality_owner_contract;
use self::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;
use self::story_continuity_ledger_owner::build_story_continuity_ledger_owner_contract;
use self::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract;
use crate::models::{chapter, project};
use crate::services::chapter_candidate_executor_default_dependency_service::{
    build_chapter_candidate_executor_default_dependency_owner_contract,
    build_default_chapter_candidate_executor_wiring_plan,
    resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
};
use crate::services::chapter_candidate_executor_production_adapter_service::build_chapter_candidate_production_adapter_owner_contract;
use crate::services::chapter_candidate_finalize_service::build_chapter_candidate_finalize_owner_contract;
use crate::services::chapter_candidate_generation_service::build_chapter_candidate_generation_owner_contract;
use crate::services::chapter_candidate_output_service::build_chapter_candidate_output_owner_contract;
use crate::services::chapter_candidate_record_service::build_chapter_candidate_record_owner_contract;
use crate::services::chapter_candidate_rerank_service::build_chapter_candidate_rerank_owner_contract;
use crate::services::chapter_candidate_route_gateway_service::build_chapter_candidate_route_gateway_owner_contract;
use crate::services::chapter_candidate_runtime_state_service::build_chapter_candidate_runtime_state_owner_contract;
use crate::services::chapter_candidate_targeted_final_repair_service::build_chapter_candidate_targeted_final_repair_owner_contract;
use crate::services::chapter_candidate_word_budget_repair_service::build_chapter_candidate_word_budget_repair_owner_contract;
use crate::services::chapter_generation_history_payload_service::build_chapter_generation_history_payload_owner_contract;
use crate::services::chapter_generation_history_persistence_service::build_chapter_generation_history_persistence_owner_contract;
use crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext;
use serde_json::{json, Value};
pub(crate) mod candidate_runtime_owner;
pub(crate) mod context_compaction_owner;
pub(crate) mod quality_runtime_context_owner;
pub(crate) mod runtime_execution_owner;
pub(crate) mod single_generation_candidate_quality_owner;
pub(crate) mod snapshot_persistence_owner;
pub(crate) mod story_continuity_ledger_owner;
pub(crate) mod story_repair_quality_context_owner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedChapterResult {
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) saved_word_count: i32,
    pub(crate) chapter_status: String,
    pub(crate) content_applied: bool,
    pub(crate) provisional_draft_saved: bool,
    pub(crate) attempt_state: String,
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_message: Option<String>,
    pub(crate) candidate_draft: Option<Value>,
    pub(crate) candidate_gateway_metadata: Option<Value>,
    pub(crate) selected_candidate_event_source: Option<Value>,
}

impl Default for GeneratedChapterResult {
    fn default() -> Self {
        Self {
            chapter_id: String::new(),
            chapter_number: 0,
            title: String::new(),
            content: String::new(),
            word_count: 0,
            saved_word_count: 0,
            chapter_status: String::new(),
            content_applied: true,
            provisional_draft_saved: false,
            attempt_state: "generated_from_runtime".to_string(),
            quality_metrics: None,
            quality_gate_action: Some("continue".to_string()),
            quality_gate_message: None,
            candidate_draft: None,
            candidate_gateway_metadata: None,
            selected_candidate_event_source: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationCandidateRuntimeExecutionContext {
    pub(crate) project_model: project::Model,
    pub(crate) chapter_model: chapter::Model,
    pub(crate) previous_chapter_exists: bool,
    pub(crate) previous_chapter_prompt_context: PreviousChapterPromptContext,
    pub(crate) story_packet: Value,
}

pub(crate) fn build_single_generation_candidate_runtime_owner_contract() -> Value {
    let wiring_plan = build_default_chapter_candidate_executor_wiring_plan();
    validate_candidate_executor_wiring_plan(&wiring_plan)
        .expect("default candidate executor wiring plan must stay valid for runtime cutover");
    let wiring_readiness = resolve_candidate_executor_wiring_readiness(&wiring_plan);

    json!({
        "owner": "chapter_generation_runtime_service",
        "scope": "shared_single_generation_candidate_runtime",
        "python_source_map": [],
        "rust_owner_map": [
            "build_single_generation_candidate_executor_request",
            "single_generation_candidate_gateway_content",
            "build_single_generation_candidate_gateway_metadata",
            "build_single_generation_direct_fallback_candidate_payload",
            "build_generated_chapter_history_payload_with_quality_metrics"
        ],
        "candidate_executor_wiring_readiness": {
            "owner": "chapter_candidate_executor_default_dependency_service",
            "stage_count": wiring_readiness.stage_count,
            "rust_owned_dependency_count": wiring_readiness.rust_owned_dependency_count,
            "external_formula_dependency_count": wiring_readiness.external_formula_dependency_count,
            "cutover_blockers": wiring_readiness.cutover_blockers,
            "rust_target_files": wiring_plan.rust_target_files,
            "python_source_files": wiring_plan.python_source_files,
        },
        "candidate_executor_default_dependency_owner_contract": build_chapter_candidate_executor_default_dependency_owner_contract(),
        "candidate_executor_production_adapter_owner_contract": build_chapter_candidate_production_adapter_owner_contract(),
        "candidate_generation_owner_contract": build_chapter_candidate_generation_owner_contract(),
        "candidate_finalize_owner_contract": build_chapter_candidate_finalize_owner_contract(),
        "candidate_output_owner_contract": build_chapter_candidate_output_owner_contract(),
        "candidate_runtime_state_owner_contract": build_chapter_candidate_runtime_state_owner_contract(),
        "candidate_rerank_owner_contract": build_chapter_candidate_rerank_owner_contract(),
        "candidate_route_gateway_owner_contract": build_chapter_candidate_route_gateway_owner_contract(),
        "candidate_record_owner_contract": build_chapter_candidate_record_owner_contract(),
        "candidate_targeted_final_repair_owner_contract": build_chapter_candidate_targeted_final_repair_owner_contract(),
        "candidate_word_budget_repair_owner_contract": build_chapter_candidate_word_budget_repair_owner_contract(),
        "context_compaction_owner_contract": build_generation_context_compaction_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "single_generation_candidate_quality_owner_contract": build_chapter_single_generation_candidate_quality_owner_contract(),
        "draft_persistence_owner_contract": build_chapter_generation_history_persistence_owner_contract(),
        "history_persistence_owner_contract": build_chapter_generation_history_persistence_owner_contract(),
        "runtime_execution_owner_contract": build_single_generation_runtime_execution_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "story_continuity_ledger_owner_contract": build_story_continuity_ledger_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "history_payload_owner_contract": build_chapter_generation_history_payload_owner_contract(),
        "behavior_contract": {
            "accepted_content_fields": ["full_content", "content"],
            "empty_content_error": "candidate route gateway returned empty generated content",
            "direct_fallback_generation_path": "direct_generation_fallback",
            "metadata_fields": [
                "execution_path",
                "fallback_applied",
                "fallback_reason",
                "rollback_boundary",
                "rust_error"
            ],
            "history_candidate_gateway_metadata_attached": true
        },
        "validation_boundary": {
            "focused_test": "chapter_generation_runtime_service",
            "active_gateway_smoke": "chapter-single-generation-active-gateway-smoke-rust",
            "manifest_probe": "chapter-single-generation-active-gateway-smoke-rust"
        },
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 17,
            "python_fallback_probe_count": 0,
            "candidate_executor_wiring_cutover_blockers": wiring_readiness.cutover_blockers,
            "context_compaction_owner": "chapter_generation_runtime_service",
            "candidate_runtime_owner": "single_generation_candidate_gateway_content",
            "direct_fallback_payload_owner": "build_single_generation_direct_fallback_candidate_payload",
            "history_metadata_owner": "build_generated_chapter_history_payload_with_quality_metrics",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "shared batch single-chapter candidate runtime source-map deleted after test-seam migration",
            "status": "rust_shared_candidate_runtime_owner_with_deleted_batch_single_chapter_source_map"
        },
        "rollback_boundary": {
            "source_map_policy": "shared_candidate_runtime_owner_is_rust_only_and_candidate_shell_source_maps_are_deleted",
            "runtime_knob": "ChapterCandidateRouteGatewayConfig",
            "rollback_owner": "legacy_single_generation_direct_ai",
            "single_generation_entry_source_maps": [],
            "shared_candidate_runtime_source_maps": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_candidate_executor_request,
        build_single_generation_candidate_gateway_metadata,
        build_single_generation_candidate_runtime_owner_contract,
        build_single_generation_direct_fallback_candidate_payload,
        runtime_execution_owner::{ChapterGenerationRuntimeContext, LoadGenerationContextError},
        single_generation_candidate_gateway_content, GeneratedChapterResult,
    };
    use crate::ai::types::AIResponse;
    use crate::models::{chapter, project};
    use crate::services::chapter_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        ChapterCandidateProductionAdapterDecision, ChapterCandidateProductionAdapterOutput,
        ChapterCandidateProductionExecutionPath, ChapterCandidateProductionFallbackContext,
    };
    use crate::services::chapter_generation_history_payload_service::{
        build_generated_chapter_history_payload_with_quality_metrics,
        generated_history_payload_view, CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH,
    };
    use crate::services::chapter_generation_history_persistence_service::build_generated_history_payload;
    use crate::services::chapter_generation_prompt_service::build_previous_chapter_prompt_context;
    use crate::services::chapter_single_generation_result_lifecycle_service::persisted_history_payload_view;
    use chrono::Utc;
    use serde_json::json;

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
            content: None,
            summary: None,
            expansion_plan: None,
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_project() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "Project".to_string(),
            description: Some("desc".to_string()),
            theme: None,
            genre: None,
            target_words: 50000,
            current_words: 1200,
            status: "draft".to_string(),
            wizard_status: "idle".to_string(),
            wizard_step: 0,
            outline_mode: "simple".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(12),
            narrative_perspective: None,
            character_count: 3,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_runtime_context() -> ChapterGenerationRuntimeContext {
        ChapterGenerationRuntimeContext {
            chapter_model: build_chapter(),
            project_model: build_project(),
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet: json!({}),
        }
    }

    #[test]
    fn should_build_generated_chapter_result_from_runtime_context_owner() {
        let response = AIResponse {
            content: "  你好\n世界  ".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let context = build_runtime_context();

        let result = context
            .build_generated_result_from_content(response.content)
            .expect("generated result");

        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
    }

    #[test]
    fn should_sanitize_generated_chapter_result_with_rust_narrative_cleaner_owner() {
        let response = AIResponse {
            content: "以下是章节正文：\n\n正常正文第一段。\n\n正常正文第二段。".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let result = build_runtime_context()
            .build_generated_result_from_content(response.content)
            .expect("generated result");

        assert_eq!(result.content, "正常正文第一段。\n\n正常正文第二段。");
        assert_eq!(result.word_count, 18);
    }

    #[test]
    fn should_reject_meta_only_generated_chapter_result_after_sanitization() {
        let response = AIResponse {
            content: "```markdown\n作为AI：我将开始执行\n流程说明".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let error = build_runtime_context()
            .build_generated_result_from_content(response.content)
            .expect_err("meta-only output should be rejected");

        assert_eq!(
            error,
            "chapter generation produced empty narrative after sanitization"
        );
    }

    #[test]
    fn should_keep_generated_chapter_result_transport_fields() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
            ..Default::default()
        };

        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
        assert_eq!(result.title, "第一章");
        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
    }

    #[test]
    fn should_build_single_generation_candidate_executor_request_for_active_route_gateway() {
        let mut ai_config = crate::ai::AIConfig::default();
        ai_config.temperature = 0.72;
        ai_config.max_tokens = 4096;

        let request =
            build_single_generation_candidate_executor_request("请生成章节正文", 2400, &ai_config);

        assert_eq!(request.base_generate_kwargs["prompt"], "请生成章节正文");
        assert_eq!(request.base_generate_kwargs["temperature"], 0.72);
        assert_eq!(request.base_generate_kwargs["max_tokens"], 4096);
        assert_eq!(request.target_word_count, 2400);
        assert_eq!(request.source, "chapter");
        assert_eq!(request.generation_label, "single_generation_candidate");
        assert_eq!(request.max_candidates, 1);
        assert!(request.runtime_state.is_none());
    }

    #[test]
    fn should_extract_candidate_gateway_content_from_candidate_or_fallback_payload() {
        let candidate = json!({
            "full_content": "候选章节正文",
            "generation_path": "single_pass"
        });
        let fallback = json!({
            "content": "直接生成正文",
            "generation_path": "direct_generation_fallback"
        });

        assert_eq!(
            single_generation_candidate_gateway_content(&candidate).expect("candidate content"),
            "候选章节正文"
        );
        assert_eq!(
            single_generation_candidate_gateway_content(&fallback).expect("fallback content"),
            "直接生成正文"
        );
    }

    #[test]
    fn should_prefer_full_content_when_candidate_payload_contains_both_content_fields() {
        let candidate = json!({
            "full_content": "候选章节正文",
            "content": "兼容字段正文"
        });

        assert_eq!(
            single_generation_candidate_gateway_content(&candidate).expect("candidate content"),
            "候选章节正文"
        );
    }

    #[test]
    fn should_preserve_candidate_event_source_on_generated_result_from_candidate() {
        let candidate = json!({
            "full_content": "候选章节正文",
            "candidate_chunks": ["候选", "章节", "正文"],
            "candidate_index": 2,
            "candidate_count": 3,
            "winner_candidate_index": 2,
            "generation_path": "rust_candidate_executor",
            "quality_gate_plan": {"action": "continue"}
        });

        let result = build_runtime_context()
            .build_generated_result_from_candidate(&candidate)
            .expect("generated candidate result");

        assert_eq!(result.content, "候选章节正文");
        assert_eq!(result.word_count, 6);
        assert_eq!(
            result.selected_candidate_event_source.as_ref(),
            Some(&candidate)
        );
    }

    #[test]
    fn should_save_candidate_draft_when_quality_gate_plan_decision_requires_retry() {
        let candidate = json!({
            "full_content": "烟测改写成功。第二段继续推进。",
            "candidate_chunks": ["烟测改写成功。", "第二段继续推进。"],
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": "auto_repair",
                    "status": "repairable",
                    "summary": "The draft still needs a targeted revision before it should be saved."
                }
            },
            "quality_metrics": {
                "quality_gate": {
                    "decision": "auto_repair",
                    "status": "repairable",
                    "summary": "The draft still needs a targeted revision before it should be saved."
                }
            }
        });

        let result = build_runtime_context()
            .build_generated_result_from_candidate(&candidate)
            .expect("generated retry candidate result");

        assert_eq!(result.quality_gate_action.as_deref(), Some("retry"));
        assert_eq!(result.content_applied, false);
        assert_eq!(result.provisional_draft_saved, true);
        assert_eq!(result.attempt_state, "retry");
        assert_eq!(result.chapter_status, "draft");
        assert_eq!(
            result.candidate_draft.as_ref().expect("candidate draft")["quality_gate_action"],
            "retry"
        );
    }

    #[test]
    fn should_block_candidate_draft_when_quality_gate_plan_decision_requires_manual_review() {
        let candidate = json!({
            "full_content": "烟测改写成功。第二段继续推进。",
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": "manual_review",
                    "status": "blocked",
                    "summary": "The draft is too short and requires manual review."
                }
            },
            "quality_metrics": {
                "quality_gate": {
                    "decision": "manual_review",
                    "status": "blocked",
                    "summary": "The draft is too short and requires manual review."
                }
            }
        });

        let result = build_runtime_context()
            .build_generated_result_from_candidate(&candidate)
            .expect("generated manual-review candidate result");

        assert_eq!(result.quality_gate_action.as_deref(), Some("manual_review"));
        assert_eq!(result.content_applied, false);
        assert_eq!(result.provisional_draft_saved, false);
        assert_eq!(result.attempt_state, "manual_review");
        assert_eq!(
            result.candidate_draft.as_ref().expect("candidate draft")["quality_gate_action"],
            "manual_review"
        );
    }

    #[test]
    fn should_reject_empty_or_missing_candidate_gateway_content() {
        for candidate in [json!({"full_content": " "}), json!({})] {
            let error = single_generation_candidate_gateway_content(&candidate)
                .expect_err("empty content should fail");

            assert_eq!(
                error,
                "candidate route gateway returned empty generated content"
            );
        }
    }

    #[test]
    fn should_build_candidate_gateway_metadata_from_production_adapter_output() {
        let output = ChapterCandidateProductionAdapterOutput {
            result: json!({
                "full_content": "Rust 候选章节正文。"
            }),
            decision: ChapterCandidateProductionAdapterDecision {
                path: ChapterCandidateProductionExecutionPath::RustCandidateExecutor,
                reason: "rust candidate executor enabled by production adapter".to_string(),
                rollback_boundary: "legacy_single_generation_direct_ai".to_string(),
            },
            fallback_applied: false,
            rust_error: None,
        };

        let metadata = build_single_generation_candidate_gateway_metadata(&output);

        assert_eq!(metadata["execution_path"], "rust_candidate_executor");
        assert_eq!(metadata["fallback_applied"], false);
        assert_eq!(
            metadata["fallback_reason"],
            "rust candidate executor enabled by production adapter"
        );
        assert_eq!(
            metadata["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
        assert!(metadata["rust_error"].is_null());
    }

    #[test]
    fn should_build_direct_fallback_candidate_payload_from_runtime_owner() {
        let payload = build_single_generation_direct_fallback_candidate_payload(
            "直接生成正文".to_string(),
            ChapterCandidateProductionFallbackContext {
                reason: "rust candidate executor failed; python fallback selected: timeout"
                    .to_string(),
                rollback_boundary: "legacy_single_generation_direct_ai".to_string(),
                rust_error: Some("timeout".to_string()),
            },
        );

        assert_eq!(payload["full_content"], "直接生成正文");
        assert_eq!(payload["generation_path"], "direct_generation_fallback");
        assert_eq!(
            payload["fallback_reason"],
            "rust candidate executor failed; python fallback selected: timeout"
        );
        assert_eq!(
            payload["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
        assert_eq!(payload["rust_error"], "timeout");
    }

    #[test]
    fn should_publish_shared_candidate_runtime_owner_contract_for_freezing_python_source_map() {
        let contract = build_single_generation_candidate_runtime_owner_contract();

        assert_eq!(contract["owner"], "chapter_generation_runtime_service");
        assert_eq!(
            contract["scope"],
            "shared_single_generation_candidate_runtime"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            0
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "build_single_generation_candidate_executor_request"
        );
        assert_eq!(
            contract["rust_owner_map"][1],
            "single_generation_candidate_gateway_content"
        );
        assert_eq!(
            contract["behavior_contract"]["accepted_content_fields"],
            json!(["full_content", "content"])
        );
        assert_eq!(
            contract["behavior_contract"]["empty_content_error"],
            "candidate route gateway returned empty generated content"
        );
        assert_eq!(
            contract["behavior_contract"]["direct_fallback_generation_path"],
            "direct_generation_fallback"
        );
        assert_eq!(
            contract["candidate_executor_wiring_readiness"]["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract["candidate_executor_wiring_readiness"]["stage_count"],
            9
        );
        assert!(
            contract["candidate_executor_wiring_readiness"]["rust_owned_dependency_count"]
                .as_u64()
                .unwrap()
                >= 56
        );
        assert_eq!(
            contract["candidate_executor_wiring_readiness"]["external_formula_dependency_count"],
            0
        );
        assert_eq!(
            contract["candidate_executor_wiring_readiness"]["cutover_blockers"],
            json!([])
        );
        assert_eq!(
            contract["candidate_executor_default_dependency_owner_contract"]["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract["candidate_executor_default_dependency_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_executor_default_dependency_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_executor_default_dependency_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["candidate_executor_default_dependency_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["single_generation_entry_source_maps"],
            json!([])
        );
        assert_eq!(
            contract["rollback_boundary"]["shared_candidate_runtime_source_maps"],
            json!([])
        );
        assert_eq!(
            contract["candidate_executor_default_dependency_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["candidate_executor_production_adapter_owner_contract"]["owner"],
            "chapter_candidate_executor_production_adapter_service"
        );
        assert_eq!(
            contract["candidate_executor_production_adapter_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_executor_production_adapter_owner_executor_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_executor_production_adapter_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["candidate_executor_production_adapter_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["candidate_executor_production_adapter_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["candidate_record_owner_contract"]["owner"],
            "chapter_candidate_record_service"
        );
        assert_eq!(
            contract["candidate_record_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_record_owner_executor_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_finalize_owner_contract"]["owner"],
            "chapter_candidate_finalize_service"
        );
        assert_eq!(
            contract["candidate_finalize_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_finalize_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_output_owner_contract"]["owner"],
            "chapter_candidate_output_service"
        );
        assert_eq!(
            contract["candidate_output_owner_contract"]["behavior_contract"]["entrypoints"][0],
            "collect_generation_candidate_output"
        );
        assert_eq!(
            contract["candidate_output_owner_contract"]["candidate_runtime_state_owner_contract"]
                ["owner"],
            "chapter_candidate_runtime_state_service"
        );
        assert_eq!(
            contract["candidate_runtime_state_owner_contract"]["owner"],
            "chapter_candidate_runtime_state_service"
        );
        assert_eq!(
            contract["candidate_runtime_state_owner_contract"]["behavior_contract"]["entrypoints"]
                [2],
            "snapshot_chapter_candidate_runtime_state"
        );
        assert_eq!(
            contract["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["candidate_route_gateway_owner_contract"]["owner"],
            "chapter_candidate_route_gateway_service"
        );
        assert_eq!(
            contract["candidate_route_gateway_owner_contract"]["behavior_contract"]
                ["gateway_entrypoints"][1],
            "execute_chapter_candidate_route_gateway_with_executor"
        );
        assert_eq!(
            contract["candidate_route_gateway_owner_contract"]["active_consumers"][4],
            "chapter_generation_runtime_service"
        );
        assert_eq!(
            contract["candidate_generation_owner_contract"]["owner"],
            "chapter_candidate_generation_service"
        );
        assert_eq!(
            contract["candidate_generation_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_generation_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_rerank_owner_contract"]["owner"],
            "chapter_candidate_rerank_service"
        );
        assert_eq!(
            contract["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_rerank_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                ["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                ["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["candidate_rerank_owner_contract"]["service_runtime_closeout_status"]
                ["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["candidate_word_budget_repair_owner_contract"]["owner"],
            "chapter_candidate_word_budget_repair_service"
        );
        assert_eq!(
            contract["candidate_word_budget_repair_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_word_budget_repair_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_word_budget_repair_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["candidate_word_budget_repair_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["candidate_word_budget_repair_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["candidate_targeted_final_repair_owner_contract"]["owner"],
            "chapter_candidate_targeted_final_repair_service"
        );
        assert_eq!(
            contract["candidate_targeted_final_repair_owner_contract"]
                ["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_targeted_final_repair_owner_source_map_deleted"
        );
        assert_eq!(
            contract["candidate_targeted_final_repair_owner_contract"]
                ["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["candidate_targeted_final_repair_owner_contract"]
                ["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["candidate_targeted_final_repair_owner_contract"]
                ["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        let rust_target_files = contract["candidate_executor_wiring_readiness"]
            ["rust_target_files"]
            .as_array()
            .unwrap();
        assert!(rust_target_files.contains(&json!(
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs"
        )));
        assert!(rust_target_files.contains(&json!(
            "backend-rs/src/services/chapter_candidate_generation_service.rs"
        )));
        assert!(rust_target_files.contains(&json!(
            "backend-rs/src/services/chapter_candidate_finalize_service.rs"
        )));
        assert!(!rust_target_files.contains(&json!(
            "backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs"
        )));
        assert_eq!(
            contract["candidate_executor_wiring_readiness"]["python_source_files"]
                .as_array()
                .expect("python source files")
                .len(),
            0
        );
        assert_eq!(
            contract["context_compaction_owner_contract"]["owner"],
            "chapter_generation_runtime_service"
        );
        assert_eq!(
            contract["context_compaction_owner_contract"]["python_source_map"],
            json!([])
        );
        assert_eq!(
            contract["context_compaction_owner_contract"]["behavior_contract"]
                ["one_to_one_skips_recent_chapters_context"],
            true
        );
        assert_eq!(
            contract["context_compaction_owner_contract"]["active_consumers"][0],
            "chapter_generation_runtime_service"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["behavior_contract"]
                ["terminal_quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["active_consumers"][0],
            "chapter_single_generation_runtime_state_service"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["write_functions"]
                [0],
            "persist_chapter_generation_runtime_snapshot"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]
                ["runtime_state_policy"][0],
            "object_payloads_merge_keywise"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["behavior_contract"]
                ["resume_precedence"][0],
            "runtime_active_story_repair_payload"
        );
        assert_eq!(
            contract["story_repair_quality_context_owner_contract"]["active_consumers"][7],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["validation_boundary"]["active_gateway_smoke"],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(17)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]
                ["candidate_executor_wiring_cutover_blockers"],
            json!([])
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["context_compaction_owner"],
            "chapter_generation_runtime_service"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_runtime_owner"],
            "single_generation_candidate_gateway_content"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_shared_candidate_runtime_owner_with_deleted_batch_single_chapter_source_map"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "shared_candidate_runtime_owner_is_rust_only_and_candidate_shell_source_maps_are_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_knob"],
            "ChapterCandidateRouteGatewayConfig"
        );
    }

    #[test]
    fn should_build_generated_history_payload_with_quality_metrics_placeholder() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
            ..Default::default()
        };

        let created_at = Utc::now().naive_utc();
        let payload = build_generated_history_payload(&result, created_at);

        assert_eq!(
            payload,
            json!({
                "log_type": "chapter_generation_quality_v1",
                "content": "你好\n世界",
                "preview": "你好\n世界",
                "quality_metrics": null,
                "generated_at": created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "content_applied": true,
                "attempt_state": "generated_from_runtime",
            })
        );
    }

    #[test]
    fn should_keep_chapter_generation_history_model_owner_constant() {
        assert_eq!(
            crate::services::chapter_single_generation_result_lifecycle_service::CHAPTER_GENERATION_HISTORY_MODEL,
            "chapter_generation_v1"
        );
    }

    #[test]
    fn should_build_generated_history_payload_with_runtime_quality_metrics() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 8.4,
                "repair_guidance": {
                    "summary": "压缩说明段"
                },
                "quality_gate": {
                    "decision": "auto_repair"
                }
            })),
            Some(&json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "rollback_boundary": "python_candidate_executor_fallback"
            })),
            true,
            Some("generated_from_runtime"),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(payload["content"], "正文");
        assert_eq!(payload["preview"], "正文");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(payload["quality_metrics"]["overall_score"], 8.4);
        assert_eq!(
            payload["quality_metrics"]["repair_guidance"]["summary"],
            "压缩说明段"
        );
        assert_eq!(
            payload["quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            persisted_history_payload_view(Some(&payload.to_string()))
                .candidate_gateway_metadata
                .as_ref()
                .expect("candidate gateway metadata")["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(payload["attempt_state"], "generated_from_runtime");
    }

    #[test]
    fn should_build_generated_history_payload_view_from_quality_metrics_and_gateway_metadata() {
        let view = generated_history_payload_view(
            Some(&json!({
                "quality_runtime_context": {
                    "scope": "chapter",
                    "source": "plot_analysis"
                },
                "story_runtime_contract": {
                    "guidance": {
                        "creative_mode": "balanced"
                    }
                }
            })),
            Some(&json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false
            })),
        );

        assert_eq!(
            view.quality_metrics.as_ref().expect("quality metrics")["quality_runtime_context"]
                ["source"],
            "plot_analysis"
        );
        assert_eq!(
            view.story_runtime_contract
                .as_ref()
                .expect("runtime contract")["guidance"]["creative_mode"],
            "balanced"
        );
        assert_eq!(
            view.story_runtime_snapshot
                .as_ref()
                .expect("runtime snapshot")["source"],
            "plot_analysis"
        );
        assert_eq!(
            view.candidate_gateway_metadata
                .as_ref()
                .expect("gateway metadata")["execution_path"],
            "rust_candidate_executor"
        );
    }

    #[test]
    fn should_build_persisted_history_payload_view_from_generated_history_payload() {
        let payload = json!({
            "content_applied": false,
            "attempt_state": "manual_review",
            "candidate_gateway": {
                "execution_path": "rust_candidate_executor",
                "rollback_boundary": "python_candidate_executor_fallback"
            }
        });

        let view = persisted_history_payload_view(Some(&payload.to_string()));

        assert_eq!(view.content_applied, false);
        assert_eq!(view.attempt_state.as_deref(), Some("manual_review"));
        assert_eq!(
            view.candidate_gateway_metadata
                .as_ref()
                .expect("candidate gateway metadata")["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
    }

    #[test]
    fn should_keep_runtime_history_metadata_when_contract_exists() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "章节正文".repeat(260).as_str(),
            Some(&json!({
                "overall_score": 8.8,
                "quality_runtime_context": {
                    "scope": "chapter",
                    "source": "plot_analysis"
                },
                "story_runtime_contract": {
                    "guidance": {
                        "creative_mode": "hook"
                    },
                    "blueprint": {
                        "current_chapter_number": 6,
                        "target_word_count": 2800
                    }
                }
            })),
            None,
            true,
            Some("generated_from_runtime"),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(
            payload["preview"]
                .as_str()
                .expect("preview text")
                .chars()
                .count(),
            CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH
        );
        assert_eq!(
            payload["story_runtime_snapshot"],
            json!({
                "scope": "chapter",
                "source": "plot_analysis"
            })
        );
        assert_eq!(
            payload["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
    }

    #[test]
    fn should_derive_story_runtime_snapshot_from_contract_when_metrics_context_missing() {
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 7.6,
                "story_runtime_contract": {
                    "guidance": {
                        "story_focus": "advance_plot",
                        "plot_stage": "climax"
                    },
                    "blueprint": {
                        "long_term_goal": "追回失落线索",
                        "chapter_count": 12,
                        "current_chapter_number": 5,
                        "target_word_count": 2600,
                        "character_focus_names": ["沈砚"],
                        "foreshadow_payoff_plan": ["回收暗号"],
                        "character_state_ledger": [],
                        "relationship_state_ledger": [],
                        "foreshadow_state_ledger": [],
                        "organization_state_ledger": [],
                        "career_state_ledger": []
                    }
                }
            })),
            None,
            true,
            Some("generated_from_runtime"),
            Utc::now().naive_utc(),
        );

        assert_eq!(
            payload["story_runtime_snapshot"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(payload["story_runtime_snapshot"]["plot_stage"], "climax");
        assert_eq!(
            payload["story_runtime_snapshot"]["story_long_term_goal"],
            "追回失落线索"
        );
        assert_eq!(payload["story_runtime_snapshot"]["chapter_count"], 12);
        assert_eq!(
            payload["story_runtime_snapshot"]["current_chapter_number"],
            5
        );
        assert_eq!(payload["story_runtime_snapshot"]["target_word_count"], 2600);
        assert_eq!(
            payload["story_runtime_snapshot"]["character_focus"],
            json!(["沈砚"])
        );
        assert_eq!(
            payload["story_runtime_snapshot"]["foreshadow_payoff_plan"],
            json!(["回收暗号"])
        );
    }

    #[test]
    fn should_map_generation_context_access_denied_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            )
            .into_runtime_message(),
            "Chapter not found or access denied"
        );
    }

    #[test]
    fn should_map_generation_context_project_missing_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::ProjectNotFound.into_runtime_message(),
            "Project not found"
        );
    }

    #[test]
    fn should_keep_generation_context_internal_owner() {
        assert_eq!(
            LoadGenerationContextError::Internal("boom".to_string()).into_runtime_message(),
            "boom"
        );
    }

    #[test]
    fn should_keep_chapter_generation_runtime_context_owner_contract() {
        let context = build_runtime_context();

        assert_eq!(context.chapter_model.id, "chapter-1");
        assert_eq!(context.project_model.id, "project-1");
        assert_eq!(context.previous_chapter, None);
    }
}
