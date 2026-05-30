use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    map_apply_partial_regenerate_error, map_create_chapter_regeneration_stream_workflow_error,
    map_create_partial_regeneration_stream_workflow_error, map_load_accessible_chapter_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_regeneration_apply_service::{
    apply_owned_partial_regenerate_payload, ApplyPartialRegenerateRequest,
};
use crate::services::chapter_regeneration_prepare_service::{
    FullChapterRegenerationStreamRequest, PartialRegenerationStreamWorkflowRequest,
};
use crate::services::chapter_regeneration_query_service::{
    load_owned_regeneration_tasks_payload, RegenerationTasksQueryRequest,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    create_chapter_regeneration_stream_workflow, create_partial_regeneration_stream_workflow,
};
use crate::utils::sse::default_sse_keep_alive;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct RegenerationTasksRouteQuery {
    limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct FullChapterRegenerationStreamRouteRequest {
    pub target_word_count: Option<i64>,
    pub custom_instructions: Option<String>,
    #[serde(default)]
    pub selected_suggestion_indices: Vec<Value>,
    #[serde(default)]
    pub focus_areas: Vec<Value>,
    pub story_creation_brief: Option<String>,
    pub quality_notes: Option<String>,
    pub story_repair_summary: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub quality_preset: Option<String>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<String>,
    pub preserve_elements: Option<Value>,
    #[serde(default)]
    pub story_repair_targets: Vec<Value>,
    #[serde(default)]
    pub story_preserve_strengths: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PartialRegenerationStreamRouteRequest {
    pub selected_text: String,
    pub start_position: usize,
    pub end_position: usize,
    pub user_instructions: String,
    pub context_chars: Option<usize>,
    pub style_id: Option<i32>,
    pub length_mode: Option<String>,
    pub target_word_count: Option<usize>,
    pub enable_web_research: Option<bool>,
    pub web_research_query: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ApplyPartialRegenerateRouteRequest {
    pub new_text: Option<String>,
    pub start_position: Option<usize>,
    pub end_position: Option<usize>,
}

async fn apply_partial_regenerate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ApplyPartialRegenerateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ApplyPartialRegenerateRequest::from_route_payload(
        body.new_text,
        body.start_position,
        body.end_position,
    );
    let payload = apply_owned_partial_regenerate_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(map_apply_partial_regenerate_error)?;
    Ok(Json(payload))
}

async fn regenerate_chapter_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<FullChapterRegenerationStreamRouteRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let request = FullChapterRegenerationStreamRequest::from_route_payload(
        body.target_word_count,
        body.custom_instructions,
        body.selected_suggestion_indices,
        body.focus_areas,
        body.story_creation_brief,
        body.quality_notes,
        body.story_repair_summary,
        body.creative_mode,
        body.story_focus,
        body.quality_preset,
        body.enable_web_research,
        body.web_research_query,
        body.preserve_elements,
        body.story_repair_targets,
        body.story_preserve_strengths,
    );
    let stream =
        create_chapter_regeneration_stream_workflow(&db, &claims.sub, &chapter_id, request)
            .await
            .map_err(map_create_chapter_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn partial_regenerate_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<PartialRegenerationStreamRouteRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let request = PartialRegenerationStreamWorkflowRequest::from_route_payload(
        body.selected_text,
        body.start_position,
        body.end_position,
        body.user_instructions,
        body.context_chars,
        body.style_id,
        body.length_mode,
        body.target_word_count,
        body.enable_web_research,
        body.web_research_query,
    );
    let stream =
        create_partial_regeneration_stream_workflow(&db, &claims.sub, &chapter_id, request)
            .await
            .map_err(map_create_partial_regeneration_stream_workflow_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

async fn get_regeneration_tasks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Query(query): Query<RegenerationTasksRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = RegenerationTasksQueryRequest::from_route_limit(query.limit);
    let payload = load_owned_regeneration_tasks_payload(&db, &chapter_id, &claims.sub, request)
        .await
        .map_err(map_load_accessible_chapter_error)?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/{chapter_id}/regenerate-stream",
            post(regenerate_chapter_stream),
        )
        .route(
            "/chapters/{chapter_id}/partial-regenerate-stream",
            post(partial_regenerate_stream),
        )
        .route(
            "/chapters/{chapter_id}/apply-partial-regenerate",
            post(apply_partial_regenerate),
        )
        .route(
            "/chapters/{chapter_id}/regeneration/tasks",
            get(get_regeneration_tasks),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyPartialRegenerateRouteRequest, FullChapterRegenerationStreamRouteRequest,
        PartialRegenerationStreamRouteRequest, RegenerationTasksRouteQuery,
    };
    use crate::services::chapter_regeneration_apply_service::ApplyPartialRegenerateRequest;
    use crate::services::chapter_regeneration_prepare_service::{
        FullChapterRegenerationStreamRequest, PartialRegenerationStreamWorkflowRequest,
    };
    use crate::services::chapter_regeneration_query_service::RegenerationTasksQueryRequest;
    use serde_json::json;

    #[test]
    fn should_normalize_regeneration_tasks_limit() {
        assert_eq!(
            RegenerationTasksQueryRequest::from_route_limit(
                RegenerationTasksRouteQuery { limit: None }.limit
            )
            .limit(),
            10
        );
        assert_eq!(
            RegenerationTasksQueryRequest::from_route_limit(
                RegenerationTasksRouteQuery { limit: Some(0) }.limit
            )
            .limit(),
            1
        );
        assert_eq!(
            RegenerationTasksQueryRequest::from_route_limit(
                RegenerationTasksRouteQuery { limit: Some(25) }.limit
            )
            .limit(),
            25
        );
        assert_eq!(
            RegenerationTasksQueryRequest::from_route_limit(
                RegenerationTasksRouteQuery { limit: Some(99) }.limit
            )
            .limit(),
            50
        );
    }

    #[test]
    fn should_keep_apply_partial_regenerate_route_payload_contract() {
        let route_request = ApplyPartialRegenerateRouteRequest {
            new_text: Some("新文本".to_string()),
            start_position: Some(12),
            end_position: Some(24),
        };
        let request = ApplyPartialRegenerateRequest::from_route_payload(
            route_request.new_text,
            route_request.start_position,
            route_request.end_position,
        );

        assert_eq!(request.new_text(), Some("新文本"));
        assert_eq!(request.start_position(), 12);
        assert_eq!(request.end_position(), 24);
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_request_from_route_payload() {
        let route_request = FullChapterRegenerationStreamRouteRequest {
            target_word_count: Some(2200),
            custom_instructions: Some("补强冲突".to_string()),
            selected_suggestion_indices: vec![json!(2), json!("skip"), json!(4)],
            focus_areas: vec![json!("结构"), json!(3), json!("情绪")],
            story_creation_brief: Some("brief".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("summary".to_string()),
            creative_mode: Some("cinematic".to_string()),
            story_focus: Some("主角成长".to_string()),
            quality_preset: Some("strict".to_string()),
            enable_web_research: Some(true),
            web_research_query: Some("检索背景资料".to_string()),
            preserve_elements: Some(json!({
                "preserve_structure": true,
                "preserve_dialogues": ["对白1", 3],
                "preserve_plot_points": ["反转点"],
                "preserve_character_traits": false
            })),
            story_repair_targets: vec![json!("逻辑"), json!(7), json!("节奏")],
            story_preserve_strengths: vec![json!("悬念"), json!(false)],
        };
        let request = FullChapterRegenerationStreamRequest::from_route_payload(
            route_request.target_word_count,
            route_request.custom_instructions,
            route_request.selected_suggestion_indices,
            route_request.focus_areas,
            route_request.story_creation_brief,
            route_request.quality_notes,
            route_request.story_repair_summary,
            route_request.creative_mode,
            route_request.story_focus,
            route_request.quality_preset,
            route_request.enable_web_research,
            route_request.web_research_query,
            route_request.preserve_elements,
            route_request.story_repair_targets,
            route_request.story_preserve_strengths,
        );

        assert_eq!(request.target_word_count(), 2200);
        assert_eq!(request.custom_instructions(), "补强冲突");
        assert_eq!(
            request.selected_suggestion_indices(),
            &["2".to_string(), "4".to_string()]
        );
        assert_eq!(
            request.focus_areas(),
            &["结构".to_string(), "情绪".to_string()]
        );
        assert_eq!(request.story_creation_brief(), "brief");
        assert_eq!(request.quality_notes(), "notes");
        assert_eq!(request.story_repair_summary(), "summary");
        assert_eq!(request.creative_mode(), "cinematic");
        assert_eq!(request.story_focus(), "主角成长");
        assert_eq!(request.quality_preset(), "strict");
        assert_eq!(request.enable_web_research(), Some(true));
        assert_eq!(request.web_research_query(), Some("检索背景资料"));
        assert!(request.preserve_structure());
        assert_eq!(request.preserve_dialogues(), &["对白1".to_string()]);
        assert_eq!(request.preserve_plot_points(), &["反转点".to_string()]);
        assert!(!request.preserve_character_traits());
        assert_eq!(
            request.story_repair_targets(),
            &["逻辑".to_string(), "节奏".to_string()]
        );
        assert_eq!(request.story_preserve_strengths(), &["悬念".to_string()]);
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_request_with_defaults() {
        let route_request = FullChapterRegenerationStreamRouteRequest::default();
        let request = FullChapterRegenerationStreamRequest::from_route_payload(
            route_request.target_word_count,
            route_request.custom_instructions,
            route_request.selected_suggestion_indices,
            route_request.focus_areas,
            route_request.story_creation_brief,
            route_request.quality_notes,
            route_request.story_repair_summary,
            route_request.creative_mode,
            route_request.story_focus,
            route_request.quality_preset,
            route_request.enable_web_research,
            route_request.web_research_query,
            route_request.preserve_elements,
            route_request.story_repair_targets,
            route_request.story_preserve_strengths,
        );

        assert_eq!(request.target_word_count(), 3000);
        assert_eq!(request.custom_instructions(), "");
        assert!(request.selected_suggestion_indices().is_empty());
        assert!(request.focus_areas().is_empty());
        assert_eq!(request.story_creation_brief(), "");
        assert_eq!(request.quality_notes(), "");
        assert_eq!(request.story_repair_summary(), "");
        assert_eq!(request.creative_mode(), "");
        assert_eq!(request.story_focus(), "");
        assert_eq!(request.quality_preset(), "");
        assert_eq!(request.enable_web_research(), None);
        assert_eq!(request.web_research_query(), None);
        assert!(!request.preserve_structure());
        assert!(request.preserve_dialogues().is_empty());
        assert!(request.preserve_plot_points().is_empty());
        assert!(request.preserve_character_traits());
        assert!(request.story_repair_targets().is_empty());
        assert!(request.story_preserve_strengths().is_empty());
    }

    #[test]
    fn should_build_partial_regeneration_stream_workflow_request_from_route_payload() {
        let route_request = PartialRegenerationStreamRouteRequest {
            selected_text: "选中文本".to_string(),
            start_position: 12,
            end_position: 24,
            user_instructions: "请更紧凑一些".to_string(),
            context_chars: Some(800),
            style_id: Some(3),
            length_mode: Some("expand".to_string()),
            target_word_count: Some(1500),
            enable_web_research: Some(true),
            web_research_query: Some("检索背景资料".to_string()),
        };
        let request = PartialRegenerationStreamWorkflowRequest::from_route_payload(
            route_request.selected_text,
            route_request.start_position,
            route_request.end_position,
            route_request.user_instructions,
            route_request.context_chars,
            route_request.style_id,
            route_request.length_mode,
            route_request.target_word_count,
            route_request.enable_web_research,
            route_request.web_research_query,
        );

        assert_eq!(request.selected_text(), "选中文本");
        assert_eq!(request.start_position(), 12);
        assert_eq!(request.end_position(), 24);
        assert_eq!(request.context_chars(), 800);
        assert_eq!(request.user_instructions(), "请更紧凑一些");
        assert_eq!(request.length_mode(), Some("expand"));
        assert_eq!(request.target_word_count(), Some(1500));
        assert_eq!(request.style_id(), Some(3));
        assert!(request.web_research_enabled());
        assert_eq!(request.web_research_query(), Some("检索背景资料"));
    }
}
