use sea_orm::DatabaseConnection;

use crate::services::chapter_single_generation_background_launch_service::{
    build_background_launch_parts_from_restored_launch,
    PreparedSingleGenerationBackgroundLaunchParts, SingleGenerationStartupSnapshotPlan,
};
use crate::services::chapter_single_generation_prepare_service::{
    load_single_chapter_generation_target, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationRequest, SingleChapterGenerationRouteRequest,
    SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_seed_service::prepare_single_generation_restored_runtime_seed_from_target;
use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
#[cfg(test)]
use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleChapterGenerationRestoredRuntimeLaunch {
    chapter_target: SingleChapterGenerationTarget,
    startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl PreparedSingleChapterGenerationRestoredRuntimeLaunch {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        request.validate_request_bounds()?;

        let chapter_target = load_single_chapter_generation_target(db, chapter_id, user_id).await?;
        Self::prepare_from_target(db, user_id, request, chapter_target).await
    }

    pub(crate) async fn prepare_from_target(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
        chapter_target: SingleChapterGenerationTarget,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        let (chapter_target, startup_snapshot_plan, runtime_input) =
            prepare_single_generation_restored_runtime_seed_from_target(
                db,
                user_id,
                request,
                chapter_target,
            )
            .await?;

        Ok(Self {
            chapter_target,
            startup_snapshot_plan,
            runtime_input,
        })
    }

    pub(crate) async fn prepare_runtime_launch_input(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
    ) -> Result<SingleGenerationRuntimeLaunchInput, PrepareSingleChapterGenerationRequestError>
    {
        Ok(Self::prepare(db, chapter_id, user_id, request)
            .await?
            .into_runtime_launch_input())
    }

    pub(crate) async fn prepare_runtime_launch_input_from_route_request(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        route_request: SingleChapterGenerationRouteRequest,
    ) -> Result<SingleGenerationRuntimeLaunchInput, PrepareSingleChapterGenerationRequestError>
    {
        Self::prepare_runtime_launch_input(
            db,
            chapter_id,
            user_id,
            &route_request.into_generation_request(),
        )
        .await
    }

    pub(crate) async fn prepare_background_launch_parts_from_target(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
        chapter_target: SingleChapterGenerationTarget,
        task_id: String,
    ) -> Result<
        PreparedSingleGenerationBackgroundLaunchParts,
        PrepareSingleChapterGenerationRequestError,
    > {
        Ok(
            Self::prepare_from_target(db, user_id, request, chapter_target)
                .await?
                .into_background_launch_parts(task_id),
        )
    }

    pub(crate) async fn prepare_background_launch_parts_from_route_target(
        db: &DatabaseConnection,
        user_id: &str,
        route_request: SingleChapterGenerationRouteRequest,
        chapter_target: SingleChapterGenerationTarget,
        task_id: String,
    ) -> Result<
        PreparedSingleGenerationBackgroundLaunchParts,
        PrepareSingleChapterGenerationRequestError,
    > {
        Self::prepare_background_launch_parts_from_target(
            db,
            user_id,
            &route_request.into_generation_request(),
            chapter_target,
            task_id,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        SingleChapterGenerationTarget,
        SingleGenerationStartupSnapshotPlan,
        SingleGenerationRuntimeLaunchInput,
    ) {
        (
            self.chapter_target,
            self.startup_snapshot_plan,
            self.runtime_input,
        )
    }

    pub(crate) fn into_runtime_launch_input(self) -> SingleGenerationRuntimeLaunchInput {
        self.runtime_input
    }

    pub(crate) fn into_background_launch_parts(
        self,
        task_id: String,
    ) -> PreparedSingleGenerationBackgroundLaunchParts {
        let Self {
            chapter_target,
            startup_snapshot_plan,
            runtime_input,
        } = self;
        build_background_launch_parts_from_restored_launch(
            task_id,
            chapter_target,
            startup_snapshot_plan,
            runtime_input,
        )
    }

    #[cfg(test)]
    pub(crate) fn startup_snapshot_plan(&self) -> &SingleGenerationStartupSnapshotPlan {
        &self.startup_snapshot_plan
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        chapter_target: SingleChapterGenerationTarget,
        runtime_state_payload: Value,
        runtime_input: SingleGenerationRuntimeLaunchInput,
    ) -> Self {
        Self {
            startup_snapshot_plan: SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
                crate::services::chapter_single_generation_background_launch_service::build_single_generation_pending_checkpoint(&chapter_target),
                runtime_state_payload,
            ),
            chapter_target,
            runtime_input,
        }
    }
}
