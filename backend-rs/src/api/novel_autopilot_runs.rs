use std::sync::Arc;

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    api::background_tasks::{
        best_effort_wait_for_human_after_schedule_failure, cancel_task_runtime,
        schedule_owned_novel_book_autopilot_tick, NovelBookAutopilotTaskScheduleError,
        NovelBookAutopilotTaskScheduleOutcome,
    },
    models::{novel_autopilot_run, novel_autopilot_step_run},
    services::{
        auth::Claims,
        book_import_service::BookImportService,
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        novel_autopilot::{
            coordinator::{NovelAutopilotNextTickLease, HUMAN_DECISION_CANDIDATE_UNAVAILABLE},
            repository::{
                CreateNovelAutopilotRun, NovelAutopilotRepository, NovelAutopilotRepositoryError,
            },
            types::{
                NovelAutopilotPrivateSnapshot, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
            },
        },
        project_service::ProjectService,
    },
    tasks::{registry::TaskRegistry, stream::TaskStreamHub},
};

const RUNS_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs";
const RUN_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}";
const STEPS_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/steps";
const PAUSE_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/pause";
const RESUME_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/resume";
const CANCEL_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/cancel";
const GUIDANCE_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/guidance";
const DECISION_ROUTE: &str = "/projects/{project_id}/novel-autopilot-runs/{run_id}/decision";
const MAX_GUIDANCE_CHARS: usize = 4_000;

type ApiError = (StatusCode, Json<Value>);
type ApiResult = Result<(StatusCode, Json<Value>), ApiError>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRunPath {
    project_id: String,
    run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRunRequest {
    #[serde(default)]
    config: NovelAutopilotRunConfig,
    total_chapters: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedControlRequest {
    expected_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GuidanceRequest {
    expected_version: i64,
    guidance: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HumanDecision {
    Accept,
    Retry,
    Repair,
    Stop,
}

impl HumanDecision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Retry => "retry",
            Self::Repair => "repair",
            Self::Stop => "stop",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanDecisionRequest {
    expected_version: i64,
    decision: HumanDecision,
    guidance: Option<String>,
}

async fn create_run(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Extension(book_import_service): Extension<Arc<BookImportService>>,
    Extension(candidate_gateway_config): Extension<ChapterCandidateRouteGatewayConfig>,
    Path(project_id): Path<String>,
    Json(request): Json<CreateRunRequest>,
) -> ApiResult {
    let project = ProjectService::get(&db, &project_id, &claims.sub)
        .await
        .map_err(internal_project_error)?
        .ok_or_else(not_found_error)?;
    let total_chapters = request
        .total_chapters
        .or_else(|| {
            project
                .chapter_count
                .and_then(|value| u32::try_from(value).ok())
        })
        .unwrap_or_default();

    let created = NovelAutopilotRepository::create_or_get_active(
        &db,
        CreateNovelAutopilotRun {
            project_id: project_id.clone(),
            user_id: claims.sub.clone(),
            total_chapters,
            config: request.config,
        },
    )
    .await
    .map_err(map_repository_error)?;

    let task = if run_needs_dispatch_retry(&created.run, &registry).await {
        Some(
            create_run_task(
                db.clone(),
                registry,
                stream_hub,
                book_import_service,
                candidate_gateway_config,
                &claims.sub,
                &created.run,
                None,
            )
            .await?,
        )
    } else {
        None
    };
    let run = NovelAutopilotRepository::find_owned(&db, &created.run.id, &claims.sub)
        .await
        .map_err(map_repository_error)?;
    let status = if created.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((
        status,
        Json(json!({
            "run": run_view(&run),
            "created": created.created,
            "background_task": task,
        })),
    ))
}

async fn list_runs(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> ApiResult {
    let runs = NovelAutopilotRepository::list_owned(&db, &project_id, &claims.sub)
        .await
        .map_err(map_repository_error)?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "items": runs.iter().map(run_view).collect::<Vec<_>>(),
        })),
    ))
}

async fn get_run(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(path): Path<ProjectRunPath>,
) -> ApiResult {
    let run = find_scoped_run(&db, &path, &claims.sub).await?;
    Ok((StatusCode::OK, Json(json!({"run": run_view(&run)}))))
}

async fn list_steps(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(path): Path<ProjectRunPath>,
) -> ApiResult {
    find_scoped_run(&db, &path, &claims.sub).await?;
    let steps = NovelAutopilotRepository::list_steps_owned(&db, &path.run_id, &claims.sub)
        .await
        .map_err(map_repository_error)?;
    let candidate_id = match steps.last() {
        Some(step) => NovelAutopilotRepository::find_waiting_chapter_candidate_id(
            &db,
            &path.project_id,
            &step.id,
        )
        .await
        .map_err(map_repository_error)?,
        None => None,
    };
    Ok((
        StatusCode::OK,
        Json(json!({
            "items": steps
                .iter()
                .map(|step| {
                    let step_candidate_id = candidate_id
                        .as_deref()
                        .filter(|candidate_id| step.id.as_str() == *candidate_id);
                    step_view(step, step_candidate_id)
                })
                .collect::<Vec<_>>(),
        })),
    ))
}

async fn pause_run(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(path): Path<ProjectRunPath>,
    Json(request): Json<VersionedControlRequest>,
) -> ApiResult {
    let current = find_scoped_run(&db, &path, &claims.sub).await?;
    let active_task_id = current.active_background_task_id.clone();
    let run = NovelAutopilotRepository::transition_owned(
        &db,
        &path.run_id,
        &claims.sub,
        request.expected_version,
        NovelAutopilotRunStatus::Paused,
    )
    .await
    .map_err(map_repository_error)?;
    cancel_runtime_if_present(
        &registry,
        &stream_hub,
        active_task_id.as_deref(),
        &claims.sub,
    )
    .await;
    Ok((StatusCode::OK, Json(json!({"run": run_view(&run)}))))
}

async fn resume_run(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Extension(book_import_service): Extension<Arc<BookImportService>>,
    Extension(candidate_gateway_config): Extension<ChapterCandidateRouteGatewayConfig>,
    Path(path): Path<ProjectRunPath>,
    Json(request): Json<VersionedControlRequest>,
) -> ApiResult {
    let current = find_scoped_run(&db, &path, &claims.sub).await?;
    let run = if current.version == request.expected_version
        && run_needs_dispatch_retry(&current, &registry).await
    {
        current
    } else {
        NovelAutopilotRepository::transition_owned(
            &db,
            &path.run_id,
            &claims.sub,
            request.expected_version,
            NovelAutopilotRunStatus::Queued,
        )
        .await
        .map_err(map_repository_error)?
    };
    let task = create_run_task(
        db.clone(),
        registry.clone(),
        stream_hub.clone(),
        book_import_service,
        candidate_gateway_config,
        &claims.sub,
        &run,
        None,
    )
    .await?;
    let run = NovelAutopilotRepository::find_owned(&db, &run.id, &claims.sub)
        .await
        .map_err(map_repository_error)?;
    Ok((
        StatusCode::OK,
        Json(json!({"run": run_view(&run), "background_task": task})),
    ))
}

async fn cancel_run(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Path(path): Path<ProjectRunPath>,
    Json(request): Json<VersionedControlRequest>,
) -> ApiResult {
    let current = find_scoped_run(&db, &path, &claims.sub).await?;
    let active_task_id = current.active_background_task_id.clone();
    let run = NovelAutopilotRepository::transition_owned(
        &db,
        &path.run_id,
        &claims.sub,
        request.expected_version,
        NovelAutopilotRunStatus::Cancelled,
    )
    .await
    .map_err(map_repository_error)?;
    cancel_runtime_if_present(
        &registry,
        &stream_hub,
        active_task_id.as_deref(),
        &claims.sub,
    )
    .await;
    Ok((StatusCode::OK, Json(json!({"run": run_view(&run)}))))
}

async fn update_guidance(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(path): Path<ProjectRunPath>,
    Json(request): Json<GuidanceRequest>,
) -> ApiResult {
    find_scoped_run(&db, &path, &claims.sub).await?;
    let guidance = normalize_guidance(&request.guidance)?;
    let digest = digest_normalized_guidance(&guidance);
    let run = NovelAutopilotRepository::update_guidance(
        &db,
        &path.run_id,
        &claims.sub,
        request.expected_version,
        &guidance,
        &digest,
    )
    .await
    .map_err(map_repository_error)?;
    Ok((StatusCode::OK, Json(json!({"run": run_view(&run)}))))
}

async fn submit_decision(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Extension(book_import_service): Extension<Arc<BookImportService>>,
    Extension(candidate_gateway_config): Extension<ChapterCandidateRouteGatewayConfig>,
    Path(path): Path<ProjectRunPath>,
    Json(request): Json<HumanDecisionRequest>,
) -> ApiResult {
    let current = find_scoped_run(&db, &path, &claims.sub).await?;
    if current.status != NovelAutopilotRunStatus::WaitingHuman.as_str() {
        return Err(conflict_error(
            "run_not_waiting_human",
            "Run is not waiting for a human decision",
        ));
    }

    if matches!(request.decision, HumanDecision::Accept) {
        ensure_accept_decision_available(&db, &path, &claims.sub, &current).await?;
    }

    let mut expected_version = request.expected_version;
    if let Some(guidance) = request
        .guidance
        .as_deref()
        .filter(|_| !matches!(request.decision, HumanDecision::Stop))
    {
        let guidance = normalize_guidance(guidance)?;
        let digest = digest_normalized_guidance(&guidance);
        let updated = NovelAutopilotRepository::update_guidance(
            &db,
            &path.run_id,
            &claims.sub,
            expected_version,
            &guidance,
            &digest,
        )
        .await
        .map_err(map_repository_error)?;
        expected_version = updated.version;
    }

    let target = match request.decision {
        HumanDecision::Stop => NovelAutopilotRunStatus::Cancelled,
        HumanDecision::Accept | HumanDecision::Retry | HumanDecision::Repair => {
            NovelAutopilotRunStatus::Queued
        }
    };
    let run = NovelAutopilotRepository::transition_owned(
        &db,
        &path.run_id,
        &claims.sub,
        expected_version,
        target,
    )
    .await
    .map_err(map_repository_error)?;

    let task = if target == NovelAutopilotRunStatus::Queued {
        match create_run_task(
            db.clone(),
            registry,
            stream_hub,
            book_import_service,
            candidate_gateway_config,
            &claims.sub,
            &run,
            Some(request.decision),
        )
        .await
        {
            Ok(task) => Some(task),
            Err(error) => {
                let lease = run_task_lease(&claims.sub, &run);
                let error_code = error
                    .1
                     .0
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("novel_autopilot_task_schedule_failed");
                best_effort_wait_for_human_after_schedule_failure(&db, &lease, error_code).await;
                return Err(error);
            }
        }
    } else {
        None
    };
    let run = NovelAutopilotRepository::find_owned(&db, &run.id, &claims.sub)
        .await
        .map_err(map_repository_error)?;
    Ok((
        StatusCode::OK,
        Json(json!({"run": run_view(&run), "background_task": task})),
    ))
}

async fn ensure_accept_decision_available(
    db: &DatabaseConnection,
    path: &ProjectRunPath,
    user_id: &str,
    run: &novel_autopilot_run::Model,
) -> Result<(), ApiError> {
    let latest_step = NovelAutopilotRepository::list_steps_owned(db, &path.run_id, user_id)
        .await
        .map_err(map_repository_error)?
        .pop();
    let candidate_id = match latest_step.as_ref() {
        Some(step) => NovelAutopilotRepository::find_waiting_chapter_candidate_id(
            db,
            &path.project_id,
            &step.id,
        )
        .await
        .map_err(map_repository_error)?,
        None => None,
    };
    let periodic_gate_without_candidate = run.last_error_code.is_none()
        && latest_step
            .as_ref()
            .is_some_and(|step| step.error_code.is_none());
    if candidate_id.is_some() || periodic_gate_without_candidate {
        return Ok(());
    }
    Err(conflict_error(
        HUMAN_DECISION_CANDIDATE_UNAVAILABLE,
        "No waiting chapter candidate is available to accept",
    ))
}

async fn find_scoped_run(
    db: &DatabaseConnection,
    path: &ProjectRunPath,
    user_id: &str,
) -> Result<novel_autopilot_run::Model, ApiError> {
    let run = NovelAutopilotRepository::find_owned(db, &path.run_id, user_id)
        .await
        .map_err(map_repository_error)?;
    if run.project_id != path.project_id {
        return Err(not_found_error());
    }
    Ok(run)
}

pub(crate) fn spawn_startup_reconciliation(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) {
    tokio::spawn(async move {
        let runs = match NovelAutopilotRepository::list_startup_recoverable(&db).await {
            Ok(runs) => runs,
            Err(error) => {
                tracing::error!(
                    event = "novel_book_autopilot_startup_scan_failed",
                    error_code = error.code(),
                    "durable novel autopilot startup reconciliation could not scan runs"
                );
                return;
            }
        };

        let mut recovered_count = 0_u64;
        for run in runs {
            let recovered = match NovelAutopilotRepository::prepare_startup_recovery(
                &db,
                &run.id,
                run.version,
                run.epoch,
            )
            .await
            {
                Ok(recovered) => recovered,
                Err(NovelAutopilotRepositoryError::StaleVersion)
                | Err(NovelAutopilotRepositoryError::StaleEpoch)
                | Err(NovelAutopilotRepositoryError::InvalidTransition) => {
                    tracing::info!(
                        event = "novel_book_autopilot_startup_recovery_skipped",
                        run_id = %run.id,
                        "durable novel autopilot run changed while startup reconciliation was running"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::error!(
                        event = "novel_book_autopilot_startup_recovery_failed",
                        run_id = %run.id,
                        error_code = error.code(),
                        "durable novel autopilot run could not be fenced for startup recovery"
                    );
                    continue;
                }
            };

            match create_run_task(
                db.clone(),
                registry.clone(),
                stream_hub.clone(),
                book_import_service.clone(),
                candidate_gateway_config.clone(),
                &recovered.user_id,
                &recovered,
                None,
            )
            .await
            {
                Ok(_) => recovered_count += 1,
                Err((status, _)) => {
                    tracing::error!(
                        event = "novel_book_autopilot_startup_schedule_failed",
                        run_id = %recovered.id,
                        http_status = %status,
                        "durable novel autopilot startup recovery could not schedule a new tick"
                    );
                }
            }
        }

        if recovered_count > 0 {
            tracing::info!(
                event = "novel_book_autopilot_startup_reconciled",
                recovered_count,
                "durable novel autopilot runs were rescheduled after startup"
            );
        }
    });
}

async fn run_needs_dispatch_retry(
    run: &novel_autopilot_run::Model,
    registry: &TaskRegistry,
) -> bool {
    if run.status != NovelAutopilotRunStatus::Queued.as_str() {
        return false;
    }

    let Some(task_id) = run.active_background_task_id.as_deref() else {
        return true;
    };
    match registry.get(task_id).await {
        Some(task) => !task.status.is_active(),
        None => true,
    }
}

fn run_task_lease(user_id: &str, run: &novel_autopilot_run::Model) -> NovelAutopilotNextTickLease {
    NovelAutopilotNextTickLease {
        run_id: run.id.clone(),
        project_id: run.project_id.clone(),
        user_id: user_id.to_string(),
        epoch: run.epoch,
        version: run.version,
        current_phase: run.current_phase.clone(),
    }
}

async fn create_run_task(
    db: DatabaseConnection,
    registry: TaskRegistry,
    stream_hub: TaskStreamHub,
    book_import_service: Arc<BookImportService>,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    user_id: &str,
    run: &novel_autopilot_run::Model,
    decision: Option<HumanDecision>,
) -> Result<Value, ApiError> {
    let lease = run_task_lease(user_id, run);
    match schedule_owned_novel_book_autopilot_tick(
        db,
        registry,
        stream_hub,
        book_import_service,
        candidate_gateway_config,
        &lease,
        decision.as_ref().map(|decision| (*decision).as_str()),
    )
    .await
    {
        Ok(NovelBookAutopilotTaskScheduleOutcome::Scheduled { task }) => Ok(task_view(&task)),
        Ok(NovelBookAutopilotTaskScheduleOutcome::Superseded) => Err(conflict_error(
            "run_task_binding_conflict",
            "Run background task binding changed concurrently",
        )),
        Err(NovelBookAutopilotTaskScheduleError::Repository(error)) => {
            Err(map_repository_error(error))
        }
        Err(error) => {
            tracing::error!(
                event = "novel_book_autopilot_task_create_failed",
                error_code = error.code(),
                run_id = %run.id,
                "durable novel autopilot background task could not be created"
            );
            Err(internal_error())
        }
    }
}

async fn cancel_runtime_if_present(
    registry: &TaskRegistry,
    stream_hub: &TaskStreamHub,
    task_id: Option<&str>,
    user_id: &str,
) {
    if let Some(task_id) = task_id {
        let _ = cancel_task_runtime(registry, stream_hub, task_id, user_id).await;
    }
}

fn normalize_guidance(guidance: &str) -> Result<String, ApiError> {
    let normalized = guidance.trim();
    let char_count = normalized.chars().count();
    if normalized.is_empty() || char_count > MAX_GUIDANCE_CHARS {
        return Err(validation_error(
            "invalid_guidance",
            "guidance must contain between 1 and 4000 characters",
        ));
    }
    Ok(normalized.to_string())
}

fn digest_normalized_guidance(guidance: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(guidance.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn digest_guidance(guidance: &str) -> Result<String, ApiError> {
    normalize_guidance(guidance).map(|guidance| digest_normalized_guidance(&guidance))
}

fn run_view(run: &novel_autopilot_run::Model) -> Value {
    let private_snapshot = NovelAutopilotPrivateSnapshot::decode(&run.config_snapshot).ok();
    let config = private_snapshot.as_ref().map(|snapshot| &snapshot.config);
    json!({
        "id": run.id,
        "project_id": run.project_id,
        "schema_version": run.schema_version,
        "status": run.status,
        "current_phase": run.current_phase,
        "current_step": run.current_step,
        "current_chapter_id": run.current_chapter_id,
        "current_chapter_number": run.current_chapter_number,
        "total_chapters": run.total_chapters,
        "completed_chapters": run.completed_chapters,
        "failed_chapter_count": run.failed_chapters.as_array().map_or(0, Vec::len),
        "pending_rewrite_count": run.pending_rewrites.as_array().map_or(0, Vec::len),
        "total_word_count": run.total_word_count,
        "execution_scope": run.execution_scope,
        "human_gate_mode": run.human_gate_mode,
        "gate_interval": run.gate_interval,
        "max_chapters": run.max_chapters,
        "max_tokens": run.max_tokens,
        "max_estimated_cost": run.max_estimated_cost,
        "max_runtime_seconds": run.max_runtime_seconds,
        "next_chapter_count": config.and_then(|config| config.next_chapter_count),
        "max_step_attempts": config.map(|config| config.max_step_attempts),
        "max_consecutive_provider_failures": config.map(|config| config.max_consecutive_provider_failures),
        "max_consecutive_quality_failures": config.map(|config| config.max_consecutive_quality_failures),
        "regenerate_existing": config.map(|config| config.regenerate_existing),
        "run_book_review": config.map(|config| config.run_book_review),
        "run_book_polish": config.map(|config| config.run_book_polish),
        "export_format": config.map(|config| config.export_format.as_str()),
        "used_tokens": run.used_tokens,
        "estimated_cost": run.estimated_cost,
        "epoch": run.epoch,
        "version": run.version,
        "consecutive_provider_failures": run.consecutive_provider_failures,
        "consecutive_quality_failures": run.consecutive_quality_failures,
        "last_error_code": run.last_error_code,
        "has_guidance": run.guidance_digest.is_some(),
        "active_background_task_id": run.active_background_task_id,
        "final_export_ref": run.final_export_ref,
        "created_at": utc_rfc3339(run.created_at),
        "updated_at": utc_rfc3339(run.updated_at),
        "started_at": optional_utc_rfc3339(run.started_at),
        "paused_at": optional_utc_rfc3339(run.paused_at),
        "completed_at": optional_utc_rfc3339(run.completed_at),
    })
}

fn step_view(step: &novel_autopilot_step_run::Model, candidate_id: Option<&str>) -> Value {
    json!({
        "id": step.id,
        "run_id": step.run_id,
        "step_key": step.step_key,
        "step_type": step.step_type,
        "phase": step.phase,
        "chapter_id": step.chapter_id,
        "chapter_number": step.chapter_number,
        "attempt": step.attempt,
        "run_epoch": step.run_epoch,
        "status": step.status,
        "background_task_id": step.background_task_id,
        "quality_decision": step.quality_decision,
        "error_code": step.error_code,
        "candidate_id": candidate_id,
        "started_at": optional_utc_rfc3339(step.started_at),
        "completed_at": optional_utc_rfc3339(step.completed_at),
        "created_at": utc_rfc3339(step.created_at),
        "updated_at": utc_rfc3339(step.updated_at),
    })
}

fn utc_rfc3339(value: chrono::NaiveDateTime) -> String {
    format!("{}Z", value.format("%Y-%m-%dT%H:%M:%S%.f"))
}

fn optional_utc_rfc3339(value: Option<chrono::NaiveDateTime>) -> Value {
    value
        .map(|value| Value::String(utc_rfc3339(value)))
        .unwrap_or(Value::Null)
}

fn task_view(payload: &Value) -> Value {
    json!({
        "task_id": payload.get("task_id").and_then(Value::as_str),
        "task_type": payload.get("task_type").and_then(Value::as_str),
        "status": payload.get("status").and_then(Value::as_str),
        "progress": payload.get("progress").and_then(Value::as_i64),
        "message": payload.get("message").and_then(Value::as_str),
    })
}

fn map_repository_error(error: NovelAutopilotRepositoryError) -> ApiError {
    match error {
        NovelAutopilotRepositoryError::NotFoundOrAccessDenied => not_found_error(),
        NovelAutopilotRepositoryError::InvalidConfig { field, code } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "Invalid novel autopilot configuration",
                "code": "invalid_novel_autopilot_config",
                "field": field,
                "reason": code,
            })),
        ),
        NovelAutopilotRepositoryError::InvalidTransition => conflict_error(
            "invalid_run_transition",
            "Run state does not allow this operation",
        ),
        NovelAutopilotRepositoryError::StaleVersion => {
            conflict_error("stale_run_version", "Run version is stale")
        }
        NovelAutopilotRepositoryError::StaleEpoch => {
            conflict_error("stale_run_epoch", "Run execution epoch is stale")
        }
        NovelAutopilotRepositoryError::BusinessDataChanged => conflict_error(
            "novel_autopilot_business_data_changed",
            "Project data changed while the step was running",
        ),
        NovelAutopilotRepositoryError::Database(detail) => {
            tracing::error!(
                event = "novel_book_autopilot_repository_failed",
                error_code = "database_error",
                error = %detail,
                "durable novel autopilot repository operation failed"
            );
            internal_error()
        }
    }
}

fn internal_project_error(detail: String) -> ApiError {
    tracing::error!(
        event = "novel_book_autopilot_project_read_failed",
        error = %detail,
        "durable novel autopilot project lookup failed"
    );
    internal_error()
}

fn validation_error(code: &'static str, detail: &'static str) -> ApiError {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"detail": detail, "code": code})),
    )
}

fn conflict_error(code: &'static str, detail: &'static str) -> ApiError {
    (
        StatusCode::CONFLICT,
        Json(json!({"detail": detail, "code": code})),
    )
}

fn not_found_error() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "detail": "Novel autopilot run not found",
            "code": "novel_autopilot_run_not_found",
        })),
    )
}

fn internal_error() -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "detail": "Unable to process novel autopilot run",
            "code": "novel_autopilot_internal_error",
        })),
    )
}

pub fn routes() -> Router {
    Router::new()
        .route(RUNS_ROUTE, post(create_run).get(list_runs))
        .route(RUN_ROUTE, get(get_run))
        .route(STEPS_ROUTE, get(list_steps))
        .route(PAUSE_ROUTE, post(pause_run))
        .route(RESUME_ROUTE, post(resume_run))
        .route(CANCEL_ROUTE, post(cancel_run))
        .route(GUIDANCE_ROUTE, post(update_guidance))
        .route(DECISION_ROUTE, post(submit_decision))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        extract::{Extension, Path},
        http::StatusCode,
        Json,
    };
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        PaginatorTrait, Schema, Set, Statement,
    };
    use serde_json::{json, Value};

    use super::{
        cancel_run, create_run, digest_guidance, get_run, list_runs, list_steps, pause_run,
        resume_run, run_needs_dispatch_retry, run_view, step_view, submit_decision, utc_rfc3339,
        CreateRunRequest, HumanDecision, HumanDecisionRequest, ProjectRunPath,
        VersionedControlRequest,
    };
    use crate::{
        api::background_tasks::best_effort_wait_for_human_after_schedule_failure,
        models::{chapter_draft_attempt, novel_autopilot_run, novel_autopilot_step_run, project},
        services::{
            auth::Claims,
            book_import_service::BookImportService,
            chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
            novel_autopilot::{
                repository::{CreateNovelAutopilotRun, NovelAutopilotRepository},
                types::{NovelAutopilotPrivateSnapshot, NovelAutopilotRunConfig},
            },
        },
        tasks::{
            registry::TaskRegistry,
            stream::TaskStreamHub,
            types::{TaskRecord, TaskStatus},
        },
    };

    #[test]
    fn guidance_digest_is_bounded_and_does_not_echo_source_text() {
        let digest = digest_guidance("  只影响后续章节  ").expect("valid guidance");
        assert!(digest.starts_with("sha256:"));
        assert!(!digest.contains("只影响后续章节"));
        assert!(digest_guidance("   ").is_err());
        assert!(digest_guidance(&"a".repeat(4_001)).is_err());
    }

    #[test]
    fn utc_time_contract_preserves_value_and_fractional_precision() {
        let timestamp = NaiveDate::from_ymd_opt(2026, 8, 1)
            .expect("valid date")
            .and_hms_micro_opt(5, 34, 58, 865_557)
            .expect("valid timestamp");

        assert_eq!(utc_rfc3339(timestamp), "2026-08-01T05:34:58.865557Z");
    }

    #[test]
    fn allowlist_views_do_not_expose_private_payloads_or_digests() {
        let now = chrono::Utc::now().naive_utc();
        let run = novel_autopilot_run::Model {
            id: "run-1".into(),
            project_id: "project-1".into(),
            user_id: "secret-user".into(),
            schema_version: "novel-autopilot/v1".into(),
            status: "running".into(),
            current_phase: "foundation".into(),
            current_step: Some("planning:foundation".into()),
            active_scope_key: Some("project-1".into()),
            current_chapter_id: None,
            current_chapter_number: None,
            total_chapters: 10,
            completed_chapters: 0,
            failed_chapters: json!([]),
            pending_rewrites: json!([]),
            total_word_count: 0,
            execution_scope: "complete_book".into(),
            human_gate_mode: "high_risk_only".into(),
            gate_interval: Some(5),
            config_snapshot: serde_json::to_value(NovelAutopilotPrivateSnapshot {
                config: NovelAutopilotRunConfig::default(),
                guidance: Some("private guidance".into()),
            })
            .expect("private snapshot"),
            max_chapters: Some(10),
            max_tokens: Some(1000),
            max_estimated_cost: None,
            max_runtime_seconds: Some(3600),
            used_tokens: 0,
            estimated_cost: 0.0,
            epoch: 1,
            version: 2,
            consecutive_provider_failures: 0,
            consecutive_quality_failures: 0,
            last_error_code: None,
            guidance_digest: Some("sha256:private".into()),
            active_background_task_id: Some("task-1".into()),
            final_export_ref: None,
            created_at: now,
            updated_at: now,
            started_at: Some(now),
            paused_at: None,
            completed_at: None,
        };
        let step = novel_autopilot_step_run::Model {
            id: "step-1".into(),
            run_id: "run-1".into(),
            step_key: "planning:foundation".into(),
            step_type: "foundation".into(),
            phase: "foundation".into(),
            chapter_id: None,
            chapter_number: None,
            attempt: 1,
            run_epoch: 1,
            status: "running".into(),
            background_task_id: Some("task-1".into()),
            input_digest: "sha256:private-input".into(),
            result_digest: Some("sha256:private-result".into()),
            quality_decision: None,
            error_code: None,
            started_at: Some(now),
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        let run_payload = run_view(&run);
        let step_payload = step_view(&step, Some("step-1"));
        assert_eq!(run_payload["max_step_attempts"], json!(3));
        assert_eq!(run_payload["run_book_review"], json!(true));
        assert_eq!(run_payload["run_book_polish"], json!(true));
        assert_eq!(run_payload["export_format"], json!("txt"));
        assert!(run_payload["created_at"]
            .as_str()
            .expect("created_at string")
            .ends_with('Z'));
        assert!(run_payload["updated_at"]
            .as_str()
            .expect("updated_at string")
            .ends_with('Z'));
        assert!(run_payload["started_at"]
            .as_str()
            .expect("started_at string")
            .ends_with('Z'));
        assert!(step_payload["created_at"]
            .as_str()
            .expect("step created_at string")
            .ends_with('Z'));
        assert!(step_payload["updated_at"]
            .as_str()
            .expect("step updated_at string")
            .ends_with('Z'));
        assert!(step_payload["started_at"]
            .as_str()
            .expect("step started_at string")
            .ends_with('Z'));
        assert_eq!(step_payload["candidate_id"], json!("step-1"));
        assert!(run_payload["paused_at"].is_null());
        assert!(run_payload["completed_at"].is_null());
        assert!(step_payload["completed_at"].is_null());
        assert!(!run_payload.to_string().contains("private guidance"));
        for forbidden in [
            "user_id",
            "active_scope_key",
            "config_snapshot",
            "guidance_digest",
        ] {
            assert!(
                run_payload.get(forbidden).is_none(),
                "must hide {forbidden}"
            );
        }
        for forbidden in ["input_digest", "result_digest"] {
            assert!(
                step_payload.get(forbidden).is_none(),
                "must hide {forbidden}"
            );
        }
    }

    async fn setup_api_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect novel autopilot api sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)))
            .await
            .expect("create novel autopilot runs table");
        db.execute(
            builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
        )
        .await
        .expect("create novel autopilot step runs table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)))
            .await
            .expect("create chapter draft attempts table");
        db.execute(Statement::from_string(
            builder,
            "CREATE UNIQUE INDEX uq_test_novel_autopilot_api_active_scope ON novel_autopilot_runs (active_scope_key)"
                .to_string(),
        ))
        .await
        .expect("create active run uniqueness index");
        db
    }

    async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str) {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("valid date")
            .and_hms_opt(8, 0, 0)
            .expect("valid time");
        project::ActiveModel {
            id: Set(id.to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(format!("Autopilot API {id}")),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set("foundation".to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("linear".to_string()),
            character_count: Set(0),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert project");
    }

    fn claims(user_id: &str) -> Claims {
        Claims {
            sub: user_id.to_string(),
            username: user_id.to_string(),
            is_admin: false,
            exp: usize::MAX,
            iat: 0,
        }
    }

    fn gateway_config() -> ChapterCandidateRouteGatewayConfig {
        ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: false,
            fallback_on_rust_error: false,
            disabled_reason: Some("api test".to_string()),
            rollback_boundary: "api_test".to_string(),
        }
    }

    async fn seed_run(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> novel_autopilot_run::Model {
        NovelAutopilotRepository::create_or_get_active(
            db,
            CreateNovelAutopilotRun {
                project_id: project_id.to_string(),
                user_id: user_id.to_string(),
                total_chapters: 3,
                config: NovelAutopilotRunConfig::default(),
            },
        )
        .await
        .expect("create active run")
        .run
    }

    fn assert_error(error: (StatusCode, Json<Value>), status: StatusCode, code: &str) {
        assert_eq!(error.0, status);
        assert_eq!(error.1 .0["code"], json!(code));
    }

    #[tokio::test]
    async fn decision_schedule_failure_restores_waiting_human_for_retry() {
        let db = setup_api_db().await;
        insert_project(&db, "project-decision-retry", "owner-decision-retry").await;
        let run = seed_run(&db, "project-decision-retry", "owner-decision-retry").await;
        let lease = super::run_task_lease("owner-decision-retry", &run);

        best_effort_wait_for_human_after_schedule_failure(
            &db,
            &lease,
            "novel_autopilot_task_schedule_failed",
        )
        .await;

        let recovered = NovelAutopilotRepository::find_owned(&db, &run.id, "owner-decision-retry")
            .await
            .expect("load compensated run");
        assert_eq!(
            recovered.status,
            crate::services::novel_autopilot::types::NovelAutopilotRunStatus::WaitingHuman.as_str()
        );
        assert_eq!(recovered.version, run.version + 1);
        assert!(recovered.active_background_task_id.is_none());
    }

    #[tokio::test]
    async fn queued_run_dispatch_retry_only_skips_active_registry_task() {
        let db = setup_api_db().await;
        insert_project(&db, "project-dispatch-retry", "owner-dispatch-retry").await;
        let mut run = seed_run(&db, "project-dispatch-retry", "owner-dispatch-retry").await;
        let registry = TaskRegistry::new();

        assert!(run_needs_dispatch_retry(&run, &registry).await);

        let mut task = TaskRecord::new(
            "task-dispatch-retry".to_string(),
            "novel_book_autopilot".to_string(),
            "owner-dispatch-retry".to_string(),
            "project-dispatch-retry".to_string(),
            "auto".to_string(),
        );
        registry.insert(task.clone()).await;
        run.active_background_task_id = Some(task.task_id.clone());
        assert!(!run_needs_dispatch_retry(&run, &registry).await);

        task.status = TaskStatus::Failed;
        registry.insert(task).await;
        assert!(run_needs_dispatch_retry(&run, &registry).await);
    }

    #[tokio::test]
    async fn api_owner_reads_scoped_run_and_non_owner_is_hidden() {
        let db = setup_api_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let run = seed_run(&db, "project-1", "owner-1").await;

        let listed = list_runs(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Path("project-1".to_string()),
        )
        .await
        .expect("owner should list runs");
        assert_eq!(listed.0, StatusCode::OK);
        assert_eq!(listed.1 .0["items"].as_array().map(Vec::len), Some(1));
        assert_eq!(listed.1 .0["items"][0]["id"], json!(run.id));

        let detail = get_run(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Path(ProjectRunPath {
                project_id: "project-1".to_string(),
                run_id: run.id.clone(),
            }),
        )
        .await
        .expect("owner should read run detail");
        assert_eq!(detail.1 .0["run"]["id"], json!(run.id));
        assert!(detail.1 .0["run"].get("config_snapshot").is_none());

        let steps = list_steps(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Path(ProjectRunPath {
                project_id: "project-1".to_string(),
                run_id: run.id.clone(),
            }),
        )
        .await
        .expect("owner should list run steps");
        assert_eq!(steps.1 .0["items"], json!([]));

        let hidden_list = list_runs(
            Extension(claims("attacker-1")),
            Extension(db.clone()),
            Path("project-1".to_string()),
        )
        .await
        .expect_err("non-owner project list must remain hidden");
        assert_error(
            hidden_list,
            StatusCode::NOT_FOUND,
            "novel_autopilot_run_not_found",
        );

        let hidden_detail = get_run(
            Extension(claims("attacker-1")),
            Extension(db.clone()),
            Path(ProjectRunPath {
                project_id: "project-1".to_string(),
                run_id: run.id.clone(),
            }),
        )
        .await
        .expect_err("non-owner must not read run detail");
        assert_error(
            hidden_detail,
            StatusCode::NOT_FOUND,
            "novel_autopilot_run_not_found",
        );

        let wrong_project = get_run(
            Extension(claims("owner-1")),
            Extension(db),
            Path(ProjectRunPath {
                project_id: "project-2".to_string(),
                run_id: run.id,
            }),
        )
        .await
        .expect_err("run must remain scoped to the route project");
        assert_error(
            wrong_project,
            StatusCode::NOT_FOUND,
            "novel_autopilot_run_not_found",
        );
    }

    #[tokio::test]
    async fn api_non_owner_cannot_create_project_run() {
        let db = setup_api_db().await;
        insert_project(&db, "project-1", "owner-1").await;

        let error = create_run(
            Extension(claims("attacker-1")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Extension(gateway_config()),
            Path("project-1".to_string()),
            Json(CreateRunRequest {
                config: NovelAutopilotRunConfig::default(),
                total_chapters: Some(3),
            }),
        )
        .await
        .expect_err("non-owner must not create a project run");
        assert_error(
            error,
            StatusCode::NOT_FOUND,
            "novel_autopilot_run_not_found",
        );
        assert_eq!(
            novel_autopilot_run::Entity::find()
                .count(&db)
                .await
                .expect("count runs"),
            0,
            "authorization must fail before creating durable state"
        );
    }

    #[tokio::test]
    async fn api_control_handlers_enforce_versions_and_legal_states() {
        let db = setup_api_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let run = seed_run(&db, "project-1", "owner-1").await;
        let path = || ProjectRunPath {
            project_id: "project-1".to_string(),
            run_id: run.id.clone(),
        };

        let invalid_pause = pause_run(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Path(path()),
            Json(VersionedControlRequest {
                expected_version: run.version,
            }),
        )
        .await
        .expect_err("queued run cannot be paused");
        assert_error(
            invalid_pause,
            StatusCode::CONFLICT,
            "invalid_run_transition",
        );

        let invalid_decision = submit_decision(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Extension(gateway_config()),
            Path(path()),
            Json(HumanDecisionRequest {
                expected_version: run.version,
                decision: HumanDecision::Accept,
                guidance: None,
            }),
        )
        .await
        .expect_err("decision requires waiting_human state");
        assert_error(
            invalid_decision,
            StatusCode::CONFLICT,
            "run_not_waiting_human",
        );

        let stale_cancel = cancel_run(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Path(path()),
            Json(VersionedControlRequest {
                expected_version: run.version + 1,
            }),
        )
        .await
        .expect_err("stale control request must be fenced");
        assert_error(stale_cancel, StatusCode::CONFLICT, "stale_run_version");

        let cancelled = cancel_run(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Path(path()),
            Json(VersionedControlRequest {
                expected_version: run.version,
            }),
        )
        .await
        .expect("queued run should be cancellable");
        assert_eq!(cancelled.1 .0["run"]["status"], json!("cancelled"));
        assert!(cancelled.1 .0["run"].get("active_scope_key").is_none());

        let cancelled_version = cancelled.1 .0["run"]["version"]
            .as_i64()
            .expect("cancel response version");
        let repeated_cancel = cancel_run(
            Extension(claims("owner-1")),
            Extension(db),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Path(path()),
            Json(VersionedControlRequest {
                expected_version: cancelled_version,
            }),
        )
        .await
        .expect_err("terminal run cannot be cancelled twice");
        assert_error(
            repeated_cancel,
            StatusCode::CONFLICT,
            "invalid_run_transition",
        );
    }

    #[tokio::test]
    async fn api_rejects_accept_when_waiting_human_has_no_candidate() {
        let db = setup_api_db().await;
        insert_project(&db, "project-no-candidate", "owner-no-candidate").await;
        let run = seed_run(&db, "project-no-candidate", "owner-no-candidate").await;
        let waiting = NovelAutopilotRepository::transition_owned(
            &db,
            &run.id,
            "owner-no-candidate",
            run.version,
            crate::services::novel_autopilot::types::NovelAutopilotRunStatus::WaitingHuman,
        )
        .await
        .expect("move run to waiting_human");
        let now = chrono::Utc::now().naive_utc();
        novel_autopilot_step_run::ActiveModel {
            id: Set("step-no-candidate".to_string()),
            run_id: Set(waiting.id.clone()),
            step_key: Set("chapter:0001:analyze".to_string()),
            step_type: Set("chapter_analyze".to_string()),
            phase: Set("chapter_loop".to_string()),
            chapter_id: Set(Some("chapter-1".to_string())),
            chapter_number: Set(Some(1)),
            attempt: Set(1),
            run_epoch: Set(waiting.epoch),
            status: Set("failed".to_string()),
            background_task_id: Set(None),
            input_digest: Set("sha256:test-input".to_string()),
            result_digest: Set(None),
            quality_decision: Set(Some("manual_review".to_string())),
            error_code: Set(Some("chapter_analysis_provider_failed".to_string())),
            started_at: Set(Some(now)),
            completed_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert no-candidate failed step");

        let error = submit_decision(
            Extension(claims("owner-no-candidate")),
            Extension(db.clone()),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Extension(gateway_config()),
            Path(ProjectRunPath {
                project_id: "project-no-candidate".to_string(),
                run_id: waiting.id.clone(),
            }),
            Json(HumanDecisionRequest {
                expected_version: waiting.version,
                decision: HumanDecision::Accept,
                guidance: None,
            }),
        )
        .await
        .expect_err("no-candidate failure must reject accept");

        assert_error(
            error,
            StatusCode::CONFLICT,
            "human_decision_candidate_unavailable",
        );
        let unchanged =
            NovelAutopilotRepository::find_owned(&db, &waiting.id, "owner-no-candidate")
                .await
                .expect("reload waiting run");
        assert_eq!(unchanged.status, "waiting_human");
        assert_eq!(unchanged.version, waiting.version);
    }

    #[tokio::test]
    async fn queued_orphan_run_resume_redispatches_background_task() {
        let db = setup_api_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let run = seed_run(&db, "project-1", "owner-1").await;

        let resumed = resume_run(
            Extension(claims("owner-1")),
            Extension(db),
            Extension(TaskRegistry::new()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Extension(gateway_config()),
            Path(ProjectRunPath {
                project_id: "project-1".to_string(),
                run_id: run.id,
            }),
            Json(VersionedControlRequest {
                expected_version: run.version,
            }),
        )
        .await
        .expect("queued orphan run should be re-dispatched");

        assert_eq!(resumed.0, StatusCode::OK);
        assert_eq!(
            resumed.1 .0["background_task"]["task_type"],
            json!("novel_book_autopilot")
        );
        assert_eq!(resumed.1 .0["background_task"]["status"], json!("pending"));
        assert_eq!(resumed.1 .0["run"]["status"], json!("queued"));
        assert!(resumed.1 .0["run"]["active_background_task_id"].is_string());
    }
}
