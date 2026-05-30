use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Json, Sse},
    routing::post,
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;

use crate::api::chapter_generation_error_mapper::map_single_chapter_generation_request_error;
use crate::services::auth::Claims;
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationRequest;
use crate::services::chapter_single_generation_stream_workflow_service::{
    create_single_generation_stream_workflow,
};
use crate::services::chapter_single_generation_write_workflow_service::start_owned_single_generation_background_write_workflow;
use crate::utils::sse::default_sse_keep_alive;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct SingleChapterGenerationRouteRequest {
    style_id: Option<i32>,
    target_word_count: Option<i32>,
    model: Option<String>,
    #[serde(default)]
    enable_analysis: Option<bool>,
    #[serde(default)]
    enable_mcp: Option<bool>,
    #[serde(default)]
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
    narrative_perspective: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    story_repair_summary: Option<String>,
    story_repair_targets: Option<Vec<String>>,
    story_preserve_strengths: Option<Vec<String>>,
}

async fn generate_chapter_content_background(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<SingleChapterGenerationRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = SingleChapterGenerationRequest::from_route_payload(
        body.style_id,
        body.target_word_count,
        body.model,
        body.enable_analysis,
        body.enable_mcp,
        body.enable_web_research,
        body.web_research_query,
        body.narrative_perspective,
        body.creative_mode,
        body.story_focus,
        body.plot_stage,
        body.story_creation_brief,
        body.quality_preset,
        body.quality_notes,
        body.story_repair_summary,
        body.story_repair_targets,
        body.story_preserve_strengths,
    );
    let result = start_owned_single_generation_background_write_workflow(
        &db,
        &chapter_id,
        &claims.sub,
        request,
    )
    .await
    .map_err(map_single_chapter_generation_request_error)?;

    Ok(Json(result))
}

async fn generate_chapter_content_stream(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<SingleChapterGenerationRouteRequest>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let request = SingleChapterGenerationRequest::from_route_payload(
        body.style_id,
        body.target_word_count,
        body.model,
        body.enable_analysis,
        body.enable_mcp,
        body.enable_web_research,
        body.web_research_query,
        body.narrative_perspective,
        body.creative_mode,
        body.story_focus,
        body.plot_stage,
        body.story_creation_brief,
        body.quality_preset,
        body.quality_notes,
        body.story_repair_summary,
        body.story_repair_targets,
        body.story_preserve_strengths,
    );
    let stream = create_single_generation_stream_workflow(
        db.clone(),
        claims.sub.clone(),
        chapter_id,
        request,
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
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationRequest;

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
        let request = SingleChapterGenerationRequest::from_route_payload(
            route_request.style_id,
            route_request.target_word_count,
            route_request.model,
            route_request.enable_analysis,
            route_request.enable_mcp,
            route_request.enable_web_research,
            route_request.web_research_query,
            route_request.narrative_perspective,
            route_request.creative_mode,
            route_request.story_focus,
            route_request.plot_stage,
            route_request.story_creation_brief,
            route_request.quality_preset,
            route_request.quality_notes,
            route_request.story_repair_summary,
            route_request.story_repair_targets,
            route_request.story_preserve_strengths,
        );

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
        assert_eq!(
            request.story_creation_brief.as_deref(),
            Some("brief")
        );
        assert_eq!(request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(
            request.story_repair_summary.as_deref(),
            Some("repair")
        );
        assert_eq!(
            request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
    }
}
