use sea_orm::{DatabaseConnection, EntityTrait};

use crate::models::chapter;

use super::chapter_service::ChapterService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadAccessibleChapterError {
    NotFoundOrAccessDenied,
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadAccessibleChapterForGenerationError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Internal(String),
}

pub async fn load_accessible_chapter(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(chapter),
        Ok(None) => Err(LoadAccessibleChapterError::NotFoundOrAccessDenied),
        Err(error) => Err(LoadAccessibleChapterError::Internal(error)),
    }
}

pub(crate) async fn load_accessible_chapter_for_generation(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<chapter::Model, LoadAccessibleChapterForGenerationError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(chapter),
        Ok(None) => {
            let chapter_exists = chapter::Entity::find_by_id(chapter_id)
                .one(db)
                .await
                .map_err(|error| {
                    LoadAccessibleChapterForGenerationError::Internal(error.to_string())
                })?
                .is_some();
            if chapter_exists {
                Err(LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied)
            } else {
                Err(LoadAccessibleChapterForGenerationError::ChapterNotFound)
            }
        }
        Err(error) => Err(LoadAccessibleChapterForGenerationError::Internal(error)),
    }
}

pub(crate) async fn load_accessible_chapters_for_generation(
    db: &DatabaseConnection,
    chapter_ids: &[String],
    user_id: &str,
) -> Result<Vec<chapter::Model>, LoadAccessibleChapterForGenerationError> {
    let mut chapters = Vec::with_capacity(chapter_ids.len());
    for chapter_id in chapter_ids {
        chapters.push(load_accessible_chapter_for_generation(db, chapter_id, user_id).await?);
    }
    Ok(chapters)
}

pub(crate) fn build_chapter_generation_access_owner_contract() -> serde_json::Value {
    serde_json::json!({
        "owner": "chapter_access_service",
        "scope": "shared_chapter_access_and_generation_specific_access_boundary",
        "python_source_map": [
            "backend/migrator_app/models/chapter.py",
            "backend/migrator_app/models/project.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "load_accessible_chapter",
                "load_accessible_chapter_for_generation",
                "load_accessible_chapters_for_generation"
            ],
            "generation_error_contract": [
                "ChapterNotFound",
                "ChapterNotFoundOrAccessDenied",
                "Internal"
            ],
            "shared_access_error_contract": [
                "NotFoundOrAccessDenied",
                "Internal"
            ],
            "bulk_loading_policy": "loads_each_requested_chapter_through_the_single_chapter_generation_access_boundary"
        },
        "validation_boundary": [
            "cargo test services::chapter_access_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-chapter-crud-owner",
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner",
                "phase5-chapter-regeneration-owner",
                "phase5-chapter-analysis-owner",
                "phase5-chapter-draft-owner"
            ],
            "chapter_crud_manifest_probe_count": 13,
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "regeneration_manifest_probe_count": 13,
            "analysis_manifest_probe_count": 8,
            "draft_manifest_probe_count": 8,
            "python_fallback_probe_count": 0,
            "aggregate_owner_package": [
                "chapter_service",
                "chapter_query_service",
                "chapter_access_service",
                "chapter_quality_metrics_query_service"
            ],
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "chapter access route-helper source-map deleted; surviving Python closeout work for this owner is now limited to chapter/project model rollback references",
            "status": "rust_chapter_access_service_owner_route_source_map_deleted"
        },
        "rollback_boundary": "chapter/project model references remain the chapter access source-map rollback boundary"
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn should_publish_chapter_generation_access_owner_contract() {
        let contract = super::build_chapter_generation_access_owner_contract();

        assert_eq!(contract["owner"], "chapter_access_service");
        assert_eq!(
            contract["scope"],
            "shared_chapter_access_and_generation_specific_access_boundary"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_access_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "load_accessible_chapter_for_generation"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/chapter.py"
        );
        assert_eq!(
            contract["python_source_map"][1],
            "backend/migrator_app/models/project.py"
        );
        assert_eq!(
            contract["behavior_contract"]["generation_error_contract"][1],
            "ChapterNotFoundOrAccessDenied"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-chapter-crud-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["chapter_crud_manifest_probe_count"],
            13
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"],
            "chapter/project model references remain the chapter access source-map rollback boundary"
        );
    }
}
