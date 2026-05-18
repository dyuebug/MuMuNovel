pub enum CreateChapterAnalysisTaskError {
    ChapterEmpty,
    ProjectMissing,
    Internal(String),
}

pub enum LoadAnalysisTaskStatusError {
    ChapterNotFound,
    Internal(String),
}

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
