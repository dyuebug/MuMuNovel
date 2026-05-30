use crate::tasks::types::{TaskListQuery, TaskStatus};

pub(crate) fn normalize_task_statuses_query(query: &TaskListQuery) -> Option<Vec<TaskStatus>> {
    query.statuses.as_ref().map(|statuses| {
        statuses
            .split(',')
            .filter_map(|part| match part.trim() {
                "pending" => Some(TaskStatus::Pending),
                "running" => Some(TaskStatus::Running),
                "completed" => Some(TaskStatus::Completed),
                "failed" => Some(TaskStatus::Failed),
                "cancelled" => Some(TaskStatus::Cancelled),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use crate::tasks::types::{TaskListQuery, TaskStatus};

    use super::normalize_task_statuses_query;

    #[test]
    fn normalize_task_statuses_query_keeps_known_status_filtering() {
        let query = TaskListQuery {
            project_id: None,
            statuses: Some("pending, running,unknown,failed".to_string()),
            active_only: Some(false),
            limit: Some(10),
        };

        let statuses = normalize_task_statuses_query(&query).expect("statuses should exist");

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

        assert_eq!(normalize_task_statuses_query(&query), None);
    }
}
