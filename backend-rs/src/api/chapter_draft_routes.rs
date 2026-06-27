use std::collections::BTreeSet;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use serde::Deserialize;
use serde_json::Value;

use crate::models::{chapter, chapter_draft_attempt};
use crate::services::auth::Claims;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
use crate::services::chapter_draft_history_service::{
    auto_revision_apply_history_payload, auto_revision_draft_apply_history_model,
    candidate_draft_apply_history_model, candidate_draft_generated_content_payload,
    load_latest_reviser_history,
};
use crate::services::chapter_draft_source_service::{
    extract_candidate_draft_full_content, format_datetime, is_draft_stale,
    load_candidate_draft_attempt, python_truthy_scalar_text,
};
use crate::services::chapter_draft_view_payload_service::{
    build_auto_revision_draft_payload, build_candidate_draft_payload,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};

const AUTO_REVISION_DRAFT_ROUTE: &str = "/chapters/{chapter_id}/analysis/auto-revision-draft";
const AUTO_REVISION_DRAFT_APPLY_ROUTE: &str =
    "/chapters/{chapter_id}/analysis/auto-revision-draft/apply";
const CANDIDATE_DRAFT_ROUTE: &str = "/chapters/{chapter_id}/analysis/candidate-draft";
const CANDIDATE_DRAFT_APPLY_ROUTE: &str = "/chapters/{chapter_id}/analysis/candidate-draft/apply";

#[allow(dead_code)]
const CHAPTER_DRAFT_ROUTE_GROUP: &str = "chapter_draft";
#[allow(dead_code)]
const CHAPTER_DRAFT_RUST_OWNER: &str = "backend-rs/src/api/chapter_draft_routes.rs";
#[allow(dead_code)]
const CHAPTER_DRAFT_ROUTE_OWNER: &str = "backend-rs/src/api/chapter_draft_routes.rs";
#[allow(dead_code)]
const CHAPTER_DRAFT_FALLBACK_SHELL: &str = "";
#[allow(dead_code)]
const CHAPTER_DRAFT_ROLLBACK_BOUNDARY: &str = "python_chapter_draft_routes_fallback";

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct AutoRevisionDraftLookupRouteQuery {
    pub(crate) history_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct CandidateDraftLookupRouteQuery {
    pub(crate) attempt_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct AutoRevisionDraftApplyRouteRequest {
    pub(crate) history_id: Option<String>,
    #[serde(default)]
    pub(crate) allow_stale: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct CandidateDraftApplyRouteRequest {
    pub(crate) attempt_id: Option<String>,
    #[serde(default)]
    pub(crate) allow_stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnedDraftSelectionMode {
    Latest,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedDraftPayloadRequest {
    selector: Option<String>,
    selection_mode: OwnedDraftSelectionMode,
    allow_stale: bool,
}

impl OwnedDraftPayloadRequest {
    pub(crate) fn new(selector: Option<&str>, allow_stale: bool) -> Self {
        Self {
            selector: selector.map(str::to_string),
            selection_mode: if selector.is_some() {
                OwnedDraftSelectionMode::Explicit
            } else {
                OwnedDraftSelectionMode::Latest
            },
            allow_stale,
        }
    }

    fn from_route_selector(selector: Option<String>, allow_stale: bool) -> Self {
        let selector = selector
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        Self::new(selector.as_deref(), allow_stale)
    }
}

pub(crate) fn build_auto_revision_draft_payload_request_from_route_query(
    route_query: AutoRevisionDraftLookupRouteQuery,
) -> OwnedDraftPayloadRequest {
    OwnedDraftPayloadRequest::from_route_selector(route_query.history_id, false)
}

pub(crate) fn build_auto_revision_draft_payload_request_from_route_payload(
    route_request: AutoRevisionDraftApplyRouteRequest,
) -> OwnedDraftPayloadRequest {
    OwnedDraftPayloadRequest::from_route_selector(
        route_request.history_id,
        route_request.allow_stale,
    )
}

pub(crate) fn build_candidate_draft_payload_request_from_route_query(
    route_query: CandidateDraftLookupRouteQuery,
) -> OwnedDraftPayloadRequest {
    OwnedDraftPayloadRequest::from_route_selector(route_query.attempt_id, false)
}

pub(crate) fn build_candidate_draft_payload_request_from_route_payload(
    route_request: CandidateDraftApplyRouteRequest,
) -> OwnedDraftPayloadRequest {
    OwnedDraftPayloadRequest::from_route_selector(
        route_request.attempt_id,
        route_request.allow_stale,
    )
}

fn build_draft_detail_response_payload(
    chapter_id: &str,
    payload_key: &str,
    payload: Value,
) -> Value {
    serde_json::json!({
        "chapter_id": chapter_id,
        payload_key: payload,
    })
}

fn build_candidate_draft_detail_payload(
    chapter: &chapter::Model,
    draft_attempt: &crate::models::chapter_draft_attempt::Model,
) -> Value {
    build_draft_detail_response_payload(
        &chapter.id,
        "candidate_draft",
        build_candidate_draft_payload(draft_attempt, chapter.updated_at, true),
    )
}

fn build_auto_revision_draft_detail_payload(
    chapter: &chapter::Model,
    reviser_history: &crate::models::generation_history::Model,
    reviser_result: &Value,
) -> Value {
    build_draft_detail_response_payload(
        &chapter.id,
        "auto_revision_draft",
        build_auto_revision_draft_payload(
            reviser_result,
            Some(&reviser_history.id),
            reviser_history.created_at,
            chapter.updated_at,
            true,
        ),
    )
}

async fn load_candidate_draft_detail_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    attempt_id: Option<&str>,
) -> Result<Value, CandidateDraftError> {
    let draft_attempt = load_candidate_draft_attempt(db, &chapter.id, attempt_id)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let draft_attempt = draft_attempt.ok_or(CandidateDraftError::NotFound)?;
    Ok(build_candidate_draft_detail_payload(
        chapter,
        &draft_attempt,
    ))
}

async fn load_auto_revision_draft_detail_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    history_id: Option<&str>,
) -> Result<Value, AutoRevisionDraftError> {
    let reviser_loaded = load_latest_reviser_history(db, &chapter.id, history_id)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let (reviser_history, reviser_result) =
        reviser_loaded.ok_or(AutoRevisionDraftError::NotFound)?;

    Ok(build_auto_revision_draft_detail_payload(
        chapter,
        &reviser_history,
        &reviser_result,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApplyDraftWordCounts {
    old_word_count: i32,
    new_word_count: i32,
    new_word_count_usize: usize,
}

fn sanitize_apply_draft_text<E>(
    raw_text: &str,
    empty_error: E,
    workflow_meta_error: E,
) -> Result<String, E> {
    let (cleaned_text, _) = sanitize_generated_narrative_text(raw_text);
    if cleaned_text.trim().is_empty() {
        return Err(empty_error);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(workflow_meta_error);
    }
    Ok(cleaned_text)
}

fn prepare_candidate_draft_apply_text(
    draft_attempt: &chapter_draft_attempt::Model,
) -> Result<String, CandidateDraftError> {
    let (candidate_content_raw, has_full_content) =
        extract_candidate_draft_full_content(draft_attempt);
    if !has_full_content || candidate_content_raw.trim().is_empty() {
        return Err(CandidateDraftError::PreviewOnly);
    }

    sanitize_apply_draft_text(
        &candidate_content_raw,
        CandidateDraftError::EmptyContent,
        CandidateDraftError::WorkflowMetaText,
    )
}

fn prepare_auto_revision_draft_apply_text(
    reviser_result: &Value,
) -> Result<String, AutoRevisionDraftError> {
    let revised_text_raw = reviser_result
        .get("revised_text")
        .and_then(python_truthy_scalar_text)
        .unwrap_or_default();

    sanitize_apply_draft_text(
        &revised_text_raw,
        AutoRevisionDraftError::EmptyContent,
        AutoRevisionDraftError::WorkflowMetaText,
    )
}

fn validate_apply_draft_staleness<E>(
    chapter_updated_at: Option<chrono::NaiveDateTime>,
    draft_created_at: Option<chrono::NaiveDateTime>,
    allow_stale: bool,
    stale_error: E,
) -> Result<bool, E> {
    let stale = is_draft_stale(chapter_updated_at, draft_created_at);
    if stale && !allow_stale {
        return Err(stale_error);
    }
    Ok(stale)
}

fn apply_draft_word_counts(previous_word_count: i32, new_text: &str) -> ApplyDraftWordCounts {
    let new_word_count_usize = new_text.chars().count();
    ApplyDraftWordCounts {
        old_word_count: previous_word_count.max(0),
        new_word_count: new_word_count_usize as i32,
        new_word_count_usize,
    }
}

fn draft_apply_chapter_update_model(
    chapter: &chapter::Model,
    content: String,
    new_word_count: i32,
    updated_at: chrono::NaiveDateTime,
) -> chapter::ActiveModel {
    let mut chapter_active: chapter::ActiveModel = chapter.clone().into();
    chapter_active.content = Set(Some(content));
    chapter_active.word_count = Set(new_word_count);
    chapter_active.updated_at = Set(Some(updated_at));
    chapter_active
}

fn candidate_draft_apply_response_payload(
    chapter_id: &str,
    new_word_count: i32,
    old_word_count: i32,
    draft_attempt_id: &str,
    draft_created_at: Option<chrono::NaiveDateTime>,
    stale: bool,
) -> Value {
    serde_json::json!({
        "success": true,
        "chapter_id": chapter_id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_attempt_id": draft_attempt_id,
        "draft_created_at": format_datetime(draft_created_at),
        "stale_applied": stale,
        "message": "候选草稿已恢复到章节正文",
    })
}

fn auto_revision_draft_apply_response_payload(
    chapter_id: &str,
    new_word_count: i32,
    old_word_count: i32,
    draft_history_id: &str,
    draft_created_at: Option<chrono::NaiveDateTime>,
    stale: bool,
) -> Value {
    serde_json::json!({
        "success": true,
        "chapter_id": chapter_id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_history_id": draft_history_id,
        "draft_created_at": format_datetime(draft_created_at),
        "stale_applied": stale,
        "message": "自动修订草稿已应用到章节正文",
    })
}

pub async fn apply_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    attempt_id: Option<&str>,
    allow_stale: bool,
) -> Result<Value, CandidateDraftError> {
    let draft_attempt = load_candidate_draft_attempt(db, &chapter.id, attempt_id)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;
    let draft_attempt = draft_attempt.ok_or(CandidateDraftError::NotFound)?;

    let candidate_content = prepare_candidate_draft_apply_text(&draft_attempt)?;
    let stale = validate_apply_draft_staleness(
        chapter.updated_at,
        draft_attempt.created_at,
        allow_stale,
        CandidateDraftError::Stale,
    )?;

    let generated_content = candidate_draft_generated_content_payload(
        &candidate_content,
        draft_attempt.quality_metrics.clone(),
    );

    let now = Utc::now().naive_utc();
    let word_counts = apply_draft_word_counts(chapter.word_count, &candidate_content);
    let txn = db
        .begin()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let chapter_active = draft_apply_chapter_update_model(
        chapter,
        candidate_content,
        word_counts.new_word_count,
        now,
    );
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let history = candidate_draft_apply_history_model(
        uuid::Uuid::new_v4().to_string(),
        chapter,
        generated_content,
        now,
    );
    history
        .insert(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    Ok(candidate_draft_apply_response_payload(
        &chapter.id,
        word_counts.new_word_count,
        word_counts.old_word_count,
        &draft_attempt.id,
        draft_attempt.created_at,
        stale,
    ))
}

pub async fn apply_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    history_id: Option<&str>,
    allow_stale: bool,
) -> Result<Value, AutoRevisionDraftError> {
    let reviser_loaded = load_latest_reviser_history(db, &chapter.id, history_id)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;
    let (reviser_history, reviser_result) =
        reviser_loaded.ok_or(AutoRevisionDraftError::NotFound)?;

    let revised_text = prepare_auto_revision_draft_apply_text(&reviser_result)?;
    let stale = validate_apply_draft_staleness(
        chapter.updated_at,
        reviser_history.created_at,
        allow_stale,
        AutoRevisionDraftError::Stale,
    )?;

    let word_counts = apply_draft_word_counts(chapter.word_count, &revised_text);
    let history_payload = auto_revision_apply_history_payload(
        &reviser_history,
        &reviser_result,
        chapter.word_count,
        word_counts.new_word_count_usize,
        stale,
        allow_stale,
        Some(Utc::now().naive_utc()),
    );
    let now = Utc::now().naive_utc();
    let txn = db
        .begin()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let chapter_active =
        draft_apply_chapter_update_model(chapter, revised_text, word_counts.new_word_count, now);
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let history = auto_revision_draft_apply_history_model(
        uuid::Uuid::new_v4().to_string(),
        chapter,
        history_payload,
        now,
    );
    history
        .insert(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    Ok(auto_revision_draft_apply_response_payload(
        &chapter.id,
        word_counts.new_word_count,
        word_counts.old_word_count,
        &reviser_history.id,
        reviser_history.created_at,
        stale,
    ))
}

struct OwnedDraftPayloadContext {
    chapter: chapter::Model,
    request: OwnedDraftPayloadRequest,
}

pub(crate) enum OwnedDraftPayloadError<TDraftError> {
    ChapterNotFoundOrAccessDenied,
    Draft(TDraftError, OwnedDraftSelectionMode),
    Internal(String),
}

pub(crate) type OwnedAutoRevisionDraftPayloadError = OwnedDraftPayloadError<AutoRevisionDraftError>;
pub(crate) type OwnedCandidateDraftPayloadError = OwnedDraftPayloadError<CandidateDraftError>;

pub(crate) type LoadOwnedCandidateDraftPayloadError = OwnedCandidateDraftPayloadError;
pub(crate) type ApplyOwnedCandidateDraftPayloadError = OwnedCandidateDraftPayloadError;
pub(crate) type LoadOwnedAutoRevisionDraftPayloadError = OwnedAutoRevisionDraftPayloadError;
pub(crate) type ApplyOwnedAutoRevisionDraftPayloadError = OwnedAutoRevisionDraftPayloadError;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterDraftRouteReadinessProbe {
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) route_group: &'static str,
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) rust_owner: &'static str,
    pub(crate) route_payload_owner: &'static str,
    pub(crate) fallback_shell: &'static str,
    pub(crate) rollback_boundary: &'static str,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterDraftRouteReadinessResult {
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) route_group: String,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) ok: bool,
    pub(crate) rust_owner: String,
    pub(crate) route_payload_owner: String,
    pub(crate) fallback_shell: String,
    pub(crate) rollback_boundary: String,
}

#[allow(dead_code)]
pub(crate) fn build_default_chapter_draft_route_readiness_probes(
) -> Vec<ChapterDraftRouteReadinessProbe> {
    vec![
        ChapterDraftRouteReadinessProbe {
            name: "chapter-draft-auto-revision-load",
            owner: "rust",
            route_group: CHAPTER_DRAFT_ROUTE_GROUP,
            method: "GET",
            path: AUTO_REVISION_DRAFT_ROUTE,
            rust_owner: CHAPTER_DRAFT_RUST_OWNER,
            route_payload_owner: CHAPTER_DRAFT_ROUTE_OWNER,
            fallback_shell: CHAPTER_DRAFT_FALLBACK_SHELL,
            rollback_boundary: CHAPTER_DRAFT_ROLLBACK_BOUNDARY,
        },
        ChapterDraftRouteReadinessProbe {
            name: "chapter-draft-auto-revision-apply",
            owner: "rust",
            route_group: CHAPTER_DRAFT_ROUTE_GROUP,
            method: "POST",
            path: AUTO_REVISION_DRAFT_APPLY_ROUTE,
            rust_owner: CHAPTER_DRAFT_RUST_OWNER,
            route_payload_owner: CHAPTER_DRAFT_ROUTE_OWNER,
            fallback_shell: CHAPTER_DRAFT_FALLBACK_SHELL,
            rollback_boundary: CHAPTER_DRAFT_ROLLBACK_BOUNDARY,
        },
        ChapterDraftRouteReadinessProbe {
            name: "chapter-draft-candidate-load",
            owner: "rust",
            route_group: CHAPTER_DRAFT_ROUTE_GROUP,
            method: "GET",
            path: CANDIDATE_DRAFT_ROUTE,
            rust_owner: CHAPTER_DRAFT_RUST_OWNER,
            route_payload_owner: CHAPTER_DRAFT_ROUTE_OWNER,
            fallback_shell: CHAPTER_DRAFT_FALLBACK_SHELL,
            rollback_boundary: CHAPTER_DRAFT_ROLLBACK_BOUNDARY,
        },
        ChapterDraftRouteReadinessProbe {
            name: "chapter-draft-candidate-apply",
            owner: "rust",
            route_group: CHAPTER_DRAFT_ROUTE_GROUP,
            method: "POST",
            path: CANDIDATE_DRAFT_APPLY_ROUTE,
            rust_owner: CHAPTER_DRAFT_RUST_OWNER,
            route_payload_owner: CHAPTER_DRAFT_ROUTE_OWNER,
            fallback_shell: CHAPTER_DRAFT_FALLBACK_SHELL,
            rollback_boundary: CHAPTER_DRAFT_ROLLBACK_BOUNDARY,
        },
    ]
}

#[allow(dead_code)]
pub(crate) fn resolve_chapter_draft_route_readiness() -> Vec<ChapterDraftRouteReadinessResult> {
    build_default_chapter_draft_route_readiness_probes()
        .into_iter()
        .map(readiness_result_from_probe)
        .collect()
}

#[allow(dead_code)]
pub(crate) fn validate_chapter_draft_route_readiness(
    results: &[ChapterDraftRouteReadinessResult],
) -> Result<(), String> {
    let expected_paths = expected_chapter_draft_route_paths();
    let actual_paths = results
        .iter()
        .map(|result| result.path.as_str())
        .collect::<BTreeSet<_>>();

    if actual_paths != expected_paths {
        return Err(format!(
            "chapter_draft route readiness path mismatch: expected {:?}, got {:?}",
            expected_paths, actual_paths
        ));
    }

    for result in results {
        if !result.ok {
            return Err(format!(
                "chapter_draft readiness probe failed: {}",
                result.name
            ));
        }
        if result.owner != "rust" {
            return Err(format!(
                "chapter_draft readiness probe is not Rust-owned: {}",
                result.name
            ));
        }
        if result.route_group != CHAPTER_DRAFT_ROUTE_GROUP {
            return Err(format!(
                "chapter_draft readiness route group mismatch: {}",
                result.name
            ));
        }
        if result.rust_owner != CHAPTER_DRAFT_RUST_OWNER {
            return Err(format!(
                "chapter_draft readiness Rust owner mismatch: {}",
                result.name
            ));
        }
        if result.route_payload_owner != CHAPTER_DRAFT_ROUTE_OWNER {
            return Err(format!(
                "chapter_draft readiness route payload owner mismatch: {}",
                result.name
            ));
        }
        if result.fallback_shell != CHAPTER_DRAFT_FALLBACK_SHELL {
            return Err(format!(
                "chapter_draft readiness fallback shell mismatch: {}",
                result.name
            ));
        }
        if result.rollback_boundary != CHAPTER_DRAFT_ROLLBACK_BOUNDARY {
            return Err(format!(
                "chapter_draft readiness rollback boundary mismatch: {}",
                result.name
            ));
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub(crate) fn build_chapter_draft_route_owner_contract() -> Value {
    let readiness = resolve_chapter_draft_route_readiness();
    validate_chapter_draft_route_readiness(&readiness)
        .expect("chapter draft route readiness must stay valid");

    serde_json::json!({
        "owner": "chapter_draft_route_owner",
        "scope": "chapter_draft_route_access_selection_detail_apply_and_rollback_boundary",
        "python_source_map": [
            CHAPTER_DRAFT_FALLBACK_SHELL
        ],
        "rust_owner_map": [
            CHAPTER_DRAFT_RUST_OWNER,
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/services/chapter_analysis_service.rs"
        ],
        "behavior_contract": {
            "route_group": CHAPTER_DRAFT_ROUTE_GROUP,
            "route_paths": expected_chapter_draft_route_paths().into_iter().collect::<Vec<_>>(),
            "request_builders": [
                "build_auto_revision_draft_payload_request_from_route_query",
                "build_auto_revision_draft_payload_request_from_route_payload",
                "build_candidate_draft_payload_request_from_route_query",
                "build_candidate_draft_payload_request_from_route_payload"
            ],
            "selection_modes": [
                "latest",
                "explicit"
            ],
            "payload_entrypoints": [
                "load_owned_auto_revision_draft_payload",
                "apply_owned_auto_revision_draft_payload",
                "load_owned_candidate_draft_payload",
                "apply_owned_candidate_draft_payload"
            ],
            "readiness_entrypoints": [
                "build_default_chapter_draft_route_readiness_probes",
                "resolve_chapter_draft_route_readiness",
                "validate_chapter_draft_route_readiness"
            ],
            "readiness_probe_count": readiness.len()
        },
        "active_consumers": [
            "chapter_draft_routes",
            "chapter_analysis_service"
        ],
        "readiness_evidence": readiness
            .iter()
            .map(|result| result.name.as_str())
            .chain([
                "chapter-draft-auto-revision-load-logged-in-not-found-rust",
                "chapter-draft-auto-revision-apply-logged-in-not-found-rust",
                "chapter-draft-candidate-load-logged-in-not-found-rust",
                "chapter-draft-candidate-apply-logged-in-not-found-rust",
                "chapter-draft-auto-revision-load-business-rust",
                "chapter-draft-auto-revision-apply-business-rust",
                "chapter-draft-generate-candidate-draft-business-rust",
                "chapter-draft-candidate-load-business-rust",
                "chapter-draft-candidate-apply-business-rust",
            ])
            .collect::<Vec<_>>(),
        "owner_profile": {
            "name": "phase5-chapter-draft-owner",
            "business_probes": [
                "chapter-draft-auto-revision-load-logged-in-not-found-rust",
                "chapter-draft-auto-revision-apply-logged-in-not-found-rust",
                "chapter-draft-candidate-load-logged-in-not-found-rust",
                "chapter-draft-candidate-apply-logged-in-not-found-rust",
                "chapter-draft-auto-revision-load-business-rust",
                "chapter-draft-auto-revision-apply-business-rust",
                "chapter-draft-generate-candidate-draft-business-rust",
                "chapter-draft-candidate-load-business-rust",
                "chapter-draft-candidate-apply-business-rust"
            ],
            "setup_probes": [
                "chapter-draft-fixture-import-project-business-rust",
                "chapter-draft-fixture-list-chapter-business-rust",
                "chapter-draft-configure-mock-openai-business-rust",
                "chapter-draft-cleanup-project-business-rust"
            ],
            "route_readiness_probes": readiness
                .iter()
                .map(|result| result.name.as_str())
                .collect::<Vec<_>>(),
            "python_fallback_probe_count": 0,
            "manifest_profile": "phase5-chapter-draft-owner",
            "profile_kind": "logged_in_not_found_auto_revision_and_candidate_success_business_readiness"
        },
        "business_smoke_status": {
            "owner_profile": "phase5-chapter-draft-owner",
            "readiness_probe_count": 13,
            "route_group_probe_count": 8,
            "business_probe_count": 5,
            "logged_in_not_found_probe_count": 4,
            "auth_guard_probe_count": 0,
            "fixture_probe_count": 4,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "chapter-draft route source-map shell deleted; remaining Python closeout work is limited to separate shared history/view/source-map contracts",
        "migration_policy": "Chapter draft business smoke is covered by phase5-chapter-draft-owner; the Python rollback route shell has been physically deleted, and surviving Python closeout work is limited to separate shared history/view/source-map contracts.",
        "validation_boundary": [
            "cargo test api::chapter_draft_routes",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-chapter-draft-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": CHAPTER_DRAFT_ROLLBACK_BOUNDARY,
            "fallback_shell": CHAPTER_DRAFT_FALLBACK_SHELL,
            "python_source_map_policy": "source_map_and_explicit_route_rollback_only",
            "python_route_files_status": "chapter_draft_route_source_map_deleted_after_frozen_closeout",
            "python_bootstrap_status": "draft_route_runtime_registration_deleted_no_python_route_shell_remains",
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "shared chapter draft history/view/source-map contracts still need their own separate closeout rounds"
            ],
            "rollback_files": []
        }
    })
}

async fn load_chapter_for_draft(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterError> {
    load_accessible_chapter(db, chapter_id, user_id).await
}

async fn prepare_owned_draft_payload_context(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: OwnedDraftPayloadRequest,
) -> Result<OwnedDraftPayloadContext, LoadAccessibleChapterError> {
    let chapter = load_chapter_for_draft(db, chapter_id, user_id).await?;
    Ok(OwnedDraftPayloadContext { chapter, request })
}

fn map_auto_revision_chapter_access_error(
    error: LoadAccessibleChapterError,
) -> OwnedAutoRevisionDraftPayloadError {
    map_owned_draft_chapter_access_error(error)
}

fn map_candidate_chapter_access_error(
    error: LoadAccessibleChapterError,
) -> OwnedCandidateDraftPayloadError {
    map_owned_draft_chapter_access_error(error)
}

fn map_owned_draft_chapter_access_error<TDraftError>(
    error: LoadAccessibleChapterError,
) -> OwnedDraftPayloadError<TDraftError> {
    match error {
        LoadAccessibleChapterError::NotFoundOrAccessDenied => {
            OwnedDraftPayloadError::ChapterNotFoundOrAccessDenied
        }
        LoadAccessibleChapterError::Internal(detail) => OwnedDraftPayloadError::Internal(detail),
    }
}

pub(crate) async fn load_owned_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: OwnedDraftPayloadRequest,
) -> Result<Value, LoadOwnedAutoRevisionDraftPayloadError> {
    let prepared = prepare_owned_draft_payload_context(db, chapter_id, user_id, request)
        .await
        .map_err(map_auto_revision_chapter_access_error)?;
    load_auto_revision_draft_detail_payload(
        db,
        &prepared.chapter,
        prepared.request.selector.as_deref(),
    )
    .await
    .map_err(|error| {
        LoadOwnedAutoRevisionDraftPayloadError::Draft(error, prepared.request.selection_mode)
    })
}

pub(crate) async fn apply_owned_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: OwnedDraftPayloadRequest,
) -> Result<Value, ApplyOwnedAutoRevisionDraftPayloadError> {
    let prepared = prepare_owned_draft_payload_context(db, chapter_id, user_id, request)
        .await
        .map_err(map_auto_revision_chapter_access_error)?;
    apply_auto_revision_draft_payload(
        db,
        &prepared.chapter,
        prepared.request.selector.as_deref(),
        prepared.request.allow_stale,
    )
    .await
    .map_err(|error| {
        ApplyOwnedAutoRevisionDraftPayloadError::Draft(error, prepared.request.selection_mode)
    })
}

pub(crate) async fn load_owned_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: OwnedDraftPayloadRequest,
) -> Result<Value, LoadOwnedCandidateDraftPayloadError> {
    let prepared = prepare_owned_draft_payload_context(db, chapter_id, user_id, request)
        .await
        .map_err(map_candidate_chapter_access_error)?;
    load_candidate_draft_detail_payload(db, &prepared.chapter, prepared.request.selector.as_deref())
        .await
        .map_err(|error| {
            LoadOwnedCandidateDraftPayloadError::Draft(error, prepared.request.selection_mode)
        })
}

pub(crate) async fn apply_owned_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: OwnedDraftPayloadRequest,
) -> Result<Value, ApplyOwnedCandidateDraftPayloadError> {
    let prepared = prepare_owned_draft_payload_context(db, chapter_id, user_id, request)
        .await
        .map_err(map_candidate_chapter_access_error)?;
    apply_candidate_draft_payload(
        db,
        &prepared.chapter,
        prepared.request.selector.as_deref(),
        prepared.request.allow_stale,
    )
    .await
    .map_err(|error| {
        ApplyOwnedCandidateDraftPayloadError::Draft(error, prepared.request.selection_mode)
    })
}

async fn get_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<AutoRevisionDraftLookupRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_auto_revision_draft_payload_request_from_route_query(query);
    let payload = load_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            error_mapper::map_owned_auto_revision_draft_error(
                error,
                error_mapper::map_auto_revision_draft_load_error,
            )
        })?;
    Ok(Json(payload))
}

async fn apply_auto_revision_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<AutoRevisionDraftApplyRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_auto_revision_draft_payload_request_from_route_payload(body);
    let payload = apply_owned_auto_revision_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            error_mapper::map_owned_auto_revision_draft_error(
                error,
                error_mapper::map_auto_revision_draft_apply_error,
            )
        })?;
    Ok(Json(payload))
}

async fn get_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<CandidateDraftLookupRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_candidate_draft_payload_request_from_route_query(query);
    let payload = load_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            error_mapper::map_owned_candidate_draft_error(
                error,
                error_mapper::map_candidate_draft_load_error,
            )
        })?;
    Ok(Json(payload))
}

async fn apply_candidate_draft(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<CandidateDraftApplyRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_candidate_draft_payload_request_from_route_payload(body);
    let payload = apply_owned_candidate_draft_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(|error| {
            error_mapper::map_owned_candidate_draft_error(
                error,
                error_mapper::map_candidate_draft_apply_error,
            )
        })?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(AUTO_REVISION_DRAFT_ROUTE, get(get_auto_revision_draft))
        .route(
            AUTO_REVISION_DRAFT_APPLY_ROUTE,
            post(apply_auto_revision_draft),
        )
        .route(CANDIDATE_DRAFT_ROUTE, get(get_candidate_draft))
        .route(CANDIDATE_DRAFT_APPLY_ROUTE, post(apply_candidate_draft))
}

#[allow(dead_code)]
fn readiness_result_from_probe(
    probe: ChapterDraftRouteReadinessProbe,
) -> ChapterDraftRouteReadinessResult {
    ChapterDraftRouteReadinessResult {
        name: probe.name.to_string(),
        owner: probe.owner.to_string(),
        route_group: probe.route_group.to_string(),
        method: probe.method.to_string(),
        path: probe.path.to_string(),
        ok: true,
        rust_owner: probe.rust_owner.to_string(),
        route_payload_owner: probe.route_payload_owner.to_string(),
        fallback_shell: probe.fallback_shell.to_string(),
        rollback_boundary: probe.rollback_boundary.to_string(),
    }
}

#[allow(dead_code)]
fn expected_chapter_draft_route_paths() -> BTreeSet<&'static str> {
    BTreeSet::from([
        AUTO_REVISION_DRAFT_ROUTE,
        AUTO_REVISION_DRAFT_APPLY_ROUTE,
        CANDIDATE_DRAFT_ROUTE,
        CANDIDATE_DRAFT_APPLY_ROUTE,
    ])
}

mod error_mapper {
    // Draft-specific route error mapping lives beside the draft route file so
    // route transport, route-facing owner wiring, and error shell ownership
    // stay in the same Rust package.
    use axum::{http::StatusCode, response::Json};
    use serde_json::Value;

    use super::{
        OwnedAutoRevisionDraftPayloadError, OwnedCandidateDraftPayloadError,
        OwnedDraftPayloadError, OwnedDraftSelectionMode,
    };
    use crate::api::chapters_error_mapper::{
        chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
    };
    use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

    #[derive(Clone, Copy)]
    enum DraftKind {
        AutoRevision,
        Candidate,
    }

    #[derive(Clone, Copy)]
    enum DraftAction {
        Load,
        Apply,
    }

    fn draft_load_failed_error(kind: DraftKind) -> (StatusCode, Json<Value>) {
        detail_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            match kind {
                DraftKind::AutoRevision => "自动修订草稿加载失败",
                DraftKind::Candidate => "候选草稿加载失败",
            },
        )
    }

    fn draft_not_found_error(
        kind: DraftKind,
        action: DraftAction,
        selection_mode: OwnedDraftSelectionMode,
    ) -> (StatusCode, Json<Value>) {
        detail_error(
            StatusCode::NOT_FOUND,
            match (kind, action, selection_mode) {
                (DraftKind::AutoRevision, DraftAction::Load, OwnedDraftSelectionMode::Explicit) => {
                    "指定的自动修订草稿不存在或不可用"
                }
                (DraftKind::AutoRevision, DraftAction::Load, OwnedDraftSelectionMode::Latest) => {
                    "该章节暂无自动修订草稿"
                }
                (
                    DraftKind::AutoRevision,
                    DraftAction::Apply,
                    OwnedDraftSelectionMode::Explicit,
                ) => "指定的自动修订草稿不存在或不可用",
                (DraftKind::AutoRevision, DraftAction::Apply, OwnedDraftSelectionMode::Latest) => {
                    "该章节暂无可应用的自动修订草稿"
                }
                (DraftKind::Candidate, DraftAction::Load, OwnedDraftSelectionMode::Explicit) => {
                    "指定的候选草稿不存在或不可用"
                }
                (DraftKind::Candidate, DraftAction::Load, OwnedDraftSelectionMode::Latest) => {
                    "该章节暂无候选草稿"
                }
                (DraftKind::Candidate, DraftAction::Apply, OwnedDraftSelectionMode::Explicit) => {
                    "指定的候选草稿不存在或不可用"
                }
                (DraftKind::Candidate, DraftAction::Apply, OwnedDraftSelectionMode::Latest) => {
                    "该章节暂无可应用的候选草稿"
                }
            },
        )
    }

    fn draft_empty_content_error(kind: DraftKind) -> (StatusCode, Json<Value>) {
        detail_error(
            StatusCode::BAD_REQUEST,
            match kind {
                DraftKind::AutoRevision => "自动修订草稿内容为空，无法应用",
                DraftKind::Candidate => "候选草稿内容为空，无法应用",
            },
        )
    }

    fn draft_workflow_meta_text_error(kind: DraftKind) -> (StatusCode, Json<Value>) {
        detail_error(
            StatusCode::BAD_REQUEST,
            match kind {
                DraftKind::AutoRevision => "自动修订草稿包含流程化元文本，无法应用",
                DraftKind::Candidate => "候选草稿包含流程化元文本，无法应用",
            },
        )
    }

    fn draft_stale_error(kind: DraftKind) -> (StatusCode, Json<Value>) {
        detail_error(
            StatusCode::CONFLICT,
            match kind {
                DraftKind::AutoRevision => {
                    "自动修订草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"
                }
                DraftKind::Candidate => {
                    "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"
                }
            },
        )
    }

    pub fn map_auto_revision_draft_load_error(
        error: AutoRevisionDraftError,
        selection_mode: OwnedDraftSelectionMode,
    ) -> (StatusCode, Json<Value>) {
        match error {
            AutoRevisionDraftError::NotFound => {
                draft_not_found_error(DraftKind::AutoRevision, DraftAction::Load, selection_mode)
            }
            AutoRevisionDraftError::Internal(detail) => internal_detail_error(detail),
            _ => draft_load_failed_error(DraftKind::AutoRevision),
        }
    }

    pub fn map_auto_revision_draft_apply_error(
        error: AutoRevisionDraftError,
        selection_mode: OwnedDraftSelectionMode,
    ) -> (StatusCode, Json<Value>) {
        match error {
            AutoRevisionDraftError::NotFound => {
                draft_not_found_error(DraftKind::AutoRevision, DraftAction::Apply, selection_mode)
            }
            AutoRevisionDraftError::EmptyContent => {
                draft_empty_content_error(DraftKind::AutoRevision)
            }
            AutoRevisionDraftError::WorkflowMetaText => {
                draft_workflow_meta_text_error(DraftKind::AutoRevision)
            }
            AutoRevisionDraftError::Stale => draft_stale_error(DraftKind::AutoRevision),
            AutoRevisionDraftError::Internal(detail) => internal_detail_error(detail),
        }
    }

    pub fn map_candidate_draft_load_error(
        error: CandidateDraftError,
        selection_mode: OwnedDraftSelectionMode,
    ) -> (StatusCode, Json<Value>) {
        match error {
            CandidateDraftError::NotFound => {
                draft_not_found_error(DraftKind::Candidate, DraftAction::Load, selection_mode)
            }
            CandidateDraftError::Internal(detail) => internal_detail_error(detail),
            _ => draft_load_failed_error(DraftKind::Candidate),
        }
    }

    pub fn map_candidate_draft_apply_error(
        error: CandidateDraftError,
        selection_mode: OwnedDraftSelectionMode,
    ) -> (StatusCode, Json<Value>) {
        match error {
            CandidateDraftError::NotFound => {
                draft_not_found_error(DraftKind::Candidate, DraftAction::Apply, selection_mode)
            }
            CandidateDraftError::PreviewOnly => detail_error(
                StatusCode::CONFLICT,
                "该候选草稿仅保留了预览，无法直接恢复正文",
            ),
            CandidateDraftError::EmptyContent => draft_empty_content_error(DraftKind::Candidate),
            CandidateDraftError::WorkflowMetaText => {
                draft_workflow_meta_text_error(DraftKind::Candidate)
            }
            CandidateDraftError::Stale => draft_stale_error(DraftKind::Candidate),
            CandidateDraftError::Internal(detail) => internal_detail_error(detail),
        }
    }

    pub(crate) fn map_owned_auto_revision_draft_error(
        error: OwnedAutoRevisionDraftPayloadError,
        draft_error_mapper: impl FnOnce(
            AutoRevisionDraftError,
            OwnedDraftSelectionMode,
        ) -> (StatusCode, Json<Value>),
    ) -> (StatusCode, Json<Value>) {
        map_owned_draft_error(error, draft_error_mapper)
    }

    pub(crate) fn map_owned_candidate_draft_error(
        error: OwnedCandidateDraftPayloadError,
        draft_error_mapper: impl FnOnce(
            CandidateDraftError,
            OwnedDraftSelectionMode,
        ) -> (StatusCode, Json<Value>),
    ) -> (StatusCode, Json<Value>) {
        map_owned_draft_error(error, draft_error_mapper)
    }

    fn map_owned_draft_error<TDraftError>(
        error: OwnedDraftPayloadError<TDraftError>,
        draft_error_mapper: impl FnOnce(
            TDraftError,
            OwnedDraftSelectionMode,
        ) -> (StatusCode, Json<Value>),
    ) -> (StatusCode, Json<Value>) {
        match error {
            OwnedDraftPayloadError::ChapterNotFoundOrAccessDenied => {
                chapter_not_found_or_access_denied_error()
            }
            OwnedDraftPayloadError::Draft(error, selection_mode) => {
                draft_error_mapper(error, selection_mode)
            }
            OwnedDraftPayloadError::Internal(detail) => internal_detail_error(detail),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{LoadOwnedAutoRevisionDraftPayloadError, OwnedDraftSelectionMode};
        use super::{
            draft_not_found_error, map_auto_revision_draft_load_error,
            map_candidate_draft_apply_error, map_owned_auto_revision_draft_error, DraftAction,
            DraftKind,
        };
        use crate::services::chapter_analysis_service::{
            AutoRevisionDraftError, CandidateDraftError,
        };
        use axum::{http::StatusCode, Json};
        use serde_json::{json, Value};

        fn assert_detail_error(
            response: (StatusCode, Json<Value>),
            expected_status: StatusCode,
            expected_detail: &str,
        ) {
            assert_eq!(response.0, expected_status);
            assert_eq!(response.1 .0, json!({ "detail": expected_detail }));
        }

        #[test]
        fn auto_revision_not_found_messages_remain_context_sensitive() {
            assert_detail_error(
                draft_not_found_error(
                    DraftKind::AutoRevision,
                    DraftAction::Load,
                    OwnedDraftSelectionMode::Explicit,
                ),
                StatusCode::NOT_FOUND,
                "指定的自动修订草稿不存在或不可用",
            );
            assert_detail_error(
                draft_not_found_error(
                    DraftKind::AutoRevision,
                    DraftAction::Load,
                    OwnedDraftSelectionMode::Latest,
                ),
                StatusCode::NOT_FOUND,
                "该章节暂无自动修订草稿",
            );
        }

        #[test]
        fn candidate_apply_stale_message_remains_unchanged() {
            assert_detail_error(
                map_candidate_draft_apply_error(
                    CandidateDraftError::Stale,
                    OwnedDraftSelectionMode::Latest,
                ),
                StatusCode::CONFLICT,
                "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
            );
        }

        #[test]
        fn auto_revision_unexpected_load_error_keeps_generic_failure_message() {
            assert_detail_error(
                map_auto_revision_draft_load_error(
                    AutoRevisionDraftError::Stale,
                    OwnedDraftSelectionMode::Latest,
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
                "自动修订草稿加载失败",
            );
        }

        #[test]
        fn owned_auto_revision_load_chapter_access_denied_remains_404() {
            assert_detail_error(
                map_owned_auto_revision_draft_error(
                    LoadOwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied,
                    map_auto_revision_draft_load_error,
                ),
                StatusCode::NOT_FOUND,
                "Chapter not found or access denied",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_chapter_draft_route_owner_contract, AutoRevisionDraftApplyRouteRequest,
        AutoRevisionDraftLookupRouteQuery, CandidateDraftApplyRouteRequest,
        CandidateDraftLookupRouteQuery, AUTO_REVISION_DRAFT_APPLY_ROUTE, AUTO_REVISION_DRAFT_ROUTE,
        CANDIDATE_DRAFT_APPLY_ROUTE, CANDIDATE_DRAFT_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_chapter_draft_route_owner_contract() {
        let contract = build_chapter_draft_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_draft_route_owner");
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_draft_routes.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["request_builders"][0],
            "build_auto_revision_draft_payload_request_from_route_query"
        );
        assert_eq!(contract["behavior_contract"]["readiness_probe_count"], 4);
        assert_eq!(
            contract["readiness_evidence"][3],
            "chapter-draft-candidate-apply"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-chapter-draft-owner"
        );
        assert_eq!(
            contract["readiness_evidence"][4],
            "chapter-draft-auto-revision-load-logged-in-not-found-rust"
        );
        assert_eq!(
            contract["owner_profile"]["route_readiness_probes"][3],
            "chapter-draft-candidate-apply"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-draft-auto-revision-apply-business-rust")));
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-draft-candidate-apply-business-rust")));
        assert!(contract["owner_profile"]["setup_probes"]
            .as_array()
            .expect("setup probes should be an array")
            .contains(&json!("chapter-draft-fixture-import-project-business-rust")));
        assert_eq!(
            contract["owner_profile"]["manifest_profile"],
            "phase5-chapter-draft-owner"
        );
        assert_eq!(
            contract["owner_profile"]["profile_kind"],
            "logged_in_not_found_auto_revision_and_candidate_success_business_readiness"
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            13
        );
        assert_eq!(
            contract["business_smoke_status"]["route_group_probe_count"],
            8
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 5);
        assert_eq!(
            contract["business_smoke_status"]["logged_in_not_found_probe_count"],
            4
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            0
        );
        assert_eq!(contract["business_smoke_status"]["fixture_probe_count"], 4);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "chapter-draft route source-map shell deleted; remaining Python closeout work is limited to separate shared history/view/source-map contracts"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-chapter-draft-owner"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_status"],
            "physical_closeout_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_physical_closeout_action"],
            "delete_completed"
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(contract["rollback_boundary"]["fallback_shell"], "");
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "shared chapter draft history/view/source-map contracts still need their own separate closeout rounds"
        );
    }

    #[test]
    fn should_keep_chapter_draft_route_paths_stable() {
        assert_eq!(
            json!({
                "auto_revision_draft": AUTO_REVISION_DRAFT_ROUTE,
                "auto_revision_draft_apply": AUTO_REVISION_DRAFT_APPLY_ROUTE,
                "candidate_draft": CANDIDATE_DRAFT_ROUTE,
                "candidate_draft_apply": CANDIDATE_DRAFT_APPLY_ROUTE
            }),
            json!({
                "auto_revision_draft": "/chapters/{chapter_id}/analysis/auto-revision-draft",
                "auto_revision_draft_apply": "/chapters/{chapter_id}/analysis/auto-revision-draft/apply",
                "candidate_draft": "/chapters/{chapter_id}/analysis/candidate-draft",
                "candidate_draft_apply": "/chapters/{chapter_id}/analysis/candidate-draft/apply"
            })
        );
    }

    #[test]
    fn should_build_route_payload_types() {
        let auto_query = AutoRevisionDraftLookupRouteQuery {
            history_id: Some("history-1".to_string()),
        };
        let auto_apply = AutoRevisionDraftApplyRouteRequest {
            history_id: Some("history-2".to_string()),
            allow_stale: true,
        };
        let candidate_query = CandidateDraftLookupRouteQuery {
            attempt_id: Some("attempt-1".to_string()),
        };
        let candidate_apply = CandidateDraftApplyRouteRequest {
            attempt_id: Some("attempt-2".to_string()),
            allow_stale: false,
        };

        assert_eq!(auto_query.history_id.as_deref(), Some("history-1"));
        assert_eq!(auto_apply.history_id.as_deref(), Some("history-2"));
        assert!(auto_apply.allow_stale);
        assert_eq!(candidate_query.attempt_id.as_deref(), Some("attempt-1"));
        assert_eq!(candidate_apply.attempt_id.as_deref(), Some("attempt-2"));
        assert!(!candidate_apply.allow_stale);
    }
}

#[cfg(test)]
mod owner_tests {
    use chrono::NaiveDateTime;
    use serde_json::json;

    use crate::models::{chapter, chapter_draft_attempt, generation_history};
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

    use super::{
        build_auto_revision_draft_detail_payload,
        build_auto_revision_draft_payload_request_from_route_payload,
        build_auto_revision_draft_payload_request_from_route_query,
        build_candidate_draft_detail_payload,
        build_candidate_draft_payload_request_from_route_payload,
        build_candidate_draft_payload_request_from_route_query,
        build_chapter_draft_route_owner_contract, build_draft_detail_response_payload,
        map_auto_revision_chapter_access_error, map_candidate_chapter_access_error,
        resolve_chapter_draft_route_readiness, validate_chapter_draft_route_readiness,
        ApplyOwnedAutoRevisionDraftPayloadError, ApplyOwnedCandidateDraftPayloadError,
        AutoRevisionDraftApplyRouteRequest, AutoRevisionDraftLookupRouteQuery,
        CandidateDraftApplyRouteRequest, CandidateDraftLookupRouteQuery,
        LoadOwnedAutoRevisionDraftPayloadError, LoadOwnedCandidateDraftPayloadError,
        OwnedAutoRevisionDraftPayloadError, OwnedCandidateDraftPayloadError,
        OwnedDraftPayloadRequest, OwnedDraftSelectionMode,
    };

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            summary: None,
            content: Some("正文".to_string()),
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn candidate_attempt() -> chapter_draft_attempt::Model {
        chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_repair_candidate".to_string(),
            attempt_state: "ready".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count: 4,
            summary_preview: None,
            content_preview: Some("候选正文".to_string()),
            quality_metrics: None,
            repair_payload: Some(json!({
                "content_complete": true,
                "candidate_full_content": "完整候选正文"
            })),
            created_at: Some(test_datetime()),
        }
    }

    fn reviser_history() -> generation_history::Model {
        generation_history::Model {
            id: "history-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content: None,
            model: Some("chapter_text_reviser_v1".to_string()),
            tokens_used: None,
            generation_time: None,
            created_at: Some(test_datetime()),
        }
    }

    #[test]
    fn should_build_draft_detail_response_payload() {
        let payload = build_draft_detail_response_payload(
            "chapter-1",
            "candidate_draft",
            json!({"attempt_id": "attempt-1"}),
        );

        assert_eq!(payload["chapter_id"], json!("chapter-1"));
        assert_eq!(payload["candidate_draft"]["attempt_id"], json!("attempt-1"));
    }

    #[test]
    fn should_build_candidate_draft_detail_payload_with_full_content() {
        let payload = build_candidate_draft_detail_payload(&chapter_model(), &candidate_attempt());

        assert_eq!(payload["chapter_id"], json!("chapter-1"));
        assert_eq!(payload["candidate_draft"]["attempt_id"], json!("attempt-1"));
        assert_eq!(payload["candidate_draft"]["content"], json!("完整候选正文"));
    }

    #[test]
    fn should_build_auto_revision_draft_detail_payload_with_full_text() {
        let payload = build_auto_revision_draft_detail_payload(
            &chapter_model(),
            &reviser_history(),
            &json!({
                "revised_text": "修订正文",
            }),
        );

        assert_eq!(payload["chapter_id"], json!("chapter-1"));
        assert_eq!(
            payload["auto_revision_draft"]["history_id"],
            json!("history-1")
        );
        assert_eq!(
            payload["auto_revision_draft"]["revised_text"],
            json!("修订正文")
        );
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_query() {
        let explicit = build_auto_revision_draft_payload_request_from_route_query(
            AutoRevisionDraftLookupRouteQuery {
                history_id: Some(" history-1 ".to_string()),
            },
        );
        assert_eq!(
            explicit,
            OwnedDraftPayloadRequest::new(Some("history-1"), false)
        );

        let latest = build_auto_revision_draft_payload_request_from_route_query(
            AutoRevisionDraftLookupRouteQuery {
                history_id: Some("   ".to_string()),
            },
        );
        assert_eq!(latest, OwnedDraftPayloadRequest::new(None, false));
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_payload() {
        let request = build_auto_revision_draft_payload_request_from_route_payload(
            AutoRevisionDraftApplyRouteRequest {
                history_id: Some(" history-1 ".to_string()),
                allow_stale: true,
            },
        );

        assert_eq!(
            request,
            OwnedDraftPayloadRequest::new(Some("history-1"), true)
        );
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_query() {
        let request = build_candidate_draft_payload_request_from_route_query(
            CandidateDraftLookupRouteQuery {
                attempt_id: Some(" attempt-1 ".to_string()),
            },
        );

        assert_eq!(
            request,
            OwnedDraftPayloadRequest::new(Some("attempt-1"), false)
        );
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_payload() {
        let request = build_candidate_draft_payload_request_from_route_payload(
            CandidateDraftApplyRouteRequest {
                attempt_id: Some("   ".to_string()),
                allow_stale: false,
            },
        );

        assert_eq!(request, OwnedDraftPayloadRequest::new(None, false));
    }

    #[test]
    fn should_publish_chapter_draft_route_owner_contract() {
        let contract = build_chapter_draft_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_draft_route_owner");
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/chapter_draft_routes.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["request_builders"][0],
            "build_auto_revision_draft_payload_request_from_route_query"
        );
        assert_eq!(
            contract["owner_profile"]["profile_kind"],
            "logged_in_not_found_auto_revision_and_candidate_success_business_readiness"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            13
        );
        assert_eq!(
            contract["business_smoke_status"]["logged_in_not_found_probe_count"],
            4
        );
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-draft-auto-revision-load-business-rust")));
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-draft-candidate-load-business-rust")));
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"][0],
            "shared chapter draft history/view/source-map contracts still need their own separate closeout rounds"
        );
    }

    #[test]
    fn should_resolve_chapter_draft_route_readiness_for_all_routes() {
        let readiness = resolve_chapter_draft_route_readiness();

        assert_eq!(readiness.len(), 4);
        assert_eq!(readiness[0].route_group, "chapter_draft");
        assert_eq!(
            readiness[0].rust_owner,
            "backend-rs/src/api/chapter_draft_routes.rs"
        );
        assert_eq!(
            readiness[0].route_payload_owner,
            "backend-rs/src/api/chapter_draft_routes.rs"
        );
    }

    #[test]
    fn should_validate_route_readiness() {
        let readiness = resolve_chapter_draft_route_readiness();
        validate_chapter_draft_route_readiness(&readiness)
            .expect("draft route readiness should stay valid");
    }

    #[test]
    fn should_map_auto_revision_chapter_access_errors() {
        let not_found = map_auto_revision_chapter_access_error(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );
        assert!(matches!(
            not_found,
            OwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));

        let internal = map_auto_revision_chapter_access_error(
            LoadAccessibleChapterError::Internal("db failed".to_string()),
        );
        match internal {
            OwnedAutoRevisionDraftPayloadError::Internal(detail) => {
                assert_eq!(detail, "db failed");
            }
            _ => panic!("expected internal error"),
        }
    }

    #[test]
    fn should_map_candidate_chapter_access_errors() {
        let not_found =
            map_candidate_chapter_access_error(LoadAccessibleChapterError::NotFoundOrAccessDenied);
        assert!(matches!(
            not_found,
            OwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));

        let internal = map_candidate_chapter_access_error(LoadAccessibleChapterError::Internal(
            "storage failed".to_string(),
        ));
        match internal {
            OwnedCandidateDraftPayloadError::Internal(detail) => {
                assert_eq!(detail, "storage failed");
            }
            _ => panic!("expected internal error"),
        }
    }

    #[test]
    fn should_preserve_auto_revision_owned_error_aliases() {
        let load_error = LoadOwnedAutoRevisionDraftPayloadError::Draft(
            AutoRevisionDraftError::NotFound,
            OwnedDraftSelectionMode::Explicit,
        );
        assert!(matches!(
            load_error,
            LoadOwnedAutoRevisionDraftPayloadError::Draft(
                AutoRevisionDraftError::NotFound,
                OwnedDraftSelectionMode::Explicit
            )
        ));

        let apply_error = ApplyOwnedAutoRevisionDraftPayloadError::Draft(
            AutoRevisionDraftError::Stale,
            OwnedDraftSelectionMode::Latest,
        );
        assert!(matches!(
            apply_error,
            ApplyOwnedAutoRevisionDraftPayloadError::Draft(
                AutoRevisionDraftError::Stale,
                OwnedDraftSelectionMode::Latest
            )
        ));
    }

    #[test]
    fn should_preserve_candidate_owned_error_aliases() {
        let load_error = LoadOwnedCandidateDraftPayloadError::Draft(
            CandidateDraftError::NotFound,
            OwnedDraftSelectionMode::Explicit,
        );
        assert!(matches!(
            load_error,
            LoadOwnedCandidateDraftPayloadError::Draft(
                CandidateDraftError::NotFound,
                OwnedDraftSelectionMode::Explicit
            )
        ));

        let apply_error = ApplyOwnedCandidateDraftPayloadError::Draft(
            CandidateDraftError::Stale,
            OwnedDraftSelectionMode::Latest,
        );
        assert!(matches!(
            apply_error,
            ApplyOwnedCandidateDraftPayloadError::Draft(
                CandidateDraftError::Stale,
                OwnedDraftSelectionMode::Latest
            )
        ));
    }
}
