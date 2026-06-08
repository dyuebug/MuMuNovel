use std::future::Future;

use futures::{Stream, StreamExt};
use serde_json::Value;

use crate::ai::service::AIService;
use crate::ai::types::{AIStreamChunk, ToolDef};
use crate::services::chapter_candidate_runtime_state_service::{
    snapshot_chapter_candidate_runtime_state, sync_chapter_candidate_runtime_state,
    ChapterCandidateRuntimeStatePatch,
};
use crate::services::chapter_narrative_cleaner_service::trim_text_to_sentence_boundary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateOutput {
    pub(crate) full_content: String,
    pub(crate) chunks: Vec<String>,
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

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use tokio_stream::iter;

    use crate::ai::types::AIStreamChunk;

    use super::{collect_generation_candidate_output_from_stream, ChapterCandidateOutputProgress};

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
}
