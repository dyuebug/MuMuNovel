use axum::{
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};

use crate::services::chapter_analysis_service::{
    AutoRevisionDraftError, CandidateDraftError, LoadAnalysisTaskStatusError,
};

pub fn map_auto_revision_draft_load_error(
    error: AutoRevisionDraftError,
    history_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        AutoRevisionDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": if history_id_provided {
                "指定的自动修订草稿不存在或不可用"
            } else {
                "该章节暂无自动修订草稿"
            }})),
        ),
        AutoRevisionDraftError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": "自动修订草稿加载失败"})),
        ),
    }
}

pub fn map_auto_revision_draft_apply_error(
    error: AutoRevisionDraftError,
    history_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        AutoRevisionDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": if history_id_provided {
                "指定的自动修订草稿不存在或不可用"
            } else {
                "该章节暂无可应用的自动修订草稿"
            }})),
        ),
        AutoRevisionDraftError::EmptyContent => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "自动修订草稿内容为空，无法应用"})),
        ),
        AutoRevisionDraftError::WorkflowMetaText => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "自动修订草稿包含流程化元文本，无法应用"})),
        ),
        AutoRevisionDraftError::Stale => (
            StatusCode::CONFLICT,
            Json(json!({"detail": "自动修订草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"})),
        ),
        AutoRevisionDraftError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

pub fn map_candidate_draft_load_error(
    error: CandidateDraftError,
    attempt_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        CandidateDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": if attempt_id_provided {
                "指定的候选草稿不存在或不可用"
            } else {
                "该章节暂无候选草稿"
            }})),
        ),
        CandidateDraftError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": "候选草稿加载失败"})),
        ),
    }
}

pub fn map_candidate_draft_apply_error(
    error: CandidateDraftError,
    attempt_id_provided: bool,
) -> (StatusCode, Json<Value>) {
    match error {
        CandidateDraftError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": if attempt_id_provided {
                "指定的候选草稿不存在或不可用"
            } else {
                "该章节暂无可应用的候选草稿"
            }})),
        ),
        CandidateDraftError::PreviewOnly => (
            StatusCode::CONFLICT,
            Json(json!({"detail": "该候选草稿仅保留了预览，无法直接恢复正文"})),
        ),
        CandidateDraftError::EmptyContent => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "候选草稿内容为空，无法应用"})),
        ),
        CandidateDraftError::WorkflowMetaText => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "候选草稿包含流程化元文本，无法应用"})),
        ),
        CandidateDraftError::Stale => (
            StatusCode::CONFLICT,
            Json(json!({"detail": "候选草稿已过期，请获取最新草稿或在请求中设置 allow_stale=true"})),
        ),
        CandidateDraftError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

pub fn map_analysis_task_status_error(
    error: LoadAnalysisTaskStatusError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadAnalysisTaskStatusError::ChapterNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Chapter not found or access denied"})),
        ),
        LoadAnalysisTaskStatusError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}
