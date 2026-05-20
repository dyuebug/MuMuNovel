use sea_orm::DatabaseConnection;

use super::chapter_single_generation_request_service::{
    prepare_single_chapter_generation_request, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationRequest,
};
use super::chapter_single_generation_stream_service::{
    build_single_chapter_generation_stream, SingleChapterGenerationStream,
};

pub(crate) fn create_single_generation_stream_workflow(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    request: SingleChapterGenerationRequest,
) -> impl std::future::Future<
    Output = Result<SingleChapterGenerationStream, PrepareSingleChapterGenerationRequestError>,
> {
    async move {
        let prepared =
            prepare_single_chapter_generation_request(&db, &chapter_id, &user_id, &request).await?;

        Ok(build_single_chapter_generation_stream(
            db,
            user_id,
            prepared.execution_input,
        ))
    }
}
