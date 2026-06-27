use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use tracing::warn;
use uuid::Uuid;

use crate::models::{chapter, project, story_memory};
use crate::services::foreshadow_service::ForeshadowService;
use crate::services::story_memory_vector_index_service::delete_story_memory_vector_records_by_chapter;

pub struct ChapterService;

pub(crate) fn project_current_words_after_delta(current_words: i32, delta: i32) -> i32 {
    current_words.saturating_add(delta).max(0)
}

pub(crate) fn python_parity_navigation_neighbors(
    chapters: &[chapter::Model],
    current_chapter_number: i32,
) -> (Option<chapter::Model>, Option<chapter::Model>) {
    let previous = chapters
        .iter()
        .filter(|chapter| chapter.chapter_number < current_chapter_number)
        .max_by_key(|chapter| chapter.chapter_number)
        .cloned();
    let next = chapters
        .iter()
        .filter(|chapter| chapter.chapter_number > current_chapter_number)
        .min_by_key(|chapter| chapter.chapter_number)
        .cloned();

    (previous, next)
}

impl ChapterService {
    async fn verify_project_access(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let exists = project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(exists.is_some())
    }

    async fn adjust_project_current_words(
        db: &DatabaseConnection,
        project_id: &str,
        delta: i32,
    ) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }

        let project = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let Some(project) = project else {
            return Ok(());
        };

        let next_current_words = project_current_words_after_delta(project.current_words, delta);
        let mut active: project::ActiveModel = project.into();
        active.current_words = Set(next_current_words);
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(())
    }

    async fn cleanup_chapter_generated_artifacts(
        db: &DatabaseConnection,
        project_id: &str,
        chapter_id: &str,
    ) {
        if let Err(error) = story_memory::Entity::delete_many()
            .filter(story_memory::Column::ProjectId.eq(project_id))
            .filter(story_memory::Column::ChapterId.eq(chapter_id))
            .exec(db)
            .await
        {
            warn!(
                project_id = %project_id,
                chapter_id = %chapter_id,
                error = %error,
                "failed to delete chapter memories during chapter deletion cleanup"
            );
        }

        if let Err(error) =
            delete_story_memory_vector_records_by_chapter(project_id, chapter_id).await
        {
            warn!(
                project_id = %project_id,
                chapter_id = %chapter_id,
                error = %error,
                "failed to delete chapter memory vectors during chapter deletion cleanup"
            );
        }

        if let Err(error) =
            ForeshadowService::delete_chapter_analysis_foreshadows(db, project_id, chapter_id).await
        {
            warn!(
                project_id = %project_id,
                chapter_id = %chapter_id,
                error = %error,
                "failed to delete chapter analysis foreshadows during chapter deletion cleanup"
            );
        }
    }

    pub async fn create(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        title: &str,
        chapter_number: i32,
        content: Option<&str>,
        summary: Option<&str>,
        status: Option<&str>,
        outline_id: Option<&str>,
        sub_index: Option<i32>,
        expansion_plan: Option<&str>,
    ) -> Result<Option<chapter::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now().naive_utc();
        let word_count = content.map(|s| s.chars().count() as i32).unwrap_or(0);
        let model = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            title: Set(title.to_string()),
            chapter_number: Set(chapter_number),
            content: Set(content.map(|s| s.to_string())),
            summary: Set(summary.map(|s| s.to_string())),
            word_count: Set(word_count),
            status: Set(status.unwrap_or("draft").to_string()),
            outline_id: Set(outline_id.map(|s| s.to_string())),
            sub_index: Set(sub_index.unwrap_or(1)),
            expansion_plan: Set(expansion_plan.map(|s| s.to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        let chapter = model
            .insert(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)?;
        Self::adjust_project_current_words(db, project_id, word_count).await?;
        Ok(chapter)
    }

    /// Create a pending chapter (used by wizard outline one-to-one mode)
    pub async fn create_pending(
        db: &DatabaseConnection,
        project_id: &str,
        title: &str,
        chapter_number: i32,
    ) -> Result<chapter::Model, String> {
        let now = Utc::now().naive_utc();
        let model = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            title: Set(title.to_string()),
            chapter_number: Set(chapter_number),
            content: Set(None),
            summary: Set(None),
            word_count: Set(0),
            status: Set("pending".to_string()),
            outline_id: Set(None),
            sub_index: Set(1),
            expansion_plan: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model.insert(db).await.map_err(|e| format!("{}", e))
    }

    pub async fn list_by_project(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<chapter::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .order_by_asc(chapter::Column::ChapterNumber)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }

    pub async fn get(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<Option<chapter::Model>, String> {
        let c = chapter::Entity::find_by_id(chapter_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        match c {
            Some(ref ch) => {
                if !Self::verify_project_access(db, &ch.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(ch.clone()))
            }
            None => Ok(None),
        }
    }

    pub async fn update(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
        status: Option<&str>,
    ) -> Result<Option<chapter::Model>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let old_word_count = model.word_count;
        let project_id = model.project_id.clone();
        let mut next_word_count = old_word_count;
        let mut active: chapter::ActiveModel = model.into();
        if let Some(v) = title {
            active.title = Set(v.to_string());
        }
        if let Some(v) = content {
            let wc = v.chars().count() as i32;
            active.content = Set(Some(v.to_string()));
            active.word_count = Set(wc);
            next_word_count = wc;
        }
        if let Some(v) = summary {
            active.summary = Set(Some(v.to_string()));
        }
        if let Some(v) = status {
            active.status = Set(v.to_string());
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        let updated = active
            .update(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)?;
        Self::adjust_project_current_words(db, &project_id, next_word_count - old_word_count)
            .await?;
        Ok(updated)
    }

    pub async fn delete(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        let Some(chapter) = existing else {
            return Ok(None);
        };
        let project_id = chapter.project_id.clone();
        let word_count = chapter.word_count;
        Self::cleanup_chapter_generated_artifacts(db, &project_id, chapter_id).await;
        chapter::Entity::delete_by_id(chapter_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Self::adjust_project_current_words(db, &project_id, -word_count).await?;
        Ok(Some(()))
    }

    pub async fn navigation(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
    ) -> Result<
        Option<(
            Option<chapter::Model>,
            Option<chapter::Model>,
            Option<chapter::Model>,
        )>,
        String,
    > {
        let current = Self::get(db, chapter_id, user_id).await?;
        let Some(ref ch) = current else {
            return Ok(None);
        };

        let navigation_chapters = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(&ch.project_id))
            .filter(
                chapter::Column::ChapterNumber
                    .lt(ch.chapter_number)
                    .or(chapter::Column::ChapterNumber.gt(ch.chapter_number)),
            )
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let (prev, next) =
            python_parity_navigation_neighbors(&navigation_chapters, ch.chapter_number);

        Ok(Some((prev, current.map(|c| c.clone()), next)))
    }

    pub async fn update_expansion_plan(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        plan: &str,
    ) -> Result<Option<chapter::Model>, String> {
        let existing = Self::get(db, chapter_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: chapter::ActiveModel = model.into();
        active.expansion_plan = Set(Some(plan.to_string()));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|e| format!("{}", e))
            .map(Some)
    }
}

#[cfg(test)]
pub(crate) fn build_chapter_service_owner_contract() -> serde_json::Value {
    serde_json::json!({
        "owner": "chapter_service",
        "scope": "chapter_crud_write_read_navigation_and_project_word_count_owner",
        "python_source_map": [
            "backend/migrator_app/models/chapter.py",
            "backend/migrator_app/models/project.py",
            "backend/migrator_app/models/memory_analysis.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_service.rs",
            "backend-rs/src/services/chapter_query_service.rs",
            "backend-rs/src/services/chapter_access_service.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "backend-rs/src/api/chapter_crud_routes.rs",
            "backend-rs/src/api/chapters.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "create",
                "create_pending",
                "list_by_project",
                "get",
                "update",
                "delete",
                "navigation",
                "update_expansion_plan"
            ],
            "project_word_count_owner": "adjust_project_current_words",
            "delete_cleanup_owners": [
                "story_memory",
                "story_memory_vector_index",
                "foreshadow_analysis_cleanup"
            ],
            "navigation_policy": "strict_chapter_number_previous_next_parity"
        },
        "service_runtime_closeout_status": {
            "owner_profiles": ["phase5-chapter-crud-owner"],
            "chapter_crud_manifest_probe_count": 13,
            "rust_manifest_probe_count": 13,
            "python_fallback_probe_count": 0,
            "aggregate_owner_package": [
                "chapter_service",
                "chapter_query_service",
                "chapter_access_service",
                "chapter_quality_metrics_query_service"
            ],
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "chapter CRUD workflow and query direct python source-maps deleted; surviving Python closeout work for this owner is now limited to chapter/project model references plus the memory analysis model rollback reference",
            "status": "rust_chapter_service_owner_workflow_source_map_deleted"
        },
        "validation_boundary": [
            "cargo test chapter_service --manifest-path backend-rs/Cargo.toml",
            "cargo test chapter_query_service --manifest-path backend-rs/Cargo.toml",
            "cargo test chapter_access_service --manifest-path backend-rs/Cargo.toml",
            "cargo test chapter_quality_metrics_query_service --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-chapter-crud-owner"
        ],
        "rollback_boundary": "chapter/project model references and the memory analysis model remain source-map rollback references"
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::{
        build_chapter_service_owner_contract, project_current_words_after_delta,
        python_parity_navigation_neighbors,
    };

    fn chapter_model(id: &str, chapter_number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number,
            title: format!("第{}章", chapter_number),
            content: None,
            summary: None,
            word_count: 0,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 1,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_adjust_project_current_words_like_python_chapter_crud_workflow() {
        assert_eq!(project_current_words_after_delta(100, 25), 125);
        assert_eq!(project_current_words_after_delta(100, -30), 70);
        assert_eq!(project_current_words_after_delta(20, -50), 0);
    }

    #[test]
    fn should_select_navigation_neighbors_by_strict_chapter_number_like_python() {
        let chapters = vec![
            chapter_model("chapter-1", 1),
            chapter_model("same-number", 2),
            chapter_model("chapter-3", 3),
            chapter_model("later-duplicate", 3),
            chapter_model("chapter-5", 5),
        ];

        let (previous, next) = python_parity_navigation_neighbors(&chapters, 3);

        assert_eq!(
            previous.map(|chapter| chapter.id),
            Some("same-number".to_string())
        );
        assert_eq!(
            next.map(|chapter| chapter.id),
            Some("chapter-5".to_string())
        );
    }

    #[test]
    fn should_publish_chapter_service_owner_contract() {
        let contract = build_chapter_service_owner_contract();

        assert_eq!(contract["owner"], "chapter_service");
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_service.rs"
        );
        assert_eq!(
            contract["python_source_map"][2],
            "backend/migrator_app/models/memory_analysis.py"
        );
        assert_eq!(contract["behavior_contract"]["entrypoints"][0], "create");
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
            "chapter/project model references and the memory analysis model remain source-map rollback references"
        );
    }
}
