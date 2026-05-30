use axum::Router;

use crate::api::{
    chapter_analysis_routes, chapter_batch_generation, chapter_crud_routes,
    chapter_generation_routes,
    chapter_regeneration_routes,
};

pub(crate) fn routes() -> Router {
    Router::new()
        .merge(chapter_crud_routes::routes())
        .merge(chapter_analysis_routes::routes())
        .merge(chapter_generation_routes::routes())
        .merge(chapter_regeneration_routes::routes())
        .merge(chapter_batch_generation::routes())
}
