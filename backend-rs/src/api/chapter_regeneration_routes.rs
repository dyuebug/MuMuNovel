use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::api::chapters_error_mapper::{
    map_apply_partial_regenerate_error, map_create_chapter_regeneration_stream_workflow_error,
    map_create_partial_regeneration_stream_workflow_error, map_load_accessible_chapter_error,
    map_regeneration_tasks_query_request_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_regeneration_apply_service::{
    apply_owned_partial_regenerate_payload,
    build_apply_partial_regenerate_request_from_route_payload, ApplyPartialRegenerateRouteRequest,
};
use crate::services::chapter_regeneration_prepare_service::{
    build_full_chapter_regeneration_stream_request_from_route_payload,
    build_partial_regeneration_stream_workflow_request_from_route_payload,
    FullChapterRegenerationStreamRouteRequest, PartialRegenerationStreamRouteRequest,
};
use crate::services::chapter_regeneration_query_service::{
    build_regeneration_tasks_query_request_from_route_query, load_owned_regeneration_tasks_payload,
    RegenerationTasksRouteQuery,
};
use crate::services::chapter_regeneration_stream_workflow_service::{
    create_chapter_regeneration_stream_workflow, create_partial_regeneration_stream_workflow,
};
use crate::utils::sse::default_sse_keep_alive;

async fn apply_partial_regenerate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<ApplyPartialRegenerateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_apply_partial_regenerate_request_from_route_payload(body);
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
    let request = build_full_chapter_regeneration_stream_request_from_route_payload(body);
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
    let request = build_partial_regeneration_stream_workflow_request_from_route_payload(body);
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
    let request = build_regeneration_tasks_query_request_from_route_query(query)
        .map_err(map_regeneration_tasks_query_request_error)?;
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
    use crate::services::chapter_regeneration_apply_service::build_apply_partial_regenerate_request_from_route_payload;
    use crate::services::chapter_regeneration_prepare_service::{
        build_full_chapter_regeneration_stream_request_from_route_payload,
        build_partial_regeneration_stream_workflow_request_from_route_payload,
    };
    use crate::services::chapter_regeneration_query_service::build_regeneration_tasks_query_request_from_route_query;
    use serde_json::json;

    #[test]
    fn should_validate_regeneration_tasks_limit_like_python_query() {
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: None,
            })
            .expect("default limit should be valid")
            .limit(),
            10
        );
        assert_eq!(
            build_regeneration_tasks_query_request_from_route_query(RegenerationTasksRouteQuery {
                limit: Some(25),
            })
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );
        assert!(build_regeneration_tasks_query_request_from_route_query(
            RegenerationTasksRouteQuery { limit: Some(0) }
        )
        .is_err());
        assert!(build_regeneration_tasks_query_request_from_route_query(
            RegenerationTasksRouteQuery { limit: Some(99) }
        )
        .is_err());
    }

    #[test]
    fn should_keep_apply_partial_regenerate_route_payload_contract() {
        let route_request = ApplyPartialRegenerateRouteRequest {
            new_text: Some(json!("新文本")),
            start_position: Some(12),
            end_position: Some(24),
        };
        let request = build_apply_partial_regenerate_request_from_route_payload(route_request);

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
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("climax".to_string()),
            quality_preset: Some("plot_drive".to_string()),
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
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);

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
        assert_eq!(request.creative_mode(), "hook");
        assert_eq!(request.story_focus(), "advance_plot");
        assert_eq!(request.plot_stage(), "climax");
        assert_eq!(request.quality_preset(), "plot_drive");
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
        let request =
            build_full_chapter_regeneration_stream_request_from_route_payload(route_request);

        assert_eq!(request.target_word_count(), 3000);
        assert_eq!(request.custom_instructions(), "");
        assert!(request.selected_suggestion_indices().is_empty());
        assert!(request.focus_areas().is_empty());
        assert_eq!(request.story_creation_brief(), "");
        assert_eq!(request.quality_notes(), "");
        assert_eq!(request.story_repair_summary(), "");
        assert_eq!(request.creative_mode(), "");
        assert_eq!(request.story_focus(), "");
        assert_eq!(request.plot_stage(), "");
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
            user_instructions: " 请更紧凑一些 ".to_string(),
            context_chars: Some(800),
            style_id: Some(3),
            length_mode: Some(" expand ".to_string()),
            target_word_count: Some(1500),
            enable_web_research: Some(true),
            web_research_query: Some(" 检索背景资料 ".to_string()),
        };
        let request =
            build_partial_regeneration_stream_workflow_request_from_route_payload(route_request);

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
