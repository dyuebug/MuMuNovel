use serde_json::{json, Value};

pub(crate) mod analysis_payload_owner;
pub(crate) mod persistence_owner;
pub(crate) mod query_owner;
pub(crate) mod state_sync_owner;
pub(crate) mod trigger_runtime_owner;
pub(crate) use self::analysis_payload_owner::build_chapter_analysis_payload_owner_contract;
#[cfg(test)]
pub(crate) use self::analysis_payload_owner::{
    build_analysis_foreshadow_sync_route_request, build_analysis_runtime_chapter_model,
    build_chapter_analysis_quality_metrics_payload, build_chapter_analysis_report,
    build_generated_chapter_analysis_overrides, extract_analysis_memories, find_text_position,
    json_f64, json_i32, ChapterAnalysisRuntimeOverrides,
};
pub(crate) use self::persistence_owner::build_chapter_analysis_persistence_owner_contract;
pub(crate) use self::state_sync_owner::build_chapter_analysis_state_sync_owner_contract;
pub(crate) use self::trigger_runtime_owner::build_chapter_analysis_trigger_runtime_owner_contract;
pub(crate) use self::trigger_runtime_owner::{
    analyze_chapter_now, analyze_generated_chapter_follow_up, prepare_chapter_analysis_execution,
    trigger_chapter_analysis_write_workflow, PrepareChapterAnalysisTriggerError,
};
#[cfg(test)]
pub(crate) use self::trigger_runtime_owner::{
    build_chapter_analysis_task_create_response_payload, ChapterAnalysisTaskCreateState,
    PreparedChapterAnalysisTriggerExecution,
};

#[allow(dead_code)]
pub(crate) fn build_chapter_analysis_runtime_owner_contract() -> Value {
    json!({
        "owner": "chapter_analysis_runtime_service",
        "scope": "chapter_analysis_runtime_trigger_prompt_ai_persistence_query_and_health_handoff",
        "python_source_map": [
            "backend/app/api/chapter_analysis_routes.py",
            "backend/app/api/chapter_analysis_task_routes.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py",
            "backend/app/services/chapter_analysis_response_service.py",
            "backend/app/services/memory_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/trigger_runtime_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/analysis_payload_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/persistence_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/state_sync_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/query_owner.rs",
            "backend-rs/src/services/chapter_analysis_service.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs"
        ],
        "behavior_contract": {
            "task_create_response_owner": "ChapterAnalysisTaskCreateState::compatibility_payload",
            "runtime_trigger_owner": "trigger_runtime_owner::execute_prepared_chapter_analysis_trigger",
            "background_execution_owner": "trigger_runtime_owner::dispatch_prepared_chapter_analysis_trigger",
            "query_owner_module": "chapter_analysis_runtime_service::query_owner",
            "quality_metrics_payload_owner": "build_chapter_analysis_quality_metrics_payload",
            "analysis_result_persistence_owner": "persist_chapter_analysis_result",
            "failed_task_recovery_owner": "trigger_runtime_owner::mark_analysis_task_failed"
        },
        "analysis_payload_owner_contract": build_chapter_analysis_payload_owner_contract(),
        "persistence_owner_contract": build_chapter_analysis_persistence_owner_contract(),
        "state_sync_owner_contract": build_chapter_analysis_state_sync_owner_contract(),
        "trigger_runtime_owner_contract": build_chapter_analysis_trigger_runtime_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-chapter-analysis-owner",
            "chapter_analysis_manifest_probe_count": 8,
            "rust_manifest_probe_count": 8,
            "python_fallback_probe_count": 0,
            "runtime_trigger_owner": "chapter_analysis_runtime_service",
            "query_owner": "chapter_analysis_runtime_service/query_owner.rs",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit_python_source_map_freeze_delete_or_repoint_approval",
            "status": "rust_service_runtime_owner_closeout_ready_python_source_map_pending"
        },
        "rollback_boundary": {
            "python_source_map_retained": true,
            "approval_required_before_python_edit": true
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;

    use super::{
        build_analysis_foreshadow_sync_route_request, build_analysis_runtime_chapter_model,
        build_chapter_analysis_quality_metrics_payload, build_chapter_analysis_report,
        build_chapter_analysis_runtime_owner_contract,
        build_chapter_analysis_task_create_response_payload,
        build_generated_chapter_analysis_overrides, extract_analysis_memories, find_text_position,
        json_f64, json_i32, ChapterAnalysisRuntimeOverrides, ChapterAnalysisTaskCreateState,
        PrepareChapterAnalysisTriggerError, PreparedChapterAnalysisTriggerExecution,
    };
    use crate::models::chapter;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

    #[test]
    fn should_clamp_json_i32_values() {
        assert_eq!(json_i32(None), 0);
        assert_eq!(json_i32(Some(42)), 42);
        assert_eq!(json_i32(Some(i64::from(i32::MAX) + 1)), i32::MAX);
        assert_eq!(json_i32(Some(i64::from(i32::MIN) - 1)), i32::MIN);
    }

    #[test]
    fn should_publish_chapter_analysis_runtime_owner_contract() {
        let contract = build_chapter_analysis_runtime_owner_contract();

        assert_eq!(contract["owner"], "chapter_analysis_runtime_service");
        assert_eq!(
            contract["behavior_contract"]["runtime_trigger_owner"],
            "trigger_runtime_owner::execute_prepared_chapter_analysis_trigger"
        );
        assert_eq!(
            contract["behavior_contract"]["query_owner_module"],
            "chapter_analysis_runtime_service::query_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["background_execution_owner"],
            "trigger_runtime_owner::dispatch_prepared_chapter_analysis_trigger"
        );
        assert_eq!(
            contract["behavior_contract"]["failed_task_recovery_owner"],
            "trigger_runtime_owner::mark_analysis_task_failed"
        );
        assert_eq!(
            contract["trigger_runtime_owner_contract"]["owner"],
            "chapter_analysis_runtime_service::trigger_runtime_owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-chapter-analysis-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["chapter_analysis_manifest_probe_count"],
            8
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            false
        );
    }

    #[test]
    fn should_filter_non_finite_json_f64_values() {
        assert_eq!(json_f64(Some(0.75)), Some(0.75));
        assert_eq!(json_f64(Some(f64::NAN)), None);
        assert_eq!(json_f64(Some(f64::INFINITY)), None);
        assert_eq!(json_f64(None), None);
    }

    #[test]
    fn should_build_chapter_analysis_report_from_payload_sections() {
        let payload = json!({
            "plot_stage": " 高潮 ",
            "conflict": {
                "description": " 正面对抗 "
            },
            "scores": {
                "score_justification": " 节奏稳定 "
            },
            "suggestions": [" 强化铺垫 ", "", 7, "压缩说明"]
        });

        let report = build_chapter_analysis_report(&payload);

        assert_eq!(
            report,
            Some("剧情阶段：高潮\n冲突分析：正面对抗\n评分说明：节奏稳定\n改进建议：强化铺垫；压缩说明".to_string())
        );
    }

    #[test]
    fn should_skip_empty_chapter_analysis_report_sections() {
        let payload = json!({
            "plot_stage": "  ",
            "conflict": {
                "description": ""
            },
            "scores": {},
            "suggestions": [" ", 1]
        });

        assert_eq!(build_chapter_analysis_report(&payload), None);
    }

    #[test]
    fn should_build_chapter_analysis_task_create_payload() {
        let payload =
            ChapterAnalysisTaskCreateState::new("task-123".to_string(), "chapter-456".to_string())
                .compatibility_payload();

        assert_eq!(
            payload,
            json!({
                "task_id": "task-123",
                "chapter_id": "chapter-456",
                "status": "pending",
                "message": "章节分析任务已创建",
            })
        );
    }

    #[test]
    fn should_keep_chapter_analysis_trigger_create_state_contract_minimal() {
        let create_state =
            ChapterAnalysisTaskCreateState::new("task-1".to_string(), "chapter-1".to_string());

        assert_eq!(create_state.task_id, "task-1");
        assert_eq!(create_state.chapter_id, "chapter-1");
        assert_eq!(create_state.compatibility_payload()["task_id"], "task-1");
        assert_eq!(create_state.task_id(), "task-1");
    }

    #[test]
    fn should_build_chapter_analysis_task_create_response_payload_from_status_owner() {
        let create_state =
            ChapterAnalysisTaskCreateState::new("task-10".to_string(), "chapter-20".to_string());
        let payload = build_chapter_analysis_task_create_response_payload(
            json!({
                "has_task": true,
                "task_id": "task-10",
                "chapter_id": "chapter-20",
                "status": "pending",
                "progress": 0,
                "error_message": null,
                "error_code": null,
                "auto_recovered": false,
                "created_at": "2026-06-02T12:00:00",
                "started_at": null,
                "completed_at": null,
            }),
            &create_state,
        );

        assert_eq!(payload["has_task"], true);
        assert_eq!(payload["task_id"], "task-10");
        assert_eq!(payload["chapter_id"], "chapter-20");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["progress"], 0);
        assert_eq!(payload["auto_recovered"], false);
        assert_eq!(payload["message"], "章节分析任务已创建");
    }

    #[test]
    fn should_keep_prepared_chapter_analysis_trigger_execution_task_identity() {
        let prepared = PreparedChapterAnalysisTriggerExecution::from_create_state(
            ChapterAnalysisTaskCreateState::new("task-2".to_string(), "chapter-2".to_string()),
        );

        assert_eq!(prepared.task_id(), "task-2");
    }

    #[test]
    fn should_keep_trigger_write_workflow_chapter_error_shape() {
        let error = PrepareChapterAnalysisTriggerError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            PrepareChapterAnalysisTriggerError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_keep_trigger_write_workflow_create_error_shape() {
        let error = PrepareChapterAnalysisTriggerError::Create(
            CreateChapterAnalysisTaskError::ChapterEmpty,
        );

        assert!(matches!(
            error,
            PrepareChapterAnalysisTriggerError::Create(
                CreateChapterAnalysisTaskError::ChapterEmpty
            )
        ));
    }

    #[test]
    fn should_build_analysis_foreshadow_sync_route_request_from_payload() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第12章".to_string(),
            content: Some("测试内容".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let payload = json!({
            "foreshadows": [
                {
                    "type": "planted",
                    "content": "主角第一次注意到钥匙上的旧纹章"
                }
            ]
        });

        let request = build_analysis_foreshadow_sync_route_request(&chapter_model, &payload)
            .expect("should build request");

        assert_eq!(
            *request.body(),
            json!({
                "chapter_id": "chapter-1",
                "chapter_number": 12,
                "analysis_foreshadows": [
                    {
                        "type": "planted",
                        "content": "主角第一次注意到钥匙上的旧纹章"
                    }
                ]
            })
        );
    }

    #[test]
    fn should_skip_analysis_foreshadow_sync_route_request_when_empty() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第12章".to_string(),
            content: Some("测试内容".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };

        assert!(build_analysis_foreshadow_sync_route_request(
            &chapter_model,
            &json!({ "foreshadows": [] })
        )
        .is_none());
        assert!(build_analysis_foreshadow_sync_route_request(&chapter_model, &json!({})).is_none());
    }

    #[test]
    fn should_find_text_position_with_exact_match() {
        assert_eq!(
            find_text_position("主角看见旧纹章钥匙。", "旧纹章钥匙"),
            (4, 5)
        );
    }

    #[test]
    fn should_extract_analysis_memories_from_payload() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("主角看见旧纹章钥匙，心中一震。双方随即爆发正面冲突。".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let payload = json!({
            "summary": "主角发现关键线索并卷入冲突",
            "hooks": [
                {
                    "type": "悬念",
                    "content": "钥匙上的纹章来历不明",
                    "position": "开篇",
                    "keyword": "旧纹章钥匙",
                    "strength": 8
                }
            ],
            "foreshadows": [
                {
                    "type": "planted",
                    "content": "钥匙暗示王室秘闻",
                    "keyword": "旧纹章钥匙",
                    "strength": 7,
                    "related_characters": ["主角"]
                }
            ],
            "plot_points": [
                {
                    "type": "turning_point",
                    "content": "主角决定追查钥匙来源",
                    "impact": "推动主线升级",
                    "keyword": "旧纹章钥匙",
                    "importance": 0.8
                }
            ],
            "character_states": [
                {
                    "character_name": "主角",
                    "state_before": "迟疑",
                    "state_after": "坚定",
                    "psychological_change": "决定主动调查"
                }
            ],
            "conflict": {
                "level": 8,
                "description": "双方围绕钥匙归属激烈争执",
                "parties": ["主角", "黑衣人"],
                "types": ["外部冲突"]
            }
        });

        let memories = extract_analysis_memories(&chapter_model, &payload);
        let memory_types = memories
            .iter()
            .map(|item| item.memory_type.as_str())
            .collect::<Vec<_>>();

        assert!(memory_types.contains(&"chapter_summary"));
        assert!(memory_types.contains(&"hook"));
        assert!(memory_types.contains(&"foreshadow"));
        assert!(memory_types.contains(&"plot_point"));
        assert!(memory_types.contains(&"character_event"));

        let summary = memories
            .iter()
            .find(|item| item.memory_type == "chapter_summary")
            .expect("chapter summary memory");
        assert_eq!(summary.title.as_deref(), Some("第12章《风雪夜》摘要"));

        let foreshadow = memories
            .iter()
            .find(|item| item.memory_type == "foreshadow")
            .expect("foreshadow memory");
        assert_eq!(
            foreshadow
                .metadata
                .get("is_foreshadow")
                .and_then(serde_json::Value::as_i64),
            Some(1)
        );
    }

    #[test]
    fn should_build_analysis_runtime_chapter_model_with_overrides() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("旧正文".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };

        let effective = build_analysis_runtime_chapter_model(
            &chapter_model,
            &ChapterAnalysisRuntimeOverrides::new(Some(" 新正文 ".to_string()), Some(4321)),
        );

        assert_eq!(effective.content.as_deref(), Some("新正文"));
        assert_eq!(effective.word_count, 4321);
        assert_eq!(effective.id, chapter_model.id);
        assert_eq!(effective.project_id, chapter_model.project_id);
    }

    #[test]
    fn should_build_generated_chapter_follow_up_analysis_overrides() {
        let overrides = build_generated_chapter_analysis_overrides(&GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: " 新生成正文 ".to_string(),
            word_count: 4321,
            ..Default::default()
        });

        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("旧正文".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let effective = build_analysis_runtime_chapter_model(&chapter_model, &overrides);

        assert_eq!(effective.content.as_deref(), Some("新生成正文"));
        assert_eq!(effective.word_count, 4321);
    }

    #[test]
    fn should_build_chapter_analysis_quality_metrics_payload_from_analysis_scores() {
        let payload = json!({
            "scores": {
                "overall": 7.6,
                "pacing": 7.1,
                "engagement": 8.4,
                "coherence": 7.8,
                "score_justification": "中段说明略多，但悬念还在。"
            },
            "hooks": [{"type": "悬念"}],
            "suggestions": ["压缩说明段", "提前冲突触发"]
        });

        let metrics =
            build_chapter_analysis_quality_metrics_payload(&payload).expect("metrics payload");

        assert_eq!(metrics["overall_score"], 7.6);
        assert_eq!(
            metrics["repair_guidance"]["repair_targets"],
            json!(["压缩说明段", "提前冲突触发"])
        );
        assert_eq!(metrics["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            metrics["quality_runtime_context"]["source"],
            "plot_analysis"
        );
    }
}
