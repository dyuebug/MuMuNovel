use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::models::chapter;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
use crate::services::chapter_draft_apply_service::{
    apply_auto_revision_draft_payload, apply_candidate_draft_payload,
};
use crate::services::chapter_draft_source_service::{
    load_candidate_draft_attempt, load_latest_reviser_history,
};
use crate::services::chapter_draft_view_payload_service::{
    build_auto_revision_draft_payload, build_candidate_draft_payload,
};
use serde_json::Value;

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

struct OwnedDraftPayloadContext {
    chapter: chapter::Model,
    request: OwnedDraftPayloadRequest,
}

pub enum OwnedDraftPayloadError<TDraftError> {
    ChapterNotFoundOrAccessDenied,
    Draft(TDraftError, OwnedDraftSelectionMode),
    Internal(String),
}

pub type OwnedAutoRevisionDraftPayloadError = OwnedDraftPayloadError<AutoRevisionDraftError>;
pub type OwnedCandidateDraftPayloadError = OwnedDraftPayloadError<CandidateDraftError>;

pub type LoadOwnedCandidateDraftPayloadError = OwnedCandidateDraftPayloadError;
pub type ApplyOwnedCandidateDraftPayloadError = OwnedCandidateDraftPayloadError;
pub type LoadOwnedAutoRevisionDraftPayloadError = OwnedAutoRevisionDraftPayloadError;
pub type ApplyOwnedAutoRevisionDraftPayloadError = OwnedAutoRevisionDraftPayloadError;

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

async fn load_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    attempt_id: Option<&str>,
) -> Result<Value, CandidateDraftError> {
    let draft_attempt = load_candidate_draft_attempt(db, &chapter.id, attempt_id)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let draft_attempt = draft_attempt.ok_or(CandidateDraftError::NotFound)?;
    Ok(serde_json::json!({
        "chapter_id": chapter.id,
        "candidate_draft": build_candidate_draft_payload(&draft_attempt, chapter.updated_at, true),
    }))
}

async fn load_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    history_id: Option<&str>,
) -> Result<Value, AutoRevisionDraftError> {
    let reviser_loaded = load_latest_reviser_history(db, &chapter.id, history_id)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let (reviser_history, reviser_result) =
        reviser_loaded.ok_or(AutoRevisionDraftError::NotFound)?;

    Ok(serde_json::json!({
        "chapter_id": chapter.id,
        "auto_revision_draft": build_auto_revision_draft_payload(
            &reviser_result,
            Some(&reviser_history.id),
            reviser_history.created_at,
            chapter.updated_at,
            true,
        ),
    }))
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
    load_auto_revision_draft_payload(db, &prepared.chapter, prepared.request.selector.as_deref())
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
    load_candidate_draft_payload(db, &prepared.chapter, prepared.request.selector.as_deref())
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

#[cfg(test)]
mod tests {
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

    use super::{
        build_auto_revision_draft_payload_request_from_route_payload,
        build_auto_revision_draft_payload_request_from_route_query,
        build_candidate_draft_payload_request_from_route_payload,
        build_candidate_draft_payload_request_from_route_query,
        map_auto_revision_chapter_access_error, map_candidate_chapter_access_error,
        ApplyOwnedAutoRevisionDraftPayloadError, ApplyOwnedCandidateDraftPayloadError,
        AutoRevisionDraftApplyRouteRequest, AutoRevisionDraftLookupRouteQuery,
        CandidateDraftApplyRouteRequest, CandidateDraftLookupRouteQuery,
        LoadOwnedAutoRevisionDraftPayloadError, LoadOwnedCandidateDraftPayloadError,
        OwnedAutoRevisionDraftPayloadError, OwnedCandidateDraftPayloadError,
        OwnedDraftPayloadRequest, OwnedDraftSelectionMode,
    };

    #[test]
    fn owned_draft_payload_request_tracks_selection_mode_and_allow_stale() {
        let explicit = OwnedDraftPayloadRequest::new(Some("history-1"), true);
        assert_eq!(explicit.selector.as_deref(), Some("history-1"));
        assert_eq!(explicit.selection_mode, OwnedDraftSelectionMode::Explicit);
        assert!(explicit.allow_stale);

        let latest = OwnedDraftPayloadRequest::new(None, false);
        assert_eq!(latest.selector, None);
        assert_eq!(latest.selection_mode, OwnedDraftSelectionMode::Latest);
        assert!(!latest.allow_stale);
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_query() {
        let explicit = build_auto_revision_draft_payload_request_from_route_query(
            AutoRevisionDraftLookupRouteQuery {
                history_id: Some(" history-1 ".to_string()),
            },
        );
        assert_eq!(explicit.selector.as_deref(), Some("history-1"));
        assert_eq!(explicit.selection_mode, OwnedDraftSelectionMode::Explicit);
        assert!(!explicit.allow_stale);

        let latest = build_auto_revision_draft_payload_request_from_route_query(
            AutoRevisionDraftLookupRouteQuery {
                history_id: Some("   ".to_string()),
            },
        );
        assert_eq!(latest.selector, None);
        assert_eq!(latest.selection_mode, OwnedDraftSelectionMode::Latest);
        assert!(!latest.allow_stale);
    }

    #[test]
    fn should_build_auto_revision_draft_payload_request_from_route_payload() {
        let request = build_auto_revision_draft_payload_request_from_route_payload(
            AutoRevisionDraftApplyRouteRequest {
                history_id: Some(" history-1 ".to_string()),
                allow_stale: true,
            },
        );

        assert_eq!(request.selector.as_deref(), Some("history-1"));
        assert_eq!(request.selection_mode, OwnedDraftSelectionMode::Explicit);
        assert!(request.allow_stale);
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_query() {
        let request = build_candidate_draft_payload_request_from_route_query(
            CandidateDraftLookupRouteQuery {
                attempt_id: Some(" attempt-1 ".to_string()),
            },
        );

        assert_eq!(request.selector.as_deref(), Some("attempt-1"));
        assert_eq!(request.selection_mode, OwnedDraftSelectionMode::Explicit);
        assert!(!request.allow_stale);
    }

    #[test]
    fn should_build_candidate_draft_payload_request_from_route_payload() {
        let request = build_candidate_draft_payload_request_from_route_payload(
            CandidateDraftApplyRouteRequest {
                attempt_id: Some("   ".to_string()),
                allow_stale: false,
            },
        );

        assert_eq!(request.selector, None);
        assert_eq!(request.selection_mode, OwnedDraftSelectionMode::Latest);
        assert!(!request.allow_stale);
    }

    #[test]
    fn auto_revision_load_and_apply_errors_share_owner() {
        let load_error: LoadOwnedAutoRevisionDraftPayloadError =
            OwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied;
        let apply_error: ApplyOwnedAutoRevisionDraftPayloadError =
            OwnedAutoRevisionDraftPayloadError::Draft(
                AutoRevisionDraftError::Stale,
                OwnedDraftSelectionMode::Latest,
            );

        assert!(matches!(
            load_error,
            OwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));
        assert!(matches!(
            apply_error,
            OwnedAutoRevisionDraftPayloadError::Draft(
                AutoRevisionDraftError::Stale,
                OwnedDraftSelectionMode::Latest
            )
        ));
    }

    #[test]
    fn candidate_load_and_apply_errors_share_owner() {
        let load_error: LoadOwnedCandidateDraftPayloadError =
            OwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied;
        let apply_error: ApplyOwnedCandidateDraftPayloadError =
            OwnedCandidateDraftPayloadError::Draft(
                CandidateDraftError::PreviewOnly,
                OwnedDraftSelectionMode::Explicit,
            );

        assert!(matches!(
            load_error,
            OwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));
        assert!(matches!(
            apply_error,
            OwnedCandidateDraftPayloadError::Draft(
                CandidateDraftError::PreviewOnly,
                OwnedDraftSelectionMode::Explicit
            )
        ));
    }

    #[test]
    fn auto_revision_owner_normalizes_chapter_access_errors() {
        assert!(matches!(
            map_auto_revision_chapter_access_error(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            ),
            OwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));
        assert!(matches!(
            map_auto_revision_chapter_access_error(LoadAccessibleChapterError::Internal(
                "db failed".to_string()
            )),
            OwnedAutoRevisionDraftPayloadError::Internal(detail) if detail == "db failed"
        ));
    }

    #[test]
    fn candidate_owner_normalizes_chapter_access_errors() {
        assert!(matches!(
            map_candidate_chapter_access_error(LoadAccessibleChapterError::NotFoundOrAccessDenied),
            OwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied
        ));
        assert!(matches!(
            map_candidate_chapter_access_error(LoadAccessibleChapterError::Internal(
                "db failed".to_string()
            )),
            OwnedCandidateDraftPayloadError::Internal(detail) if detail == "db failed"
        ));
    }
}
