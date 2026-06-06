use chrono::{Duration, NaiveDateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::models::analysis_task;
use crate::services::chapter_access_service::load_accessible_chapter;
use crate::services::chapter_analysis_service::LoadAnalysisTaskStatusError;
use crate::services::chapter_analysis_task_state_service::{
    apply_analysis_task_state_by_id, AnalysisTaskStage,
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

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate, Utc};
    use serde_json::json;

    use super::{
        build_batch_analysis_status_request_from_route_payload,
        build_batch_analysis_task_status_payload, classify_analysis_error_code,
        empty_batch_analysis_task_status_payload, resolve_analysis_task_auto_recovery_error,
        BatchAnalysisStatusRouteRequest,
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
