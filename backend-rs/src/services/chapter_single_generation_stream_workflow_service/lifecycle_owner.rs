use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::utils::sse::{sse_error, SseProgress};

use super::success_owner::SingleGenerationStreamSuccessArtifacts;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_single_generation_prepare_service::{
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRouteRequest,
};
use crate::services::chapter_single_generation_runtime_restore_workflow_service::PreparedSingleChapterGenerationRestoredRuntimeLaunch;
use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

pub(crate) type SingleChapterGenerationStream = ReceiverStream<Result<Event, Infallible>>;

pub(crate) fn build_single_generation_stream_lifecycle_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_stream_workflow_service::lifecycle_owner",
        "scope": "single_generation_stream_entry_and_runtime_lifecycle",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service/lifecycle_owner.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "create_owned_single_generation_stream",
                "SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config",
                "SingleGenerationStreamLifecyclePlan::spawn",
                "SingleGenerationStreamLifecyclePlan::run"
            ],
            "runtime_path": [
                "prepare_runtime_launch_input_from_route_request",
                "execute_generation_with_gateway_config",
                "SingleGenerationStreamSuccessArtifacts::from_generated_result",
                "emit_success"
            ],
            "sse_progress_steps": [
                "tracker.start",
                "tracker.preparing",
                "tracker.generating",
                "sse_error on runtime failure"
            ]
        },
        "validation_boundary": [
            "cargo test chapter_single_generation_stream_workflow_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "python_source_map_retained": false,
            "same_round_python_edit_required": false
        }
    })
}

pub(crate) async fn create_owned_single_generation_stream(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    route_request: SingleChapterGenerationRouteRequest,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<SingleChapterGenerationStream, PrepareSingleChapterGenerationRequestError> {
    let runtime_input =
        PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_runtime_launch_input_from_route_request(
            &db,
            &chapter_id,
            &user_id,
            route_request,
        )
        .await?;

    Ok(
        SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config(
            runtime_input,
            candidate_gateway_config,
        )
        .spawn(db),
    )
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationStreamLifecyclePlan {
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) enable_analysis: bool,
    pub(crate) runtime_user_id: String,
    pub(crate) candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl SingleGenerationStreamLifecyclePlan {
    pub(crate) fn from_runtime_launch_with_gateway_config(
        runtime_input: SingleGenerationRuntimeLaunchInput,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        let target_word_count = runtime_input.execution_input.target_word_count;
        let compat_options = runtime_input.execution_input.compat_options.clone();
        let enable_analysis = compat_options.enable_analysis();
        let runtime_user_id = runtime_input.user_id.clone();

        Self {
            target_word_count,
            compat_options,
            enable_analysis,
            runtime_user_id,
            candidate_gateway_config,
            runtime_input,
        }
    }

    pub(crate) fn spawn(self, db: DatabaseConnection) -> SingleChapterGenerationStream {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

        tokio::spawn(async move {
            self.run(db, tx).await;
        });

        ReceiverStream::new(rx)
    }

    async fn run(self, db: DatabaseConnection, tx: mpsc::Sender<Result<Event, Infallible>>) {
        let mut tracker = SseProgress::new("Chapter Generation");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(
                tracker.preparing(Some("Preparing chapter generation..."))
            ))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Generating chapter content..."),
                (15, 95),
                self.target_word_count as usize,
                None,
            )))
            .await;

        match self
            .runtime_input
            .execute_generation_with_gateway_config(&db, None, self.candidate_gateway_config)
            .await
        {
            Ok(result) => {
                let analysis = SingleGenerationStreamSuccessArtifacts::from_generated_result(
                    &db,
                    &self.runtime_user_id,
                    self.target_word_count,
                    &self.compat_options,
                    self.enable_analysis,
                    &result,
                )
                .await;
                analysis.emit_success(&result, &tx, &mut tracker).await;
            }
            Err(error_message) => {
                let _ = tx.send(Ok(sse_error(&error_message, 500))).await;
            }
        }
    }
}
