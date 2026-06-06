use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::regeneration_task;
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};

pub type LoadRegenerationTasksPayloadError = LoadAccessibleChapterError;
const REGENERATION_TASKS_LIMIT_DEFAULT: u64 = 10;
const REGENERATION_TASKS_LIMIT_MIN: i64 = 1;
const REGENERATION_TASKS_LIMIT_MAX: u64 = 50;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct RegenerationTasksRouteQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegenerationTasksQueryRequest {
    limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationTasksQueryRequestError {
    LimitTooSmall,
    LimitTooLarge,
}

impl RegenerationTasksQueryRequest {
    fn from_route_query(
        route_query: RegenerationTasksRouteQuery,
    ) -> Result<Self, RegenerationTasksQueryRequestError> {
        let Some(limit) = route_query.limit else {
            return Ok(Self {
                limit: REGENERATION_TASKS_LIMIT_DEFAULT,
            });
        };

        if limit < REGENERATION_TASKS_LIMIT_MIN {
            return Err(RegenerationTasksQueryRequestError::LimitTooSmall);
        }
        if limit > REGENERATION_TASKS_LIMIT_MAX as i64 {
            return Err(RegenerationTasksQueryRequestError::LimitTooLarge);
        }

        Ok(Self {
            limit: limit as u64,
        })
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }
}

pub fn build_regeneration_tasks_query_request_from_route_query(
    route_query: RegenerationTasksRouteQuery,
) -> Result<RegenerationTasksQueryRequest, RegenerationTasksQueryRequestError> {
    RegenerationTasksQueryRequest::from_route_query(route_query)
}

pub fn datetime_to_string(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub async fn load_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    limit: u64,
) -> Result<Value, String> {
    let tasks = regeneration_task::Entity::find()
        .filter(regeneration_task::Column::ChapterId.eq(chapter_id.to_string()))
        .order_by_desc(regeneration_task::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let task_items: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "task_id": task.id,
                "status": task.status,
                "version_number": task.version_number,
                "version_note": task.version_note,
                "original_word_count": task.original_word_count,
                "regenerated_word_count": task.regenerated_word_count,
                "created_at": datetime_to_string(task.created_at),
                "completed_at": datetime_to_string(task.completed_at),
            })
        })
        .collect();

    Ok(json!({
        "chapter_id": chapter_id,
        "total": task_items.len(),
        "tasks": task_items,
    }))
}

pub async fn load_owned_regeneration_tasks_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: RegenerationTasksQueryRequest,
) -> Result<Value, LoadRegenerationTasksPayloadError> {
    let _ = load_accessible_chapter(db, chapter_id, user_id).await?;

    load_regeneration_tasks_payload(db, chapter_id, request.limit())
        .await
        .map_err(LoadAccessibleChapterError::Internal)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::services::chapter_access_service::LoadAccessibleChapterError;

    use super::{
        build_regeneration_tasks_query_request_from_route_query, datetime_to_string,
        LoadRegenerationTasksPayloadError, RegenerationTasksQueryRequestError,
        RegenerationTasksRouteQuery,
    };

    #[test]
    fn should_format_regeneration_task_datetime() {
        let datetime = NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse");

        assert_eq!(
            datetime_to_string(Some(datetime)),
            Some("2026-05-17T12:30:45".to_string())
        );
        assert_eq!(datetime_to_string(None), None);
    }

    #[test]
    fn should_alias_access_not_found_error_for_regeneration_tasks_query() {
        let error: LoadRegenerationTasksPayloadError =
            LoadAccessibleChapterError::NotFoundOrAccessDenied;

        assert_eq!(error, LoadAccessibleChapterError::NotFoundOrAccessDenied);
    }

    #[test]
    fn should_alias_access_internal_error_for_regeneration_tasks_query() {
        let error: LoadRegenerationTasksPayloadError =
            LoadAccessibleChapterError::Internal("boom".to_string());

        assert_eq!(
            error,
            LoadAccessibleChapterError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_validate_regeneration_tasks_query_request_limit_like_python_query() {
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: None
            })
            .expect("default limit should be valid")
            .limit(),
            10
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(25)
            })
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(0)
            }),
            Err(RegenerationTasksQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(-1)
            }),
            Err(RegenerationTasksQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(99)
            }),
            Err(RegenerationTasksQueryRequestError::LimitTooLarge)
        );
    }
}
