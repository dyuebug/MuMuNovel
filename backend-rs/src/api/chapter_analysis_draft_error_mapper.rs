use axum::{http::StatusCode, response::Json};
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    chapter_not_found_or_access_denied_error, detail_error, internal_detail_error,
};
use crate::services::chapter_analysis_draft_service::{
    ApplyOwnedAutoRevisionDraftPayloadError, ApplyOwnedCandidateDraftPayloadError,
    LoadOwnedAutoRevisionDraftPayloadError, LoadOwnedCandidateDraftPayloadError,
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
    explicit_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    detail_error(
        StatusCode::NOT_FOUND,
        match (kind, action, explicit_id_provided) {
            (DraftKind::AutoRevision, DraftAction::Load, true) => {
                "指定的自动修订草稿不存在或不可用"
            }
            (DraftKind::AutoRevision, DraftAction::Load, false) => "该章节暂无自动修订草稿",
            (DraftKind::AutoRevision, DraftAction::Apply, true) => {
                "指定的自动修订草稿不存在或不可用"
            }
            (DraftKind::AutoRevision, DraftAction::Apply, false) => {
                "该章节暂无可应用的自动修订草稿"
            }
            (DraftKind::Candidate, DraftAction::Load, true) => "指定的候选草稿不存在或不可用",
            (DraftKind::Candidate, DraftAction::Load, false) => "该章节暂无候选草稿",
            (DraftKind::Candidate, DraftAction::Apply, true) => "指定的候选草稿不存在或不可用",
            (DraftKind::Candidate, DraftAction::Apply, false) => "该章节暂无可应用的候选草稿",
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
    history_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        AutoRevisionDraftError::NotFound => draft_not_found_error(
            DraftKind::AutoRevision,
            DraftAction::Load,
            history_id_provided,
        ),
        AutoRevisionDraftError::Internal(detail) => internal_detail_error(detail),
        _ => draft_load_failed_error(DraftKind::AutoRevision),
    }
}

pub fn map_auto_revision_draft_apply_error(
    error: AutoRevisionDraftError,
    history_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        AutoRevisionDraftError::NotFound => draft_not_found_error(
            DraftKind::AutoRevision,
            DraftAction::Apply,
            history_id_provided,
        ),
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
    attempt_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        CandidateDraftError::NotFound => {
            draft_not_found_error(DraftKind::Candidate, DraftAction::Load, attempt_id_provided)
        }
        CandidateDraftError::Internal(detail) => internal_detail_error(detail),
        _ => draft_load_failed_error(DraftKind::Candidate),
    }
}

pub fn map_candidate_draft_apply_error(
    error: CandidateDraftError,
    attempt_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        CandidateDraftError::NotFound => draft_not_found_error(
            DraftKind::Candidate,
            DraftAction::Apply,
            attempt_id_provided,
        ),
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

pub fn map_owned_auto_revision_draft_load_error(
    error: LoadOwnedAutoRevisionDraftPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadOwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadOwnedAutoRevisionDraftPayloadError::Draft(error, history_id_provided) => {
            map_auto_revision_draft_load_error(error, history_id_provided)
        }
        LoadOwnedAutoRevisionDraftPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_owned_auto_revision_draft_apply_error(
    error: ApplyOwnedAutoRevisionDraftPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        ApplyOwnedAutoRevisionDraftPayloadError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        ApplyOwnedAutoRevisionDraftPayloadError::Draft(error, history_id_provided) => {
            map_auto_revision_draft_apply_error(error, history_id_provided)
        }
        ApplyOwnedAutoRevisionDraftPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_owned_candidate_draft_load_error(
    error: LoadOwnedCandidateDraftPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadOwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        LoadOwnedCandidateDraftPayloadError::Draft(error, attempt_id_provided) => {
            map_candidate_draft_load_error(error, attempt_id_provided)
        }
        LoadOwnedCandidateDraftPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

pub fn map_owned_candidate_draft_apply_error(
    error: ApplyOwnedCandidateDraftPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        ApplyOwnedCandidateDraftPayloadError::ChapterNotFoundOrAccessDenied => {
            chapter_not_found_or_access_denied_error()
        }
        ApplyOwnedCandidateDraftPayloadError::Draft(error, attempt_id_provided) => {
            map_candidate_draft_apply_error(error, attempt_id_provided)
        }
        ApplyOwnedCandidateDraftPayloadError::Internal(detail) => internal_detail_error(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        draft_not_found_error, map_auto_revision_draft_load_error, map_candidate_draft_apply_error,
        DraftAction, DraftKind,
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
            draft_not_found_error(DraftKind::AutoRevision, DraftAction::Load, true),
            StatusCode::NOT_FOUND,
            "指定的自动修订草稿不存在或不可用",
        );
        assert_detail_error(
            draft_not_found_error(DraftKind::AutoRevision, DraftAction::Load, false),
            StatusCode::NOT_FOUND,
            "该章节暂无自动修订草稿",
        );
    }

    #[test]
    fn candidate_apply_stale_message_remains_unchanged() {
        assert_detail_error(
            map_candidate_draft_apply_error(CandidateDraftError::Stale, false),
            StatusCode::CONFLICT,
            "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true",
        );
    }

    #[test]
    fn auto_revision_unexpected_load_error_keeps_generic_failure_message() {
        assert_detail_error(
            map_auto_revision_draft_load_error(AutoRevisionDraftError::Stale, false),
            StatusCode::INTERNAL_SERVER_ERROR,
            "自动修订草稿加载失败",
        );
    }
}
