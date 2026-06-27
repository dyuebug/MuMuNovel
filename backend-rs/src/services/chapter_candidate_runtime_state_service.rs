// Rust owner for candidate runtime state originally mapped from Python
// chapter_candidate_runtime_state_service.py. Generation, output collection,
// repair, finalize, event, and batch payload owners now consume this module
// directly for attempt labels, snapshots, checkpoint fields, and state sync.

use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateAttemptLabels {
    pub(crate) generation_path: &'static str,
    pub(crate) attempt_kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRuntimeStateSnapshot {
    pub(crate) candidate_total: i64,
    pub(crate) candidate_count: i64,
    pub(crate) candidate_index: i64,
    pub(crate) current_chars: i64,
    pub(crate) word_count: i64,
    pub(crate) chunk_count: i64,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
    pub(crate) rerank_used: bool,
    pub(crate) word_budget_repair_used: bool,
    pub(crate) winner_candidate_index: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChapterCandidateRuntimeStatePatch {
    pub(crate) current_chars: Option<i64>,
    pub(crate) chunk_count: Option<i64>,
    pub(crate) generation_path: Option<String>,
    pub(crate) attempt_kind: Option<String>,
    pub(crate) rerank_used: Option<bool>,
    pub(crate) word_budget_repair_used: Option<bool>,
    pub(crate) winner_candidate_index: Option<i64>,
}

pub(crate) fn resolve_generation_attempt_labels(
    candidate_index: i64,
    is_word_budget_repair: bool,
) -> ChapterCandidateAttemptLabels {
    let normalized_candidate_index = candidate_index.max(1);
    if is_word_budget_repair {
        return ChapterCandidateAttemptLabels {
            generation_path: "word_budget_repair",
            attempt_kind: "word_budget_repair",
        };
    }
    if normalized_candidate_index > 1 {
        return ChapterCandidateAttemptLabels {
            generation_path: "rerank_retry",
            attempt_kind: "rerank_candidate",
        };
    }

    ChapterCandidateAttemptLabels {
        generation_path: "single_pass",
        attempt_kind: "initial_candidate",
    }
}

#[cfg(test)]
pub(crate) fn build_chapter_candidate_runtime_state(max_candidates: i64) -> Value {
    let normalized_max_candidates = max_candidates.max(1);
    json!({
        "candidate_total": normalized_max_candidates,
        "candidate_count": normalized_max_candidates,
        "candidate_index": 1,
        "current_chars": 0,
        "word_count": 0,
        "chunk_count": 0,
        "generation_path": "single_pass",
        "attempt_kind": "initial_candidate",
        "rerank_used": false,
        "word_budget_repair_used": false,
        "winner_candidate_index": Value::Null,
    })
}

pub(crate) fn snapshot_chapter_candidate_runtime_state(
    runtime_state: Option<&Value>,
    default_candidate_total: i64,
) -> ChapterCandidateRuntimeStateSnapshot {
    let normalized_default_candidate_total = default_candidate_total.max(1);
    let candidate_index = positive_i64_from_object(runtime_state, "candidate_index", 1).max(1);
    let candidate_total = positive_i64_from_object(
        runtime_state,
        "candidate_total",
        normalized_default_candidate_total,
    )
    .max(candidate_index);
    let candidate_count =
        positive_i64_from_object(runtime_state, "candidate_count", candidate_total).max(1);
    let current_chars = non_negative_i64_from_object(runtime_state, "current_chars", 0);
    let word_count = non_negative_i64_from_object(runtime_state, "word_count", current_chars);
    let chunk_count = non_negative_i64_from_object(runtime_state, "chunk_count", 0);
    let generation_path =
        trimmed_string_from_object(runtime_state, "generation_path", "single_pass");
    let attempt_kind =
        trimmed_string_from_object(runtime_state, "attempt_kind", "initial_candidate");
    let rerank_used = runtime_state
        .and_then(|state| state.get("rerank_used"))
        .is_some_and(value_is_python_truthy);
    let word_budget_repair_used = runtime_state
        .and_then(|state| state.get("word_budget_repair_used"))
        .is_some_and(value_is_python_truthy);
    let winner_candidate_index = runtime_state
        .and_then(|state| state.get("winner_candidate_index"))
        .and_then(value_to_i64_like_python)
        .map(|value| value.max(1));

    ChapterCandidateRuntimeStateSnapshot {
        candidate_total,
        candidate_count,
        candidate_index,
        current_chars,
        word_count,
        chunk_count,
        generation_path,
        attempt_kind,
        rerank_used,
        word_budget_repair_used,
        winner_candidate_index,
    }
}

pub(crate) fn sync_chapter_candidate_runtime_state(
    runtime_state: Option<&mut Value>,
    candidate_index: i64,
    candidate_total: i64,
    patch: ChapterCandidateRuntimeStatePatch,
) {
    let Some(runtime_state) = runtime_state else {
        return;
    };
    if !runtime_state.is_object() {
        *runtime_state = json!({});
    }

    let Some(state) = runtime_state.as_object_mut() else {
        return;
    };
    let normalized_candidate_index = candidate_index.max(1);
    let normalized_candidate_total = candidate_total.max(normalized_candidate_index);
    state.insert(
        "candidate_index".to_string(),
        json!(normalized_candidate_index),
    );
    state.insert(
        "candidate_total".to_string(),
        json!(normalized_candidate_total),
    );
    state.insert(
        "candidate_count".to_string(),
        json!(normalized_candidate_total),
    );

    if let Some(current_chars) = patch.current_chars {
        let normalized_chars = current_chars.max(0);
        state.insert("current_chars".to_string(), json!(normalized_chars));
        state.insert("word_count".to_string(), json!(normalized_chars));
    }
    if let Some(chunk_count) = patch.chunk_count {
        state.insert("chunk_count".to_string(), json!(chunk_count.max(0)));
    }
    if let Some(generation_path) = non_empty_trimmed_string(patch.generation_path) {
        state.insert("generation_path".to_string(), json!(generation_path));
    }
    if let Some(attempt_kind) = non_empty_trimmed_string(patch.attempt_kind) {
        state.insert("attempt_kind".to_string(), json!(attempt_kind));
    }
    if let Some(rerank_used) = patch.rerank_used {
        state.insert("rerank_used".to_string(), json!(rerank_used));
    }
    if let Some(word_budget_repair_used) = patch.word_budget_repair_used {
        state.insert(
            "word_budget_repair_used".to_string(),
            json!(word_budget_repair_used),
        );
    }
    if let Some(winner_candidate_index) = patch.winner_candidate_index {
        state.insert(
            "winner_candidate_index".to_string(),
            json!(winner_candidate_index.max(1)),
        );
    }
}

pub(crate) fn insert_python_query_snapshot_candidate_runtime_fields(
    checkpoint: &mut Map<String, Value>,
) {
    const RAW_FIELDS: [&str; 6] = [
        "candidate_index",
        "candidate_count",
        "word_count",
        "generation_path",
        "attempt_kind",
        "winner_candidate_index",
    ];
    const BOOL_FIELDS: [&str; 2] = ["rerank_used", "word_budget_repair_used"];

    for key in RAW_FIELDS {
        checkpoint
            .entry(key.to_string())
            .or_insert_with(|| Value::Null);
    }
    for key in BOOL_FIELDS {
        let value = checkpoint
            .get(key)
            .and_then(Value::as_bool)
            .map(Value::Bool)
            .unwrap_or(Value::Null);
        checkpoint.insert(key.to_string(), value);
    }
}

fn positive_i64_from_object(runtime_state: Option<&Value>, key: &str, default_value: i64) -> i64 {
    runtime_state
        .and_then(|state| state.get(key))
        .and_then(value_to_i64_like_python)
        .unwrap_or(default_value)
        .max(1)
}

fn non_negative_i64_from_object(
    runtime_state: Option<&Value>,
    key: &str,
    default_value: i64,
) -> i64 {
    runtime_state
        .and_then(|state| state.get(key))
        .and_then(value_to_i64_like_python)
        .unwrap_or(default_value)
        .max(0)
}

fn trimmed_string_from_object(
    runtime_state: Option<&Value>,
    key: &str,
    default_value: &str,
) -> String {
    runtime_state
        .and_then(|state| state.get(key))
        .map(value_to_python_string)
        .and_then(non_empty_trimmed_string)
        .unwrap_or_else(|| default_value.to_string())
}

fn non_empty_trimmed_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn value_to_i64_like_python(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|value| value as i64)),
        Value::Bool(value) => Some(if *value { 1 } else { 0 }),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_python_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_is_python_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Null => false,
        Value::Number(number) => {
            number.as_i64().is_some_and(|value| value != 0)
                || number.as_u64().is_some_and(|value| value != 0)
                || number.as_f64().is_some_and(|value| value != 0.0)
        }
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

pub(crate) fn build_chapter_candidate_runtime_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_candidate_runtime_state_service",
        "scope": "candidate_attempt_label_snapshot_sync_and_checkpoint_field_owner",
        "python_source_map": [
            "backend/tests/test_services/test_chapter_candidate_runtime_state_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_generation_attempt_labels",
                "build_chapter_candidate_runtime_state",
                "snapshot_chapter_candidate_runtime_state",
                "sync_chapter_candidate_runtime_state",
                "insert_python_query_snapshot_candidate_runtime_fields"
            ],
            "state_fields": [
                "candidate_total",
                "candidate_count",
                "candidate_index",
                "current_chars",
                "word_count",
                "chunk_count",
                "generation_path",
                "attempt_kind",
                "rerank_used",
                "word_budget_repair_used",
                "winner_candidate_index"
            ],
            "attempt_label_policy": [
                "candidate_index <= 1 and no repair resolves to single_pass / initial_candidate",
                "candidate_index > 1 resolves to rerank_retry / rerank_candidate",
                "word budget repair overrides candidate_index and resolves to word_budget_repair"
            ],
            "snapshot_policy": [
                "default candidate total is normalized to at least 1",
                "candidate_total is never below candidate_index",
                "candidate_count is normalized to at least 1",
                "negative current_chars, word_count, and chunk_count normalize to zero",
                "generation_path and attempt_kind are trimmed with Python defaults",
                "rerank and word-budget flags use Python truthiness",
                "winner_candidate_index is parsed like Python int and normalized to at least 1"
            ],
            "sync_policy": [
                "None runtime state is a no-op",
                "non-object runtime state is replaced with an object",
                "candidate_index and candidate_total are normalized and mirrored into candidate_count",
                "current_chars also updates word_count",
                "empty generation_path and attempt_kind patches are ignored",
                "winner_candidate_index is normalized to at least 1"
            ],
            "checkpoint_policy": [
                "Python query snapshot fields are inserted as null when absent",
                "rerank_used and word_budget_repair_used remain bool-only in query snapshots"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_runtime_state_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_output_service",
            "chapter_candidate_generation_service",
            "chapter_candidate_finalize_service",
            "chapter_candidate_word_budget_repair_service",
            "chapter_candidate_targeted_final_repair_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_read_context_service",
            "chapter_candidate_record_service",
            "chapter_candidate_executor_default_dependency_service"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_runtime_state_python_source_map",
            "python_query_snapshot_compatibility": "insert_python_query_snapshot_candidate_runtime_fields",
            "python_fallback_removal_ready": true,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        },
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
            "attempt_label_owner": "resolve_generation_attempt_labels",
            "runtime_state_snapshot_owner": "snapshot_chapter_candidate_runtime_state",
            "runtime_state_sync_owner": "sync_chapter_candidate_runtime_state",
            "python_query_checkpoint_field_owner": "insert_python_query_snapshot_candidate_runtime_fields",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "candidate runtime-state production python source-map deleted; surviving Python closeout work for this owner is now limited to focused Python regression coverage",
            "status": "rust_chapter_candidate_runtime_state_owner_source_map_deleted"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_chapter_candidate_runtime_state,
        build_chapter_candidate_runtime_state_owner_contract,
        insert_python_query_snapshot_candidate_runtime_fields, resolve_generation_attempt_labels,
        snapshot_chapter_candidate_runtime_state, sync_chapter_candidate_runtime_state,
        ChapterCandidateRuntimeStatePatch,
    };

    #[test]
    fn should_resolve_generation_attempt_labels_like_python_service() {
        let initial = resolve_generation_attempt_labels(1, false);
        assert_eq!(initial.generation_path, "single_pass");
        assert_eq!(initial.attempt_kind, "initial_candidate");

        let rerank = resolve_generation_attempt_labels(2, false);
        assert_eq!(rerank.generation_path, "rerank_retry");
        assert_eq!(rerank.attempt_kind, "rerank_candidate");

        let repair = resolve_generation_attempt_labels(1, true);
        assert_eq!(repair.generation_path, "word_budget_repair");
        assert_eq!(repair.attempt_kind, "word_budget_repair");
    }

    #[test]
    fn should_build_candidate_runtime_state_like_python_service() {
        let state = build_chapter_candidate_runtime_state(0);

        assert_eq!(state["candidate_total"], 1);
        assert_eq!(state["candidate_count"], 1);
        assert_eq!(state["candidate_index"], 1);
        assert_eq!(state["current_chars"], 0);
        assert_eq!(state["word_count"], 0);
        assert_eq!(state["chunk_count"], 0);
        assert_eq!(state["generation_path"], "single_pass");
        assert_eq!(state["attempt_kind"], "initial_candidate");
        assert_eq!(state["rerank_used"], false);
        assert_eq!(state["word_budget_repair_used"], false);
        assert_eq!(state["winner_candidate_index"], Value::Null);
    }

    #[test]
    fn should_snapshot_candidate_runtime_state_with_python_defaults() {
        let snapshot = snapshot_chapter_candidate_runtime_state(
            Some(&json!({
                "candidate_index": 3,
                "candidate_total": 2,
                "candidate_count": 0,
                "current_chars": -5,
                "chunk_count": "4",
                "generation_path": " rerank_retry ",
                "attempt_kind": " rerank_candidate ",
                "rerank_used": "non-empty",
                "word_budget_repair_used": false,
                "winner_candidate_index": "2"
            })),
            0,
        );

        assert_eq!(snapshot.candidate_index, 3);
        assert_eq!(snapshot.candidate_total, 3);
        assert_eq!(snapshot.candidate_count, 1);
        assert_eq!(snapshot.current_chars, 0);
        assert_eq!(snapshot.word_count, 0);
        assert_eq!(snapshot.chunk_count, 4);
        assert_eq!(snapshot.generation_path, "rerank_retry");
        assert_eq!(snapshot.attempt_kind, "rerank_candidate");
        assert!(snapshot.rerank_used);
        assert!(!snapshot.word_budget_repair_used);
        assert_eq!(snapshot.winner_candidate_index, Some(2));
    }

    #[test]
    fn should_sync_candidate_runtime_state_like_python_service() {
        let mut state = json!({"generation_path": "single_pass"});
        sync_chapter_candidate_runtime_state(
            Some(&mut state),
            2,
            1,
            ChapterCandidateRuntimeStatePatch {
                current_chars: Some(128),
                chunk_count: Some(3),
                generation_path: Some(" rerank_retry ".to_string()),
                attempt_kind: Some(" rerank_candidate ".to_string()),
                rerank_used: Some(true),
                word_budget_repair_used: Some(false),
                winner_candidate_index: Some(0),
            },
        );

        assert_eq!(state["candidate_index"], 2);
        assert_eq!(state["candidate_total"], 2);
        assert_eq!(state["candidate_count"], 2);
        assert_eq!(state["current_chars"], 128);
        assert_eq!(state["word_count"], 128);
        assert_eq!(state["chunk_count"], 3);
        assert_eq!(state["generation_path"], "rerank_retry");
        assert_eq!(state["attempt_kind"], "rerank_candidate");
        assert_eq!(state["rerank_used"], true);
        assert_eq!(state["word_budget_repair_used"], false);
        assert_eq!(state["winner_candidate_index"], 1);
    }

    #[test]
    fn should_insert_python_query_snapshot_candidate_runtime_fields() {
        let mut checkpoint = json!({
            "candidate_index": 2,
            "rerank_used": true,
            "word_budget_repair_used": "not-a-bool"
        })
        .as_object()
        .cloned()
        .expect("object checkpoint");

        insert_python_query_snapshot_candidate_runtime_fields(&mut checkpoint);

        assert_eq!(checkpoint["candidate_index"], 2);
        assert_eq!(checkpoint["candidate_count"], Value::Null);
        assert_eq!(checkpoint["word_count"], Value::Null);
        assert_eq!(checkpoint["generation_path"], Value::Null);
        assert_eq!(checkpoint["attempt_kind"], Value::Null);
        assert_eq!(checkpoint["winner_candidate_index"], Value::Null);
        assert_eq!(checkpoint["rerank_used"], true);
        assert_eq!(checkpoint["word_budget_repair_used"], Value::Null);
    }

    #[test]
    fn should_publish_chapter_candidate_runtime_state_owner_contract() {
        let contract = build_chapter_candidate_runtime_state_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_runtime_state_service");
        assert_eq!(
            contract["scope"],
            "candidate_attempt_label_snapshot_sync_and_checkpoint_field_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/tests/test_services/test_chapter_candidate_runtime_state_service.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_runtime_state_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "snapshot_chapter_candidate_runtime_state"
        );
        assert_eq!(
            contract["behavior_contract"]["state_fields"][10],
            "winner_candidate_index"
        );
        assert_eq!(
            contract["behavior_contract"]["snapshot_policy"][5],
            "rerank and word-budget flags use Python truthiness"
        );
        assert_eq!(
            contract["behavior_contract"]["sync_policy"][1],
            "non-object runtime state is replaced with an object"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_output_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
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
            contract["service_runtime_closeout_status"]["runtime_state_snapshot_owner"],
            "snapshot_chapter_candidate_runtime_state"
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
            "rust_chapter_candidate_runtime_state_owner_source_map_deleted"
        );
    }
}
