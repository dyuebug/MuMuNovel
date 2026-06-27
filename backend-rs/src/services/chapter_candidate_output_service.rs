use std::future::Future;

use futures::{Stream, StreamExt};
use serde_json::Value;

use crate::ai::service::AIService;
use crate::ai::types::{AIStreamChunk, ToolDef};
use crate::services::chapter_candidate_runtime_state_service::{
    build_chapter_candidate_runtime_state_owner_contract, snapshot_chapter_candidate_runtime_state,
    sync_chapter_candidate_runtime_state, ChapterCandidateRuntimeStatePatch,
};
use crate::services::chapter_narrative_cleaner_service::trim_text_to_sentence_boundary;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateOutput {
    pub(crate) full_content: String,
    pub(crate) chunks: Vec<String>,
    pub(crate) runtime_state: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChapterCandidateOutputProgress {
    pub(crate) current_chars: usize,
    pub(crate) chunk_count: usize,
}

pub(crate) struct ChapterCandidateOutputRequest<'a> {
    pub(crate) ai_service: AIService,
    pub(crate) prompt: String,
    pub(crate) system_prompt: Option<String>,
    pub(crate) tools: Option<Vec<ToolDef>>,
    pub(crate) candidate_index: i64,
    pub(crate) max_output_chars: Option<usize>,
    pub(crate) runtime_state: Option<&'a mut Value>,
}

pub(crate) async fn collect_generation_candidate_output<F, Fut>(
    request: ChapterCandidateOutputRequest<'_>,
    on_chunk: F,
) -> Result<ChapterCandidateOutput, String>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let stream = request.ai_service.generate_text_stream(
        request.prompt,
        request.system_prompt,
        request.tools,
    );
    collect_generation_candidate_output_from_stream(
        stream,
        request.candidate_index,
        request.max_output_chars,
        request.runtime_state,
        on_chunk,
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_from_stream<S, F, Fut>(
    mut stream: S,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    mut on_chunk: F,
) -> Result<ChapterCandidateOutput, String>
where
    S: Stream<Item = Result<AIStreamChunk, String>> + Unpin,
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let mut full_content = String::new();
    let mut chunks = Vec::new();
    let normalized_candidate_index = candidate_index.max(1);
    let mut runtime_state = runtime_state;
    let mut candidate_total = normalized_candidate_index;

    if let Some(state) = runtime_state.as_deref() {
        candidate_total =
            snapshot_chapter_candidate_runtime_state(Some(state), normalized_candidate_index)
                .candidate_total;
    }
    sync_candidate_output_runtime_state(
        runtime_state.as_deref_mut(),
        normalized_candidate_index,
        candidate_total,
        0,
        0,
    );

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        let chunk_content = chunk.content.unwrap_or_default();
        full_content.push_str(&chunk_content);
        chunks.push(chunk_content.clone());

        let progress = ChapterCandidateOutputProgress {
            current_chars: full_content.chars().count(),
            chunk_count: chunks.len(),
        };
        sync_candidate_output_runtime_state(
            runtime_state.as_deref_mut(),
            normalized_candidate_index,
            candidate_total,
            progress.current_chars,
            progress.chunk_count,
        );
        on_chunk(chunk_content.clone(), progress).await?;

        if max_output_chars.is_some_and(|limit| limit > 0 && full_content.chars().count() >= limit)
        {
            break;
        }
    }

    if let Some(limit) = max_output_chars.filter(|limit| *limit > 0) {
        if full_content.chars().count() > limit {
            full_content = trim_text_to_sentence_boundary(&full_content, limit);
            chunks = if full_content.is_empty() {
                Vec::new()
            } else {
                vec![full_content.clone()]
            };
        }
    }

    Ok(ChapterCandidateOutput {
        full_content,
        chunks,
        runtime_state: runtime_state.as_deref().cloned(),
    })
}

fn sync_candidate_output_runtime_state(
    runtime_state: Option<&mut Value>,
    candidate_index: i64,
    candidate_total: i64,
    current_chars: usize,
    chunk_count: usize,
) {
    sync_chapter_candidate_runtime_state(
        runtime_state,
        candidate_index,
        candidate_total,
        ChapterCandidateRuntimeStatePatch {
            current_chars: Some(current_chars as i64),
            chunk_count: Some(chunk_count as i64),
            ..ChapterCandidateRuntimeStatePatch::default()
        },
    );
}

pub(crate) fn build_chapter_candidate_output_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_output_service",
        "scope": "candidate_provider_output_stream_chunk_runtime_state_owner",
        "python_source_map": [
            "backend/tests/test_services/test_chapter_candidate_output_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_narrative_cleaner_service.rs",
            "backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "collect_generation_candidate_output",
                "collect_generation_candidate_output_from_stream"
            ],
            "request_fields": [
                "ai_service",
                "prompt",
                "system_prompt",
                "tools",
                "candidate_index",
                "max_output_chars",
                "runtime_state"
            ],
            "output_fields": [
                "full_content",
                "chunks",
                "runtime_state"
            ],
            "stream_policy": [
                "candidate_index is normalized to at least 1",
                "candidate_total is read from runtime_state snapshot when available",
                "every provider chunk is appended to full_content and chunks",
                "on_chunk receives cloned chunk text and current character/chunk progress",
                "provider stream errors are propagated without fallback mutation",
                "collection stops once positive max_output_chars is reached or exceeded"
            ],
            "runtime_state_policy": [
                "runtime state is initialized to zero current_chars and chunk_count before streaming",
                "current_chars counts Rust chars to match Python len for Unicode narrative text",
                "chunk_count follows emitted provider chunks before optional final trimming",
                "non-object runtime state is replaced through the shared runtime-state owner",
                "returned runtime_state mirrors the in-place state after collection"
            ],
            "trimming_policy": [
                "positive max_output_chars triggers sentence-boundary trimming only after over-limit content",
                "trimmed non-empty content becomes a single final chunk",
                "empty trimmed content clears chunks",
                "trimming is delegated to chapter_narrative_cleaner_service"
            ],
            "error_contract": [
                "provider stream error string is returned unchanged",
                "on_chunk callback error string is returned unchanged"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_output_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_generation_service",
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_word_budget_repair_service",
            "chapter_candidate_targeted_final_repair_service",
            "chapter_candidate_route_gateway_service",
            "chapter_regeneration_stream_workflow_service"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_output_python_source_map",
            "runtime_state_owner": "chapter_candidate_runtime_state_service",
            "trimming_owner": "chapter_narrative_cleaner_service",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        },
        "candidate_runtime_state_owner_contract": build_chapter_candidate_runtime_state_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapter-regeneration-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "regeneration_manifest_probe_count": 13,
            "python_fallback_probe_count": 0,
            "stream_collection_owner": "collect_generation_candidate_output",
            "stream_from_provider_owner": "collect_generation_candidate_output_from_stream",
            "runtime_state_sync_owner": "sync_candidate_output_runtime_state",
            "runtime_state_snapshot_owner": "snapshot_chapter_candidate_runtime_state",
            "trimming_owner": "trim_text_to_sentence_boundary",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate output production python source-map deleted; surviving Python closeout work for this owner is now limited to focused Python regression coverage",
            "status": "rust_chapter_candidate_output_owner_executor_source_map_deleted"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use tokio_stream::iter;

    use crate::ai::types::AIStreamChunk;

    use super::{
        build_chapter_candidate_output_owner_contract,
        collect_generation_candidate_output_from_stream, ChapterCandidateOutputProgress,
    };

    fn text_chunk(content: &str) -> Result<AIStreamChunk, String> {
        Ok(AIStreamChunk {
            content: Some(content.to_string()),
            tool_calls: None,
            done: false,
            finish_reason: None,
        })
    }

    #[tokio::test]
    async fn should_collect_candidate_output_chunks_like_python_service() {
        let output = collect_generation_candidate_output_from_stream(
            iter(vec![text_chunk("第一段"), text_chunk("第二段")]),
            1,
            None,
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("candidate output");

        assert_eq!(output.full_content, "第一段第二段");
        assert_eq!(output.chunks, vec!["第一段", "第二段"]);
        assert!(output.runtime_state.is_none());
    }

    #[tokio::test]
    async fn should_sync_candidate_runtime_state_during_output_collection() {
        let mut runtime_state = json!({
            "candidate_total": 3,
            "generation_path": "single_pass"
        });

        let output = collect_generation_candidate_output_from_stream(
            iter(vec![text_chunk("甲乙"), text_chunk("丙丁")]),
            2,
            None,
            Some(&mut runtime_state),
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("candidate output");

        assert_eq!(output.full_content, "甲乙丙丁");
        assert_eq!(output.runtime_state.as_ref().unwrap()["candidate_index"], 2);
        assert_eq!(output.runtime_state.as_ref().unwrap()["current_chars"], 4);
        assert_eq!(runtime_state["candidate_index"], 2);
        assert_eq!(runtime_state["candidate_total"], 3);
        assert_eq!(runtime_state["candidate_count"], 3);
        assert_eq!(runtime_state["current_chars"], 4);
        assert_eq!(runtime_state["word_count"], 4);
        assert_eq!(runtime_state["chunk_count"], 2);
    }

    #[tokio::test]
    async fn should_trim_over_limit_candidate_output_to_sentence_boundary() {
        let output = collect_generation_candidate_output_from_stream(
            iter(vec![text_chunk(
                "第一句还在铺垫。第二句推进冲突！第三句继续延展。",
            )]),
            1,
            Some(17),
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("candidate output");

        assert_eq!(output.full_content, "第一句还在铺垫。第二句推进冲突！");
        assert_eq!(output.chunks, vec!["第一句还在铺垫。第二句推进冲突！"]);
        assert!(output.runtime_state.is_none());
    }

    #[tokio::test]
    async fn should_emit_candidate_output_progress_callback() {
        let mut progress_events = Vec::<ChapterCandidateOutputProgress>::new();

        collect_generation_candidate_output_from_stream(
            iter(vec![text_chunk("甲乙"), text_chunk("丙")]),
            1,
            None,
            None,
            |_chunk, progress| {
                progress_events.push(progress);
                async { Ok(()) }
            },
        )
        .await
        .expect("candidate output");

        assert_eq!(
            progress_events,
            vec![
                ChapterCandidateOutputProgress {
                    current_chars: 2,
                    chunk_count: 1,
                },
                ChapterCandidateOutputProgress {
                    current_chars: 3,
                    chunk_count: 2,
                },
            ]
        );
    }

    #[tokio::test]
    async fn should_propagate_candidate_output_stream_error() {
        let error = collect_generation_candidate_output_from_stream(
            iter(vec![
                text_chunk("正文"),
                Err("provider stream failed".to_string()),
            ]),
            1,
            None,
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect_err("stream error");

        assert_eq!(error, "provider stream failed");
    }

    #[tokio::test]
    async fn should_leave_missing_runtime_state_unmodified() {
        let output = collect_generation_candidate_output_from_stream(
            iter(Vec::<Result<AIStreamChunk, String>>::new()),
            0,
            Some(10),
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("empty candidate output");

        assert_eq!(output.full_content, "");
        assert_eq!(output.chunks, Vec::<String>::new());
    }

    #[tokio::test]
    async fn should_replace_non_object_runtime_state_like_python_sync_owner() {
        let mut runtime_state = Value::String("legacy".to_string());

        collect_generation_candidate_output_from_stream(
            iter(vec![text_chunk("正文")]),
            0,
            None,
            Some(&mut runtime_state),
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("candidate output");

        assert_eq!(runtime_state["candidate_index"], 1);
        assert_eq!(runtime_state["candidate_total"], 1);
        assert_eq!(runtime_state["current_chars"], 2);
    }

    fn assert_no_deleted_python_service_source_map(contract: &serde_json::Value) {
        for key in ["python_source_map", "source_map_files", "rollback_files"] {
            let Some(items) = contract.get(key).and_then(|value| value.as_array()) else {
                continue;
            };
            assert!(
                !items.iter().any(|item| item
                    .as_str()
                    .is_some_and(|path| path.starts_with("backend/app/services/"))),
                "{key} must not retain deleted backend/app/services source-map paths"
            );
        }

        if let Some(rollback_files) = contract
            .get("rollback_boundary")
            .and_then(|value| value.get("rollback_files"))
            .and_then(|value| value.as_array())
        {
            assert!(
                !rollback_files.iter().any(|item| item
                    .as_str()
                    .is_some_and(|path| path.starts_with("backend/app/services/"))),
                "rollback_boundary.rollback_files must not retain deleted backend/app/services paths"
            );
        }
    }

    #[test]
    fn should_publish_chapter_candidate_output_owner_contract() {
        let contract = build_chapter_candidate_output_owner_contract();
        assert_no_deleted_python_service_source_map(&contract);

        assert_eq!(contract["owner"], "chapter_candidate_output_service");
        assert_eq!(
            contract["scope"],
            "candidate_provider_output_stream_chunk_runtime_state_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/tests/test_services/test_chapter_candidate_output_service.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_output_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "collect_generation_candidate_output"
        );
        assert_eq!(
            contract["behavior_contract"]["stream_policy"][2],
            "every provider chunk is appended to full_content and chunks"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_policy"][3],
            "non-object runtime state is replaced through the shared runtime-state owner"
        );
        assert_eq!(
            contract["behavior_contract"]["trimming_policy"][3],
            "trimming is delegated to chapter_narrative_cleaner_service"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_generation_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["candidate_runtime_state_owner_contract"]["owner"],
            "chapter_candidate_runtime_state_service"
        );
        assert_eq!(
            contract["candidate_runtime_state_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_runtime_state_owner_source_map_deleted"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][2],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["stream_collection_owner"],
            "collect_generation_candidate_output"
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
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_output_owner_executor_source_map_deleted"
        );
    }
}
