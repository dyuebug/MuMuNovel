pub(crate) mod existing_background_owner;

pub(crate) use self::existing_background_owner::{
    build_single_generation_existing_background_task_owner_contract,
    load_owned_single_generation_existing_background_task_payload,
};

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_existing_background_task_owner_contract,
        existing_background_owner::{
            build_single_generation_existing_background_task_payload,
            single_generation_existing_background_task_contains_chapter,
            SingleGenerationExistingBackgroundTaskReadState,
        },
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract;
    use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;
    use crate::services::chapter_single_generation_prepare_service::build_single_generation_prepare_owner_contract;
    use crate::services::chapter_single_generation_runtime_state_service::build_single_generation_runtime_state_owner_contract;
    use serde_json::json;

    fn build_existing_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", {"id": "chapter-2"}]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_publish_single_generation_existing_background_task_owner_contract() {
        let contract = build_single_generation_existing_background_task_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_existing_background_task_service"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "load_owned_single_generation_existing_background_task_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["response_payload_fields"][10],
            "active_story_repair_payload"
        );
        assert_eq!(
            contract["prepare_owner_contract"]["owner"],
            build_single_generation_prepare_owner_contract()["owner"]
        );
        assert_eq!(
            contract["runtime_state_owner_contract"]["owner"],
            build_single_generation_runtime_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            build_chapter_generation_snapshot_owner_contract()["owner"]
        );
        assert_eq!(
            contract["quality_runtime_owner_contract"]["owner"],
            build_generation_quality_runtime_owner_contract()["owner"]
        );
    }

    #[test]
    fn should_match_single_generation_existing_background_task_for_string_or_object_chapter_ids() {
        let task = build_existing_task();

        assert!(single_generation_existing_background_task_contains_chapter(
            &task,
            "chapter-1"
        ));
        assert!(single_generation_existing_background_task_contains_chapter(
            &task,
            "chapter-2"
        ));
        assert!(!single_generation_existing_background_task_contains_chapter(&task, "chapter-9"));
    }

    #[test]
    fn should_build_single_generation_existing_background_read_state_from_task_and_snapshot() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 91})),
            quality_metrics_history: Some(json!([{"overall_score": 91}])),
            quality_metrics_summary: Some(json!({"chapter_count": 1})),
            workflow_runtime_state: Some(json!({
                "progress": 55,
                "active_story_repair_payload": {
                    "summary": "沿用修复建议"
                }
            })),
            created_at: None,
            updated_at: None,
        };

        let read_state = SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
            build_existing_task(),
            Some(&snapshot),
        );

        assert_eq!(read_state.task().id, "task-1");
        assert_eq!(
            read_state
                .workflow_runtime_state()
                .and_then(|state| state.get("progress"))
                .and_then(serde_json::Value::as_i64),
            Some(55)
        );
        assert_eq!(
            read_state
                .quality_status_context()
                .latest_quality_metrics()
                .and_then(|metrics| metrics.get("overall_score")),
            Some(&json!(91))
        );
    }

    #[test]
    fn should_build_single_generation_existing_background_task_payload() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 92})),
            quality_metrics_history: Some(json!([{"overall_score": 88}, {"overall_score": 92}])),
            quality_metrics_summary: Some(json!({"chapter_count": 2})),
            workflow_runtime_state: Some(json!({
                "progress": 77,
                "active_story_repair_payload": {
                    "summary": "保持冲突升级"
                }
            })),
            created_at: None,
            updated_at: None,
        };
        let read_state = SingleGenerationExistingBackgroundTaskReadState::from_task_and_snapshot(
            batch_generation_task::Model {
                id: "task-1".to_string(),
                project_id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                start_chapter_number: 2,
                chapter_count: 1,
                chapter_ids: json!(["chapter-2"]),
                style_id: None,
                target_word_count: 3200,
                enable_analysis: true,
                status: "running".to_string(),
                total_chapters: 1,
                completed_chapters: 0,
                failed_chapters: json!([]),
                current_chapter_id: Some("chapter-2".to_string()),
                current_chapter_number: Some(2),
                current_retry_count: 0,
                max_retries: 3,
                created_at: None,
                started_at: None,
                completed_at: None,
                error_message: None,
            },
            Some(&snapshot),
        );
        let payload = build_single_generation_existing_background_task_payload(read_state);

        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["chapter_id"], "chapter-2");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["message"], "已有后台生成任务正在执行");
        assert_eq!(payload["estimated_time_minutes"], 3);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 92);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 92);
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "保持冲突升级"
        );
    }
}
