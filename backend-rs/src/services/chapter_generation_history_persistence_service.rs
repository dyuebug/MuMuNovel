pub(crate) mod persistence_owner;
#[cfg(test)]
pub(crate) use persistence_owner::build_generated_history_payload;
pub(crate) use persistence_owner::{
    build_chapter_generation_history_persistence_owner_contract,
    persist_single_generation_candidate_draft_attempt, persist_single_generation_generated_result,
};

#[cfg(test)]
mod tests {
    use super::{
        build_chapter_generation_history_persistence_owner_contract,
        persist_single_generation_generated_result,
    };
    use crate::models::{chapter, chapter_draft_attempt, generation_history};
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Schema, Set};
    use serde_json::json;

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-history-1".to_string(),
            project_id: "project-history-1".to_string(),
            title: "历史章节".to_string(),
            chapter_number: 3,
            content: Some("上一版正文".to_string()),
            summary: None,
            expansion_plan: None,
            status: "writing".to_string(),
            word_count: 12,
            outline_id: None,
            sub_index: 0,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: Some(chrono::Utc::now().naive_utc()),
        }
    }

    #[tokio::test]
    async fn should_persist_single_generation_generated_result_history_and_chapter_update() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
        let builder = db.get_database_backend();

        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapter table");
        db.execute(builder.build(&schema.create_table_from_entity(generation_history::Entity)))
            .await
            .expect("create history table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)))
            .await
            .expect("create draft attempt table");

        chapter::ActiveModel {
            id: Set("chapter-history-1".to_string()),
            project_id: Set("project-history-1".to_string()),
            chapter_number: Set(3),
            title: Set("历史章节".to_string()),
            content: Set(Some("上一版正文".to_string())),
            word_count: Set(12),
            status: Set("writing".to_string()),
            summary: Set(None),
            outline_id: Set(None),
            sub_index: Set(0),
            expansion_plan: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(Some(chrono::Utc::now().naive_utc())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert chapter");

        let result = GeneratedChapterResult {
            chapter_id: "chapter-history-1".to_string(),
            chapter_number: 3,
            title: "历史章节".to_string(),
            content: "新的章节正文".to_string(),
            word_count: 18,
            saved_word_count: 18,
            chapter_status: "completed".to_string(),
            content_applied: true,
            attempt_state: "applied".to_string(),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "continue"
                }
            })),
            ..Default::default()
        };

        let persisted = persist_single_generation_generated_result(
            &db,
            &build_chapter(),
            "prompt-body".to_string(),
            result,
        )
        .await
        .expect("persist generated result");

        assert_eq!(persisted.saved_word_count, 18);

        let updated_chapter = chapter::Entity::find_by_id("chapter-history-1")
            .one(&db)
            .await
            .expect("load chapter")
            .expect("chapter exists");
        assert_eq!(updated_chapter.content.as_deref(), Some("新的章节正文"));
        assert_eq!(updated_chapter.status, "completed");

        let histories = generation_history::Entity::find()
            .all(&db)
            .await
            .expect("load histories");
        assert_eq!(histories.len(), 1);
        assert_eq!(histories[0].model.as_deref(), Some("chapter_generation_v1"));
        assert!(histories[0]
            .generated_content
            .as_deref()
            .expect("generated content")
            .contains("\"attempt_state\":\"applied\""));

        let attempts = chapter_draft_attempt::Entity::find()
            .all(&db)
            .await
            .expect("load draft attempts");
        assert!(attempts.is_empty());
    }

    #[tokio::test]
    async fn should_persist_single_generation_generated_result_history_and_draft_attempt_for_retry()
    {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
        let builder = db.get_database_backend();

        db.execute(builder.build(&schema.create_table_from_entity(chapter::Entity)))
            .await
            .expect("create chapter table");
        db.execute(builder.build(&schema.create_table_from_entity(generation_history::Entity)))
            .await
            .expect("create history table");
        db.execute(builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)))
            .await
            .expect("create draft attempt table");

        chapter::ActiveModel {
            id: Set("chapter-history-1".to_string()),
            project_id: Set("project-history-1".to_string()),
            chapter_number: Set(3),
            title: Set("历史章节".to_string()),
            content: Set(Some("上一版正文".to_string())),
            word_count: Set(12),
            status: Set("writing".to_string()),
            summary: Set(None),
            outline_id: Set(None),
            sub_index: Set(0),
            expansion_plan: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(Some(chrono::Utc::now().naive_utc())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert chapter");

        let result = GeneratedChapterResult {
            chapter_id: "chapter-history-1".to_string(),
            chapter_number: 3,
            title: "历史章节".to_string(),
            content: "候选正文需要继续修复".to_string(),
            word_count: 18,
            chapter_status: "draft".to_string(),
            content_applied: false,
            provisional_draft_saved: true,
            attempt_state: "retry".to_string(),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "decision": "auto_repair",
                    "summary": "建议继续修复"
                }
            })),
            quality_gate_action: Some("retry".to_string()),
            quality_gate_message: Some("建议继续修复".to_string()),
            ..Default::default()
        };

        let persisted = persist_single_generation_generated_result(
            &db,
            &build_chapter(),
            "prompt-body".to_string(),
            result,
        )
        .await
        .expect("persist retry result");

        assert_eq!(persisted.saved_word_count, 18);
        assert_eq!(
            persisted.candidate_draft.as_ref().expect("candidate draft")["quality_gate_action"],
            "retry"
        );

        let histories = generation_history::Entity::find()
            .all(&db)
            .await
            .expect("load histories");
        assert_eq!(histories.len(), 1);
        assert!(histories[0]
            .generated_content
            .as_deref()
            .expect("generated content")
            .contains("\"attempt_state\":\"retry\""));

        let attempts = chapter_draft_attempt::Entity::find()
            .all(&db)
            .await
            .expect("load draft attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].attempt_state, "retry");
        assert_eq!(attempts[0].quality_gate_action.as_deref(), Some("retry"));
    }

    #[test]
    fn should_publish_chapter_generation_history_persistence_owner_contract() {
        let contract = build_chapter_generation_history_persistence_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_history_persistence_service"
        );
        assert_eq!(
            contract["behavior_contract"]["history_model"],
            "chapter_generation_v1"
        );
        assert_eq!(
            contract["behavior_contract"]["persistence_steps"][0],
            "chapter_update_when_content_applied_or_provisional_draft_saved"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_generation_runtime_service"
        );
    }
}
