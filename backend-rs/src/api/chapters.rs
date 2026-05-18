use axum::Router;

use crate::api::{
    chapter_analysis_routes, chapter_crud_routes, chapter_regeneration_routes,
};

pub fn routes() -> Router {
    Router::new()
        .merge(chapter_crud_routes::routes())
        .merge(chapter_analysis_routes::routes())
        .merge(chapter_regeneration_routes::routes())
}
