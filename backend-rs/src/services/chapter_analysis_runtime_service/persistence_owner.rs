use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, plot_analysis};
use crate::services::chapter_analysis_runtime_service::analysis_payload_owner::{
    build_chapter_analysis_quality_metrics_payload, build_chapter_analysis_report, json_f64,
    json_i32, replace_analysis_memories_after_persist, sync_analysis_foreshadows_after_persist,
};
use crate::services::chapter_analysis_runtime_service::state_sync_owner::{
    sync_analysis_character_states_after_persist, sync_analysis_organization_states_after_persist,
};
use crate::services::chapter_analysis_service::{
    apply_analysis_task_state_by_id, AnalysisTaskStage,
};
use crate::services::chapter_single_generation_result_lifecycle_service::update_latest_generated_chapter_history_quality_metrics;

pub(crate) async fn persist_chapter_analysis_result(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    task_id: &str,
    payload: &Value,
) -> Result<Value, String> {
    let now = Utc::now().naive_utc();
    let scores = payload.get("scores").cloned().unwrap_or(Value::Null);
    let conflict = payload.get("conflict").cloned().unwrap_or(Value::Null);
    let emotional_arc = payload.get("emotional_arc").cloned().unwrap_or(Value::Null);
    let quality_metrics_payload = build_chapter_analysis_quality_metrics_payload(payload);

    let analysis = plot_analysis::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(chapter_model.id.clone()),
        plot_stage: Set(payload
            .get("plot_stage")
            .and_then(Value::as_str)
            .map(str::to_string)),
        conflict_level: Set(Some(json_i32(
            conflict.get("level").and_then(Value::as_i64),
        ))),
        conflict_types: Set(conflict.get("types").cloned()),
        emotional_tone: Set(emotional_arc
            .get("primary_emotion")
            .and_then(Value::as_str)
            .map(str::to_string)),
        emotional_intensity: Set(json_f64(
            emotional_arc.get("intensity").and_then(Value::as_f64),
        )),
        emotional_curve: Set(emotional_arc
            .get("curve")
            .cloned()
            .or_else(|| emotional_arc.get("secondary_emotions").cloned())),
        hooks: Set(payload.get("hooks").cloned()),
        hooks_count: Set(payload
            .get("hooks")
            .and_then(Value::as_array)
            .map(|items| items.len() as i32)
            .unwrap_or(0)),
        hooks_avg_strength: Set(payload
            .get("hooks")
            .and_then(Value::as_array)
            .and_then(|items| {
                let strengths = items
                    .iter()
                    .filter_map(|item| item.get("strength").and_then(Value::as_f64))
                    .collect::<Vec<_>>();
                if strengths.is_empty() {
                    None
                } else {
                    Some(strengths.iter().sum::<f64>() / strengths.len() as f64)
                }
            })),
        foreshadows: Set(payload.get("foreshadows").cloned()),
        foreshadows_planted: Set(payload
            .get("foreshadows")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("planted"))
                    .count() as i32
            })
            .unwrap_or(0)),
        foreshadows_resolved: Set(payload
            .get("foreshadows")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("resolved"))
                    .count() as i32
            })
            .unwrap_or(0)),
        plot_points: Set(payload.get("plot_points").cloned()),
        plot_points_count: Set(payload
            .get("plot_points")
            .and_then(Value::as_array)
            .map(|items| items.len() as i32)
            .unwrap_or(0)),
        character_states: Set(payload.get("character_states").cloned()),
        scenes: Set(payload
            .get("scenes")
            .cloned()
            .or_else(|| payload.get("serial_rhythm").cloned())),
        pacing: Set(payload
            .get("pacing")
            .and_then(Value::as_str)
            .map(str::to_string)),
        overall_quality_score: Set(json_f64(scores.get("overall").and_then(Value::as_f64))),
        pacing_score: Set(json_f64(scores.get("pacing").and_then(Value::as_f64))),
        engagement_score: Set(json_f64(scores.get("engagement").and_then(Value::as_f64))),
        coherence_score: Set(json_f64(scores.get("coherence").and_then(Value::as_f64))),
        analysis_report: Set(
            build_chapter_analysis_report(payload).or_else(|| Some(payload.to_string()))
        ),
        suggestions: Set(payload.get("suggestions").cloned()),
        word_count: Set(Some(chapter_model.word_count)),
        dialogue_ratio: Set(json_f64(
            payload.get("dialogue_ratio").and_then(Value::as_f64),
        )),
        description_ratio: Set(json_f64(
            payload.get("description_ratio").and_then(Value::as_f64),
        )),
        created_at: Set(Some(now)),
    };

    let saved_analysis = analysis
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;

    let memories_count =
        replace_analysis_memories_after_persist(db, user_id, chapter_model, payload).await?;
    let foreshadow_stats = sync_analysis_foreshadows_after_persist(db, chapter_model, payload)
        .await
        .unwrap_or_else(|| {
            json!({
                "planted_count": 0,
                "resolved_count": 0,
                "created_count": 0,
            })
        });
    sync_analysis_character_states_after_persist(db, chapter_model, payload).await;
    sync_analysis_organization_states_after_persist(db, chapter_model, payload).await;

    if let Some(quality_metrics_payload) = quality_metrics_payload.as_ref() {
        let chapter_content = chapter_model.content.clone().unwrap_or_default();
        let _ = update_latest_generated_chapter_history_quality_metrics(
            db,
            &chapter_model.id,
            &chapter_content,
            quality_metrics_payload,
        )
        .await;
    }

    if !task_id.trim().is_empty() {
        let _ =
            apply_analysis_task_state_by_id(db, task_id, AnalysisTaskStage::Completed, None, now)
                .await
                .map_err(|error| error.to_string())?;
    }

    Ok(json!({
        "analysis": saved_analysis,
        "quality_metrics": quality_metrics_payload,
        "memories_count": memories_count,
        "foreshadow_stats": foreshadow_stats,
    }))
}

pub(crate) fn build_chapter_analysis_persistence_owner_contract() -> Value {
    json!({
        "owner": "chapter_analysis_runtime_service::persistence_owner",
        "scope": "plot_analysis_record_persist_memory_refresh_foreshadow_sync_character_and_organization_state_sync_quality_history_patch_and_task_completion",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/persistence_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/analysis_payload_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/state_sync_owner.rs"
        ],
        "behavior_contract": {
            "entrypoint": "persist_chapter_analysis_result",
            "analysis_record_owner": "plot_analysis::ActiveModel insert",
            "quality_metrics_owner": "build_chapter_analysis_quality_metrics_payload",
            "memory_refresh_owner": "replace_analysis_memories_after_persist",
            "foreshadow_sync_owner": "sync_analysis_foreshadows_after_persist",
            "character_state_sync_owner": "sync_analysis_character_states_after_persist",
            "organization_state_sync_owner": "sync_analysis_organization_states_after_persist",
            "task_completion_owner": "apply_analysis_task_state_by_id(... Completed ...)"
        },
        "validation_boundary": [
            "cargo test chapter_analysis_runtime_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "python_source_map_retained": true,
            "same_round_python_edit_required": false
        }
    })
}
