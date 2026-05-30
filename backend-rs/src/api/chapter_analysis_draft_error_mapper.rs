use axum::{http::StatusCode, response::Json};
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
};
use crate::services::chapter_analysis_draft_service::{
    OwnedAutoRevisionDraftPayloadError, OwnedCandidateDraftPayloadError, OwnedDraftPayloadError,
    OwnedDraftSelectionMode,
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
            (DraftKind::AutoRevision, DraftAction::Apply, OwnedDraftSelectionMode::Explicit) => {
                "指定的自动修订草稿不存在或不可用"
            }
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
            DraftKind::Candidate => "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
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
        AutoRevisionDraftError::EmptyContent => draft_empty_content_error(DraftKind::AutoRevision),
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
    draft_error_mapper: impl FnOnce(TDraftError, OwnedDraftSelectionMode) -> (StatusCode, Json<Value>),
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
    use super::{
        draft_not_found_error, map_auto_revision_draft_load_error, map_candidate_draft_apply_error,
        map_owned_auto_revision_draft_error, DraftAction, DraftKind,
    };
    use crate::services::chapter_analysis_draft_service::{
        LoadOwnedAutoRevisionDraftPayloadError, OwnedDraftSelectionMode,
    };
    use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
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
