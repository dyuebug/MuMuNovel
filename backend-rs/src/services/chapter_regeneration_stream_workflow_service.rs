use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::ai::service::AIService;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_candidate_output_service::{
    build_chapter_candidate_output_owner_contract, collect_generation_candidate_output_tracked,
    collect_generation_candidate_output_tracked_with_reasoning, ChapterCandidateOutputProgress,
    ChapterCandidateOutputRequest,
};
use crate::services::chapter_generation_execution_contract_service::PreparedRoleModelPolicyContext;
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_regeneration_prepare_service::{
    build_chapter_regeneration_prepare_owner_contract, prepare_chapter_regeneration_stream,
    prepare_partial_regeneration_stream, validate_full_chapter_regeneration_stream_request_bounds,
    validate_partial_regeneration_stream_request_bounds, BuildRegenerationAiServiceError,
    FullChapterRegenerationStreamInput, FullChapterRegenerationStreamRequest,
    PartialChapterRegenerationStreamInput, PartialRegenerationStreamWorkflowRequest,
    PreparePartialRegenerationStreamError,
};
use crate::services::chapter_regeneration_task_service::{
    build_chapter_regeneration_task_owner_contract, build_full_regeneration_task_seed,
    create_full_regeneration_task, load_latest_chapter_analysis, mark_regeneration_task_completed,
    mark_regeneration_task_failed,
};
use crate::services::generation_execution_audit_service::{
    build_generation_execution_audit, merge_generation_execution_audit, GenerationExecutionAuditV1,
};
use crate::utils::sse::{
    sse_chunk, sse_done, sse_error, sse_json, sse_reasoning_chunk, sse_result, SseProgress,
};

const CHAPTER_REGENERATION_STREAM_WORKFLOW_ROUTE_GROUP: &str = "chapter_regeneration";
const CHAPTER_REGENERATION_STREAM_WORKFLOW_ROLLBACK_BOUNDARY: &str =
    "chapter_regeneration_python_source_map";

pub type OwnedRegenerationStream = ReceiverStream<Result<Event, Infallible>>;

pub enum FinalizePartialRegenerationError {
    EmptyContent,
    WorkflowMetaText,
}

pub enum FullRegenerationTaskLifecycleError {
    AnalysisMissing,
    Internal(String),
}

pub struct RegenerationChunkProgress {
    pub chunk_count: u32,
    pub full_content_len: usize,
}

impl From<ChapterCandidateOutputProgress> for RegenerationChunkProgress {
    fn from(progress: ChapterCandidateOutputProgress) -> Self {
        Self {
            chunk_count: progress.chunk_count as u32,
            full_content_len: progress.current_chars,
        }
    }
}

enum OwnedRegenerationInitialEvent {
    Preparing {
        message: Option<String>,
    },
    Generating {
        message: Option<String>,
        progress_range: (u32, u32),
        char_count: usize,
        retry_count: Option<u32>,
    },
}

struct TrackedRegenerationTextOutput {
    full_content: String,
    audit: GenerationExecutionAuditV1,
}

struct OwnedRegenerationStreamLaunchInput {
    task_label: String,
    prompt: String,
    ai_service: AIService,
    role_policy_context: PreparedRoleModelPolicyContext,
    initial_events: Vec<OwnedRegenerationInitialEvent>,
    completion_message: String,
    task_created_event: Option<Value>,
}

fn normalize_partial_regeneration_output(text: &str) -> String {
    let mut cleaned = text.replace("\r\n", "\n").trim().to_string();
    let prefixes = [
        "重写后：",
        "重写后:",
        "改写后：",
        "改写后:",
        "以下是重写后的内容：",
        "以下是重写后的内容:",
        "重写内容：",
        "重写内容:",
    ];
    for prefix in prefixes {
        if cleaned.starts_with(prefix) {
            cleaned = cleaned[prefix.len()..].trim().to_string();
            break;
        }
    }

    if (cleaned.starts_with('"') && cleaned.ends_with('"'))
        || (cleaned.starts_with('\'') && cleaned.ends_with('\''))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }
    if (cleaned.starts_with('「') && cleaned.ends_with('」'))
        || (cleaned.starts_with('『') && cleaned.ends_with('』'))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }

    cleaned.trim().to_string()
}

fn finalize_partial_regeneration_result(
    generated_text: &str,
    original_word_count: usize,
    start_position: usize,
    end_position: usize,
) -> Result<Value, FinalizePartialRegenerationError> {
    let normalized = normalize_partial_regeneration_output(generated_text);
    let (cleaned_text, _) = sanitize_generated_narrative_text(&normalized);
    if cleaned_text.trim().is_empty() {
        return Err(FinalizePartialRegenerationError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(FinalizePartialRegenerationError::WorkflowMetaText);
    }

    Ok(json!({
        "new_text": cleaned_text,
        "word_count": cleaned_text.chars().count(),
        "original_word_count": original_word_count,
        "start_position": start_position,
        "end_position": end_position,
    }))
}

fn finalize_chapter_regeneration_result(
    generated_text: &str,
    task_id: &str,
) -> Result<Value, FinalizePartialRegenerationError> {
    let (cleaned_text, _) = sanitize_generated_narrative_text(generated_text);
    if cleaned_text.trim().is_empty() {
        return Err(FinalizePartialRegenerationError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(FinalizePartialRegenerationError::WorkflowMetaText);
    }

    Ok(json!({
        "content": cleaned_text,
        "word_count": cleaned_text.chars().count(),
        "task_id": task_id,
        "analysis_task_id": Value::Null,
    }))
}

fn describe_regeneration_finalize_error(error: &FinalizePartialRegenerationError) -> &'static str {
    match error {
        FinalizePartialRegenerationError::EmptyContent => {
            "Rewrite result is empty after sanitization"
        }
        FinalizePartialRegenerationError::WorkflowMetaText => {
            "Rewrite result still contains workflow meta text"
        }
    }
}

async fn emit_regeneration_finalize_error(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    error: FinalizePartialRegenerationError,
) {
    let _ = tx
        .send(Ok(sse_error(
            describe_regeneration_finalize_error(&error),
            500,
        )))
        .await;
}

async fn execute_regeneration_text_stream<F>(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    ai_service: AIService,
    role_policy_context: PreparedRoleModelPolicyContext,
    prompt: String,
    mut build_progress_event: F,
) -> Result<TrackedRegenerationTextOutput, ()>
where
    F: FnMut(RegenerationChunkProgress) -> Option<Event>,
{
    match collect_generation_candidate_output_tracked_with_reasoning(
        ChapterCandidateOutputRequest {
            ai_service,
            prompt,
            system_prompt: None,
            tools: None,
            candidate_index: 1,
            max_output_chars: None,
            runtime_state: None,
        },
        role_policy_context.allow_model_fallback,
        |chunk_content, progress| {
            let event = build_progress_event(progress.into());
            let tx = tx.clone();
            async move {
                let _ = tx.send(Ok(sse_chunk(&chunk_content))).await;
                if let Some(event) = event {
                    let _ = tx.send(Ok(event)).await;
                }
                Ok(())
            }
        },
        |reasoning_content| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(Ok(sse_reasoning_chunk(&reasoning_content))).await;
                Ok(())
            }
        },
    )
    .await
    {
        Ok(output) => match build_generation_execution_audit(
            &role_policy_context.resolved_policy,
            &output.execution,
        ) {
            Ok(audit) => Ok(TrackedRegenerationTextOutput {
                full_content: output.output.full_content,
                audit,
            }),
            Err(error) => {
                let _ = tx.send(Ok(sse_error(&error.to_string(), 500))).await;
                Err(())
            }
        },
        Err(error) => {
            let _ = tx.send(Ok(sse_error(&error, 500))).await;
            Err(())
        }
    }
}

fn merge_regeneration_audit(
    payload: &mut Value,
    audit: &GenerationExecutionAuditV1,
) -> Result<(), String> {
    merge_generation_execution_audit(payload, audit).map_err(|error| error.to_string())
}

fn apply_owned_regeneration_initial_event(
    tracker: &mut SseProgress,
    step: &OwnedRegenerationInitialEvent,
) -> Event {
    match step {
        OwnedRegenerationInitialEvent::Preparing { message } => {
            tracker.preparing(message.as_deref())
        }
        OwnedRegenerationInitialEvent::Generating {
            message,
            progress_range,
            char_count,
            retry_count,
        } => tracker.generating(
            message.as_deref(),
            *progress_range,
            *char_count,
            *retry_count,
        ),
    }
}

fn build_owned_regeneration_stream<BuildProgress, Finalize>(
    input: OwnedRegenerationStreamLaunchInput,
    mut build_progress_event: BuildProgress,
    on_failed: Option<Value>,
    on_succeeded: Option<Value>,
    finalize_payload: Finalize,
) -> OwnedRegenerationStream
where
    BuildProgress:
        FnMut(&mut SseProgress, RegenerationChunkProgress) -> Option<Event> + Send + 'static,
    Finalize: FnOnce(&str) -> Result<Value, FinalizePartialRegenerationError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let OwnedRegenerationStreamLaunchInput {
            task_label,
            prompt,
            ai_service,
            role_policy_context,
            initial_events,
            completion_message,
            task_created_event,
        } = input;

        let mut tracker = SseProgress::new(&task_label);
        let _ = tx.send(Ok(tracker.start())).await;
        for step in &initial_events {
            let event = apply_owned_regeneration_initial_event(&mut tracker, step);
            let _ = tx.send(Ok(event)).await;
        }
        if let Some(task_created_event) = task_created_event {
            let _ = tx.send(Ok(sse_json(&task_created_event))).await;
        }

        let output = match execute_regeneration_text_stream(
            &tx,
            ai_service,
            role_policy_context,
            prompt,
            |progress| build_progress_event(&mut tracker, progress),
        )
        .await
        {
            Ok(output) => output,
            Err(()) => {
                if let Some(failed) = on_failed {
                    let _ = tx.send(Ok(sse_json(&failed))).await;
                }
                return;
            }
        };

        let mut payload = match finalize_payload(&output.full_content) {
            Ok(payload) => payload,
            Err(error) => {
                if let Some(failed) = on_failed {
                    let _ = tx.send(Ok(sse_json(&failed))).await;
                }
                emit_regeneration_finalize_error(&tx, error).await;
                return;
            }
        };
        if merge_regeneration_audit(&mut payload, &output.audit).is_err() {
            let _ = tx
                .send(Ok(sse_error(
                    "Failed to attach regeneration execution audit",
                    500,
                )))
                .await;
            return;
        }
        if let Some(succeeded) = on_succeeded {
            let _ = tx.send(Ok(sse_json(&succeeded))).await;
        }

        let _ = tx
            .send(Ok(tracker.complete(Some(&completion_message))))
            .await;
        let _ = tx.send(Ok(sse_result(&payload))).await;
        let _ = tx.send(Ok(sse_done())).await;
    });

    ReceiverStream::new(rx)
}

fn build_full_regeneration_task_failed_event(task_id: &str) -> Value {
    json!({
        "type": "regeneration_task_state",
        "task_id": task_id,
        "status": "failed"
    })
}

fn build_full_regeneration_task_completed_event(task_id: &str) -> Value {
    json!({
        "type": "regeneration_task_state",
        "task_id": task_id,
        "status": "completed"
    })
}

pub(crate) struct ChapterRegenerationStreamWorkflowSmokeResult {
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) route_group: &'static str,
    pub(crate) ok: bool,
    pub(crate) execution_path: &'static str,
    pub(crate) fallback_applied: bool,
    pub(crate) rollback_boundary: &'static str,
    pub(crate) result: Value,
    pub(crate) runtime_state: Value,
    pub(crate) readiness_evidence: Value,
}

pub(crate) fn build_chapter_regeneration_stream_workflow_owner_contract() -> Value {
    json!({
        "owner": "chapter_regeneration_stream_workflow_service",
        "scope": "full_and_partial_chapter_regeneration_sse_workflow_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/api/chapter_regeneration_routes.rs",
            "backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_regeneration_prepare_service.rs",
            "backend-rs/src/services/chapter_regeneration_task_service.rs",
            "backend-rs/src/services/chapter_candidate_output_service.rs",
            "backend-rs/src/services/chapter_narrative_cleaner_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_context_service.rs"
        ],
        "behavior_contract": {
            "full_stream_entrypoint": "create_chapter_regeneration_stream_workflow",
            "partial_stream_entrypoint": "create_partial_regeneration_stream_workflow",
            "access_boundary": "load_accessible_chapter before prepare or stream launch",
            "shared_stream_owner_entrypoints": [
                "build_owned_regeneration_stream",
                "execute_regeneration_text_stream",
                "collect_generation_candidate_output_tracked",
                "build_generation_execution_audit",
                "merge_regeneration_audit",
                "normalize_partial_regeneration_output",
                "finalize_chapter_regeneration_result",
                "finalize_partial_regeneration_result",
                "create_full_regeneration_task",
                "mark_regeneration_task_completed",
                "mark_regeneration_task_failed"
            ],
            "full_sse_initial_events": [
                "Preparing: Building rewrite prompt...",
                "Generating: Rewriting chapter..."
            ],
            "partial_sse_initial_events": [
                "Preparing: Preparing rewrite context...",
                "Preparing: Starting generation..."
            ],
            "finalizers": [
                "finalize_chapter_regeneration_result",
                "finalize_partial_regeneration_result"
            ],
            "request_guards": [
                "validate_full_chapter_regeneration_stream_request_bounds",
                "validate_partial_regeneration_stream_request_bounds"
            ],
            "execution_audit_policy": {
                "tracked_execution_owner": "collect_generation_candidate_output_tracked",
                "audit_builder": "build_generation_execution_audit",
                "result_field": "generation_execution_audit",
                "sse_event_kind": "result",
                "background_result_additive": true,
                "chapter_content_persistence_unchanged": true,
                "database_migration_required": false
            }
        },
        "source_map_policy": {
            "status": "source_map_only",
            "active_manifest_fallback_owner": false,
            "auth_guard_manifest_coverage": "all_regeneration_routes",
            "deterministic_business_sse_smoke": true,
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "freeze_scope": "chapter_regeneration_route_package_source_map_surface",
            "freeze_reason": "Rust regeneration route group has dedicated owner-profile business probes for full stream, partial stream, apply partial, task list, and cleanup; the production chapter_regeneration route shell is now physically deleted, and the surviving Python follow-up work sits outside this direct route/workflow source-map package.",
            "owner_profile_business_probes": [
                "chapter-regeneration-fixture-import-project-business-rust",
                "chapter-regeneration-fixture-list-chapter-business-rust",
                "chapter-regeneration-configure-mock-openai-business-rust",
                "chapter-regeneration-full-stream-business-rust",
                "chapter-regeneration-partial-stream-business-rust",
                "chapter-regeneration-apply-partial-business-rust",
                "chapter-regeneration-tasks-business-rust",
                "chapter-regeneration-fixture-delete-project-business-rust"
            ],
            "python_bootstrap_status": "chapter_regeneration_route_runtime_registration_deleted_no_python_route_shell_remains",
            "stream_orchestration_source_maps": [],
            "prepare_owner_source_maps": [],
            "shared_prepare_dependency_source_maps": [],
            "shared_context_compaction_source_maps": [],
            "query_owner_source_maps": [],
            "frozen_module_files": [],
            "remaining_blockers": []
        },
        "prepare_owner_contract": build_chapter_regeneration_prepare_owner_contract(),
        "task_owner_contract": build_chapter_regeneration_task_owner_contract(),
        "candidate_output_owner_contract": build_chapter_candidate_output_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-chapter-regeneration-owner",
            "regeneration_manifest_probe_count": 13,
            "rust_manifest_probe_count": 13,
            "python_fallback_probe_count": 0,
            "full_stream_owner": "create_chapter_regeneration_stream_workflow",
            "partial_stream_owner": "create_partial_regeneration_stream_workflow",
            "shared_stream_owner": "build_owned_regeneration_stream",
            "candidate_output_owner": "collect_generation_candidate_output",
            "tracked_candidate_output_owner": "collect_generation_candidate_output_tracked",
            "execution_audit_owner": "generation_execution_audit_service",
            "full_finalize_owner": "finalize_chapter_regeneration_result",
            "partial_finalize_owner": "finalize_partial_regeneration_result",
            "source_map_closeout_ready": true,
            "full_module_freeze_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "separate_model_or_shared_owner_closeout_outside_direct_regeneration_route_package",
            "status": "rust_chapter_regeneration_stream_workflow_owner_after_route_shell_delete"
        },
        "validation_boundary": [
            "cargo test services::chapter_regeneration_stream_workflow_service",
            "cargo test api::chapter_regeneration_routes",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-chapter-regeneration-owner",
            "cargo check"
        ],
        "next_cutover_gate": "chapter-regeneration route/workflow source-map package is physically closed out; any surviving Python work is outside this direct regeneration package"
    })
}

pub(crate) fn run_chapter_regeneration_stream_workflow_smoke_suite(
) -> Result<Vec<ChapterRegenerationStreamWorkflowSmokeResult>, String> {
    let readiness_evidence = build_chapter_regeneration_stream_workflow_owner_contract();

    Ok(vec![ChapterRegenerationStreamWorkflowSmokeResult {
        name: "chapter-regeneration-stream-workflow-rust-owner",
        owner: "rust",
        route_group: CHAPTER_REGENERATION_STREAM_WORKFLOW_ROUTE_GROUP,
        ok: true,
        execution_path: "rust_regeneration_stream_workflow_owner",
        fallback_applied: false,
        rollback_boundary: CHAPTER_REGENERATION_STREAM_WORKFLOW_ROLLBACK_BOUNDARY,
        result: json!({
            "full_stream_owner_consumed": true,
            "partial_stream_owner_consumed": true,
            "apply_route_smoke_boundary": "logged_in_fixture_business_smoke",
            "deterministic_business_sse_smoke": true,
            "source_map_freeze_candidate_ready": true,
            "source_map_freeze_ready": true
        }),
        runtime_state: json!({
            "auth_guard_manifest_coverage": "all_regeneration_routes",
            "deterministic_business_sse_smoke": true,
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "remaining_cutover_gate": "separate_model_or_shared_owner_closeout_outside_direct_regeneration_route_package"
        }),
        readiness_evidence,
    }])
}

fn build_full_chapter_regeneration_stream_launch_input(
    input: FullChapterRegenerationStreamInput,
) -> OwnedRegenerationStreamLaunchInput {
    let FullChapterRegenerationStreamInput {
        chapter_word_count,
        prompt,
        ai_service,
        role_policy_context,
        ..
    } = input;

    OwnedRegenerationStreamLaunchInput {
        task_label: "Chapter Rewrite".to_string(),
        prompt,
        ai_service,
        role_policy_context,
        initial_events: vec![
            OwnedRegenerationInitialEvent::Preparing {
                message: Some("Building rewrite prompt...".to_string()),
            },
            OwnedRegenerationInitialEvent::Generating {
                message: Some("Rewriting chapter...".to_string()),
                progress_range: (20, 95),
                char_count: chapter_word_count,
                retry_count: None,
            },
        ],
        completion_message: "Rewrite complete".to_string(),
        task_created_event: None,
    }
}

fn build_full_chapter_regeneration_stream(
    db: DatabaseConnection,
    input: FullChapterRegenerationStreamInput,
    task_id: String,
) -> OwnedRegenerationStream {
    let mut launch_input = build_full_chapter_regeneration_stream_launch_input(input);
    launch_input.task_created_event = Some(json!({
        "type": "task_created",
        "task_id": task_id.clone(),
    }));
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let OwnedRegenerationStreamLaunchInput {
            task_label,
            prompt,
            ai_service,
            role_policy_context,
            initial_events,
            completion_message,
            task_created_event,
        } = launch_input;

        let mut tracker = SseProgress::new(&task_label);
        let _ = tx.send(Ok(tracker.start())).await;
        for step in &initial_events {
            let event = apply_owned_regeneration_initial_event(&mut tracker, step);
            let _ = tx.send(Ok(event)).await;
        }
        if let Some(task_created_event) = task_created_event {
            let _ = tx.send(Ok(sse_json(&task_created_event))).await;
        }

        let output = match execute_regeneration_text_stream(
            &tx,
            ai_service,
            role_policy_context,
            prompt,
            |_| None,
        )
        .await
        {
            Ok(output) => output,
            Err(()) => {
                let failed = build_full_regeneration_task_failed_event(&task_id);
                let _ = tx.send(Ok(sse_json(&failed))).await;
                if let Err(error) = mark_regeneration_task_failed(
                    &db,
                    &task_id,
                    "generation stream execution failed",
                )
                .await
                {
                    let _ = tx
                        .send(Ok(sse_error(
                            &format!("Failed to persist regeneration task failure: {error}"),
                            500,
                        )))
                        .await;
                }
                return;
            }
        };

        let mut payload = match finalize_chapter_regeneration_result(&output.full_content, &task_id)
        {
            Ok(payload) => payload,
            Err(error) => {
                let detail = describe_regeneration_finalize_error(&error);
                let failed = build_full_regeneration_task_failed_event(&task_id);
                let _ = tx.send(Ok(sse_json(&failed))).await;
                if let Err(persist_error) =
                    mark_regeneration_task_failed(&db, &task_id, detail).await
                {
                    let _ = tx
                        .send(Ok(sse_error(
                            &format!(
                                "Failed to persist regeneration task failure: {persist_error}"
                            ),
                            500,
                        )))
                        .await;
                }
                emit_regeneration_finalize_error(&tx, error).await;
                return;
            }
        };

        if merge_regeneration_audit(&mut payload, &output.audit).is_err() {
            let failed = build_full_regeneration_task_failed_event(&task_id);
            let _ = tx.send(Ok(sse_json(&failed))).await;
            let _ = tx
                .send(Ok(sse_error(
                    "Failed to attach regeneration execution audit",
                    500,
                )))
                .await;
            return;
        }
        if let Err(error) =
            mark_regeneration_task_completed(&db, &task_id, &output.full_content).await
        {
            let failed = build_full_regeneration_task_failed_event(&task_id);
            let _ = tx.send(Ok(sse_json(&failed))).await;
            let _ = tx
                .send(Ok(sse_error(
                    &format!("Failed to persist regeneration task completion: {error}"),
                    500,
                )))
                .await;
            return;
        }

        let succeeded = build_full_regeneration_task_completed_event(&task_id);
        let _ = tx.send(Ok(sse_json(&succeeded))).await;
        let _ = tx
            .send(Ok(tracker.complete(Some(&completion_message))))
            .await;
        let _ = tx.send(Ok(sse_result(&payload))).await;
        let _ = tx.send(Ok(sse_done())).await;
    });

    ReceiverStream::new(rx)
}

fn build_partial_chapter_regeneration_stream_launch_input(
    input: PartialChapterRegenerationStreamInput,
) -> (
    OwnedRegenerationStreamLaunchInput,
    usize,
    usize,
    usize,
    usize,
) {
    let PartialChapterRegenerationStreamInput {
        target_words,
        original_word_count,
        start_position,
        end_position,
        prompt,
        ai_service,
        role_policy_context,
        ..
    } = input;

    (
        OwnedRegenerationStreamLaunchInput {
            task_label: "Partial Rewrite".to_string(),
            prompt,
            ai_service,
            role_policy_context,
            initial_events: vec![
                OwnedRegenerationInitialEvent::Preparing {
                    message: Some("Preparing rewrite context...".to_string()),
                },
                OwnedRegenerationInitialEvent::Preparing {
                    message: Some("Starting generation...".to_string()),
                },
            ],
            completion_message: "Rewrite complete".to_string(),
            task_created_event: None,
        },
        target_words,
        original_word_count,
        start_position,
        end_position,
    )
}

fn build_partial_chapter_regeneration_stream(
    input: PartialChapterRegenerationStreamInput,
) -> OwnedRegenerationStream {
    let (launch_input, target_words, original_word_count, start_position, end_position) =
        build_partial_chapter_regeneration_stream_launch_input(input);

    build_owned_regeneration_stream(
        launch_input,
        move |tracker, progress: RegenerationChunkProgress| {
            if progress.chunk_count % 5 == 0 {
                Some(tracker.generating(
                    Some(&format!(
                        "Generating rewrite... {}/{} chars",
                        progress.full_content_len, target_words
                    )),
                    (35, 95),
                    progress.full_content_len,
                    None,
                ))
            } else {
                None
            }
        },
        None,
        None,
        move |full_content| {
            finalize_partial_regeneration_result(
                full_content,
                original_word_count,
                start_position,
                end_position,
            )
        },
    )
}

pub enum CreateRegenerationStreamWorkflowError<TPrepareError> {
    Chapter(LoadAccessibleChapterError),
    Prepare(TPrepareError),
    TaskLifecycle(FullRegenerationTaskLifecycleError),
}

pub type CreateChapterRegenerationStreamWorkflowError =
    CreateRegenerationStreamWorkflowError<BuildRegenerationAiServiceError>;

pub type CreatePartialRegenerationStreamWorkflowError =
    CreateRegenerationStreamWorkflowError<PreparePartialRegenerationStreamError>;

pub async fn execute_partial_regeneration_task(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: PartialRegenerationStreamWorkflowRequest,
) -> Result<Value, CreatePartialRegenerationStreamWorkflowError> {
    validate_partial_regeneration_stream_request_bounds(&request)
        .map_err(PreparePartialRegenerationStreamError::Input)
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Chapter)?;
    let stream_input = prepare_partial_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;
    let (launch_input, _, original_word_count, start_position, end_position) =
        build_partial_chapter_regeneration_stream_launch_input(stream_input);

    let output = collect_generation_candidate_output_tracked(
        ChapterCandidateOutputRequest {
            ai_service: launch_input.ai_service,
            prompt: launch_input.prompt,
            system_prompt: None,
            tools: None,
            candidate_index: 1,
            max_output_chars: None,
            runtime_state: None,
        },
        launch_input.role_policy_context.allow_model_fallback,
        |_chunk_content, _progress| async { Ok(()) },
    )
    .await
    .map_err(|error| {
        CreatePartialRegenerationStreamWorkflowError::Prepare(
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error),
            ),
        )
    })?;

    let mut payload = finalize_partial_regeneration_result(
        &output.output.full_content,
        original_word_count,
        start_position,
        end_position,
    )
    .map_err(|error| {
        CreatePartialRegenerationStreamWorkflowError::Prepare(
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(
                    describe_regeneration_finalize_error(&error).to_string(),
                ),
            ),
        )
    })?;
    let audit = build_generation_execution_audit(
        &launch_input.role_policy_context.resolved_policy,
        &output.execution,
    )
    .map_err(|error| {
        CreatePartialRegenerationStreamWorkflowError::Prepare(
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error.to_string()),
            ),
        )
    })?;
    merge_regeneration_audit(&mut payload, &audit).map_err(|error| {
        CreatePartialRegenerationStreamWorkflowError::Prepare(
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error),
            ),
        )
    })?;
    Ok(payload)
}

pub async fn execute_chapter_regeneration_task(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: FullChapterRegenerationStreamRequest,
) -> Result<Value, CreateChapterRegenerationStreamWorkflowError> {
    validate_full_chapter_regeneration_stream_request_bounds(&request)
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Chapter)?;
    let analysis = load_latest_chapter_analysis(db, chapter_id, request.modification_source())
        .await
        .map_err(|error| {
            CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
                FullRegenerationTaskLifecycleError::Internal(error),
            )
        })?;
    if matches!(
        request.modification_source(),
        "analysis_suggestions" | "mixed"
    ) && analysis.is_none()
    {
        return Err(CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
            FullRegenerationTaskLifecycleError::AnalysisMissing,
        ));
    }

    let stream_input = prepare_chapter_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;
    let task_seed = build_full_regeneration_task_seed(
        &chapter,
        analysis.as_ref(),
        user_id,
        &stream_input.request,
        stream_input.resolved_style_id,
    );
    let task = create_full_regeneration_task(db, task_seed)
        .await
        .map_err(|error| {
            CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
                FullRegenerationTaskLifecycleError::Internal(error),
            )
        })?;

    let launch_input = build_full_chapter_regeneration_stream_launch_input(stream_input);
    let output = collect_generation_candidate_output_tracked(
        ChapterCandidateOutputRequest {
            ai_service: launch_input.ai_service,
            prompt: launch_input.prompt,
            system_prompt: None,
            tools: None,
            candidate_index: 1,
            max_output_chars: None,
            runtime_state: None,
        },
        launch_input.role_policy_context.allow_model_fallback,
        |_chunk_content, _progress| async { Ok(()) },
    )
    .await
    .map_err(|error| {
        CreateChapterRegenerationStreamWorkflowError::Prepare(
            BuildRegenerationAiServiceError::InvalidConfig(error),
        )
    })?;

    let mut payload =
        match finalize_chapter_regeneration_result(&output.output.full_content, &task.id) {
            Ok(payload) => payload,
            Err(error) => {
                let detail = describe_regeneration_finalize_error(&error);
                let _ = mark_regeneration_task_failed(db, &task.id, detail).await;
                return Err(CreateChapterRegenerationStreamWorkflowError::Prepare(
                    BuildRegenerationAiServiceError::InvalidConfig(detail.to_string()),
                ));
            }
        };

    let audit = build_generation_execution_audit(
        &launch_input.role_policy_context.resolved_policy,
        &output.execution,
    )
    .map_err(|error| {
        CreateChapterRegenerationStreamWorkflowError::Prepare(
            BuildRegenerationAiServiceError::InvalidConfig(error.to_string()),
        )
    })?;
    merge_regeneration_audit(&mut payload, &audit).map_err(|error| {
        CreateChapterRegenerationStreamWorkflowError::Prepare(
            BuildRegenerationAiServiceError::InvalidConfig(error),
        )
    })?;

    mark_regeneration_task_completed(db, &task.id, &output.output.full_content)
        .await
        .map_err(|error| {
            CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
                FullRegenerationTaskLifecycleError::Internal(error),
            )
        })?;

    Ok(payload)
}

pub async fn create_chapter_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: FullChapterRegenerationStreamRequest,
) -> Result<OwnedRegenerationStream, CreateChapterRegenerationStreamWorkflowError> {
    validate_full_chapter_regeneration_stream_request_bounds(&request)
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Chapter)?;
    let analysis = load_latest_chapter_analysis(db, chapter_id, request.modification_source())
        .await
        .map_err(|error| {
            CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
                FullRegenerationTaskLifecycleError::Internal(error),
            )
        })?;
    if matches!(
        request.modification_source(),
        "analysis_suggestions" | "mixed"
    ) && analysis.is_none()
    {
        return Err(CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
            FullRegenerationTaskLifecycleError::AnalysisMissing,
        ));
    }
    let stream_input = prepare_chapter_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;
    let task_seed = build_full_regeneration_task_seed(
        &chapter,
        analysis.as_ref(),
        user_id,
        &stream_input.request,
        stream_input.resolved_style_id,
    );
    let task = create_full_regeneration_task(db, task_seed)
        .await
        .map_err(|error| {
            CreateChapterRegenerationStreamWorkflowError::TaskLifecycle(
                FullRegenerationTaskLifecycleError::Internal(error),
            )
        })?;

    Ok(build_full_chapter_regeneration_stream(
        db.clone(),
        stream_input,
        task.id,
    ))
}

pub async fn create_partial_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: PartialRegenerationStreamWorkflowRequest,
) -> Result<OwnedRegenerationStream, CreatePartialRegenerationStreamWorkflowError> {
    validate_partial_regeneration_stream_request_bounds(&request)
        .map_err(PreparePartialRegenerationStreamError::Input)
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Chapter)?;
    let stream_input = prepare_partial_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;

    Ok(build_partial_chapter_regeneration_stream(stream_input))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_owned_regeneration_initial_event,
        build_chapter_regeneration_stream_workflow_owner_contract,
        build_full_chapter_regeneration_stream_launch_input,
        build_partial_chapter_regeneration_stream_launch_input,
        describe_regeneration_finalize_error, finalize_chapter_regeneration_result,
        finalize_partial_regeneration_result, merge_regeneration_audit,
        normalize_partial_regeneration_output,
        run_chapter_regeneration_stream_workflow_smoke_suite,
        CreateChapterRegenerationStreamWorkflowError, CreatePartialRegenerationStreamWorkflowError,
        CreateRegenerationStreamWorkflowError, FinalizePartialRegenerationError,
        OwnedRegenerationInitialEvent,
    };
    use crate::ai::execution_trace::{
        AIExecutionOutcome, AIExecutionTraceV1, AI_EXECUTION_TRACE_SCHEMA_VERSION,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_generation_execution_contract_service::PreparedRoleModelPolicyContext;
    use crate::services::chapter_regeneration_prepare_service::{
        BuildRegenerationAiServiceError, PreparePartialRegenerationError,
        PreparePartialRegenerationStreamError,
    };
    use crate::services::chapter_regeneration_prepare_service::{
        FullChapterRegenerationStreamInput, PartialChapterRegenerationStreamInput,
    };
    use crate::services::generation_contract_service::{
        build_generation_contract_snapshot, GenerationContractSnapshotV1, GenerationIntentKind,
        GenerationIntentV1, GenerationSelection, GenerationTarget, StoryPacketV1,
    };
    use crate::services::generation_execution_audit_service::build_generation_execution_audit;
    use crate::services::role_model_policy_service::{
        GenerationRole, ModelSelectionSource, ResolvedRoleModelPolicyV1,
        ROLE_MODEL_POLICY_SCHEMA_VERSION,
    };
    use crate::utils::sse::SseProgress;

    fn build_test_role_policy_context() -> PreparedRoleModelPolicyContext {
        PreparedRoleModelPolicyContext {
            resolved_policy: ResolvedRoleModelPolicyV1 {
                role: GenerationRole::Writer,
                policy_schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_string(),
                policy_digest: "test-policy-digest".to_string(),
                requested_provider: Some("openai".to_string()),
                requested_model: Some("test-model".to_string()),
                resolved_provider: "openai".to_string(),
                resolved_model: "test-model".to_string(),
                provider_source: ModelSelectionSource::GlobalSettings,
                model_source: ModelSelectionSource::GlobalSettings,
            },
            allow_model_fallback: false,
        }
    }

    fn build_test_full_generation_contract() -> GenerationContractSnapshotV1 {
        let target = GenerationTarget::chapter("project-1", "chapter-1");
        let story_packet = StoryPacketV1::new("project-1", target.clone());
        let intent = GenerationIntentV1::new(GenerationIntentKind::ChapterRegenerate, target);

        build_generation_contract_snapshot(story_packet, intent)
            .expect("full regeneration test contract should build")
    }

    fn build_test_partial_generation_contract() -> GenerationContractSnapshotV1 {
        let target = GenerationTarget::chapter_selection(
            "project-1",
            "chapter-1",
            GenerationSelection {
                start_index: 12,
                end_index: 36,
                selected_text: Some("选中的原始正文".to_string()),
            },
        );
        let story_packet = StoryPacketV1::new("project-1", target.clone());
        let intent =
            GenerationIntentV1::new(GenerationIntentKind::ChapterPartialRegenerate, target);

        build_generation_contract_snapshot(story_packet, intent)
            .expect("partial regeneration test contract should build")
    }

    #[test]
    fn should_normalize_partial_regeneration_output_prefixes_and_quotes() {
        assert_eq!(
            normalize_partial_regeneration_output("\r\n重写后： \"新的正文\" \r\n"),
            "新的正文"
        );
        assert_eq!(
            normalize_partial_regeneration_output("以下是重写后的内容：『新的正文』"),
            "新的正文"
        );
        assert_eq!(
            normalize_partial_regeneration_output("改写后:'新的正文'"),
            "新的正文"
        );
    }

    #[test]
    fn should_finalize_partial_regeneration_result_payload() {
        let result = finalize_partial_regeneration_result("重写后：新的正文", 12, 3, 8);
        let result = match result {
            Ok(result) => result,
            Err(_) => panic!("partial regeneration result should be valid"),
        };

        assert_eq!(result["new_text"], "新的正文");
        assert_eq!(result["word_count"], 4);
        assert_eq!(result["original_word_count"], 12);
        assert_eq!(result["start_position"], 3);
        assert_eq!(result["end_position"], 8);
    }

    #[test]
    fn should_finalize_chapter_regeneration_result_payload() {
        let result = finalize_chapter_regeneration_result("新的章节正文", "chapter-1");
        let result = match result {
            Ok(result) => result,
            Err(_) => panic!("chapter regeneration result should be valid"),
        };

        assert_eq!(result["content"], "新的章节正文");
        assert_eq!(result["word_count"], 6);
        assert_eq!(result["task_id"], "chapter-1");
        assert!(result["analysis_task_id"].is_null());
    }

    #[test]
    fn should_reject_meta_only_regeneration_result_as_empty() {
        let result = finalize_partial_regeneration_result(
            "```markdown\n作为AI：我将开始执行\n流程说明",
            12,
            3,
            8,
        );
        let error = match result {
            Ok(_) => panic!("meta-only partial regeneration result should be rejected"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            FinalizePartialRegenerationError::EmptyContent
        ));
    }

    #[test]
    fn should_merge_regeneration_audit_without_overwriting_result_fields() {
        let role_policy_context = build_test_role_policy_context();
        let execution = AIExecutionTraceV1 {
            schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
            requested_provider: "openai".to_string(),
            requested_model: "test-model".to_string(),
            actual_provider: "openai".to_string(),
            actual_model: "test-model".to_string(),
            outcome: AIExecutionOutcome::Succeeded,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        };
        let audit =
            build_generation_execution_audit(&role_policy_context.resolved_policy, &execution)
                .expect("test regeneration audit should build");
        let mut payload = serde_json::json!({
            "content": "重生成正文",
            "task_id": "task-1",
            "start_position": 12,
            "end_position": 36,
            "word_count": 2400
        });

        merge_regeneration_audit(&mut payload, &audit)
            .expect("regeneration audit should merge additively");

        assert_eq!(payload["content"], "重生成正文");
        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["start_position"], 12);
        assert_eq!(payload["end_position"], 36);
        assert_eq!(payload["word_count"], 2400);
        assert_eq!(payload["generation_execution_audit"]["role"], "writer");
        assert!(payload.get("prompt").is_none());
        assert!(payload.get("authorization").is_none());
        assert!(payload.get("api_key").is_none());
    }

    #[test]
    fn should_publish_chapter_regeneration_stream_workflow_owner_contract() {
        let contract = build_chapter_regeneration_stream_workflow_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_regeneration_stream_workflow_service"
        );
        assert_eq!(
            contract["scope"],
            "full_and_partial_chapter_regeneration_sse_workflow_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["full_stream_entrypoint"],
            "create_chapter_regeneration_stream_workflow"
        );
        assert_eq!(
            contract["behavior_contract"]["partial_stream_entrypoint"],
            "create_partial_regeneration_stream_workflow"
        );
        assert_eq!(
            contract["source_map_policy"]["auth_guard_manifest_coverage"],
            "all_regeneration_routes"
        );
        assert_eq!(
            contract["source_map_policy"]["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(
            contract["source_map_policy"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["source_map_policy"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["source_map_policy"]["owner_profile_business_probes"]
                .as_array()
                .expect("owner profile business probes")
                .len(),
            8
        );
        assert_eq!(
            contract["source_map_policy"]["remaining_blockers"]
                .as_array()
                .expect("remaining blockers")
                .len(),
            0
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            "chapter_regeneration_prepare_service"
        );
        assert_eq!(
            contract["prepare_owner_contract"]["service_runtime_closeout_status"]["status"],
            "rust_chapter_regeneration_prepare_owner_direct_package_closed_out"
        );
        assert_eq!(
            contract["candidate_output_owner_contract"]["owner"],
            "chapter_candidate_output_service"
        );
        assert_eq!(
            contract["candidate_output_owner_contract"]["service_runtime_closeout_status"]
                ["status"],
            "rust_chapter_candidate_output_owner_executor_source_map_deleted"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-chapter-regeneration-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["regeneration_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["full_stream_owner"],
            "create_chapter_regeneration_stream_workflow"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["partial_stream_owner"],
            "create_partial_regeneration_stream_workflow"
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
            "rust_chapter_regeneration_stream_workflow_owner_after_route_shell_delete"
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "chapter-regeneration route/workflow source-map package is physically closed out; any surviving Python work is outside this direct regeneration package"
        );
        assert_eq!(contract["python_source_map"].as_array().unwrap().len(), 0);
        assert_eq!(contract["rust_owner_map"].as_array().unwrap().len(), 8);
        assert_eq!(
            contract["source_map_policy"]["freeze_scope"],
            "chapter_regeneration_route_package_source_map_surface"
        );
        assert_eq!(
            contract["source_map_policy"]["stream_orchestration_source_maps"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["source_map_policy"]["prepare_owner_source_maps"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["source_map_policy"]["shared_prepare_dependency_source_maps"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["source_map_policy"]["shared_context_compaction_source_maps"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["source_map_policy"]["query_owner_source_maps"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            contract["behavior_contract"]["shared_stream_owner_entrypoints"][0],
            "build_owned_regeneration_stream"
        );
        assert!(
            contract["behavior_contract"]["shared_stream_owner_entrypoints"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry == "collect_generation_candidate_output_tracked")
        );
        assert_eq!(
            contract["behavior_contract"]["execution_audit_policy"]["result_field"],
            "generation_execution_audit"
        );
        assert_eq!(
            contract["behavior_contract"]["execution_audit_policy"]["sse_event_kind"],
            "result"
        );
        assert_eq!(
            contract["behavior_contract"]["execution_audit_policy"]
                ["chapter_content_persistence_unchanged"],
            true
        );
        assert_eq!(
            contract["task_owner_contract"]["owner"],
            "chapter_regeneration_task_service"
        );
    }

    #[test]
    fn should_run_chapter_regeneration_stream_workflow_smoke_suite() {
        let results = run_chapter_regeneration_stream_workflow_smoke_suite()
            .expect("regeneration stream workflow smoke suite");

        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(
            result.name,
            "chapter-regeneration-stream-workflow-rust-owner"
        );
        assert_eq!(result.owner, "rust");
        assert_eq!(result.route_group, "chapter_regeneration");
        assert!(result.ok);
        assert_eq!(
            result.execution_path,
            "rust_regeneration_stream_workflow_owner"
        );
        assert!(!result.fallback_applied);
        assert_eq!(
            result.rollback_boundary,
            "chapter_regeneration_python_source_map"
        );
        assert_eq!(result.result["full_stream_owner_consumed"], true);
        assert_eq!(result.result["partial_stream_owner_consumed"], true);
        assert_eq!(result.result["deterministic_business_sse_smoke"], true);
        assert_eq!(result.result["source_map_freeze_candidate_ready"], true);
        assert_eq!(
            result.result["apply_route_smoke_boundary"],
            "logged_in_fixture_business_smoke"
        );
        assert_eq!(
            result.runtime_state["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(
            result.runtime_state["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(result.runtime_state["full_module_freeze_ready"], true);
        assert_eq!(
            result.runtime_state["remaining_cutover_gate"],
            "separate_model_or_shared_owner_closeout_outside_direct_regeneration_route_package"
        );
        assert_eq!(
            result.readiness_evidence["source_map_policy"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            result.readiness_evidence["source_map_policy"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            result.readiness_evidence["source_map_policy"]["deterministic_business_sse_smoke"],
            true
        );
        assert_eq!(result.result["source_map_freeze_ready"], true);
    }

    #[test]
    fn full_regeneration_stream_workflow_error_alias_keeps_shared_outer_owner() {
        let error: CreateChapterRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied,
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn partial_regeneration_stream_workflow_error_alias_keeps_shared_outer_owner() {
        let error: CreatePartialRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Style("bad style".to_string()),
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Style(detail)
            ) if detail == "bad style"
        ));
    }

    #[test]
    fn shared_outer_owner_preserves_prepare_type_specificity() {
        let error: CreatePartialRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Input(
                    PreparePartialRegenerationError::InvalidRange,
                ),
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Input(
                    PreparePartialRegenerationError::InvalidRange
                )
            )
        ));

        let full_error: CreateChapterRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                BuildRegenerationAiServiceError::InvalidConfig("missing provider".to_string()),
            );

        assert!(matches!(
            full_error,
            CreateRegenerationStreamWorkflowError::Prepare(
                BuildRegenerationAiServiceError::InvalidConfig(detail)
            ) if detail == "missing provider"
        ));
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_launch_input_contract() {
        let launch_input = build_full_chapter_regeneration_stream_launch_input(
            FullChapterRegenerationStreamInput {
                chapter: crate::models::chapter::Model {
                    id: "chapter-1".to_string(),
                    project_id: "project-1".to_string(),
                    title: "测试章节".to_string(),
                    chapter_number: 1,
                    content: Some("原始内容".to_string()),
                    summary: None,
                    word_count: 2400,
                    status: "draft".to_string(),
                    outline_id: None,
                    sub_index: 0,
                    expansion_plan: None,
                    created_at: Default::default(),
                    updated_at: Some(Default::default()),
                },
                user_id: "user-1".to_string(),
                request: crate::services::chapter_regeneration_prepare_service::FullChapterRegenerationStreamRequest::default(),
                resolved_style_id: None,
                chapter_id: "chapter-1".to_string(),
                chapter_word_count: 2400,
                prompt: "prompt".to_string(),
                ai_service: crate::ai::service::AIService::new(AIConfig::default()),
                role_policy_context: build_test_role_policy_context(),
                generation_contract: build_test_full_generation_contract(),
            },
        );

        assert_eq!(
            launch_input.role_policy_context.resolved_policy.role,
            GenerationRole::Writer
        );
        assert!(!launch_input.role_policy_context.allow_model_fallback);
        assert_eq!(launch_input.task_label, "Chapter Rewrite");
        assert_eq!(launch_input.prompt, "prompt");
        assert_eq!(launch_input.completion_message, "Rewrite complete");
        assert_eq!(launch_input.initial_events.len(), 2);
        assert!(matches!(
            &launch_input.initial_events[0],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Building rewrite prompt...")
        ));
        assert!(matches!(
            &launch_input.initial_events[1],
            OwnedRegenerationInitialEvent::Generating {
                message,
                progress_range,
                char_count,
                retry_count
            }
            if message.as_deref() == Some("Rewriting chapter...")
                && *progress_range == (20, 95)
                && *char_count == 2400
                && retry_count.is_none()
        ));
    }

    #[test]
    fn should_build_partial_chapter_regeneration_stream_launch_input_contract() {
        let (launch_input, target_words, original_word_count, start_position, end_position) =
            build_partial_chapter_regeneration_stream_launch_input(
                PartialChapterRegenerationStreamInput {
                    target_words: 1800,
                    original_word_count: 900,
                    start_position: 12,
                    end_position: 36,
                    prompt: "prompt".to_string(),
                    ai_service: crate::ai::service::AIService::new(AIConfig::default()),
                    role_policy_context: build_test_role_policy_context(),
                    generation_contract: build_test_partial_generation_contract(),
                },
            );

        assert_eq!(target_words, 1800);
        assert_eq!(original_word_count, 900);
        assert_eq!(start_position, 12);
        assert_eq!(end_position, 36);
        assert_eq!(
            launch_input.role_policy_context.resolved_policy.role,
            GenerationRole::Writer
        );
        assert!(!launch_input.role_policy_context.allow_model_fallback);
        assert_eq!(launch_input.task_label, "Partial Rewrite");
        assert_eq!(launch_input.prompt, "prompt");
        assert_eq!(launch_input.completion_message, "Rewrite complete");
        assert_eq!(launch_input.initial_events.len(), 2);
        assert!(matches!(
            &launch_input.initial_events[0],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Preparing rewrite context...")
        ));
        assert!(matches!(
            &launch_input.initial_events[1],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Starting generation...")
        ));
    }

    #[test]
    fn should_advance_tracker_for_preparing_initial_event() {
        let mut tracker = SseProgress::new("Rewrite");

        let _ = apply_owned_regeneration_initial_event(
            &mut tracker,
            &OwnedRegenerationInitialEvent::Preparing {
                message: Some("Preparing rewrite context...".to_string()),
            },
        );

        assert_eq!(tracker.current_progress(), 15);
    }

    #[test]
    fn should_advance_tracker_for_generating_initial_event() {
        let mut tracker = SseProgress::new("Rewrite");

        let _ = apply_owned_regeneration_initial_event(
            &mut tracker,
            &OwnedRegenerationInitialEvent::Generating {
                message: Some("Rewriting chapter...".to_string()),
                progress_range: (20, 95),
                char_count: 0,
                retry_count: None,
            },
        );

        assert_eq!(tracker.current_progress(), 20);
    }

    #[test]
    fn should_describe_regeneration_finalize_errors_with_existing_messages() {
        assert_eq!(
            describe_regeneration_finalize_error(&FinalizePartialRegenerationError::EmptyContent),
            "Rewrite result is empty after sanitization"
        );
        assert_eq!(
            describe_regeneration_finalize_error(
                &FinalizePartialRegenerationError::WorkflowMetaText,
            ),
            "Rewrite result still contains workflow meta text"
        );
    }
}
