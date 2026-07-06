use chrono::{Duration, NaiveDateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::models::{analysis_task, chapter, plot_analysis, story_memory};
use crate::services::chapter_access_service::load_accessible_chapter;
use crate::services::chapter_analysis_service::{
    apply_analysis_task_state_by_id, load_chapter_analysis_read_context, AnalysisTaskStage,
    ChapterAnalysisQueryContextError, LoadAnalysisTaskStatusError,
};
use crate::services::chapter_draft_history_service::ChapterAnalysisCheckerFragments;
use crate::services::chapter_draft_view_payload_service::{
    build_chapter_draft_analysis_view_fragments, ChapterDraftAnalysisViewFragments,
};
use crate::services::chapter_quality_metrics_query_service::{
    build_chapter_analysis_quality_fragments, ChapterAnalysisQualityFragments,
};
use crate::services::chapter_service::ChapterService;

const BATCH_ANALYSIS_STATUS_CHAPTER_LIMIT: usize = 200;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct BatchAnalysisStatusRouteRequest {
    pub(crate) chapter_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BatchAnalysisStatusRequest {
    chapter_ids: Vec<String>,
}

impl BatchAnalysisStatusRequest {
    fn from_route_request(route_request: BatchAnalysisStatusRouteRequest) -> Self {
        let mut normalized = Vec::new();
        for raw_id in route_request
            .chapter_ids
            .into_iter()
            .take(BATCH_ANALYSIS_STATUS_CHAPTER_LIMIT)
        {
            let chapter_id = raw_id.trim().to_string();
            if !chapter_id.is_empty() && !normalized.contains(&chapter_id) {
                normalized.push(chapter_id);
            }
        }

        Self {
            chapter_ids: normalized,
        }
    }

    fn into_chapter_ids(self) -> Vec<String> {
        self.chapter_ids
    }
}

pub(crate) fn build_batch_analysis_status_request_from_route_payload(
    route_request: BatchAnalysisStatusRouteRequest,
) -> BatchAnalysisStatusRequest {
    BatchAnalysisStatusRequest::from_route_request(route_request)
}

fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn classify_analysis_error_code(error_message: Option<&str>) -> Option<&'static str> {
    let message = error_message?;
    if message.contains("正在重试(") {
        Some("retrying")
    } else if message.contains("JSON解析失败") || message.contains("AI返回格式异常") {
        Some("json_parse_failed")
    } else if message.contains("AI响应为空或过短") {
        Some("ai_empty")
    } else if message.contains("流式响应中断") || message.contains("流式生成出错") {
        Some("stream_interrupted")
    } else if message.contains("任务超时") || message.contains("启动超时") {
        Some("timeout")
    } else if message.contains("章节不存在或内容为空") {
        Some("chapter_empty")
    } else if message.contains("项目不存在") {
        Some("project_missing")
    } else {
        Some("unknown")
    }
}

fn empty_batch_analysis_task_status_payload() -> Value {
    json!({
        "project_id": "",
        "total": 0,
        "items": {},
    })
}

fn build_batch_analysis_task_status_payload(
    response_project_id: String,
    items: Map<String, Value>,
) -> Value {
    json!({
        "project_id": response_project_id,
        "total": items.len(),
        "items": items,
    })
}

fn resolve_analysis_task_auto_recovery_error(
    task: &analysis_task::Model,
    now: NaiveDateTime,
) -> Option<String> {
    let timeout_minutes = if task
        .error_message
        .as_deref()
        .map(|message| message.contains("重试"))
        .unwrap_or(false)
    {
        15
    } else {
        10
    };

    if task.status == "running" {
        if let Some(started_at) = task.started_at {
            if now - started_at > Duration::minutes(timeout_minutes) {
                return Some(format!(
                    "任务超时（超过{}分钟未完成，已自动恢复）",
                    timeout_minutes
                ));
            }
        }
    } else if task.status == "pending" {
        if let Some(created_at) = task.created_at {
            if now - created_at > Duration::minutes(3) {
                return Some("任务启动超时（超过3分钟未启动，已自动恢复）".to_string());
            }
        }
    }

    None
}

async fn recover_analysis_task_if_needed(
    db: &DatabaseConnection,
    task: &analysis_task::Model,
    now: NaiveDateTime,
) -> Result<(analysis_task::Model, bool), sea_orm::DbErr> {
    let Some(error_message) = resolve_analysis_task_auto_recovery_error(task, now) else {
        return Ok((task.clone(), false));
    };

    apply_analysis_task_state_by_id(
        db,
        &task.id,
        AnalysisTaskStage::AutoRecoveredAsFailed,
        Some(error_message),
        now,
    )
    .await
    .map(|updated| (updated.unwrap_or_else(|| task.clone()), true))
}

pub async fn latest_analysis_task(
    db: &DatabaseConnection,
    chapter_id: &str,
) -> Result<Option<analysis_task::Model>, sea_orm::DbErr> {
    analysis_task::Entity::find()
        .filter(analysis_task::Column::ChapterId.eq(chapter_id))
        .order_by_desc(analysis_task::Column::CreatedAt)
        .limit(1)
        .one(db)
        .await
}

async fn has_chapter_analysis_result(
    db: &DatabaseConnection,
    chapter_id: &str,
) -> Result<bool, sea_orm::DbErr> {
    let result = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
        .limit(1)
        .one(db)
        .await?;

    Ok(result.is_some())
}

fn missing_analysis_result_status_payload(chapter_id: &str) -> Value {
    json!({
        "has_task": false,
        "chapter_id": chapter_id,
        "status": "none",
        "progress": 0,
        "error_message": "分析任务已完成，但分析结果缺失，请重新分析",
        "error_code": "analysis_missing",
        "auto_recovered": false,
        "missing_result": true,
        "task_id": null,
        "created_at": null,
        "started_at": null,
        "completed_at": null,
    })
}

pub async fn analysis_task_status_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    task: Option<analysis_task::Model>,
) -> Result<Value, sea_orm::DbErr> {
    let Some(task) = task else {
        return Ok(json!({
            "has_task": false,
            "chapter_id": chapter_id,
            "status": "none",
            "progress": 0,
            "error_message": null,
            "auto_recovered": false,
            "task_id": null,
            "created_at": null,
            "started_at": null,
            "completed_at": null,
        }));
    };

    let now = Utc::now().naive_utc();
    let (task, auto_recovered) = recover_analysis_task_if_needed(db, &task, now).await?;
    if task.status == "completed" && !has_chapter_analysis_result(db, chapter_id).await? {
        return Ok(missing_analysis_result_status_payload(chapter_id));
    }

    Ok(json!({
        "has_task": true,
        "task_id": task.id,
        "chapter_id": task.chapter_id,
        "status": task.status,
        "progress": task.progress,
        "error_message": task.error_message,
        "error_code": classify_analysis_error_code(task.error_message.as_deref()),
        "auto_recovered": auto_recovered,
        "created_at": format_datetime(task.created_at),
        "started_at": format_datetime(task.started_at),
        "completed_at": format_datetime(task.completed_at),
    }))
}

pub async fn load_analysis_task_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, LoadAnalysisTaskStatusError> {
    let _chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(LoadAnalysisTaskStatusError::Chapter)?;

    let task = latest_analysis_task(db, chapter_id)
        .await
        .map_err(|error| LoadAnalysisTaskStatusError::Internal(error.to_string()))?;
    analysis_task_status_payload(db, chapter_id, task)
        .await
        .map_err(|error| LoadAnalysisTaskStatusError::Internal(error.to_string()))
}

pub async fn load_batch_analysis_task_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    request: BatchAnalysisStatusRequest,
) -> Result<Value, String> {
    let chapter_ids = request.into_chapter_ids();
    if chapter_ids.is_empty() {
        return Ok(empty_batch_analysis_task_status_payload());
    }

    let mut response_project_id = String::new();
    let mut items = Map::new();
    for chapter_id in &chapter_ids {
        let chapter = ChapterService::get(db, chapter_id, user_id)
            .await
            .map_err(|error| error.to_string())?;

        if let Some(chapter) = chapter {
            if response_project_id.is_empty() {
                response_project_id = chapter.project_id;
            }
            let task = latest_analysis_task(db, chapter_id)
                .await
                .map_err(|error| error.to_string())?;
            let payload = analysis_task_status_payload(db, chapter_id, task)
                .await
                .map_err(|error| error.to_string())?;
            items.insert(chapter_id.clone(), payload);
        } else {
            let payload = analysis_task_status_payload(db, chapter_id, None)
                .await
                .map_err(|error| error.to_string())?;
            items.insert(chapter_id.clone(), payload);
        }
    }

    Ok(build_batch_analysis_task_status_payload(
        response_project_id,
        items,
    ))
}

pub enum LoadChapterAnalysisViewPayloadError {
    Context(ChapterAnalysisQueryContextError),
    AnalysisNotFound,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChapterAnalysisViewOptions {
    include_full_draft: bool,
}

impl ChapterAnalysisViewOptions {
    pub fn new(include_full_draft: bool) -> Self {
        Self { include_full_draft }
    }

    fn include_full_draft(self) -> bool {
        self.include_full_draft
    }
}

fn value_or_null(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

fn array_or_empty(value: Option<Value>) -> Value {
    value.filter(Value::is_array).unwrap_or_else(|| json!([]))
}

fn f64_or_zero(value: Option<f64>) -> Value {
    json!(value.unwrap_or(0.0))
}

fn build_chapter_analysis_view_payload(
    chapter: &chapter::Model,
    analysis: plot_analysis::Model,
    memories: Vec<story_memory::Model>,
    checker_fragments: ChapterAnalysisCheckerFragments,
    draft_fragments: ChapterDraftAnalysisViewFragments,
    quality_fragments: ChapterAnalysisQualityFragments,
    created_at: String,
    analysis_created_at: Option<String>,
) -> Value {
    json!({
        "chapter_id": chapter.id,
        "analysis": {
            "id": analysis.id,
            "project_id": analysis.project_id,
            "chapter_id": analysis.chapter_id,
            "plot_stage": analysis.plot_stage,
            "conflict_level": analysis.conflict_level,
            "conflict_types": array_or_empty(analysis.conflict_types),
            "emotional_tone": analysis.emotional_tone,
            "emotional_intensity": f64_or_zero(analysis.emotional_intensity),
            "emotional_curve": value_or_null(analysis.emotional_curve),
            "hooks": array_or_empty(analysis.hooks),
            "hooks_count": analysis.hooks_count,
            "hooks_avg_strength": analysis.hooks_avg_strength,
            "foreshadows": array_or_empty(analysis.foreshadows),
            "foreshadows_planted": analysis.foreshadows_planted,
            "foreshadows_resolved": analysis.foreshadows_resolved,
            "plot_points": array_or_empty(analysis.plot_points),
            "plot_points_count": analysis.plot_points_count,
            "character_states": array_or_empty(analysis.character_states),
            "scenes": array_or_empty(analysis.scenes),
            "pacing": analysis.pacing,
            "overall_quality_score": f64_or_zero(analysis.overall_quality_score),
            "pacing_score": f64_or_zero(analysis.pacing_score),
            "engagement_score": f64_or_zero(analysis.engagement_score),
            "coherence_score": f64_or_zero(analysis.coherence_score),
            "analysis_report": analysis.analysis_report,
            "suggestions": array_or_empty(analysis.suggestions),
            "word_count": analysis.word_count,
            "dialogue_ratio": f64_or_zero(analysis.dialogue_ratio),
            "description_ratio": f64_or_zero(analysis.description_ratio),
            "created_at": analysis_created_at,
        },
        "memories": memories.into_iter().map(|memory| json!({
            "id": memory.id,
            "type": memory.memory_type,
            "title": memory.title,
            "content": memory.content,
            "importance": memory.importance_score,
            "tags": value_or_null(memory.tags),
            "is_foreshadow": memory.is_foreshadow != 0,
            "position": memory.chapter_position,
            "related_characters": value_or_null(memory.related_characters),
        })).collect::<Vec<_>>(),
        "checker_result": checker_fragments.checker_result,
        "checker_created_at": checker_fragments.checker_created_at,
        "auto_revision_draft": draft_fragments.auto_revision_draft,
        "candidate_draft": draft_fragments.candidate_draft,
        "quality_metrics": quality_fragments.quality_metrics,
        "quality_metrics_summary": quality_fragments.quality_metrics_summary,
        "created_at": created_at,
    })
}

pub async fn load_chapter_analysis_view_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    options: ChapterAnalysisViewOptions,
) -> Result<Value, LoadChapterAnalysisViewPayloadError> {
    let chapter_id = chapter.id.clone();

    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(&chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error.to_string()),
            )
        })?
        .ok_or(LoadChapterAnalysisViewPayloadError::AnalysisNotFound)?;

    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .all(db)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error.to_string()),
            )
        })?;
    let read_context = load_chapter_analysis_read_context(db, &chapter_id)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error),
            )
        })?;

    let checker_fragments =
        ChapterAnalysisCheckerFragments::from_histories(&read_context.histories);
    let draft_fragments = build_chapter_draft_analysis_view_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
        chapter.updated_at,
        options.include_full_draft(),
    );
    let quality_fragments = build_chapter_analysis_quality_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
    );
    let analysis_created_at = format_datetime(analysis.created_at);
    let created_at = analysis_created_at
        .clone()
        .or_else(|| format_datetime(chapter.updated_at))
        .unwrap_or_default();

    Ok(build_chapter_analysis_view_payload(
        chapter,
        analysis,
        memories,
        checker_fragments,
        draft_fragments,
        quality_fragments,
        created_at,
        analysis_created_at,
    ))
}

pub async fn load_owned_chapter_analysis_view_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    options: ChapterAnalysisViewOptions,
) -> Result<Value, LoadChapterAnalysisViewPayloadError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(ChapterAnalysisQueryContextError::Chapter(
                error,
            ))
        })?;

    load_chapter_analysis_view_payload(db, &chapter, options).await
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, Utc};
    use serde_json::json;

    use super::{
        build_batch_analysis_status_request_from_route_payload,
        build_batch_analysis_task_status_payload, classify_analysis_error_code,
        empty_batch_analysis_task_status_payload, missing_analysis_result_status_payload,
        resolve_analysis_task_auto_recovery_error, BatchAnalysisStatusRouteRequest,
    };
    use crate::models::analysis_task;

    fn build_task(status: &str) -> analysis_task::Model {
        analysis_task::Model {
            id: "task-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            project_id: "project-1".to_string(),
            status: status.to_string(),
            progress: 35,
            error_message: None,
            created_at: None,
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn should_classify_known_analysis_error_codes() {
        let cases = [
            (Some("正在重试(1/3)：临时失败"), Some("retrying")),
            (Some("JSON解析失败：字段缺失"), Some("json_parse_failed")),
            (Some("AI返回格式异常：不是对象"), Some("json_parse_failed")),
            (Some("AI响应为空或过短"), Some("ai_empty")),
            (Some("流式响应中断：连接关闭"), Some("stream_interrupted")),
            (
                Some("流式生成出错：provider failed"),
                Some("stream_interrupted"),
            ),
            (Some("任务超时（超过10分钟未完成）"), Some("timeout")),
            (Some("启动超时（超过3分钟未启动）"), Some("timeout")),
            (Some("章节不存在或内容为空"), Some("chapter_empty")),
            (Some("项目不存在"), Some("project_missing")),
        ];

        for (message, expected) in cases {
            assert_eq!(classify_analysis_error_code(message), expected);
        }
    }

    #[test]
    fn should_classify_unknown_and_missing_analysis_error_codes() {
        assert_eq!(
            classify_analysis_error_code(Some("供应商返回未知错误")),
            Some("unknown")
        );
        assert_eq!(classify_analysis_error_code(None), None);
    }

    #[test]
    fn should_resolve_analysis_task_auto_recovery_error_for_running_timeout() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 25, 0)
            .expect("valid time");
        let mut task = build_task("running");
        task.started_at = Some(now - Duration::minutes(11));

        let error_message = resolve_analysis_task_auto_recovery_error(&task, now);

        assert_eq!(
            error_message.as_deref(),
            Some("任务超时（超过10分钟未完成，已自动恢复）")
        );
    }

    #[test]
    fn should_resolve_analysis_task_auto_recovery_error_for_pending_timeout() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 28, 0)
            .expect("valid time");
        let mut task = build_task("pending");
        task.created_at = Some(now - Duration::minutes(4));

        let error_message = resolve_analysis_task_auto_recovery_error(&task, now);

        assert_eq!(
            error_message.as_deref(),
            Some("任务启动超时（超过3分钟未启动，已自动恢复）")
        );
    }

    #[test]
    fn should_keep_analysis_task_when_auto_recovery_not_needed() {
        let now = Utc::now().naive_utc();
        let mut task = build_task("running");
        task.started_at = Some(now - Duration::minutes(2));

        let error_message = resolve_analysis_task_auto_recovery_error(&task, now);

        assert_eq!(error_message, None);
    }

    #[test]
    fn should_normalize_batch_analysis_status_request_from_route_ids() {
        let request = build_batch_analysis_status_request_from_route_payload(
            BatchAnalysisStatusRouteRequest {
                chapter_ids: vec![
                    " chapter-1 ".to_string(),
                    "".to_string(),
                    "chapter-2".to_string(),
                    "chapter-1".to_string(),
                    "   ".to_string(),
                ],
            },
        );

        assert_eq!(
            request.into_chapter_ids(),
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
    }

    #[test]
    fn should_build_empty_batch_analysis_task_status_payload() {
        assert_eq!(
            empty_batch_analysis_task_status_payload(),
            json!({
                "project_id": "",
                "total": 0,
                "items": {},
            })
        );
    }

    #[test]
    fn should_build_missing_analysis_result_status_payload() {
        assert_eq!(
            missing_analysis_result_status_payload("chapter-1"),
            json!({
                "has_task": false,
                "chapter_id": "chapter-1",
                "status": "none",
                "progress": 0,
                "error_message": "分析任务已完成，但分析结果缺失，请重新分析",
                "error_code": "analysis_missing",
                "auto_recovered": false,
                "missing_result": true,
                "task_id": null,
                "created_at": null,
                "started_at": null,
                "completed_at": null,
            })
        );
    }

    #[test]
    fn should_build_batch_analysis_task_status_payload() {
        let payload = build_batch_analysis_task_status_payload(
            "project-1".to_string(),
            [(
                "chapter-1".to_string(),
                json!({
                    "has_task": true,
                    "chapter_id": "chapter-1",
                    "status": "running",
                }),
            )]
            .into_iter()
            .collect(),
        );

        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"]["chapter-1"]["status"], "running");
    }
}

#[cfg(test)]
mod view_payload_tests {
    use chrono::NaiveDateTime;
    use serde_json::{json, Value};

    use crate::models::{chapter, plot_analysis, story_memory};
    use crate::services::chapter_draft_history_service::ChapterAnalysisCheckerFragments;
    use crate::services::chapter_draft_view_payload_service::ChapterDraftAnalysisViewFragments;
    use crate::services::chapter_quality_metrics_query_service::ChapterAnalysisQualityFragments;

    use super::{
        array_or_empty, build_chapter_analysis_view_payload, f64_or_zero, value_or_null,
        ChapterAnalysisViewOptions,
    };

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 1,
            title: "测试章节".to_string(),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn plot_analysis_model() -> plot_analysis::Model {
        plot_analysis::Model {
            id: "analysis-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            plot_stage: Some("climax".to_string()),
            conflict_level: Some(4),
            conflict_types: Some(json!(["inner"])),
            emotional_tone: Some("tense".to_string()),
            emotional_intensity: Some(0.8),
            emotional_curve: None,
            hooks: None,
            hooks_count: 1,
            hooks_avg_strength: Some(0.7),
            foreshadows: None,
            foreshadows_planted: 2,
            foreshadows_resolved: 1,
            plot_points: Some(json!(["point"])),
            plot_points_count: 1,
            character_states: None,
            scenes: None,
            pacing: Some("fast".to_string()),
            overall_quality_score: Some(88.0),
            pacing_score: Some(86.0),
            engagement_score: Some(87.0),
            coherence_score: Some(89.0),
            analysis_report: Some("report".to_string()),
            suggestions: Some(json!(["suggestion"])),
            word_count: Some(1200),
            dialogue_ratio: Some(0.3),
            description_ratio: Some(0.7),
            created_at: Some(test_datetime()),
        }
    }

    fn memory_model() -> story_memory::Model {
        story_memory::Model {
            id: "memory-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            memory_type: "plot".to_string(),
            title: Some("伏笔".to_string()),
            content: "记忆内容".to_string(),
            full_context: None,
            related_characters: None,
            related_locations: None,
            tags: Some(json!(["tag"])),
            importance_score: Some(0.9),
            story_timeline: 1,
            chapter_position: 20,
            text_length: 4,
            is_foreshadow: 1,
            foreshadow_resolved_at: None,
            foreshadow_strength: None,
            vector_id: None,
            embedding_model: None,
            created_at: Some(test_datetime()),
            updated_at: Some(test_datetime()),
        }
    }

    #[test]
    fn should_convert_optional_value_to_json_null() {
        assert_eq!(value_or_null(Some(json!({"k": "v"}))), json!({"k": "v"}));
        assert_eq!(value_or_null(None), Value::Null);
    }

    #[test]
    fn should_match_python_default_analysis_collections_and_scores() {
        let chapter = chapter_model();
        let mut analysis = plot_analysis_model();
        analysis.conflict_types = None;
        analysis.emotional_intensity = None;
        analysis.hooks = None;
        analysis.foreshadows = None;
        analysis.plot_points = None;
        analysis.character_states = None;
        analysis.scenes = None;
        analysis.overall_quality_score = None;
        analysis.pacing_score = None;
        analysis.engagement_score = None;
        analysis.coherence_score = None;
        analysis.suggestions = None;
        analysis.dialogue_ratio = None;
        analysis.description_ratio = None;

        let payload = build_chapter_analysis_view_payload(
            &chapter,
            analysis,
            vec![],
            ChapterAnalysisCheckerFragments {
                checker_result: None,
                checker_created_at: None,
            },
            ChapterDraftAnalysisViewFragments {
                auto_revision_draft: None,
                candidate_draft: None,
            },
            ChapterAnalysisQualityFragments {
                quality_metrics: None,
                quality_metrics_summary: None,
            },
            "2026-05-17T12:30:45".to_string(),
            Some("2026-05-17T12:30:45".to_string()),
        );

        assert_eq!(payload["analysis"]["conflict_types"], json!([]));
        assert_eq!(payload["analysis"]["hooks"], json!([]));
        assert_eq!(payload["analysis"]["foreshadows"], json!([]));
        assert_eq!(payload["analysis"]["plot_points"], json!([]));
        assert_eq!(payload["analysis"]["character_states"], json!([]));
        assert_eq!(payload["analysis"]["scenes"], json!([]));
        assert_eq!(payload["analysis"]["suggestions"], json!([]));
        assert_eq!(payload["analysis"]["emotional_intensity"], json!(0.0));
        assert_eq!(payload["analysis"]["overall_quality_score"], json!(0.0));
        assert_eq!(payload["analysis"]["pacing_score"], json!(0.0));
        assert_eq!(payload["analysis"]["engagement_score"], json!(0.0));
        assert_eq!(payload["analysis"]["coherence_score"], json!(0.0));
        assert_eq!(payload["analysis"]["dialogue_ratio"], json!(0.0));
        assert_eq!(payload["analysis"]["description_ratio"], json!(0.0));
    }

    #[test]
    fn should_normalize_analysis_default_helpers() {
        assert_eq!(array_or_empty(None), json!([]));
        assert_eq!(array_or_empty(Some(json!({"k": "v"}))), json!([]));
        assert_eq!(array_or_empty(Some(json!(["x"]))), json!(["x"]));
        assert_eq!(f64_or_zero(None), json!(0.0));
        assert_eq!(f64_or_zero(Some(8.6)), json!(8.6));
    }

    #[test]
    fn should_build_chapter_analysis_view_payload_with_fragments_and_memories() {
        let chapter = chapter_model();
        let payload = build_chapter_analysis_view_payload(
            &chapter,
            plot_analysis_model(),
            vec![memory_model()],
            ChapterAnalysisCheckerFragments {
                checker_result: Some(json!({"score": 91})),
                checker_created_at: Some("2026-05-17T12:30:45".to_string()),
            },
            ChapterDraftAnalysisViewFragments {
                auto_revision_draft: Some(json!({"history_id": "history-1"})),
                candidate_draft: None,
            },
            ChapterAnalysisQualityFragments {
                quality_metrics: Some(json!({"score": 88})),
                quality_metrics_summary: Some(json!({"summary": "ok"})),
            },
            "2026-05-17T12:30:45".to_string(),
            Some("2026-05-17T12:30:45".to_string()),
        );

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["analysis"]["id"], "analysis-1");
        assert_eq!(payload["analysis"]["conflict_types"], json!(["inner"]));
        assert!(payload["analysis"]["emotional_curve"].is_null());
        assert_eq!(payload["memories"][0]["id"], "memory-1");
        assert_eq!(payload["memories"][0]["is_foreshadow"], true);
        assert!(payload["memories"][0]["related_characters"].is_null());
        assert_eq!(payload["checker_result"], json!({"score": 91}));
        assert_eq!(payload["auto_revision_draft"]["history_id"], "history-1");
        assert!(payload["candidate_draft"].is_null());
        assert_eq!(payload["quality_metrics"], json!({"score": 88}));
        assert_eq!(payload["quality_metrics_summary"], json!({"summary": "ok"}));
        assert_eq!(payload["created_at"], "2026-05-17T12:30:45");
    }

    #[test]
    fn should_build_chapter_analysis_view_options_with_include_full_draft_flag() {
        let options = ChapterAnalysisViewOptions::new(true);
        let default_options = ChapterAnalysisViewOptions::default();

        assert_eq!(options, ChapterAnalysisViewOptions::new(true));
        assert_ne!(options, default_options);
    }
}
