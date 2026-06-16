use serde_json::{json, Value};

use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

pub(crate) fn build_batch_generation_selected_candidate_event_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::selected_candidate_event_owner",
        "scope": "selected_candidate_snapshot_and_chunk_event_projection",
        "python_source_map": [
            "backend/app/services/chapter_candidate_event_service.py",
            "backend/app/services/chapter_candidate_view_service.py",
            "backend/app/services/batch_generation_candidate_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/selected_candidate_event_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_batch_generation_selected_candidate_event_snapshot",
                "build_batch_generation_selected_candidate_event_batch"
            ],
            "projection_helpers": [
                "snapshot_chapter_candidate_event_view",
                "build_batch_generation_selected_candidate_progress_event",
                "build_batch_generation_chunk_event",
                "quality_gate_plan_allows_selected_candidate_chunks"
            ],
            "selected_candidate_view_fields": [
                "candidate_index",
                "candidate_count",
                "winner_candidate_index",
                "word_count",
                "generation_path",
                "attempt_kind",
                "rerank_used",
                "word_budget_repair_used",
                "full_content",
                "candidate_chunks",
                "quality_metrics",
                "quality_gate_plan"
            ],
            "selected_candidate_batch_contract": {
                "stream_task_required": true,
                "progress_event_first": true,
                "chunk_events_require_stream_chunks": true,
                "chunk_events_require_continue_gate": true,
                "snapshot_view_preserved_when_stream_task_missing": true
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_candidate_selected_event_projection_as_source_map_until_same_round_runtime_readiness_closeout",
            "runtime_state_keys": [
                "selected_candidate",
                "selected_candidate_events",
                "last_event"
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateEventChapter {
    pub(crate) id: String,
    pub(crate) chapter_number: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateEventView {
    pub(crate) candidate_index: i64,
    pub(crate) candidate_count: i64,
    pub(crate) winner_candidate_index: i64,
    pub(crate) word_count: i64,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
    pub(crate) rerank_used: bool,
    pub(crate) word_budget_repair_used: bool,
    pub(crate) full_content: String,
    pub(crate) candidate_chunks: Vec<String>,
    pub(crate) quality_metrics: serde_json::Map<String, Value>,
    pub(crate) quality_gate_plan: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationSelectedCandidateEventBatchInput {
    pub(crate) stream_task_id: Option<String>,
    pub(crate) stream_chunks: bool,
    pub(crate) chapter: ChapterCandidateEventChapter,
    pub(crate) selected_candidate: Value,
    pub(crate) candidate_word_count: i64,
    pub(crate) quality_gate_plan: serde_json::Map<String, Value>,
    pub(crate) chapter_context_stats: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationSelectedCandidateEventBatch {
    pub(crate) stream_task_id: Option<String>,
    pub(crate) selected_candidate_view: ChapterCandidateEventView,
    pub(crate) events: Vec<Value>,
}

pub(crate) fn build_batch_generation_selected_candidate_event_batch(
    input: BatchGenerationSelectedCandidateEventBatchInput,
) -> BatchGenerationSelectedCandidateEventBatch {
    let selected_candidate_view =
        snapshot_chapter_candidate_event_view(Some(&input.selected_candidate));
    let Some(stream_task_id) = input.stream_task_id else {
        return BatchGenerationSelectedCandidateEventBatch {
            stream_task_id: None,
            selected_candidate_view,
            events: Vec::new(),
        };
    };

    let mut events = vec![build_batch_generation_selected_candidate_progress_event(
        &input.chapter,
        &selected_candidate_view,
        input.candidate_word_count,
        &input.chapter_context_stats,
    )];

    if input.stream_chunks
        && quality_gate_plan_allows_selected_candidate_chunks(&input.quality_gate_plan)
    {
        events.extend(
            selected_candidate_view
                .candidate_chunks
                .iter()
                .map(|chunk| build_batch_generation_chunk_event(&input.chapter, chunk)),
        );
    }

    BatchGenerationSelectedCandidateEventBatch {
        stream_task_id: Some(stream_task_id),
        selected_candidate_view,
        events,
    }
}

pub(crate) fn build_batch_generation_selected_candidate_event_snapshot(
    generated: &GeneratedChapterResult,
    stream_chunks: bool,
) -> Option<Value> {
    let selected_candidate = generated.selected_candidate_event_source.as_ref()?;
    let quality_gate_plan = selected_candidate
        .get("quality_gate_plan")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let batch = build_batch_generation_selected_candidate_event_batch(
        BatchGenerationSelectedCandidateEventBatchInput {
            stream_task_id: Some("batch-runtime-snapshot".to_string()),
            stream_chunks,
            chapter: ChapterCandidateEventChapter {
                id: generated.chapter_id.clone(),
                chapter_number: generated.chapter_number,
            },
            selected_candidate: selected_candidate.clone(),
            candidate_word_count: generated.word_count as i64,
            quality_gate_plan,
            chapter_context_stats: serde_json::Map::new(),
        },
    );

    if batch.events.is_empty() {
        None
    } else {
        Some(json!({
            "last_event": "selected_candidate",
            "selected_candidate_events": batch.events,
        }))
    }
}

fn build_batch_generation_selected_candidate_progress_event(
    chapter: &ChapterCandidateEventChapter,
    selected_candidate_view: &ChapterCandidateEventView,
    candidate_word_count: i64,
    chapter_context_stats: &serde_json::Map<String, Value>,
) -> Value {
    json!({
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": format!(
            "Selected chapter {} candidate {}/{} ({} chars)",
            chapter.chapter_number,
            selected_candidate_view.winner_candidate_index,
            selected_candidate_view.candidate_count,
            candidate_word_count
        ),
        "progress": 70,
        "status": "running",
        "phase": "generating",
        "candidate_index": selected_candidate_view.candidate_index,
        "candidate_count": selected_candidate_view.candidate_count,
        "word_count": candidate_word_count.max(0),
        "generation_path": selected_candidate_view.generation_path,
        "attempt_kind": selected_candidate_view.attempt_kind,
        "rerank_used": selected_candidate_view.rerank_used,
        "word_budget_repair_used": selected_candidate_view.word_budget_repair_used,
        "winner_candidate_index": selected_candidate_view.winner_candidate_index,
        "pre_compaction_total_length": chapter_context_stats.get("pre_compaction_total_length").cloned(),
        "context_budget_limit": chapter_context_stats.get("context_budget_limit").cloned(),
        "compaction_applied": chapter_context_stats.get("compaction_applied").cloned(),
        "compaction_details": chapter_context_stats.get("compaction_details").cloned(),
    })
}

fn build_batch_generation_chunk_event(
    chapter: &ChapterCandidateEventChapter,
    chunk: &str,
) -> Value {
    json!({
        "type": "chunk",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "content": chunk,
    })
}

fn snapshot_chapter_candidate_event_view(candidate: Option<&Value>) -> ChapterCandidateEventView {
    let source = candidate.and_then(Value::as_object);
    let full_content = source
        .and_then(|item| item.get("full_content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let candidate_index = positive_i64_from_event_object(source, "candidate_index", 1).max(1);
    let candidate_count = positive_i64_from_event_object(source, "candidate_count", 1).max(1);
    let winner_candidate_index =
        positive_i64_from_event_object(source, "winner_candidate_index", candidate_index).max(1);
    let word_count = source
        .and_then(|item| item.get("word_count"))
        .and_then(value_to_i64_like_python_from_event)
        .unwrap_or_else(|| full_content.chars().count() as i64)
        .max(0);
    let candidate_chunks = source
        .and_then(|item| item.get("candidate_chunks"))
        .and_then(Value::as_array)
        .map(|chunks| {
            chunks
                .iter()
                .map(|chunk| {
                    chunk
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| chunk.to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    ChapterCandidateEventView {
        candidate_index,
        candidate_count,
        winner_candidate_index,
        word_count,
        generation_path: trimmed_string_from_event_object(source, "generation_path"),
        attempt_kind: trimmed_string_from_event_object(source, "attempt_kind"),
        rerank_used: source
            .and_then(|item| item.get("rerank_used"))
            .is_some_and(value_is_python_truthy_from_event),
        word_budget_repair_used: source
            .and_then(|item| item.get("word_budget_repair_used"))
            .is_some_and(value_is_python_truthy_from_event),
        full_content,
        candidate_chunks,
        quality_metrics: object_field_from_event_object(source, "quality_metrics"),
        quality_gate_plan: object_field_from_event_object(source, "quality_gate_plan"),
    }
}

fn positive_i64_from_event_object(
    source: Option<&serde_json::Map<String, Value>>,
    key: &str,
    default: i64,
) -> i64 {
    source
        .and_then(|item| item.get(key))
        .and_then(value_to_i64_like_python_from_event)
        .unwrap_or(default)
}

fn trimmed_string_from_event_object(
    source: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> String {
    source
        .and_then(|item| item.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn object_field_from_event_object(
    source: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> serde_json::Map<String, Value> {
    source
        .and_then(|item| item.get(key))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn quality_gate_plan_allows_selected_candidate_chunks(
    plan: &serde_json::Map<String, Value>,
) -> bool {
    let action = plan
        .get("action")
        .filter(|value| value_is_python_truthy_from_event(value))
        .map(value_to_python_string_from_event)
        .unwrap_or_else(|| "continue".to_string());
    action == "continue"
}

fn value_to_python_string_from_event(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "None".to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn value_to_i64_like_python_from_event(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(value) => value.trim().parse::<f64>().ok().map(|value| value as i64),
        Value::Bool(value) => Some(i64::from(*value)),
        _ => None,
    }
}

fn value_is_python_truthy_from_event(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
