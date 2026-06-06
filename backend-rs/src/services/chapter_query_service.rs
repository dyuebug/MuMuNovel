use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{chapter, plot_analysis, story_memory};
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_generation_prerequisite_service::{
    check_chapter_generation_prerequisites, ChapterGenerationPrerequisiteCheck,
};
use crate::services::chapter_quality_metrics_query_service::load_latest_quality_metric_records_for_chapter_ids;
use crate::services::chapter_service::ChapterService;
use crate::services::chapter_story_repair_quality_context_service::normalize_quality_metrics_history_item;
use crate::services::project_quality_trend_snapshot_service::resolve_project_quality_trend_snapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadQueryPayloadError<TNotFound> {
    NotFound(TNotFound),
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterReadNotFound {
    ChapterNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectReadNotFound {
    ProjectNotFound,
}

pub type ChapterQueryPayloadError = ReadQueryPayloadError<ChapterReadNotFound>;
pub type LoadQualityTrendPayloadError = ReadQueryPayloadError<ProjectReadNotFound>;
pub type LoadAnnotationsPayloadError = LoadAccessibleChapterError;
pub type LoadNavigationPayloadError = ChapterQueryPayloadError;
pub type LoadCanGeneratePayloadError = ChapterQueryPayloadError;

const QUALITY_TREND_LIMIT_DEFAULT: usize = 12;
const QUALITY_TREND_LIMIT_MIN: i64 = 1;
const QUALITY_TREND_LIMIT_MAX: i64 = 50;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct QualityTrendRouteQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityTrendQueryRequest {
    limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityTrendQueryRequestError {
    LimitTooSmall,
    LimitTooLarge,
}

impl QualityTrendQueryRequest {
    fn from_route_query(
        route_query: QualityTrendRouteQuery,
    ) -> Result<Self, QualityTrendQueryRequestError> {
        let Some(limit) = route_query.limit else {
            return Ok(Self {
                limit: QUALITY_TREND_LIMIT_DEFAULT,
            });
        };

        if limit < QUALITY_TREND_LIMIT_MIN {
            return Err(QualityTrendQueryRequestError::LimitTooSmall);
        }
        if limit > QUALITY_TREND_LIMIT_MAX {
            return Err(QualityTrendQueryRequestError::LimitTooLarge);
        }

        Ok(Self {
            limit: limit as usize,
        })
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

pub fn build_quality_trend_query_request_from_route_query(
    route_query: QualityTrendRouteQuery,
) -> Result<QualityTrendQueryRequest, QualityTrendQueryRequestError> {
    QualityTrendQueryRequest::from_route_query(route_query)
}

fn json_object_items(value: Option<&Value>) -> Vec<&serde_json::Map<String, Value>> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

fn find_keyword_position(chapter_content: &str, keyword: Option<&Value>) -> (i64, i64) {
    let keyword_text = keyword
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if keyword_text.is_empty() || chapter_content.is_empty() {
        return (-1, 0);
    }

    match chapter_content.find(keyword_text) {
        Some(byte_position) => (
            chapter_content[..byte_position].chars().count() as i64,
            keyword_text.chars().count() as i64,
        ),
        None => (-1, 0),
    }
}

fn text_value(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or_default()
}

fn story_memory_json_array(value: &Option<Value>) -> Value {
    value
        .clone()
        .filter(Value::is_array)
        .unwrap_or_else(|| json!([]))
}

fn python_annotation_importance(value: Option<f64>) -> f64 {
    value.filter(|importance| *importance != 0.0).unwrap_or(0.5)
}

fn resolve_annotation_position_and_metadata(
    memory: &story_memory::Model,
    analysis: Option<&plot_analysis::Model>,
    chapter_content: &str,
) -> (i64, i64, serde_json::Map<String, Value>) {
    let mut position = i64::from(memory.chapter_position);
    let mut length = i64::from(memory.text_length);
    let mut metadata_extra = serde_json::Map::new();

    let Some(analysis) = analysis else {
        return (position, length, metadata_extra);
    };

    let hooks = json_object_items(analysis.hooks.as_ref());
    let foreshadows = json_object_items(analysis.foreshadows.as_ref());
    let plot_points = json_object_items(analysis.plot_points.as_ref());

    if position == -1 && !chapter_content.is_empty() {
        if memory.memory_type == "hook" {
            for hook in hooks {
                let hook_type = text_value(hook.get("type")).trim();
                if memory
                    .title
                    .as_deref()
                    .is_some_and(|title| !hook_type.is_empty() && title.contains(hook_type))
                {
                    let (found_position, found_length) =
                        find_keyword_position(chapter_content, hook.get("keyword"));
                    if found_position != -1 {
                        position = found_position;
                        length = found_length;
                    }
                    metadata_extra.insert(
                        "strength".to_string(),
                        hook.get("strength").cloned().unwrap_or_else(|| json!(5)),
                    );
                    metadata_extra.insert(
                        "position_desc".to_string(),
                        hook.get("position").cloned().unwrap_or_else(|| json!("")),
                    );
                    break;
                }
            }
        } else if memory.memory_type == "foreshadow" {
            for foreshadow in foreshadows {
                let content = text_value(foreshadow.get("content")).trim();
                if !content.is_empty() && memory.content.contains(content) {
                    let (found_position, found_length) =
                        find_keyword_position(chapter_content, foreshadow.get("keyword"));
                    if found_position != -1 {
                        position = found_position;
                        length = found_length;
                    }
                    metadata_extra.insert(
                        "foreshadow_type".to_string(),
                        foreshadow
                            .get("type")
                            .cloned()
                            .unwrap_or_else(|| json!("planted")),
                    );
                    metadata_extra.insert(
                        "strength".to_string(),
                        foreshadow
                            .get("strength")
                            .cloned()
                            .unwrap_or_else(|| json!(5)),
                    );
                    break;
                }
            }
        } else if memory.memory_type == "plot_point" {
            for plot_point in plot_points {
                let content = text_value(plot_point.get("content")).trim();
                if !content.is_empty() && memory.content.contains(content) {
                    let (found_position, found_length) =
                        find_keyword_position(chapter_content, plot_point.get("keyword"));
                    if found_position != -1 {
                        position = found_position;
                        length = found_length;
                    }
                    break;
                }
            }
        }
    } else if memory.memory_type == "hook" {
        for hook in hooks {
            let hook_type = text_value(hook.get("type")).trim();
            if memory
                .title
                .as_deref()
                .is_some_and(|title| !hook_type.is_empty() && title.contains(hook_type))
            {
                metadata_extra.insert(
                    "strength".to_string(),
                    hook.get("strength").cloned().unwrap_or_else(|| json!(5)),
                );
                metadata_extra.insert(
                    "position_desc".to_string(),
                    hook.get("position").cloned().unwrap_or_else(|| json!("")),
                );
                break;
            }
        }
    } else if memory.memory_type == "foreshadow" {
        for foreshadow in foreshadows {
            let content = text_value(foreshadow.get("content")).trim();
            if !content.is_empty() && memory.content.contains(content) {
                metadata_extra.insert(
                    "foreshadow_type".to_string(),
                    foreshadow
                        .get("type")
                        .cloned()
                        .unwrap_or_else(|| json!("planted")),
                );
                metadata_extra.insert(
                    "strength".to_string(),
                    foreshadow
                        .get("strength")
                        .cloned()
                        .unwrap_or_else(|| json!(5)),
                );
                break;
            }
        }
    }

    (position, length, metadata_extra)
}

fn annotation_item_payload(
    chapter: &chapter::Model,
    analysis: Option<&plot_analysis::Model>,
    memory: &story_memory::Model,
) -> Value {
    let chapter_content = chapter.content.as_deref().unwrap_or_default();
    let (position, length, metadata_extra) =
        resolve_annotation_position_and_metadata(memory, analysis, chapter_content);
    let mut metadata = serde_json::Map::from_iter([
        ("is_foreshadow".to_string(), json!(memory.is_foreshadow)),
        (
            "related_characters".to_string(),
            story_memory_json_array(&memory.related_characters),
        ),
        (
            "related_locations".to_string(),
            story_memory_json_array(&memory.related_locations),
        ),
    ]);
    metadata.extend(metadata_extra);

    json!({
        "id": memory.id,
        "type": memory.memory_type,
        "title": memory.title,
        "content": memory.content,
        "importance": python_annotation_importance(memory.importance_score),
        "position": position,
        "length": length,
        "tags": story_memory_json_array(&memory.tags),
        "metadata": metadata,
    })
}

fn annotations_payload(
    chapter: &chapter::Model,
    analysis: Option<&plot_analysis::Model>,
    memories: &[story_memory::Model],
) -> Value {
    let annotations = memories
        .iter()
        .map(|memory| annotation_item_payload(chapter, analysis, memory))
        .collect::<Vec<_>>();

    json!({
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "title": chapter.title,
        "word_count": chapter.word_count,
        "annotations": annotations,
        "has_analysis": analysis.is_some(),
        "summary": {
            "total_annotations": annotations.len(),
            "hooks": annotations.iter().filter(|item| item["type"] == "hook").count(),
            "foreshadows": annotations.iter().filter(|item| item["type"] == "foreshadow").count(),
            "plot_points": annotations.iter().filter(|item| item["type"] == "plot_point").count(),
            "character_events": annotations.iter().filter(|item| item["type"] == "character_event").count(),
        }
    })
}

fn navigation_payload(
    previous: Option<chapter::Model>,
    current: Option<chapter::Model>,
    next: Option<chapter::Model>,
) -> Value {
    json!({
        "previous": previous,
        "current": current,
        "next": next,
    })
}

fn previous_chapter_info_payload(chapter: &chapter::Model) -> Value {
    json!({
        "id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "title": chapter.title,
        "has_content": chapter.content.as_ref().is_some_and(|content| !content.trim().is_empty()),
        "word_count": chapter.word_count,
    })
}

fn can_generate_payload(
    chapter: &chapter::Model,
    prerequisite: &ChapterGenerationPrerequisiteCheck,
) -> Value {
    json!({
        "can_generate": prerequisite.can_generate,
        "reason": if prerequisite.can_generate {
            ""
        } else {
            prerequisite.error_message.as_str()
        },
        "previous_chapters": prerequisite
            .previous_chapters
            .iter()
            .map(previous_chapter_info_payload)
            .collect::<Vec<_>>(),
        "chapter_number": chapter.chapter_number,
    })
}

fn quality_trend_items_and_history(
    chapters: &[chapter::Model],
    records_by_chapter: &std::collections::HashMap<
        String,
        crate::services::chapter_quality_metrics_query_service::LatestQualityMetricRecord,
    >,
    limit: usize,
) -> (Vec<Value>, Vec<Value>, Option<String>) {
    let mut items = Vec::new();
    let mut metrics_history = Vec::new();
    let mut last_generated_at_dt = None;
    let mut last_generated_at = None;

    for chapter in chapters {
        let Some(record) = records_by_chapter.get(&chapter.id) else {
            continue;
        };

        let normalized_metrics =
            normalize_quality_metrics_history_item(&record.latest_quality_metrics, "batch")
                .unwrap_or_else(|| record.latest_quality_metrics.clone());
        metrics_history.push(normalized_metrics.clone());
        if let Some(generated_at_dt) = record.generated_at_dt {
            if last_generated_at_dt.is_none_or(|current| generated_at_dt > current) {
                last_generated_at_dt = Some(generated_at_dt);
                last_generated_at = record.generated_at.clone();
            }
        } else if let Some(generated_at) = record.generated_at.as_ref() {
            if last_generated_at
                .as_ref()
                .is_none_or(|current| generated_at > current)
            {
                last_generated_at = Some(generated_at.clone());
            }
        }
        items.push(json!({
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "title": chapter.title,
            "status": chapter.status,
            "history_id": record.history_id,
            "generated_at": record.generated_at,
            "latest_quality_metrics": normalized_metrics,
        }));
    }

    if limit > 0 && items.len() > limit {
        let keep_from = items.len() - limit;
        items = items.split_off(keep_from);
        metrics_history = metrics_history.split_off(metrics_history.len() - limit);
    }

    (items, metrics_history, last_generated_at)
}

fn quality_trend_payload(
    project_id: &str,
    chapters: &[chapter::Model],
    items: Vec<Value>,
    analyzed_chapters: i64,
    quality_metrics_summary: Option<Value>,
) -> Value {
    let total_chapters = chapters.len() as i64;

    json!({
        "project_id": project_id,
        "has_metrics": analyzed_chapters > 0,
        "total_chapters": total_chapters,
        "analyzed_chapters": analyzed_chapters,
        "items": items,
        "quality_metrics_summary": quality_metrics_summary,
    })
}

pub async fn load_navigation_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadNavigationPayloadError> {
    match ChapterService::navigation(db, chapter_id, user_id).await {
        Ok(Some((previous, current, next))) => Ok(navigation_payload(previous, current, next)),
        Ok(None) => Err(ReadQueryPayloadError::NotFound(
            ChapterReadNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ReadQueryPayloadError::Internal(error)),
    }
}

pub async fn load_can_generate_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadCanGeneratePayloadError> {
    let chapter = match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => chapter,
        Ok(None) => {
            return Err(ReadQueryPayloadError::NotFound(
                ChapterReadNotFound::ChapterNotFound,
            ))
        }
        Err(error) => return Err(ReadQueryPayloadError::Internal(error)),
    };

    let prerequisite = check_chapter_generation_prerequisites(db, &chapter)
        .await
        .map_err(ReadQueryPayloadError::Internal)?;

    Ok(can_generate_payload(&chapter, &prerequisite))
}

pub async fn load_annotations_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadAnnotationsPayloadError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id).await?;
    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| LoadAccessibleChapterError::Internal(error.to_string()))?;
    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ChapterId.eq(chapter_id))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .all(db)
        .await
        .map_err(|error| LoadAccessibleChapterError::Internal(error.to_string()))?;

    Ok(annotations_payload(&chapter, analysis.as_ref(), &memories))
}

pub async fn load_quality_trend_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: QualityTrendQueryRequest,
) -> Result<Value, LoadQualityTrendPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => {
            let chapter_ids = chapters
                .iter()
                .map(|chapter| chapter.id.clone())
                .collect::<Vec<_>>();
            let records_by_chapter =
                load_latest_quality_metric_records_for_chapter_ids(db, &chapter_ids)
                    .await
                    .map_err(ReadQueryPayloadError::Internal)?;
            let (items, metrics_history, last_generated_at) =
                quality_trend_items_and_history(&chapters, &records_by_chapter, request.limit());
            let analyzed_chapters = metrics_history.len() as i64;
            let resolved_snapshot = resolve_project_quality_trend_snapshot(
                project_id,
                request.limit(),
                &items,
                &metrics_history,
                chapters.len() as i64,
                analyzed_chapters,
                last_generated_at.as_deref(),
            )
            .await
            .map_err(ReadQueryPayloadError::Internal)?;

            Ok(quality_trend_payload(
                project_id,
                &chapters,
                resolved_snapshot.items,
                analyzed_chapters,
                resolved_snapshot.summary,
            ))
        }
        Ok(None) => Err(ReadQueryPayloadError::NotFound(
            ProjectReadNotFound::ProjectNotFound,
        )),
        Err(error) => Err(ReadQueryPayloadError::Internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use serde_json::{json, Value};
    use std::collections::HashMap;

    use crate::models::{chapter, plot_analysis, story_memory};
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_generation_prerequisite_service::ChapterGenerationPrerequisiteCheck;
    use crate::services::chapter_quality_metrics_query_service::LatestQualityMetricRecord;

    use super::{
        annotation_item_payload, annotations_payload,
        build_quality_trend_query_request_from_route_query, can_generate_payload,
        find_keyword_position, navigation_payload, previous_chapter_info_payload,
        python_annotation_importance, quality_trend_items_and_history, quality_trend_payload,
        resolve_annotation_position_and_metadata, ChapterReadNotFound, LoadAnnotationsPayloadError,
        LoadCanGeneratePayloadError, LoadNavigationPayloadError, LoadQualityTrendPayloadError,
        ProjectReadNotFound, QualityTrendQueryRequestError, QualityTrendRouteQuery,
        ReadQueryPayloadError,
    };

    fn chapter_model(id: &str, number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number: number,
            title: format!("第{}章", number),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_navigation_payload() {
        let payload = navigation_payload(
            Some(chapter_model("chapter-1", 1)),
            Some(chapter_model("chapter-2", 2)),
            None,
        );

        assert_eq!(payload["previous"]["id"], "chapter-1");
        assert_eq!(payload["current"]["id"], "chapter-2");
        assert!(payload["next"].is_null());
    }

    #[test]
    fn should_build_can_generate_payload() {
        let chapter = chapter_model("chapter-3", 3);
        let previous_chapters = vec![
            chapter_model("chapter-1", 1),
            chapter::Model {
                content: Some("   ".to_string()),
                word_count: 0,
                ..chapter_model("chapter-2", 2)
            },
        ];
        let prerequisite = ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: "前置章节尚未完成: 2 章".to_string(),
            previous_chapters,
        };

        let payload = can_generate_payload(&chapter, &prerequisite);

        assert_eq!(payload["can_generate"], false);
        assert_eq!(payload["reason"], "前置章节尚未完成: 2 章");
        assert_eq!(payload["chapter_number"], 3);
        assert_eq!(payload["previous_chapters"][0]["id"], "chapter-1");
        assert_eq!(payload["previous_chapters"][0]["has_content"], true);
        assert_eq!(payload["previous_chapters"][1]["id"], "chapter-2");
        assert_eq!(payload["previous_chapters"][1]["has_content"], false);
    }

    #[test]
    fn should_clear_can_generate_reason_when_python_prerequisite_passes() {
        let chapter = chapter_model("chapter-1", 1);
        let prerequisite = ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: "ignored".to_string(),
            previous_chapters: Vec::new(),
        };

        let payload = can_generate_payload(&chapter, &prerequisite);

        assert_eq!(payload["can_generate"], true);
        assert_eq!(payload["reason"], "");
        assert_eq!(payload["chapter_number"], 1);
        assert_eq!(
            payload["previous_chapters"].as_array().map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn should_build_previous_chapter_info_payload_like_python_route() {
        let payload = previous_chapter_info_payload(&chapter_model("chapter-1", 1));

        assert_eq!(payload["id"], "chapter-1");
        assert_eq!(payload["chapter_number"], 1);
        assert_eq!(payload["title"], "第1章");
        assert_eq!(payload["has_content"], true);
        assert_eq!(payload["word_count"], 2);
    }

    fn story_memory_model(id: &str, memory_type: &str, title: Option<&str>) -> story_memory::Model {
        story_memory::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            memory_type: memory_type.to_string(),
            title: title.map(str::to_string),
            content: "主角发现暗门".to_string(),
            full_context: None,
            related_characters: Some(json!(["主角"])),
            related_locations: Some(json!(["旧宅"])),
            tags: Some(json!(["悬疑"])),
            importance_score: Some(0.8),
            story_timeline: 1,
            chapter_position: -1,
            text_length: 0,
            is_foreshadow: 1,
            foreshadow_resolved_at: None,
            foreshadow_strength: None,
            vector_id: None,
            embedding_model: None,
            created_at: Some(NaiveDateTime::default()),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    fn plot_analysis_model() -> plot_analysis::Model {
        plot_analysis::Model {
            id: "analysis-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            plot_stage: None,
            conflict_level: None,
            conflict_types: None,
            emotional_tone: None,
            emotional_intensity: None,
            emotional_curve: None,
            hooks: Some(json!([
                {"type": "悬念", "keyword": "暗门", "strength": 8, "position": "中段"}
            ])),
            hooks_count: 1,
            hooks_avg_strength: Some(8.0),
            foreshadows: Some(json!([
                {"content": "暗门", "keyword": "暗门", "type": "planted", "strength": 7}
            ])),
            foreshadows_planted: 1,
            foreshadows_resolved: 0,
            plot_points: Some(json!([
                {"content": "暗门", "keyword": "暗门"}
            ])),
            plot_points_count: 1,
            character_states: None,
            scenes: None,
            pacing: None,
            overall_quality_score: None,
            pacing_score: None,
            engagement_score: None,
            coherence_score: None,
            analysis_report: None,
            suggestions: None,
            word_count: None,
            dialogue_ratio: None,
            description_ratio: None,
            created_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_empty_annotations_payload() {
        let payload = annotations_payload(&chapter_model("chapter-1", 1), None, &[]);

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["chapter_number"], 1);
        assert_eq!(payload["title"], "第1章");
        assert_eq!(payload["word_count"], 2);
        assert_eq!(payload["annotations"].as_array().map(Vec::len), Some(0));
        assert_eq!(payload["has_analysis"], false);
        assert_eq!(payload["summary"]["total_annotations"], 0);
    }

    #[test]
    fn should_build_annotations_payload_from_story_memories_like_python_route() {
        let mut chapter = chapter_model("chapter-1", 1);
        chapter.content = Some("主角推开暗门，发现旧宅深处的秘密。".to_string());
        let analysis = plot_analysis_model();
        let memories = vec![
            story_memory_model("memory-1", "hook", Some("悬念线索")),
            story_memory_model("memory-2", "foreshadow", Some("暗门伏笔")),
        ];

        let payload = annotations_payload(&chapter, Some(&analysis), &memories);

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["has_analysis"], true);
        assert_eq!(payload["annotations"].as_array().map(Vec::len), Some(2));
        assert_eq!(payload["annotations"][0]["id"], "memory-1");
        assert_eq!(payload["annotations"][0]["type"], "hook");
        assert_eq!(payload["annotations"][0]["importance"], 0.8);
        assert_eq!(
            payload["annotations"][0]["metadata"]["related_characters"][0],
            "主角"
        );
        assert_eq!(payload["annotations"][0]["metadata"]["strength"], 8);
        assert_eq!(
            payload["annotations"][0]["metadata"]["position_desc"],
            "中段"
        );
        assert_eq!(
            payload["annotations"][1]["metadata"]["foreshadow_type"],
            "planted"
        );
        assert_eq!(payload["summary"]["total_annotations"], 2);
        assert_eq!(payload["summary"]["hooks"], 1);
        assert_eq!(payload["summary"]["foreshadows"], 1);
    }

    #[test]
    fn should_find_keyword_position_by_character_offset_like_python_string_find() {
        let (position, length) = find_keyword_position("主角推开暗门", Some(&json!("暗门")));

        assert_eq!(position, 4);
        assert_eq!(length, 2);
    }

    #[test]
    fn should_resolve_annotation_position_metadata_from_existing_memory_position() {
        let mut memory = story_memory_model("memory-1", "hook", Some("悬念线索"));
        memory.chapter_position = 3;
        memory.text_length = 2;
        let analysis = plot_analysis_model();

        let (position, length, metadata) =
            resolve_annotation_position_and_metadata(&memory, Some(&analysis), "主角推开暗门");

        assert_eq!(position, 3);
        assert_eq!(length, 2);
        assert_eq!(metadata.get("strength"), Some(&json!(8)));
    }

    #[test]
    fn should_build_annotation_item_with_default_memory_values_like_python() {
        let chapter = chapter_model("chapter-1", 1);
        let mut memory = story_memory_model("memory-1", "character_event", None);
        memory.importance_score = None;
        memory.tags = None;
        memory.related_characters = None;
        memory.related_locations = None;

        let payload = annotation_item_payload(&chapter, None, &memory);

        assert_eq!(payload["id"], "memory-1");
        assert_eq!(payload["type"], "character_event");
        assert_eq!(payload["importance"], 0.5);
        assert_eq!(payload["tags"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            payload["metadata"]["related_characters"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn should_default_zero_annotation_importance_like_python_or_expression() {
        assert_eq!(python_annotation_importance(Some(0.0)), 0.5);
        assert_eq!(python_annotation_importance(None), 0.5);
        assert_eq!(python_annotation_importance(Some(0.8)), 0.8);
    }

    #[test]
    fn should_build_quality_trend_payload() {
        let chapters = vec![
            chapter_model("chapter-1", 1),
            chapter_model("chapter-2", 2),
            chapter_model("chapter-3", 3),
        ];
        let records_by_chapter = HashMap::from([
            (
                "chapter-1".to_string(),
                LatestQualityMetricRecord {
                    chapter_id: "chapter-1".to_string(),
                    latest_quality_metrics: json!({
                        "overall_score": 78.0,
                        "conflict_chain_hit_rate": 62.0,
                        "rule_grounding_hit_rate": 80.0,
                        "outline_alignment_rate": 64.0,
                        "dialogue_naturalness_rate": 79.0,
                        "opening_hook_rate": 72.0,
                        "payoff_chain_rate": 58.0,
                        "cliffhanger_rate": 84.0,
                        "pacing_score": 6.9,
                        "quality_runtime_context": {
                            "plot_stage": "development",
                            "chapter_count": 12,
                            "current_chapter_number": 9,
                            "foreshadow_payoff_plan": ["王城密钥"],
                            "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约尚未兑现"],
                        }
                    }),
                    history_id: "history-1".to_string(),
                    generated_at: Some("2026-05-17T12:00:00".to_string()),
                    generated_at_dt: None,
                },
            ),
            (
                "chapter-2".to_string(),
                LatestQualityMetricRecord {
                    chapter_id: "chapter-2".to_string(),
                    latest_quality_metrics: json!({
                        "overall_score": 77.0,
                        "conflict_chain_hit_rate": 60.0,
                        "rule_grounding_hit_rate": 78.0,
                        "outline_alignment_rate": 63.0,
                        "dialogue_naturalness_rate": 77.0,
                        "opening_hook_rate": 70.0,
                        "payoff_chain_rate": 56.0,
                        "cliffhanger_rate": 86.0,
                        "pacing_score": 6.7,
                        "quality_runtime_context": {
                            "plot_stage": "development",
                            "chapter_count": 12,
                            "current_chapter_number": 10,
                            "foreshadow_payoff_plan": ["王城密钥", "苏离盟约"],
                            "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约尚未兑现", "档案馆真相仍被压住"],
                        }
                    }),
                    history_id: "history-2".to_string(),
                    generated_at: Some("2026-05-17T12:01:00".to_string()),
                    generated_at_dt: None,
                },
            ),
        ]);

        let (items, metrics_history, last_generated_at) =
            quality_trend_items_and_history(&chapters, &records_by_chapter, 2);
        let analyzed_chapters = metrics_history.len() as i64;
        let mut quality_metrics_summary = crate::services::chapter_story_repair_quality_context_service::build_quality_metrics_summary_from_state(
            crate::services::chapter_story_repair_quality_context_service::build_quality_metrics_summary_state_from_history(&metrics_history, "batch").as_ref(),
            &metrics_history,
            "batch",
        );
        if let Some(summary) = quality_metrics_summary
            .as_mut()
            .and_then(Value::as_object_mut)
        {
            summary.insert("total_chapters".to_string(), json!(chapters.len() as i64));
            summary.insert("analyzed_chapters".to_string(), json!(analyzed_chapters));
            summary.insert(
                "last_generated_at".to_string(),
                last_generated_at.map(Value::from).unwrap_or(Value::Null),
            );
        }
        let payload = quality_trend_payload(
            "project-1",
            &chapters,
            items,
            analyzed_chapters,
            quality_metrics_summary,
        );

        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["has_metrics"], true);
        assert_eq!(payload["total_chapters"], 3);
        assert_eq!(payload["analyzed_chapters"], 2);
        assert_eq!(payload["items"].as_array().map(Vec::len), Some(2));
        assert_eq!(payload["items"][0]["chapter_id"], "chapter-1");
        assert_eq!(payload["items"][1]["chapter_number"], 2);
        assert_eq!(
            payload["items"][1]["latest_quality_metrics"]["repair_guidance"]["summary"].is_string(),
            true
        );
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary"]["total_chapters"], 3);
        assert_eq!(payload["quality_metrics_summary"]["analyzed_chapters"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["last_generated_at"],
            "2026-05-17T12:01:00"
        );
        assert!(payload["quality_metrics_summary"]["pacing_imbalance"]["status"].is_string());
        assert!(
            payload["quality_metrics_summary"]["volume_goal_completion"]["completion_rate"]
                .is_number()
        );
        assert!(
            payload["quality_metrics_summary"]["foreshadow_payoff_delay"]["delay_index"]
                .is_number()
        );
        assert!(
            payload["quality_metrics_summary"]["repair_effectiveness"]["success_rate"].is_number()
        );
    }

    #[test]
    fn should_keep_quality_trend_recent_metrics_window_for_python_limit_contract() {
        let chapters = vec![
            chapter_model("chapter-1", 1),
            chapter_model("chapter-2", 2),
            chapter_model("chapter-3", 3),
        ];
        let records_by_chapter = HashMap::from([
            (
                "chapter-1".to_string(),
                LatestQualityMetricRecord {
                    chapter_id: "chapter-1".to_string(),
                    latest_quality_metrics: json!({"overall_score": 70.0}),
                    history_id: "history-1".to_string(),
                    generated_at: Some("2026-05-17T12:00:00".to_string()),
                    generated_at_dt: None,
                },
            ),
            (
                "chapter-2".to_string(),
                LatestQualityMetricRecord {
                    chapter_id: "chapter-2".to_string(),
                    latest_quality_metrics: json!({"overall_score": 75.0}),
                    history_id: "history-2".to_string(),
                    generated_at: Some("2026-05-17T12:01:00".to_string()),
                    generated_at_dt: None,
                },
            ),
            (
                "chapter-3".to_string(),
                LatestQualityMetricRecord {
                    chapter_id: "chapter-3".to_string(),
                    latest_quality_metrics: json!({"overall_score": 80.0}),
                    history_id: "history-3".to_string(),
                    generated_at: Some("2026-05-17T12:02:00".to_string()),
                    generated_at_dt: None,
                },
            ),
        ]);

        let (items, metrics_history, _) =
            quality_trend_items_and_history(&chapters, &records_by_chapter, 2);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["chapter_id"], "chapter-2");
        assert_eq!(items[1]["chapter_id"], "chapter-3");
        assert_eq!(metrics_history.len(), 2);
        assert_eq!(metrics_history[0]["overall_score"], 75.0);
        assert_eq!(metrics_history[1]["overall_score"], 80.0);
    }

    #[test]
    fn should_validate_quality_trend_query_limit_like_python_query() {
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: None
            })
            .expect("default limit should be valid")
            .limit(),
            12
        );
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(50)
            })
            .expect("upper bound should be valid")
            .limit(),
            50
        );
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(0)
            }),
            Err(QualityTrendQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(-1)
            }),
            Err(QualityTrendQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(51)
            }),
            Err(QualityTrendQueryRequestError::LimitTooLarge)
        );
    }

    #[test]
    fn should_alias_navigation_query_error_owner() {
        let error: LoadNavigationPayloadError =
            ReadQueryPayloadError::NotFound(ChapterReadNotFound::ChapterNotFound);

        assert!(matches!(
            error,
            ReadQueryPayloadError::NotFound(ChapterReadNotFound::ChapterNotFound)
        ));
    }

    #[test]
    fn should_alias_can_generate_query_error_owner() {
        let error: LoadCanGeneratePayloadError =
            ReadQueryPayloadError::Internal("boom".to_string());

        assert!(matches!(
            error,
            ReadQueryPayloadError::Internal(detail) if detail == "boom"
        ));
    }

    #[test]
    fn should_alias_access_not_found_error_for_annotations_query() {
        let error: LoadAnnotationsPayloadError = LoadAccessibleChapterError::NotFoundOrAccessDenied;

        assert_eq!(error, LoadAccessibleChapterError::NotFoundOrAccessDenied);
    }

    #[test]
    fn should_alias_access_internal_error_for_annotations_query() {
        let error: LoadAnnotationsPayloadError =
            LoadAccessibleChapterError::Internal("boom".to_string());

        assert_eq!(
            error,
            LoadAccessibleChapterError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_alias_quality_trend_query_error_owner() {
        let error: LoadQualityTrendPayloadError =
            ReadQueryPayloadError::NotFound(ProjectReadNotFound::ProjectNotFound);

        assert!(matches!(
            error,
            ReadQueryPayloadError::NotFound(ProjectReadNotFound::ProjectNotFound)
        ));
    }

    #[test]
    fn should_keep_quality_trend_internal_detail() {
        let error: LoadQualityTrendPayloadError =
            ReadQueryPayloadError::Internal("boom".to_string());

        assert!(matches!(
            error,
            ReadQueryPayloadError::Internal(detail) if detail == "boom"
        ));
    }
}
