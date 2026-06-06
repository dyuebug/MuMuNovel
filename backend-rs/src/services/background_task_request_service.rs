use crate::tasks::types::{TaskListQuery, TaskStatus};

const TASK_LIST_LIMIT_DEFAULT: usize = 20;
const TASK_LIST_LIMIT_MIN: i64 = 1;
const TASK_LIST_LIMIT_MAX: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskListQueryRequestError {
    InvalidStatuses(Vec<String>),
    LimitTooSmall,
    LimitTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskListRequest {
    project_id: Option<String>,
    statuses: Option<Vec<TaskStatus>>,
    active_only: bool,
    limit: usize,
}

impl TaskListRequest {
    pub(crate) fn from_route_query(
        query: TaskListQuery,
    ) -> Result<Self, TaskListQueryRequestError> {
        let statuses = normalize_task_statuses_query(&query)?;
        let limit = normalize_task_list_limit(query.limit)?;

        Ok(Self {
            project_id: query.project_id,
            statuses,
            active_only: query.active_only.unwrap_or(false),
            limit,
        })
    }

    pub(crate) fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    pub(crate) fn statuses(&self) -> Option<&[TaskStatus]> {
        self.statuses.as_deref()
    }

    pub(crate) fn active_only(&self) -> bool {
        self.active_only
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }
}

pub(crate) fn normalize_task_statuses_query(
    query: &TaskListQuery,
) -> Result<Option<Vec<TaskStatus>>, TaskListQueryRequestError> {
    let Some(statuses) = query.statuses.as_ref() else {
        return Ok(None);
    };

    let mut parsed = Vec::new();
    let mut invalid = Vec::new();

    for part in statuses.split(',') {
        let status = part.trim().to_lowercase();
        if status.is_empty() {
            continue;
        }

        match status.as_str() {
            "pending" => parsed.push(TaskStatus::Pending),
            "running" => parsed.push(TaskStatus::Running),
            "completed" => parsed.push(TaskStatus::Completed),
            "failed" => parsed.push(TaskStatus::Failed),
            "cancelled" => parsed.push(TaskStatus::Cancelled),
            _ => invalid.push(status),
        }
    }

    if invalid.is_empty() {
        Ok(Some(parsed))
    } else {
        invalid.sort();
        invalid.dedup();
        Err(TaskListQueryRequestError::InvalidStatuses(invalid))
    }
}

fn normalize_task_list_limit(limit: Option<i64>) -> Result<usize, TaskListQueryRequestError> {
    let Some(limit) = limit else {
        return Ok(TASK_LIST_LIMIT_DEFAULT);
    };

    if limit < TASK_LIST_LIMIT_MIN {
        return Err(TaskListQueryRequestError::LimitTooSmall);
    }
    if limit > TASK_LIST_LIMIT_MAX as i64 {
        return Err(TaskListQueryRequestError::LimitTooLarge);
    }

    Ok(limit as usize)
}

#[cfg(test)]
mod tests {
    use crate::tasks::types::{TaskListQuery, TaskStatus};

    use super::{normalize_task_statuses_query, TaskListQueryRequestError, TaskListRequest};

    #[test]
    fn normalize_task_statuses_query_accepts_known_status_filtering() {
        let query = TaskListQuery {
            project_id: None,
            statuses: Some("pending, running,failed".to_string()),
            active_only: Some(false),
            limit: Some(10),
        };

        let statuses = normalize_task_statuses_query(&query)
            .expect("known statuses should be valid")
            .expect("statuses should exist");

        assert_eq!(
            statuses,
            vec![TaskStatus::Pending, TaskStatus::Running, TaskStatus::Failed]
        );
    }

    #[test]
    fn normalize_task_statuses_query_keeps_none_when_query_missing() {
        let query = TaskListQuery {
            project_id: None,
            statuses: None,
            active_only: None,
            limit: None,
        };

        assert_eq!(normalize_task_statuses_query(&query).unwrap(), None);
    }

    #[test]
    fn normalize_task_statuses_query_rejects_unknown_status_like_python_route() {
        let query = TaskListQuery {
            project_id: None,
            statuses: Some("pending, unknown,invalid,unknown".to_string()),
            active_only: Some(false),
            limit: Some(10),
        };

        assert_eq!(
            normalize_task_statuses_query(&query),
            Err(TaskListQueryRequestError::InvalidStatuses(vec![
                "invalid".to_string(),
                "unknown".to_string()
            ]))
        );
    }

    #[test]
    fn task_list_request_validates_limit_like_python_query() {
        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: Some("project-1".to_string()),
                statuses: None,
                active_only: None,
                limit: None,
            })
            .expect("default limit should be valid")
            .limit(),
            20
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: Some(true),
                limit: Some(25),
            })
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(0),
            }),
            Err(TaskListQueryRequestError::LimitTooSmall)
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(-1),
            }),
            Err(TaskListQueryRequestError::LimitTooSmall)
        );

        assert_eq!(
            TaskListRequest::from_route_query(TaskListQuery {
                project_id: None,
                statuses: None,
                active_only: None,
                limit: Some(101),
            }),
            Err(TaskListQueryRequestError::LimitTooLarge)
        );
    }
}
