use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, generation_history};
use crate::services::chapter_batch_generation_write_workflow_service::{
    active_story_repair_payload_from_runtime_state, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_batch_generation_snapshot_service::upsert_batch_generation_runtime_snapshot;
use crate::services::chapter_quality_metrics_query_service::build_chapter_analysis_quality_fragments;
use crate::services::chapter_quality_metrics_query_service::build_chapter_quality_metrics_fragments;
use crate::services::chapter_analysis_read_context_service::load_chapter_analysis_read_context;
use crate::services::chapter_story_repair_quality_context_service::{
    aggregate_story_repair_quality_summaries, extract_quality_history_context,
    merge_active_story_repair_payloads,
    restore_active_story_repair_payload_from_quality_context,
    restore_story_repair_compat_options_from_active_snapshot,
};

use super::chapter_single_generation_prepare_service::{
    prepare_single_chapter_generation_request, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    SingleChapterGenerationRequest, SingleChapterGenerationTarget,
};
use super::chapter_single_generation_runtime_state_service::{
    dispatch_single_chapter_generation_runtime, SingleGenerationRuntimeLaunchInput,
};

const SINGLE_GENERATION_BACKGROUND_ESTIMATED_MINUTES: i32 = 2;

fn build_single_generation_runtime_state_payload_from_sources(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    derived_source: &str,
    derived_source_label: &str,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let explicit_story_repair_payload =
        request_runtime_state.active_story_repair_payload_with_scope("chapter");
    let derived_story_repair_payload = restore_active_story_repair_payload_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
        "chapter",
        derived_source,
        derived_source_label,
    );
    let active_story_repair_payload = merge_active_story_repair_payloads(
        explicit_story_repair_payload.as_ref(),
        derived_story_repair_payload.as_ref(),
        "chapter",
        derived_source,
        derived_source_label,
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    payload.insert(
        "quality_metrics_summary".to_string(),
        quality_metrics_summary.cloned().unwrap_or(Value::Null),
    );
    payload.insert(
        "quality_history_context".to_string(),
        extract_quality_history_context(quality_metrics_summary).unwrap_or(Value::Null),
    );

    Value::Object(payload)
}

fn build_single_generation_runtime_state_payload_from_parts(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    build_single_generation_runtime_state_payload_from_sources(
        request_runtime_state,
        quality_metrics_summary,
        latest_quality_metrics,
        "current_chapter_quality",
        "Current chapter quality",
    )
}

async fn load_recent_single_generation_story_repair_quality_summary(
    db: &DatabaseConnection,
    project_id: &str,
    before_chapter_number: i32,
) -> Result<Option<Value>, String> {
    if before_chapter_number <= 1 {
        return Ok(None);
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .filter(chapter::Column::ChapterNumber.lt(before_chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .limit(3)
        .all(db)
        .await
        .map_err(|error| {
            format!("load previous chapters for single story repair failed: {error}")
        })?;

    if previous_chapters.is_empty() {
        return Ok(None);
    }

    let mut summaries = Vec::new();
    for previous_chapter in previous_chapters {
        let histories = generation_history::Entity::find()
            .filter(generation_history::Column::ChapterId.eq(Some(previous_chapter.id.clone())))
            .order_by_desc(generation_history::Column::CreatedAt)
            .limit(30)
            .all(db)
            .await
            .map_err(|error| {
                format!("load generation histories for single story repair failed: {error}")
            })?;
        let quality_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        if let Some(summary) = quality_fragments.quality_metrics_summary {
            summaries.push(summary);
        }
    }

    Ok(aggregate_story_repair_quality_summaries(&summaries, "chapter"))
}

async fn build_single_generation_runtime_state_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<Value, String> {
    let read_context = load_chapter_analysis_read_context(db, &chapter_target.chapter_id).await?;
    let quality_fragments = build_chapter_quality_metrics_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
    );

    if quality_fragments.quality_metrics_summary.is_some()
        || quality_fragments.latest_quality_metrics.is_some()
    {
        return Ok(build_single_generation_runtime_state_payload_from_parts(
            request_runtime_state,
            quality_fragments.quality_metrics_summary.as_ref(),
            quality_fragments.latest_quality_metrics.as_ref(),
        ));
    }

    let recent_history_summary = load_recent_single_generation_story_repair_quality_summary(
        db,
        &chapter_target.project_id,
        chapter_target.chapter_number,
    )
    .await?;

    Ok(build_single_generation_runtime_state_payload_from_sources(
        request_runtime_state,
        recent_history_summary.as_ref(),
        None,
        "recent_history_summary",
        "Recent history summary",
    ))
}

fn resolve_single_generation_runtime_compat_options_from_seed(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_payload: &Value,
) -> SingleChapterGenerationCompatOptions {
    restore_story_repair_compat_options_from_active_snapshot(
        &request_runtime_state.compat_options,
        active_story_repair_payload_from_runtime_state(Some(runtime_state_payload)).as_ref(),
        runtime_state_payload.get("quality_metrics_summary"),
        None,
    )
}

pub(crate) async fn start_owned_single_generation_background_write_workflow(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: SingleChapterGenerationRequest,
) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
    let now = Utc::now().naive_utc();
    let task_id = Uuid::new_v4().to_string();
    let (chapter_target, execution_input) =
        prepare_single_chapter_generation_request(db, chapter_id, user_id, &request).await?;
    let request_runtime_state = BatchGenerationRequestRuntimeState::new(
        execution_input.compat_options.clone(),
        request.model.clone(),
    );
    let runtime_state_payload = build_single_generation_runtime_state_payload(
        db,
        &chapter_target,
        &request_runtime_state,
    )
    .await
    .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
    let resolved_compat_options = resolve_single_generation_runtime_compat_options_from_seed(
        &request_runtime_state,
        &runtime_state_payload,
    );
    let runtime_input = SingleGenerationRuntimeLaunchInput {
        chapter_id: chapter_target.chapter_id.clone(),
        user_id: user_id.to_string(),
        execution_input: SingleChapterGenerationExecutionInput {
            target_word_count: execution_input.target_word_count,
            compat_options: resolved_compat_options,
            execution_config: execution_input.execution_config,
        },
    };
    let mut checkpoint = chapter_target.pending_checkpoint();
    if let (Some(checkpoint_object), Value::Object(runtime_state_object)) = (
        checkpoint.as_object_mut(),
        runtime_state_payload,
    ) {
        checkpoint_object.extend(runtime_state_object);
    }
    let response_payload = chapter_target.background_response_payload(
        &task_id,
        SINGLE_GENERATION_BACKGROUND_ESTIMATED_MINUTES,
    );
    let task = chapter_target.background_task_active_model(
        task_id.clone(),
        user_id.to_string(),
        runtime_input.execution_input.target_word_count,
        now,
    );

    task
        .insert(db)
        .await
        .map_err(|error| PrepareSingleChapterGenerationRequestError::Internal(error.to_string()))?;
    upsert_batch_generation_runtime_snapshot(db, &task_id, checkpoint)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
    dispatch_single_chapter_generation_runtime(db.clone(), task_id, runtime_input);

    Ok(response_payload)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        build_single_generation_runtime_state_payload_from_parts,
        build_single_generation_runtime_state_payload_from_sources,
        resolve_single_generation_runtime_compat_options_from_seed,
        SingleGenerationRuntimeLaunchInput,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_story_repair_quality_context_service::aggregate_story_repair_quality_summaries;
    use crate::services::chapter_single_generation_prepare_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
        SingleChapterGenerationTarget,
    };

    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        }
    }

    #[test]
    fn should_build_single_generation_background_parts_from_prepared_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: empty_compat_options(),
            execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                ai_config: AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
            },
        };

        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 5, 0)
            .expect("valid time");
        let task_id = Uuid::new_v4().to_string();
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: chapter_target.chapter_id.clone(),
            user_id: "user-1".to_string(),
            execution_input: execution_input.clone(),
        };
        let checkpoint = chapter_target.pending_checkpoint();
        let response_payload = chapter_target.background_response_payload(&task_id, 2);
        let task = chapter_target.background_task_active_model(
            task_id.clone(),
            "user-1".to_string(),
            runtime_input.execution_input.target_word_count,
            now,
        );

        assert_eq!(task_id.len(), 36);
        assert_eq!(task.total_chapters, Set(1));
        assert_eq!(
            task.current_chapter_id,
            Set(Some("chapter-7".to_string()))
        );
        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(runtime_input.execution_input.target_word_count, 2600);
        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
    }

    #[test]
    fn should_keep_single_generation_background_active_model_defaults() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(1, 5, 0)
            .expect("valid time");

        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let active = crate::services::chapter_batch_generation_task_model_service::build_batch_generation_task_active_model(
            "task-7".to_string(),
            chapter_target.project_id,
            "user-1".to_string(),
            chapter_target.chapter_number,
            1,
            json!([{
                "id": chapter_target.chapter_id,
                "chapter_number": chapter_target.chapter_number,
                "title": chapter_target.title,
            }]),
            None,
            2600,
            false,
            1,
            Some("chapter-7".to_string()),
            Some(7),
            0,
            now,
        );

        assert_eq!(active.id, Set("task-7".to_string()));
        assert_eq!(active.total_chapters, Set(1));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-7".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(7)));
        assert_eq!(active.enable_analysis, Set(false));
    }

    #[test]
    fn should_seed_single_generation_snapshot_with_runtime_state_for_resume() {
        let mut checkpoint = serde_json::json!({
            "phase": "pending",
            "status": "pending",
            "chapter_id": "chapter-7",
            "current_chapter_id": "chapter-7"
        });
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                style_id: Some(7),
                enable_analysis: true,
                enable_mcp: false,
                web_research_enabled: true,
                web_research_query: Some("旧都城的废墟".to_string()),
                narrative_perspective: Some("第一人称".to_string()),
                creative_mode: Some("balanced".to_string()),
                story_focus: Some("character".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("角色在遗迹中寻找真相".to_string()),
                quality_preset: Some("strict".to_string()),
                quality_notes: Some("强化悬念".to_string()),
                story_repair_summary: Some("补强上一章伏笔回收".to_string()),
                story_repair_targets: vec!["伏笔".to_string()],
                story_preserve_strengths: vec!["氛围".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );

        if let (
            Some(checkpoint_object),
            serde_json::Value::Object(runtime_state_object),
        ) = (
            checkpoint.as_object_mut(),
            crate::services::chapter_batch_generation_write_workflow_service::batch_generation_runtime_state_payload(&runtime_state),
        ) {
            checkpoint_object.extend(runtime_state_object);
        }

        let seeded_state = crate::services::chapter_batch_generation_write_workflow_service::parse_batch_generation_request_runtime_state(
            Some(&checkpoint),
        );
        assert_eq!(seeded_state, runtime_state);
        assert_eq!(
            checkpoint["active_story_repair_payload"]["summary"],
            "补强上一章伏笔回收"
        );
        assert_eq!(
            checkpoint["active_story_repair_payload"]["repair_targets"],
            json!(["伏笔"])
        );
    }

    #[test]
    fn should_seed_single_generation_runtime_state_from_current_chapter_quality_only() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            Some("gpt-4.1".to_string()),
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "压缩当前章节解释段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["悬念氛围"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章说明偏多",
                "failed_metrics": [{"label": "节奏"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 84}],
                "history_scope": "chapter"
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "压缩当前章节解释段"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "current_chapter_quality"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 84}],
                "history_scope": "chapter"
            })
        );
    }

    #[test]
    fn should_merge_manual_and_current_chapter_quality_into_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工长板".to_string()],
                ..empty_compat_options()
            },
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "当前质量摘要",
                "repair_targets": ["共同目标", "质量目标"],
                "preserve_strengths": ["质量长板"],
                "focus_areas": ["节奏", "冲突"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "当前章节奏不稳",
                "failed_metrics": [{"label": "节奏"}]
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
        );

        assert_eq!(payload["active_story_repair_payload"]["summary"], "手工摘要");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "质量目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工长板", "质量长板"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_current_chapter_quality"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source_label"],
            "Manual + current chapter quality"
        );
    }

    #[test]
    fn should_seed_single_generation_runtime_state_from_recent_history_summary_when_current_quality_missing() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "沿用前序章节修复建议",
                "repair_targets": ["压缩说明", "前置冲突"],
                "preserve_strengths": ["人物张力"],
                "focus_areas": ["节奏", "信息密度"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "前序章节存在节奏问题",
                "failed_metrics": [{"label": "节奏"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 81}],
                "history_scope": "chapter"
            }
        });

        let payload = super::build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            "recent_history_summary",
            "Recent history summary",
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用前序章节修复建议"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source_label"],
            "Recent history summary"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 81}],
                "history_scope": "chapter"
            })
        );
    }

    #[test]
    fn should_aggregate_recent_history_summaries_before_seeding_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            None,
        );
        let first_summary = json!({
            "overall_score": 85,
            "repair_guidance": {
                "summary": "优先压缩当前说明段",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["人物张力"],
                "focus_areas": ["pacing", "conflict"]
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let second_summary = json!({
            "overall_score": 80,
            "repair_guidance": {
                "summary": "补角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["对白辨识度"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });
        let aggregated = aggregate_story_repair_quality_summaries(
            &[first_summary, second_summary],
            "chapter",
        )
        .expect("aggregated chapter summary");

        let payload = super::build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&aggregated),
            None,
            "recent_history_summary",
            "Recent history summary",
        );
        let compat = resolve_single_generation_runtime_compat_options_from_seed(
            &runtime_state,
            &payload,
        );

        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(compat.story_repair_summary(), "优先压缩当前说明段");
        assert_eq!(
            compat.story_repair_targets(),
            &[
                "压缩说明".to_string(),
                "提前冲突".to_string(),
                "强化动机".to_string()
            ]
        );
    }

    #[test]
    fn should_merge_manual_and_recent_history_summary_into_single_generation_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工长板".to_string()],
                ..empty_compat_options()
            },
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "前序章节质量摘要",
                "repair_targets": ["共同目标", "历史目标"],
                "preserve_strengths": ["历史长板"],
                "focus_areas": ["节奏", "信息密度"]
            }
        });

        let payload = super::build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            "recent_history_summary",
            "Recent history summary",
        );

        assert_eq!(payload["active_story_repair_payload"]["summary"], "手工摘要");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工长板", "历史长板"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_recent_history_summary"
        );
    }

    #[test]
    fn should_restore_single_generation_runtime_compat_options_from_seeded_story_repair_payload() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            empty_compat_options(),
            None,
        );
        let quality_metrics_summary = json!({
            "repair_guidance": {
                "summary": "沿用单章历史修复建议",
                "repair_targets": ["压缩说明", "补强冲突"],
                "preserve_strengths": ["人物张力"]
            }
        });

        let payload = build_single_generation_runtime_state_payload_from_sources(
            &runtime_state,
            Some(&quality_metrics_summary),
            None,
            "recent_history_summary",
            "Recent history summary",
        );
        let compat = resolve_single_generation_runtime_compat_options_from_seed(
            &runtime_state,
            &payload,
        );

        assert_eq!(compat.story_repair_summary(), "沿用单章历史修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "补强冲突".to_string()]
        );
        assert_eq!(
            compat.story_preserve_strengths(),
            &["人物张力".to_string()]
        );
    }
}
