use crate::ai::config::AIConfig;
use crate::models::{chapter, generation_history, project};
use crate::services::chapter_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_history_payload_service::payload_owner::generated_history_runtime_snapshot_from_payload;
use crate::services::chapter_generation_history_persistence_service::persist_single_generation_generated_result;
use crate::services::chapter_generation_prompt_service::{
    build_previous_chapter_prompt_context, PreviousChapterPromptContext,
};
use crate::services::chapter_generation_prompt_service::{
    ChapterGenerationPromptOverrides, PromptContextProviderPayload,
};
#[cfg(test)]
use crate::services::chapter_generation_runtime_service::candidate_runtime_owner::{
    build_single_generation_runtime_generated_result_from_candidate,
    build_single_generation_runtime_generated_result_from_content,
};
use crate::services::chapter_generation_runtime_service::story_continuity_ledger_owner::{
    load_project_continuity_ledger, ProjectContinuityLedger,
};
use crate::services::chapter_generation_runtime_service::{
    execute_single_generation_candidate_runtime, GeneratedChapterResult,
};
use crate::services::wizard_service::build_project_long_term_goal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadGenerationContextError {
    Chapter(LoadAccessibleChapterForGenerationError),
    ProjectNotFound,
    Internal(String),
}

impl LoadGenerationContextError {
    pub(crate) fn into_runtime_message(self) -> String {
        match self {
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFound,
            ) => "Chapter not found".to_string(),
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ) => "Chapter not found or access denied".to_string(),
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::Internal(error),
            )
            | LoadGenerationContextError::Internal(error) => error,
            LoadGenerationContextError::ProjectNotFound => "Project not found".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGenerationRuntimeContext {
    pub(crate) chapter_model: chapter::Model,
    pub(crate) project_model: project::Model,
    pub(crate) previous_chapter: Option<chapter::Model>,
    pub(crate) previous_chapter_prompt_context: PreviousChapterPromptContext,
    pub(crate) story_packet: Value,
}

impl ChapterGenerationRuntimeContext {
    async fn persist_generated_result(
        self,
        db: &DatabaseConnection,
        prompt: String,
        result: GeneratedChapterResult,
    ) -> Result<GeneratedChapterResult, String> {
        persist_single_generation_generated_result(db, &self.chapter_model, prompt, result).await
    }

    #[cfg(test)]
    pub(crate) fn build_generated_result_from_content(
        &self,
        content: String,
    ) -> Result<GeneratedChapterResult, String> {
        build_single_generation_runtime_generated_result_from_content(&self.chapter_model, content)
    }

    #[cfg(test)]
    pub(crate) fn build_generated_result_from_candidate(
        &self,
        candidate: &serde_json::Value,
    ) -> Result<GeneratedChapterResult, String> {
        build_single_generation_runtime_generated_result_from_candidate(
            &self.chapter_model,
            candidate,
        )
    }

    pub(crate) async fn generate_and_persist_with_candidate_route_gateway(
        self,
        db: &DatabaseConnection,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<GeneratedChapterResult, String> {
        let execution_context =
            crate::services::chapter_generation_runtime_service::SingleGenerationCandidateRuntimeExecutionContext {
                project_model: self.project_model.clone(),
                chapter_model: self.chapter_model.clone(),
                previous_chapter_exists: self.previous_chapter.is_some(),
                previous_chapter_prompt_context: self.previous_chapter_prompt_context.clone(),
                story_packet: self.story_packet.clone(),
            };
        let (prompt, result) = execute_single_generation_candidate_runtime(
            &execution_context,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
        )
        .await?;
        self.persist_generated_result(db, prompt, result).await
    }
}

pub(crate) fn build_single_generation_runtime_execution_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service::single_generation_runtime_execution",
        "scope": "single_generation_runtime_context_loading_and_persistence_orchestration",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/runtime_execution_owner.rs"
        ],
        "behavior_contract": {
            "loads": [
                "accessible_chapter",
                "owning_project",
                "previous_chapter_prompt_context",
                "previous_story_runtime_snapshot",
                "active_story_packet"
            ],
            "delegates_candidate_runtime_owner": "chapter_generation_runtime_service",
            "delegates_history_persistence_owner": "chapter_generation_history_persistence_service",
            "error_mapping": [
                "Chapter not found",
                "Chapter not found or access denied",
                "Project not found"
            ]
        },
        "active_consumers": [
            "chapter_generation_runtime_service",
            "chapter_single_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service"
        ]
    })
}

pub(crate) async fn load_generation_context(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<ChapterGenerationRuntimeContext, LoadGenerationContextError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(LoadGenerationContextError::Chapter)?;

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| LoadGenerationContextError::Internal(error.to_string()))?
        .ok_or(LoadGenerationContextError::ProjectNotFound)?;

    let previous_chapter = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .one(db)
        .await
        .map_err(|error| LoadGenerationContextError::Internal(error.to_string()))?;
    let previous_chapter_prompt_context =
        build_previous_chapter_prompt_context(previous_chapter.as_ref());
    let previous_story_runtime_snapshot =
        load_previous_story_runtime_snapshot(db, previous_chapter.as_ref()).await?;
    let project_continuity_ledger = load_project_continuity_ledger(db, Some(&project_model.id), 4)
        .await
        .map_err(LoadGenerationContextError::Internal)?;
    let story_packet = build_single_generation_story_packet(
        &project_model,
        &chapter_model,
        previous_story_runtime_snapshot.as_ref(),
        Some(&project_continuity_ledger),
    );

    Ok(ChapterGenerationRuntimeContext {
        chapter_model,
        project_model,
        previous_chapter,
        previous_chapter_prompt_context,
        story_packet,
    })
}

async fn load_previous_story_runtime_snapshot(
    db: &DatabaseConnection,
    previous_chapter: Option<&chapter::Model>,
) -> Result<Option<Value>, LoadGenerationContextError> {
    let Some(previous_chapter) = previous_chapter else {
        return Ok(None);
    };

    let history = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(previous_chapter.id.clone())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| LoadGenerationContextError::Internal(error.to_string()))?;

    Ok(history
        .as_ref()
        .and_then(|item| item.generated_content.as_deref())
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
        .and_then(|payload| generated_history_runtime_snapshot_from_payload(&payload)))
}

fn build_single_generation_story_packet(
    project_model: &project::Model,
    chapter_model: &chapter::Model,
    previous_story_runtime_snapshot: Option<&Value>,
    project_continuity_ledger: Option<&ProjectContinuityLedger>,
) -> Value {
    let mut packet = serde_json::Map::new();
    packet.insert(
        "source".to_string(),
        json!("single_generation_active_route"),
    );
    packet.insert("project_id".to_string(), json!(project_model.id.clone()));
    packet.insert("chapter_id".to_string(), json!(chapter_model.id.clone()));
    packet.insert(
        "current_chapter_number".to_string(),
        json!(chapter_model.chapter_number),
    );
    packet.insert(
        "chapter_count".to_string(),
        project_model
            .chapter_count
            .map_or(Value::Null, |value| json!(value)),
    );
    packet.insert(
        "target_word_count".to_string(),
        json!(project_model.target_words),
    );

    if let Some(long_term_goal) = build_project_long_term_goal(
        project_model.theme.as_deref(),
        project_model.description.as_deref(),
        project_model.default_story_creation_brief.as_deref(),
        project_model
            .chapter_count
            .and_then(|value| usize::try_from(value).ok()),
        usize::try_from(project_model.target_words).ok(),
    ) {
        packet.insert("story_long_term_goal".to_string(), json!(long_term_goal));
    }

    if let Some(snapshot) = previous_story_runtime_snapshot.and_then(Value::as_object) {
        for field_name in [
            "story_long_term_goal",
            "character_focus",
            "foreshadow_payoff_plan",
            "character_state_ledger",
            "relationship_state_ledger",
            "foreshadow_state_ledger",
            "organization_state_ledger",
            "career_state_ledger",
        ] {
            if let Some(value) = snapshot
                .get(field_name)
                .cloned()
                .filter(|value| !value.is_null())
            {
                packet.insert(field_name.to_string(), value);
            }
        }
    }

    if let Some(project_continuity_ledger) = project_continuity_ledger {
        project_continuity_ledger.fill_missing_story_packet_ledgers(&mut packet);
    }

    Value::Object(packet)
}

pub(crate) async fn generate_and_persist_chapter_content_with_candidate_route_gateway(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    ai_config: AIConfig,
    gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<GeneratedChapterResult, String> {
    load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?
        .generate_and_persist_with_candidate_route_gateway(
            db,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{build_single_generation_story_packet, load_generation_context};
    use crate::models::{
        career, chapter, character, character_career, generation_history, organization,
        plot_analysis, project, relationship, story_memory,
    };
    use crate::services::chapter_generation_runtime_service::story_continuity_ledger_owner::{
        ProjectContinuityLedger, ProjectContinuityLedgerEntry,
    };
    use chrono::{NaiveDate, NaiveDateTime, Utc};
    use sea_orm::{
        ConnectionTrait, Database, DatabaseBackend, EntityTrait, IntoActiveModel, Schema,
    };
    use serde_json::json;

    fn build_project() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "Project".to_string(),
            description: Some("desc".to_string()),
            theme: Some("命运与代价".to_string()),
            genre: None,
            target_words: 50000,
            current_words: 1200,
            status: "draft".to_string(),
            wizard_status: "idle".to_string(),
            wizard_step: 0,
            outline_mode: "simple".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(12),
            narrative_perspective: None,
            character_count: 3,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: Some("围绕主线秘密持续升级代价".to_string()),
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-2".to_string(),
            project_id: "project-1".to_string(),
            title: "第二章".to_string(),
            chapter_number: 2,
            content: None,
            summary: None,
            expansion_plan: None,
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_build_single_generation_story_packet_from_previous_runtime_snapshot() {
        let packet = build_single_generation_story_packet(
            &build_project(),
            &build_chapter(),
            Some(&json!({
                "story_long_term_goal": "追回主线伏笔",
                "character_focus": ["沈砚", "苏槿"],
                "foreshadow_payoff_plan": ["回收旧约定"],
                "character_state_ledger": [{"label": "沈砚", "summary": "情绪收紧"}],
                "relationship_state_ledger": [{"label": "沈砚/苏槿", "summary": "互相试探"}],
                "foreshadow_state_ledger": [{"label": "旧约定", "summary": "尚未兑现"}],
                "organization_state_ledger": [{"label": "夜巡司", "summary": "开始施压"}],
                "career_state_ledger": [{"label": "沈砚/夜巡人", "summary": "晋升受阻"}]
            })),
            None,
        );

        assert_eq!(packet["source"], "single_generation_active_route");
        assert_eq!(packet["current_chapter_number"], 2);
        assert_eq!(packet["chapter_count"], 12);
        assert_eq!(packet["target_word_count"], 50000);
        assert_eq!(packet["story_long_term_goal"], "追回主线伏笔");
        assert_eq!(packet["character_focus"][0], "沈砚");
        assert_eq!(packet["foreshadow_payoff_plan"][0], "回收旧约定");
        assert_eq!(packet["organization_state_ledger"][0]["label"], "夜巡司");
    }

    #[test]
    fn should_fill_missing_story_packet_ledgers_from_project_continuity_ledger() {
        let packet = build_single_generation_story_packet(
            &build_project(),
            &build_chapter(),
            Some(&json!({
                "character_state_ledger": [{"label": "快照角色", "summary": "保留快照优先级"}],
                "relationship_state_ledger": []
            })),
            Some(&ProjectContinuityLedger {
                character_state_ledger: vec![ledger_entry("DB角色", "不应覆盖快照")],
                relationship_state_ledger: vec![ledger_entry("林河/白露", "互相隐瞒代价")],
                foreshadow_state_ledger: vec![ledger_entry("断裂的铜钥匙", "尚未兑现")],
                organization_state_ledger: vec![ledger_entry("白塔", "封锁港口")],
                career_state_ledger: vec![ledger_entry("林河/剑修", "stage 4")],
            }),
        );

        assert_eq!(packet["character_state_ledger"][0]["label"], "快照角色");
        assert_eq!(packet["relationship_state_ledger"][0]["label"], "林河/白露");
        assert_eq!(
            packet["foreshadow_state_ledger"][0]["label"],
            "断裂的铜钥匙"
        );
        assert_eq!(packet["organization_state_ledger"][0]["label"], "白塔");
        assert_eq!(packet["career_state_ledger"][0]["summary"], "stage 4");
    }

    fn ledger_entry(label: &str, summary: &str) -> ProjectContinuityLedgerEntry {
        ProjectContinuityLedgerEntry {
            label: Some(label.to_string()),
            summary: Some(summary.to_string()),
            status: None,
            target_chapter: None,
        }
    }

    #[tokio::test]
    async fn should_load_logged_in_story_packet_with_db_backed_continuity_ledger() {
        let db = setup_runtime_context_db().await;
        seed_runtime_context_project_and_chapters(&db).await;
        seed_previous_runtime_snapshot(&db).await;
        seed_runtime_context_continuity_sources(&db).await;

        let context = load_generation_context(&db, "user-1", "chapter-current")
            .await
            .expect("load logged-in generation context");

        assert_eq!(context.chapter_model.id, "chapter-current");
        assert_eq!(context.project_model.id, "project-1");
        assert_eq!(
            context
                .previous_chapter
                .as_ref()
                .map(|chapter| chapter.id.as_str()),
            Some("chapter-prev")
        );
        assert_eq!(
            context.story_packet["source"],
            "single_generation_active_route"
        );
        assert_eq!(context.story_packet["project_id"], "project-1");
        assert_eq!(context.story_packet["chapter_id"], "chapter-current");

        assert_eq!(
            context.story_packet["character_state_ledger"][0]["label"],
            "快照角色"
        );
        assert_eq!(
            context.story_packet["relationship_state_ledger"][0]["label"],
            "林河/白露"
        );
        assert_eq!(
            context.story_packet["relationship_state_ledger"][0]["summary"],
            "盟友; 互相隐瞒代价"
        );
        assert_eq!(
            context.story_packet["foreshadow_state_ledger"][0]["label"],
            "断裂的铜钥匙"
        );
        assert_eq!(
            context.story_packet["organization_state_ledger"][0]["label"],
            "白塔"
        );
        assert_eq!(
            context.story_packet["career_state_ledger"][0]["summary"],
            "stage 4; progress 60%"
        );
    }

    async fn setup_runtime_context_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let schema = Schema::new(DatabaseBackend::Sqlite);
        let builder = db.get_database_backend();
        for statement in [
            builder.build(&schema.create_table_from_entity(project::Entity)),
            builder.build(&schema.create_table_from_entity(chapter::Entity)),
            builder.build(&schema.create_table_from_entity(generation_history::Entity)),
            builder.build(&schema.create_table_from_entity(character::Entity)),
            builder.build(&schema.create_table_from_entity(relationship::Entity)),
            builder.build(&schema.create_table_from_entity(story_memory::Entity)),
            builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
            builder.build(&schema.create_table_from_entity(organization::Entity)),
            builder.build(&schema.create_table_from_entity(career::Entity)),
            builder.build(&schema.create_table_from_entity(character_career::Entity)),
        ] {
            db.execute(statement)
                .await
                .expect("create runtime context table");
        }
        db
    }

    async fn seed_runtime_context_project_and_chapters(db: &sea_orm::DatabaseConnection) {
        project::Entity::insert(build_project().into_active_model())
            .exec(db)
            .await
            .expect("insert project");

        chapter::Entity::insert(
            chapter::Model {
                id: "chapter-prev".to_string(),
                project_id: "project-1".to_string(),
                title: "第一章".to_string(),
                chapter_number: 1,
                content: Some("上一章内容".to_string()),
                summary: Some("上一章摘要".to_string()),
                expansion_plan: None,
                status: "completed".to_string(),
                word_count: 4,
                outline_id: None,
                sub_index: 0,
                created_at: dt(1),
                updated_at: Some(dt(1)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert previous chapter");

        chapter::Entity::insert(
            chapter::Model {
                id: "chapter-current".to_string(),
                project_id: "project-1".to_string(),
                title: "第二章".to_string(),
                chapter_number: 2,
                content: None,
                summary: None,
                expansion_plan: None,
                status: "pending".to_string(),
                word_count: 0,
                outline_id: None,
                sub_index: 0,
                created_at: dt(2),
                updated_at: Some(dt(2)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert current chapter");
    }

    async fn seed_previous_runtime_snapshot(db: &sea_orm::DatabaseConnection) {
        generation_history::Entity::insert(
            generation_history::Model {
                id: "history-prev".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: Some("chapter-prev".to_string()),
                prompt: Some("prompt".to_string()),
                generated_content: Some(
                    json!({
                        "story_runtime_snapshot": {
                            "character_state_ledger": [
                                {"label": "快照角色", "summary": "保留快照优先级"}
                            ],
                            "relationship_state_ledger": [],
                            "foreshadow_state_ledger": [],
                            "organization_state_ledger": [],
                            "career_state_ledger": []
                        }
                    })
                    .to_string(),
                ),
                model: Some("test-model".to_string()),
                tokens_used: Some(12),
                generation_time: Some(0.1),
                created_at: Some(dt(3)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert generation history");
    }

    async fn seed_runtime_context_continuity_sources(db: &sea_orm::DatabaseConnection) {
        character::Entity::insert(
            character::Model {
                id: "char-1".to_string(),
                project_id: "project-1".to_string(),
                name: "林河".to_string(),
                age: None,
                gender: None,
                is_organization: false,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "injured".to_string(),
                status_changed_chapter: Some(4),
                current_state: Some("灵力受损 仍保留铜钥匙".to_string()),
                state_updated_chapter: Some(9),
                main_career_id: Some("career-main".to_string()),
                main_career_stage: Some(3),
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: dt(4),
                updated_at: Some(dt(9)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert character");

        character::Entity::insert(
            character::Model {
                id: "char-2".to_string(),
                project_id: "project-1".to_string(),
                name: "白露".to_string(),
                age: None,
                gender: None,
                is_organization: false,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: Some("守住北港入口".to_string()),
                state_updated_chapter: Some(7),
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: dt(5),
                updated_at: Some(dt(7)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert second character");

        character::Entity::insert(
            character::Model {
                id: "org-char".to_string(),
                project_id: "project-1".to_string(),
                name: "白塔".to_string(),
                age: None,
                gender: None,
                is_organization: true,
                role_type: None,
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: Some("封锁港口".to_string()),
                state_updated_chapter: Some(8),
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: dt(6),
                updated_at: Some(dt(8)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert organization character");

        relationship::Entity::insert(
            relationship::Model {
                id: "rel-1".to_string(),
                project_id: "project-1".to_string(),
                character_from_id: "char-1".to_string(),
                character_to_id: "char-2".to_string(),
                relationship_type_id: None,
                relationship_name: Some("盟友".to_string()),
                intimacy_level: 6,
                status: "strained".to_string(),
                description: Some("互相隐瞒代价".to_string()),
                started_at: None,
                ended_at: None,
                source: "manual".to_string(),
                created_at: dt(7),
                updated_at: Some(dt(9)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert relationship");

        story_memory::Entity::insert(
            story_memory::Model {
                id: "memory-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: None,
                memory_type: "foreshadow".to_string(),
                title: Some("断裂的铜钥匙".to_string()),
                content: "断裂的铜钥匙藏在祭坛下方".to_string(),
                full_context: None,
                related_characters: None,
                related_locations: None,
                tags: None,
                importance_score: Some(0.9),
                story_timeline: 5,
                chapter_position: 0,
                text_length: 18,
                is_foreshadow: 1,
                foreshadow_resolved_at: None,
                foreshadow_strength: Some(0.7),
                vector_id: None,
                embedding_model: None,
                created_at: Some(dt(8)),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert memory");

        organization::Entity::insert(
            organization::Model {
                id: "org-1".to_string(),
                character_id: "org-char".to_string(),
                project_id: "project-1".to_string(),
                parent_org_id: None,
                level: 2,
                power_level: 8,
                member_count: 30,
                location: Some("北港".to_string()),
                motto: None,
                color: None,
                created_at: dt(9),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert organization");

        career::Entity::insert(
            career::Model {
                id: "career-main".to_string(),
                project_id: "project-1".to_string(),
                name: "剑修".to_string(),
                career_type: "main".to_string(),
                description: None,
                category: None,
                stages: "[]".to_string(),
                max_stage: 9,
                requirements: None,
                special_abilities: None,
                worldview_rules: None,
                attribute_bonuses: None,
                source: "manual".to_string(),
                created_at: dt(10),
                updated_at: Some(dt(10)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert career");

        character_career::Entity::insert(
            character_career::Model {
                id: "char-career-1".to_string(),
                character_id: "char-1".to_string(),
                career_id: "career-main".to_string(),
                career_type: "main".to_string(),
                current_stage: 4,
                stage_progress: Some(60),
                started_at: None,
                reached_current_stage_at: None,
                notes: Some("突破失败".to_string()),
                created_at: dt(11),
                updated_at: Some(dt(12)),
            }
            .into_active_model(),
        )
        .exec(db)
        .await
        .expect("insert character career");
    }

    fn dt(day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, day)
            .expect("valid test date")
            .and_hms_opt(0, 0, 0)
            .expect("valid test time")
    }
}
