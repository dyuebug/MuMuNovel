use crate::services::chapter_access_service::LoadAccessibleChapterError;

#[derive(Debug)]
pub enum CreateChapterAnalysisTaskError {
    ChapterEmpty,
    ProjectMissing,
    Internal(String),
}

pub enum ChapterAnalysisQueryContextError {
    Chapter(LoadAccessibleChapterError),
    Internal(String),
}

pub type LoadAnalysisTaskStatusError = ChapterAnalysisQueryContextError;

pub enum CandidateDraftError {
    NotFound,
    PreviewOnly,
    EmptyContent,
    WorkflowMetaText,
    Stale,
    Internal(String),
}

pub enum AutoRevisionDraftError {
    NotFound,
    EmptyContent,
    WorkflowMetaText,
    Stale,
    Internal(String),
}
