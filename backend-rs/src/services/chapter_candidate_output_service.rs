use std::{
    fmt,
    future::Future,
    sync::{Arc, Mutex},
};

use futures::{Stream, StreamExt};
use serde_json::Value;

use crate::ai::execution_trace::{AIExecutionOutcome, AIExecutionTraceV1, TrackedAIStream};
use crate::ai::service::AIService;
use crate::ai::types::{AIRequestError, AIStreamChunk, ToolDef};
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrackedChapterCandidateOutput {
    pub(crate) output: ChapterCandidateOutput,
    pub(crate) execution: AIExecutionTraceV1,
}

#[derive(Debug)]
pub(crate) enum TrackedChapterCandidateOutputError {
    Provider(AIRequestError),
    Other(String),
}

impl fmt::Display for TrackedChapterCandidateOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(formatter),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TrackedChapterCandidateOutputError {}

struct CollectedChapterCandidateOutput {
    output: ChapterCandidateOutput,
    stopped_by_max_output_chars: bool,
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
    collect_generation_candidate_output_with_reasoning(request, on_chunk, |_reasoning| async {
        Ok(())
    })
    .await
}

pub(crate) async fn collect_generation_candidate_output_with_reasoning<F, Fut, R, RFut>(
    request: ChapterCandidateOutputRequest<'_>,
    on_chunk: F,
    on_reasoning: R,
) -> Result<ChapterCandidateOutput, String>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    let stream = request.ai_service.generate_text_stream(
        request.prompt,
        request.system_prompt,
        request.tools,
    );
    collect_generation_candidate_output_from_stream_with_reasoning(
        stream,
        request.candidate_index,
        request.max_output_chars,
        request.runtime_state,
        on_chunk,
        on_reasoning,
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_tracked<F, Fut>(
    request: ChapterCandidateOutputRequest<'_>,
    allow_model_fallback: bool,
    on_chunk: F,
) -> Result<TrackedChapterCandidateOutput, TrackedChapterCandidateOutputError>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    collect_generation_candidate_output_tracked_with_reasoning(
        request,
        allow_model_fallback,
        on_chunk,
        |_reasoning| async { Ok(()) },
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_tracked_with_reasoning<F, Fut, R, RFut>(
    request: ChapterCandidateOutputRequest<'_>,
    allow_model_fallback: bool,
    on_chunk: F,
    on_reasoning: R,
) -> Result<TrackedChapterCandidateOutput, TrackedChapterCandidateOutputError>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    let tracked_stream = request.ai_service.generate_text_stream_tracked(
        request.prompt,
        request.system_prompt,
        request.tools,
        allow_model_fallback,
    );
    collect_generation_candidate_output_from_tracked_stream_with_reasoning(
        tracked_stream,
        request.candidate_index,
        request.max_output_chars,
        request.runtime_state,
        on_chunk,
        on_reasoning,
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_from_stream<S, F, Fut>(
    stream: S,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    on_chunk: F,
) -> Result<ChapterCandidateOutput, String>
where
    S: Stream<Item = Result<AIStreamChunk, String>> + Unpin,
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    collect_generation_candidate_output_from_stream_with_reasoning(
        stream,
        candidate_index,
        max_output_chars,
        runtime_state,
        on_chunk,
        |_reasoning| async { Ok(()) },
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_from_stream_with_reasoning<
    S,
    F,
    Fut,
    R,
    RFut,
>(
    stream: S,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    on_chunk: F,
    on_reasoning: R,
) -> Result<ChapterCandidateOutput, String>
where
    S: Stream<Item = Result<AIStreamChunk, String>> + Unpin,
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    collect_generation_candidate_output_from_stream_internal(
        stream,
        candidate_index,
        max_output_chars,
        runtime_state,
        on_chunk,
        on_reasoning,
    )
    .await
    .map(|collected| collected.output)
}

pub(crate) async fn collect_generation_candidate_output_from_tracked_stream<F, Fut>(
    tracked_stream: TrackedAIStream,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    on_chunk: F,
) -> Result<TrackedChapterCandidateOutput, TrackedChapterCandidateOutputError>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    collect_generation_candidate_output_from_tracked_stream_with_reasoning(
        tracked_stream,
        candidate_index,
        max_output_chars,
        runtime_state,
        on_chunk,
        |_reasoning| async { Ok(()) },
    )
    .await
}

pub(crate) async fn collect_generation_candidate_output_from_tracked_stream_with_reasoning<
    F,
    Fut,
    R,
    RFut,
>(
    tracked_stream: TrackedAIStream,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    on_chunk: F,
    on_reasoning: R,
) -> Result<TrackedChapterCandidateOutput, TrackedChapterCandidateOutputError>
where
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    let provider_error = Arc::new(Mutex::new(None));
    let stream_provider_error = Arc::clone(&provider_error);
    let string_stream = tracked_stream.stream.map(move |chunk_result| {
        chunk_result.map_err(|error| {
            let message = error.to_string();
            if let Ok(mut slot) = stream_provider_error.lock() {
                *slot = Some(error);
            }
            message
        })
    });
    let collected = collect_generation_candidate_output_from_stream_internal(
        string_stream,
        candidate_index,
        max_output_chars,
        runtime_state,
        on_chunk,
        on_reasoning,
    )
    .await
    .map_err(|message| {
        provider_error
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .map(TrackedChapterCandidateOutputError::Provider)
            .unwrap_or(TrackedChapterCandidateOutputError::Other(message))
    })?;
    let mut execution = tracked_stream.completion.await.map_err(|_| {
        TrackedChapterCandidateOutputError::Other(
            "candidate execution trace completion channel closed".to_string(),
        )
    })?;

    if collected.stopped_by_max_output_chars && execution.outcome == AIExecutionOutcome::Failed {
        execution.outcome = AIExecutionOutcome::Succeeded;
    }

    Ok(TrackedChapterCandidateOutput {
        output: collected.output,
        execution,
    })
}

async fn collect_generation_candidate_output_from_stream_internal<S, F, Fut, R, RFut>(
    mut stream: S,
    candidate_index: i64,
    max_output_chars: Option<usize>,
    runtime_state: Option<&mut Value>,
    mut on_chunk: F,
    mut on_reasoning: R,
) -> Result<CollectedChapterCandidateOutput, String>
where
    S: Stream<Item = Result<AIStreamChunk, String>> + Unpin,
    F: FnMut(String, ChapterCandidateOutputProgress) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    let mut full_content = String::new();
    let mut chunks = Vec::new();
    let normalized_candidate_index = candidate_index.max(1);
    let mut runtime_state = runtime_state;
    let mut candidate_total = normalized_candidate_index;
    let mut stopped_by_max_output_chars = false;

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
        if let Some(reasoning_content) = chunk.reasoning_content.filter(|value| !value.is_empty()) {
            on_reasoning(reasoning_content).await?;
        }
        let Some(chunk_content) = chunk.content.filter(|value| !value.is_empty()) else {
            continue;
        };
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
            stopped_by_max_output_chars = true;
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

    Ok(CollectedChapterCandidateOutput {
        output: ChapterCandidateOutput {
            full_content,
            chunks,
            runtime_state: runtime_state.as_deref().cloned(),
        },
        stopped_by_max_output_chars,
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
    use tokio::sync::{mpsc, oneshot};
    use tokio_stream::{iter, wrappers::ReceiverStream};

    use crate::ai::execution_trace::{
        AIExecutionOutcome, AIExecutionTraceV1, TrackedAIStream, AI_EXECUTION_TRACE_SCHEMA_VERSION,
    };
    use crate::ai::types::AIStreamChunk;

    use super::{
        build_chapter_candidate_output_owner_contract,
        collect_generation_candidate_output_from_stream,
        collect_generation_candidate_output_from_stream_with_reasoning,
        collect_generation_candidate_output_from_tracked_stream, ChapterCandidateOutputProgress,
    };

    fn text_chunk(content: &str) -> Result<AIStreamChunk, String> {
        Ok(AIStreamChunk {
            content: Some(content.to_string()),
            reasoning_content: None,
            tool_calls: None,
            done: false,
            finish_reason: None,
        })
    }

    fn reasoning_chunk(content: &str) -> Result<AIStreamChunk, String> {
        Ok(AIStreamChunk {
            content: None,
            reasoning_content: Some(content.to_string()),
            tool_calls: None,
            done: false,
            finish_reason: None,
        })
    }

    fn execution_trace(outcome: AIExecutionOutcome) -> AIExecutionTraceV1 {
        AIExecutionTraceV1 {
            schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
            requested_provider: "openai".to_string(),
            requested_model: "gpt-primary".to_string(),
            actual_provider: "openai".to_string(),
            actual_model: "gpt-primary".to_string(),
            outcome,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        }
    }

    async fn tracked_stream(
        items: Vec<Result<AIStreamChunk, String>>,
        outcome: AIExecutionOutcome,
    ) -> TrackedAIStream {
        tracked_stream_with_typed_errors(
            items
                .into_iter()
                .map(|item| item.map_err(crate::ai::types::AIRequestError::new))
                .collect(),
            outcome,
        )
        .await
    }

    async fn tracked_stream_with_typed_errors(
        items: Vec<Result<AIStreamChunk, crate::ai::types::AIRequestError>>,
        outcome: AIExecutionOutcome,
    ) -> TrackedAIStream {
        let (tx, rx) = mpsc::channel(items.len().max(1));
        for item in items {
            tx.send(item).await.expect("seed tracked stream");
        }
        drop(tx);
        let (completion_tx, completion_rx) = oneshot::channel();
        completion_tx
            .send(execution_trace(outcome))
            .expect("seed execution trace");
        TrackedAIStream {
            stream: ReceiverStream::new(rx),
            completion: completion_rx,
        }
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
    async fn should_emit_reasoning_without_mixing_it_into_candidate_output() {
        let mut content_events = Vec::<String>::new();
        let mut reasoning_events = Vec::<String>::new();

        let output = collect_generation_candidate_output_from_stream_with_reasoning(
            iter(vec![
                reasoning_chunk("分析一"),
                text_chunk("正文"),
                reasoning_chunk("分析二"),
            ]),
            1,
            None,
            None,
            |chunk, _progress| {
                content_events.push(chunk);
                async { Ok(()) }
            },
            |reasoning| {
                reasoning_events.push(reasoning);
                async { Ok(()) }
            },
        )
        .await
        .expect("candidate output");

        assert_eq!(output.full_content, "正文");
        assert_eq!(output.chunks, vec!["正文"]);
        assert_eq!(content_events, vec!["正文"]);
        assert_eq!(reasoning_events, vec!["分析一", "分析二"]);
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
    async fn should_normalize_tracked_channel_close_failure_after_controlled_char_limit() {
        let output = collect_generation_candidate_output_from_tracked_stream(
            tracked_stream(
                vec![text_chunk("甲乙丙"), text_chunk("不应继续消费")],
                AIExecutionOutcome::Failed,
            )
            .await,
            1,
            Some(2),
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("tracked candidate output");

        assert_eq!(output.output.full_content, "甲乙。");
        assert_eq!(output.execution.outcome, AIExecutionOutcome::Succeeded);
    }

    #[tokio::test]
    async fn should_preserve_tracked_failed_outcome_without_controlled_char_limit() {
        let output = collect_generation_candidate_output_from_tracked_stream(
            tracked_stream(vec![text_chunk("正文")], AIExecutionOutcome::Failed).await,
            1,
            None,
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect("tracked candidate output");

        assert_eq!(output.output.full_content, "正文");
        assert_eq!(output.execution.outcome, AIExecutionOutcome::Failed);
    }

    #[tokio::test]
    async fn should_propagate_tracked_stream_error_before_reading_completion() {
        let provider_error =
            crate::ai::types::AIRequestError::with_transport_status_and_retry_after(
                "provider stream failed",
                json!({"safe": true}),
                Some(429),
                Some(120),
            );
        let error = collect_generation_candidate_output_from_tracked_stream(
            tracked_stream_with_typed_errors(
                vec![
                    text_chunk("正文").map_err(crate::ai::types::AIRequestError::new),
                    Err(provider_error),
                ],
                AIExecutionOutcome::Failed,
            )
            .await,
            1,
            None,
            None,
            |_chunk, _progress| async { Ok(()) },
        )
        .await
        .expect_err("tracked stream error");

        match error {
            super::TrackedChapterCandidateOutputError::Provider(error) => {
                assert_eq!(error.message, "provider stream failed");
                assert_eq!(error.status_code, Some(429));
                assert_eq!(error.retry_after_seconds, Some(120));
            }
            super::TrackedChapterCandidateOutputError::Other(message) => {
                panic!("expected typed provider error, got: {message}");
            }
        }
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
