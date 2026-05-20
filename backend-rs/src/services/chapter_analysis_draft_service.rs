use std::collections::HashMap;

use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_analysis_draft_request_service::{
    parse_auto_revision_draft_apply_request, parse_auto_revision_draft_lookup_request,
    parse_candidate_draft_apply_request, parse_candidate_draft_lookup_request,
};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
use crate::services::chapter_draft_apply_service::{
    apply_auto_revision_draft_payload, apply_candidate_draft_payload,
};
use crate::services::chapter_draft_query_service::{
    load_auto_revision_draft_payload, load_candidate_draft_payload,
};

pub enum LoadOwnedAutoRevisionDraftPayloadError {
    ChapterNotFoundOrAccessDenied,
    Draft(AutoRevisionDraftError, bool),
    Internal(String),
}

pub enum ApplyOwnedAutoRevisionDraftPayloadError {
    ChapterNotFoundOrAccessDenied,
    Draft(AutoRevisionDraftError, bool),
    Internal(String),
}

pub enum LoadOwnedCandidateDraftPayloadError {
    ChapterNotFoundOrAccessDenied,
    Draft(CandidateDraftError, bool),
    Internal(String),
}

pub enum ApplyOwnedCandidateDraftPayloadError {
    ChapterNotFoundOrAccessDenied,
    Draft(CandidateDraftError, bool),
    Internal(String),
}

async fn load_chapter_for_draft(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterError> {
    load_accessible_chapter(db, chapter_id, user_id).await
}

pub async fn load_owned_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    query: &HashMap<String, String>,
) -> Result<Value, LoadOwnedAutoRevisionDraftPayloadError> {
    let chapter = load_chapter_for_draft(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                LoadOwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                LoadOwnedAutoRevisionDraftPayloadError::Internal(detail)
            }
        })?;
    let request = parse_auto_revision_draft_lookup_request(query);
    let history_id = request.history_id();
    let history_id_provided = history_id.is_some();
    load_auto_revision_draft_payload(db, &chapter, history_id)
        .await
        .map_err(|error| LoadOwnedAutoRevisionDraftPayloadError::Draft(error, history_id_provided))
}

pub async fn apply_owned_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    body: &Value,
) -> Result<Value, ApplyOwnedAutoRevisionDraftPayloadError> {
    let chapter = load_chapter_for_draft(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                ApplyOwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                ApplyOwnedAutoRevisionDraftPayloadError::Internal(detail)
            }
        })?;
    let request = parse_auto_revision_draft_apply_request(body);
    let history_id = request.history_id();
    let history_id_provided = history_id.is_some();
    apply_auto_revision_draft_payload(db, &chapter, history_id, request.allow_stale)
        .await
        .map_err(|error| ApplyOwnedAutoRevisionDraftPayloadError::Draft(error, history_id_provided))
}

pub async fn load_owned_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    query: &HashMap<String, String>,
) -> Result<Value, LoadOwnedCandidateDraftPayloadError> {
    let chapter = load_chapter_for_draft(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                LoadOwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                LoadOwnedCandidateDraftPayloadError::Internal(detail)
            }
        })?;
    let request = parse_candidate_draft_lookup_request(query);
    let attempt_id = request.attempt_id();
    let attempt_id_provided = attempt_id.is_some();
    load_candidate_draft_payload(db, &chapter, attempt_id)
        .await
        .map_err(|error| LoadOwnedCandidateDraftPayloadError::Draft(error, attempt_id_provided))
}

pub async fn apply_owned_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    body: &Value,
) -> Result<Value, ApplyOwnedCandidateDraftPayloadError> {
    let chapter = load_chapter_for_draft(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                ApplyOwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                ApplyOwnedCandidateDraftPayloadError::Internal(detail)
            }
        })?;
    let request = parse_candidate_draft_apply_request(body);
    let attempt_id = request.attempt_id();
    let attempt_id_provided = attempt_id.is_some();
    apply_candidate_draft_payload(db, &chapter, attempt_id, request.allow_stale)
        .await
        .map_err(|error| ApplyOwnedCandidateDraftPayloadError::Draft(error, attempt_id_provided))
}
