use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::batch_generation_task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationTaskPersistenceSeed {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) user_id: String,
    pub(crate) start_chapter_number: i32,
    pub(crate) chapter_count: i32,
    pub(crate) chapter_ids: Value,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: i32,
    pub(crate) enable_analysis: bool,
    pub(crate) total_chapters: i32,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) max_retries: i32,
}

impl SingleGenerationTaskPersistenceSeed {
    pub(crate) fn into_active_model(
        self,
        now: NaiveDateTime,
    ) -> batch_generation_task::ActiveModel {
        batch_generation_task::ActiveModel {
            id: Set(self.id),
            project_id: Set(self.project_id),
            user_id: Set(self.user_id),
            start_chapter_number: Set(self.start_chapter_number),
            chapter_count: Set(self.chapter_count),
            chapter_ids: Set(self.chapter_ids),
            style_id: Set(self.style_id),
            target_word_count: Set(self.target_word_count),
            enable_analysis: Set(self.enable_analysis),
            status: Set("pending".to_string()),
            total_chapters: Set(self.total_chapters),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(self.current_chapter_id),
            current_chapter_number: Set(self.current_chapter_number),
            current_retry_count: Set(0),
            max_retries: Set(self.max_retries),
            created_at: Set(Some(now)),
            started_at: Set(None),
            completed_at: Set(None),
            error_message: Set(None),
        }
    }
}

pub(crate) fn build_single_generation_background_task_persistence_seed(
    task_id: String,
    project_id: &str,
    user_id: String,
    chapter_id: &str,
    chapter_number: i32,
    title: &str,
    target_word_count: i32,
) -> SingleGenerationTaskPersistenceSeed {
    SingleGenerationTaskPersistenceSeed {
        id: task_id,
        project_id: project_id.to_string(),
        user_id,
        start_chapter_number: chapter_number,
        chapter_count: 1,
        chapter_ids: json!([{
            "id": chapter_id,
            "chapter_number": chapter_number,
            "title": title,
        }]),
        style_id: None,
        target_word_count,
        enable_analysis: false,
        total_chapters: 1,
        current_chapter_id: Some(chapter_id.to_string()),
        current_chapter_number: Some(chapter_number),
        max_retries: 0,
    }
}

pub(crate) fn build_single_generation_background_task_active_model(
    task_id: String,
    project_id: &str,
    user_id: String,
    chapter_id: &str,
    chapter_number: i32,
    title: &str,
    target_word_count: i32,
    now: NaiveDateTime,
) -> batch_generation_task::ActiveModel {
    build_single_generation_background_task_persistence_seed(
        task_id,
        project_id,
        user_id,
        chapter_id,
        chapter_number,
        title,
        target_word_count,
    )
    .into_active_model(now)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelFieldUpdate<T> {
    Keep,
    Set(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskTimestampUpdate {
    Keep,
    Clear,
    Now,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationTaskStage {
    Preparing,
    Completed,
    Failed,
}

impl SingleGenerationTaskStage {
    pub(crate) fn status(self) -> &'static str {
        match self {
            SingleGenerationTaskStage::Preparing => "running",
            SingleGenerationTaskStage::Completed => "completed",
            SingleGenerationTaskStage::Failed => "failed",
        }
    }

    pub(crate) fn started_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Now,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Keep
            }
        }
    }

    pub(crate) fn completed_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Clear,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Now
            }
        }
    }

    pub(crate) fn completed_chapters_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(1),
        }
    }

    pub(crate) fn current_retry_count_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing => ModelFieldUpdate::Set(0),
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
        }
    }

    pub(crate) fn current_chapter_id_update(
        self,
        chapter_id: &str,
    ) -> ModelFieldUpdate<Option<String>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => {
                ModelFieldUpdate::Set(Some(chapter_id.to_string()))
            }
            SingleGenerationTaskStage::Failed => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn current_chapter_number_update(
        self,
        chapter_number: Option<i32>,
    ) -> ModelFieldUpdate<Option<i32>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(chapter_number),
        }
    }

    pub(crate) async fn persist_for_task(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: NaiveDateTime,
    ) -> Result<(), String> {
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(&mut active, chapter_id, chapter_number, error_message, now);
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    pub(crate) fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: NaiveDateTime,
    ) {
        active.status = Set(self.status().to_string());

        match self.started_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.started_at = Set(None),
            TaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.completed_at = Set(None),
            TaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }

        active.error_message = Set(match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => None,
            SingleGenerationTaskStage::Failed => error_message,
        });

        match self.completed_chapters_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.completed_chapters = Set(value),
        }

        match self.current_retry_count_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_retry_count = Set(value),
        }

        match self.current_chapter_id_update(chapter_id) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_id = Set(value),
        }

        match self.current_chapter_number_update(chapter_number) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_number = Set(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        build_single_generation_background_task_active_model,
        build_single_generation_background_task_persistence_seed, ModelFieldUpdate,
        SingleGenerationTaskPersistenceSeed, SingleGenerationTaskStage, TaskTimestampUpdate,
    };
    use crate::models::batch_generation_task;

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_build_single_generation_task_persistence_seed_for_background_target() {
        let seed = build_single_generation_background_task_persistence_seed(
            "task-7".to_string(),
            "project-7",
            "user-7".to_string(),
            "chapter-7",
            7,
            "Seven",
            2600,
        );

        assert_eq!(
            seed,
            SingleGenerationTaskPersistenceSeed {
                id: "task-7".to_string(),
                project_id: "project-7".to_string(),
                user_id: "user-7".to_string(),
                start_chapter_number: 7,
                chapter_count: 1,
                chapter_ids: json!([{
                    "id": "chapter-7",
                    "chapter_number": 7,
                    "title": "Seven",
                }]),
                style_id: None,
                target_word_count: 2600,
                enable_analysis: false,
                total_chapters: 1,
                current_chapter_id: Some("chapter-7".to_string()),
                current_chapter_number: Some(7),
                max_retries: 0,
            }
        );
    }

    #[test]
    fn should_build_single_generation_background_task_active_model_with_single_defaults() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(2, 0, 0)
            .expect("valid time");
        let active = build_single_generation_background_task_active_model(
            "task-8".to_string(),
            "project-8",
            "user-8".to_string(),
            "chapter-8",
            8,
            "Eight",
            2800,
            now,
        );

        assert_eq!(active.status, Set("pending".to_string()));
        assert_eq!(active.completed_chapters, Set(0));
        assert_eq!(active.failed_chapters, Set(json!([])));
        assert_eq!(active.current_retry_count, Set(0));
        assert_eq!(active.max_retries, Set(0));
        assert_eq!(active.target_word_count, Set(2800));
        assert_eq!(
            active.chapter_ids,
            Set(json!([{
                "id": "chapter-8",
                "chapter_number": 8,
                "title": "Eight",
            }]))
        );
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-8".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(8)));
        assert_eq!(active.created_at, Set(Some(now)));
    }

    #[test]
    fn should_resolve_single_generation_task_stage_mutation_contracts() {
        let preparing = SingleGenerationTaskStage::Preparing;
        assert_eq!(preparing.status(), "running");
        assert!(matches!(
            preparing.started_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            preparing.completed_at_update(),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            preparing.current_retry_count_update(),
            ModelFieldUpdate::Set(0)
        ));
        assert!(matches!(
            preparing.current_chapter_id_update("chapter-1"),
            ModelFieldUpdate::Set(Some(ref id)) if id == "chapter-1"
        ));

        let completed = SingleGenerationTaskStage::Completed;
        assert_eq!(completed.status(), "completed");
        assert!(matches!(
            completed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            completed.completed_chapters_update(),
            ModelFieldUpdate::Set(1)
        ));
        assert!(matches!(
            completed.current_chapter_number_update(Some(2)),
            ModelFieldUpdate::Set(Some(2))
        ));

        let failed = SingleGenerationTaskStage::Failed;
        assert_eq!(failed.status(), "failed");
        assert!(matches!(
            failed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            failed.current_chapter_id_update("chapter-3"),
            ModelFieldUpdate::Keep
        ));
    }

    #[test]
    fn should_apply_single_generation_task_mutation_plan() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(0, 20, 0)
            .expect("valid time");
        let mut active: batch_generation_task::ActiveModel = build_task("pending").into();

        SingleGenerationTaskStage::Completed.apply_to_active_model(
            &mut active,
            "chapter-8",
            Some(8),
            None,
            now,
        );

        assert_eq!(active.status, Set("completed".to_string()));
        assert_eq!(active.completed_at, Set(Some(now)));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.completed_chapters, Set(1));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-8".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(8)));
    }
}
