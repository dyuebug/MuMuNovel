use axum::response::sse::Event;
use axum::{
    extract::{Extension, Path},
    response::Sse,
    routing::post,
    Json, Router,
};
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;

use serde_json::json;

use crate::services::auth::Claims;
use crate::services::project_service::ProjectService;
use crate::services::wizard_request_service::{
    build_cleanup_wizard_data_request_from_route_payload, execute_career_system_request,
    execute_characters_request, execute_outline_request, execute_regenerate_world_building_request,
    execute_world_building_request, resolve_effective_user_id, CareerSystemRequest,
    CharactersRequest, CleanupWizardDataRouteRequest, OutlineRequest,
    RegenerateWorldBuildingRequest, WorldBuildingRequest,
};

fn spawn_world_building_stream(
    claims: Claims,
    db: DatabaseConnection,
    body: WorldBuildingRequest,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_world_building_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn world_building(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<WorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    spawn_world_building_stream(claims, db, body)
}

async fn world_building_with_project_id(
    Path(_project_id): Path<String>,
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<WorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    spawn_world_building_stream(claims, db, body)
}

async fn career_system(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CareerSystemRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_career_system_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn characters(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CharactersRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_characters_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn outline(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<OutlineRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_outline_request(&db, &channel, &user_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn regenerate_world_building(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<RegenerateWorldBuildingRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = resolve_effective_user_id(body.user_id.clone(), &claims.sub);

    tokio::spawn(async move {
        execute_regenerate_world_building_request(&db, &channel, &user_id, &project_id, body).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

async fn cleanup_wizard_data(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
    Json(body): Json<CleanupWizardDataRouteRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);
    let request = build_cleanup_wizard_data_request_from_route_payload(body);

    tokio::spawn(async move {
        let _raw_body = request.body();

        channel
            .progress("正在清理旧的向导数据...", 0, "processing")
            .await;

        match ProjectService::cleanup_wizard_data(&db, &project_id, &claims.sub).await {
            Ok(Some(deleted)) => {
                let response = json!({
                    "message": "向导旧数据清理完成",
                    "deleted": {
                        "characters": deleted.characters,
                        "outlines": deleted.outlines,
                        "chapters": deleted.chapters,
                    }
                });
                channel.progress("清理完成", 100, "success").await;
                channel.result(&response).await;
                channel.done().await;
            }
            Ok(None) => {
                channel.error("项目不存在或无权访问", 404).await;
            }
            Err(e) => {
                channel.error(&format!("清理失败: {}", e), 500).await;
            }
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

pub fn routes() -> Router {
    Router::new()
        .route("/wizard-stream/world-building", post(world_building))
        .route(
            "/wizard-stream/world-building/{project_id}",
            post(world_building_with_project_id),
        )
        .route(
            "/wizard-stream/world-building/{project_id}/regenerate",
            post(regenerate_world_building),
        )
        .route("/wizard-stream/career-system", post(career_system))
        .route("/wizard-stream/characters", post(characters))
        .route("/wizard-stream/outline", post(outline))
        .route(
            "/wizard-stream/cleanup/{project_id}",
            post(cleanup_wizard_data),
        )
}
