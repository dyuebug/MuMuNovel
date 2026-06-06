use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Json, Sse},
    routing::post,
    Router,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::api::chapter_generation_error_mapper::map_single_chapter_generation_request_error;
use crate::services::auth::Claims;
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationRouteRequest;
use crate::services::chapter_single_generation_stream_workflow_service::create_single_generation_stream_workflow_from_route_payload;
use crate::services::chapter_single_generation_write_workflow_service::start_owned_single_generation_background_write_workflow_from_route_payload;
use crate::utils::sse::default_sse_keep_alive;

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    body: Option<Json<SingleChapterGenerationRouteRequest>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = start_owned_single_generation_background_write_workflow_from_route_payload(
        &db,
        &chapter_id,
        &claims.sub,
        body.map(|Json(payload)| payload).unwrap_or_default(),
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Json(result))
}

async fn generate_chapter_content_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    body: Option<Json<SingleChapterGenerationRouteRequest>>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let stream = create_single_generation_stream_workflow_from_route_payload(
        db.clone(),
        claims.sub.clone(),
        chapter_id,
        body.map(|Json(payload)| payload).unwrap_or_default(),
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Sse::new(stream).keep_alive(default_sse_keep_alive()))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            "/chapters/{chapter_id}/generate-stream",
            post(generate_chapter_content_stream),
        )
        .route(
            "/chapters/{chapter_id}/generate-background",
            post(generate_chapter_content_background),
        )
}

#[cfg(test)]
mod tests {
    use crate::services::chapter_single_generation_prepare_service::build_single_chapter_generation_request_from_route_payload;

    use super::SingleChapterGenerationRouteRequest;

    #[test]
    fn should_keep_single_chapter_generation_route_payload_contract() {
        let route_request = SingleChapterGenerationRouteRequest {
            style_id: Some(7),
            target_word_count: Some(1800),
            model: Some("gpt-test".to_string()),
            enable_analysis: Some(true),
            enable_mcp: Some(true),
            enable_web_research: Some(true),
            web_research_query: Some("hero backstory".to_string()),
            narrative_perspective: Some("third_person".to_string()),
            creative_mode: Some("balanced".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            story_creation_brief: Some("brief".to_string()),
            quality_preset: Some("balanced".to_string()),
            quality_notes: Some("notes".to_string()),
            story_repair_summary: Some("repair".to_string()),
            story_repair_targets: Some(vec!["target-a".to_string()]),
            story_preserve_strengths: Some(vec!["strength-a".to_string()]),
        };
        let request = build_single_chapter_generation_request_from_route_payload(route_request);

        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(1800));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
        assert_eq!(request.enable_analysis, Some(true));
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(true));
        assert_eq!(
            request.web_research_query.as_deref(),
            Some("hero backstory")
        );
        assert_eq!(
            request.narrative_perspective.as_deref(),
            Some("third_person")
        );
        assert_eq!(request.creative_mode.as_deref(), Some("balanced"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(request.story_creation_brief.as_deref(), Some("brief"));
        assert_eq!(request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(request.story_repair_summary.as_deref(), Some("repair"));
        assert_eq!(
            request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
    }

    #[test]
    fn should_accept_empty_single_chapter_generation_route_payload() {
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest::default(),
        );

        assert_eq!(request.style_id, None);
        assert_eq!(request.target_word_count, None);
        assert_eq!(request.model, None);
        assert_eq!(request.enable_analysis, None);
        assert_eq!(request.enable_mcp, None);
        assert_eq!(request.enable_web_research, None);
    }
}
