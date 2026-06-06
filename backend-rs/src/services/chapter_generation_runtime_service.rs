use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::ai::types::AIResponse;
use crate::models::{chapter, generation_history, project};
use crate::services::chapter_generation_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_generation_context_compaction_service::compact_generation_context;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_prompt_service::{
    build_previous_chapter_prompt_context, build_prompt_with_provider_payload,
    ChapterGenerationPromptOverrides, PreviousChapterPromptContext,
};

const CHAPTER_GENERATION_HISTORY_MODEL: &str = "chapter_generation_v1";
const CHAPTER_GENERATION_HISTORY_LOG_TYPE: &str = "chapter_generation_quality_v1";
const CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoadGenerationContextError {
    Chapter(LoadAccessibleChapterForGenerationError),
    ProjectNotFound,
    Internal(String),
}

impl LoadGenerationContextError {
    fn into_runtime_message(self) -> String {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedChapterResult {
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) word_count: i32,
}

fn build_generated_history_preview(content: &str) -> String {
    content
        .chars()
        .take(CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH)
        .collect()
}

fn build_generated_history_story_runtime_snapshot_from_contract(
    story_runtime_contract: &Value,
) -> Option<Value> {
    let guidance = story_runtime_contract
        .get("guidance")
        .and_then(Value::as_object);
    let blueprint = story_runtime_contract
        .get("blueprint")
        .and_then(Value::as_object);
    if guidance.is_none() && blueprint.is_none() {
        return None;
    }

    let mut snapshot = serde_json::Map::new();
    if let Some(guidance) = guidance {
        for field_name in [
            "creative_mode",
            "story_focus",
            "plot_stage",
            "story_creation_brief",
            "quality_preset",
            "quality_notes",
        ] {
            if let Some(value) = guidance
                .get(field_name)
                .cloned()
                .filter(|value| !value.is_null())
            {
                snapshot.insert(field_name.to_string(), value);
            }
        }
    }

    if let Some(blueprint) = blueprint {
        snapshot.insert(
            "story_long_term_goal".to_string(),
            blueprint
                .get("long_term_goal")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        snapshot.insert(
            "chapter_count".to_string(),
            blueprint
                .get("chapter_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "current_chapter_number".to_string(),
            blueprint
                .get("current_chapter_number")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "target_word_count".to_string(),
            blueprint
                .get("target_word_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "character_focus".to_string(),
            blueprint
                .get("character_focus_names")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_payoff_plan".to_string(),
            blueprint
                .get("foreshadow_payoff_plan")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "character_state_ledger".to_string(),
            blueprint
                .get("character_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "relationship_state_ledger".to_string(),
            blueprint
                .get("relationship_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_state_ledger".to_string(),
            blueprint
                .get("foreshadow_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "organization_state_ledger".to_string(),
            blueprint
                .get("organization_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "career_state_ledger".to_string(),
            blueprint
                .get("career_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
    }

    (!snapshot.is_empty()).then_some(Value::Object(snapshot))
}

fn generated_history_story_runtime_contract(quality_metrics: Option<&Value>) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("story_runtime_contract"))
        .filter(|payload| payload.is_object())
        .cloned()
}

fn generated_history_story_runtime_snapshot(
    quality_metrics: Option<&Value>,
    story_runtime_contract: Option<&Value>,
) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("quality_runtime_context"))
        .and_then(|payload| payload.as_object().filter(|payload| !payload.is_empty()))
        .map(|payload| Value::Object(payload.clone()))
        .or_else(|| {
            story_runtime_contract
                .and_then(build_generated_history_story_runtime_snapshot_from_contract)
        })
}

fn build_generated_chapter_quality_history_payload(
    content: &str,
    quality_metrics: Option<&Value>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    let story_runtime_contract = generated_history_story_runtime_contract(quality_metrics);
    let story_runtime_snapshot =
        generated_history_story_runtime_snapshot(quality_metrics, story_runtime_contract.as_ref());

    let mut payload = serde_json::Map::from_iter([
        (
            "log_type".to_string(),
            json!(CHAPTER_GENERATION_HISTORY_LOG_TYPE),
        ),
        ("content".to_string(), json!(content)),
        (
            "preview".to_string(),
            json!(build_generated_history_preview(content)),
        ),
        (
            "quality_metrics".to_string(),
            quality_metrics.cloned().unwrap_or(Value::Null),
        ),
        (
            "generated_at".to_string(),
            json!(created_at.format("%Y-%m-%dT%H:%M:%S").to_string()),
        ),
        ("content_applied".to_string(), json!(true)),
        ("attempt_state".to_string(), json!("generated_from_runtime")),
    ]);

    if let Some(story_runtime_snapshot) = story_runtime_snapshot {
        payload.insert("story_runtime_snapshot".to_string(), story_runtime_snapshot);
    }
    if let Some(story_runtime_contract) = story_runtime_contract {
        payload.insert("story_runtime_contract".to_string(), story_runtime_contract);
    }

    Value::Object(payload)
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterGenerationRuntimeContext {
    chapter_model: chapter::Model,
    project_model: project::Model,
    previous_chapter: Option<chapter::Model>,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
}

impl ChapterGenerationRuntimeContext {
    fn build_generated_history_payload(
        result: &GeneratedChapterResult,
        created_at: chrono::NaiveDateTime,
    ) -> Value {
        build_generated_chapter_quality_history_payload(&result.content, None, created_at)
    }

    fn build_generated_history_model(
        &self,
        prompt: String,
        result: &GeneratedChapterResult,
        created_at: chrono::NaiveDateTime,
    ) -> generation_history::ActiveModel {
        generation_history::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(self.chapter_model.project_id.clone()),
            chapter_id: Set(Some(self.chapter_model.id.clone())),
            prompt: Set(Some(prompt)),
            generated_content: Set(Some(
                Self::build_generated_history_payload(result, created_at).to_string(),
            )),
            model: Set(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
            tokens_used: Set(None),
            generation_time: Set(None),
            created_at: Set(Some(created_at)),
        }
    }

    fn build_generated_result(&self, response: AIResponse) -> GeneratedChapterResult {
        let cleaned_content = response.content.trim().to_string();
        let word_count = cleaned_content.chars().count() as i32;

        GeneratedChapterResult {
            chapter_id: self.chapter_model.id.clone(),
            chapter_number: self.chapter_model.chapter_number,
            title: self.chapter_model.title.clone(),
            content: cleaned_content,
            word_count,
        }
    }

    async fn persist_generated_result(
        self,
        db: &DatabaseConnection,
        prompt: String,
        result: GeneratedChapterResult,
    ) -> Result<GeneratedChapterResult, String> {
        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(|error| error.to_string())?;

        let history = self.build_generated_history_model(prompt, &result, now);
        let mut active: chapter::ActiveModel = self.chapter_model.into();
        active.content = Set(Some(result.content.clone()));
        active.word_count = Set(result.word_count);
        active.status = Set("draft".to_string());
        active.updated_at = Set(Some(now));
        active
            .update(&txn)
            .await
            .map_err(|error| error.to_string())?;

        history
            .insert(&txn)
            .await
            .map_err(|error| error.to_string())?;

        txn.commit().await.map_err(|error| error.to_string())?;

        Ok(result)
    }

    fn build_prompt(
        &self,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
    ) -> Result<String, String> {
        let (provider_payload, previous_chapter_prompt_context) = compact_generation_context(
            &self.project_model.outline_mode,
            target_word_count,
            provider_payload,
            self.previous_chapter_prompt_context.clone(),
        );
        build_prompt_with_provider_payload(
            &self.chapter_model,
            &self.project_model,
            previous_chapter_prompt_context,
            self.previous_chapter.is_some(),
            target_word_count,
            provider_payload,
            overrides,
        )
    }

    async fn generate_and_persist(
        self,
        db: &DatabaseConnection,
        user_ai_service: &AIService,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
    ) -> Result<GeneratedChapterResult, String> {
        let prompt = self.build_prompt(target_word_count, provider_payload, overrides)?;
        let response = user_ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        let generated_result = self.build_generated_result(response);
        self.persist_generated_result(db, prompt, generated_result)
            .await
    }
}

pub(crate) fn build_generated_chapter_history_payload_with_quality_metrics(
    content: &str,
    quality_metrics: Option<&Value>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    build_generated_chapter_quality_history_payload(content, quality_metrics, created_at)
}

pub(crate) async fn update_latest_generated_chapter_history_quality_metrics(
    db: &DatabaseConnection,
    chapter_id: &str,
    content: &str,
    quality_metrics: &Value,
) -> Result<(), String> {
    let Some(history_model) = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .filter(
            generation_history::Column::Model
                .eq(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
        )
        .order_by_desc(generation_history::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };

    let mut active: generation_history::ActiveModel = history_model.into();
    let created_at = active
        .created_at
        .clone()
        .take()
        .flatten()
        .unwrap_or_else(|| Utc::now().naive_utc());
    active.generated_content = Set(Some(
        build_generated_chapter_history_payload_with_quality_metrics(
            content,
            Some(quality_metrics),
            created_at,
        )
        .to_string(),
    ));
    active.update(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn load_generation_context(
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

    Ok(ChapterGenerationRuntimeContext {
        chapter_model,
        project_model,
        previous_chapter,
        previous_chapter_prompt_context,
    })
}

pub async fn generate_and_persist_chapter_content_with_provider_payload(
    db: &DatabaseConnection,
    user_ai_service: &AIService,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<GeneratedChapterResult, String> {
    load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?
        .generate_and_persist(
            db,
            user_ai_service,
            target_word_count,
            provider_payload,
            overrides,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        build_generated_chapter_history_payload_with_quality_metrics,
        build_previous_chapter_prompt_context, ChapterGenerationRuntimeContext,
        GeneratedChapterResult, LoadGenerationContextError, CHAPTER_GENERATION_HISTORY_MODEL,
        CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH,
    };
    use crate::ai::types::AIResponse;
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_access_service::LoadAccessibleChapterForGenerationError;
    use chrono::Utc;
    use serde_json::json;

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
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
    fn should_build_generated_chapter_result_from_runtime_context_owner() {
        let chapter_model = build_chapter();
        let response = AIResponse {
            content: "  你好\n世界  ".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let context = ChapterGenerationRuntimeContext {
            chapter_model,
            project_model: project::Model {
                id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                title: "Project".to_string(),
                description: Some("desc".to_string()),
                theme: None,
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
                default_story_creation_brief: None,
                default_quality_preset: None,
                default_quality_notes: None,
                created_at: Utc::now().naive_utc(),
                updated_at: None,
            },
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
        };

        let result = context.build_generated_result(response);

        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
    }

    #[test]
    fn should_keep_generated_chapter_result_transport_fields() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
        };

        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
        assert_eq!(result.title, "第一章");
        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
    }

    #[test]
    fn should_build_generated_history_payload_with_quality_metrics_placeholder() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
        };

        let created_at = Utc::now().naive_utc();
        let payload =
            ChapterGenerationRuntimeContext::build_generated_history_payload(&result, created_at);

        assert_eq!(
            payload,
            json!({
                "log_type": "chapter_generation_quality_v1",
                "content": "你好\n世界",
                "preview": "你好\n世界",
                "quality_metrics": null,
                "generated_at": created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "content_applied": true,
                "attempt_state": "generated_from_runtime",
            })
        );
    }

    #[test]
    fn should_keep_chapter_generation_history_model_owner_constant() {
        assert_eq!(CHAPTER_GENERATION_HISTORY_MODEL, "chapter_generation_v1");
    }

    #[test]
    fn should_build_generated_history_payload_with_runtime_quality_metrics() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 8.4,
                "repair_guidance": {
                    "summary": "压缩说明段"
                },
                "quality_gate": {
                    "decision": "auto_repair"
                }
            })),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(payload["content"], "正文");
        assert_eq!(payload["preview"], "正文");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(payload["quality_metrics"]["overall_score"], 8.4);
        assert_eq!(
            payload["quality_metrics"]["repair_guidance"]["summary"],
            "压缩说明段"
        );
        assert_eq!(
            payload["quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["attempt_state"], "generated_from_runtime");
    }

    #[test]
    fn should_keep_python_compat_runtime_history_metadata_when_contract_exists() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "章节正文".repeat(260).as_str(),
            Some(&json!({
                "overall_score": 8.8,
                "quality_runtime_context": {
                    "scope": "chapter",
                    "source": "plot_analysis"
                },
                "story_runtime_contract": {
                    "guidance": {
                        "creative_mode": "hook"
                    },
                    "blueprint": {
                        "current_chapter_number": 6,
                        "target_word_count": 2800
                    }
                }
            })),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(
            payload["preview"]
                .as_str()
                .expect("preview text")
                .chars()
                .count(),
            CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH
        );
        assert_eq!(
            payload["story_runtime_snapshot"],
            json!({
                "scope": "chapter",
                "source": "plot_analysis"
            })
        );
        assert_eq!(
            payload["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
    }

    #[test]
    fn should_derive_story_runtime_snapshot_from_contract_when_metrics_context_missing() {
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 7.6,
                "story_runtime_contract": {
                    "guidance": {
                        "story_focus": "advance_plot",
                        "plot_stage": "climax"
                    },
                    "blueprint": {
                        "long_term_goal": "追回失落线索",
                        "chapter_count": 12,
                        "current_chapter_number": 5,
                        "target_word_count": 2600,
                        "character_focus_names": ["沈砚"],
                        "foreshadow_payoff_plan": ["回收暗号"],
                        "character_state_ledger": [],
                        "relationship_state_ledger": [],
                        "foreshadow_state_ledger": [],
                        "organization_state_ledger": [],
                        "career_state_ledger": []
                    }
                }
            })),
            Utc::now().naive_utc(),
        );

        assert_eq!(
            payload["story_runtime_snapshot"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(payload["story_runtime_snapshot"]["plot_stage"], "climax");
        assert_eq!(
            payload["story_runtime_snapshot"]["story_long_term_goal"],
            "追回失落线索"
        );
        assert_eq!(payload["story_runtime_snapshot"]["chapter_count"], 12);
        assert_eq!(
            payload["story_runtime_snapshot"]["current_chapter_number"],
            5
        );
        assert_eq!(payload["story_runtime_snapshot"]["target_word_count"], 2600);
        assert_eq!(
            payload["story_runtime_snapshot"]["character_focus"],
            json!(["沈砚"])
        );
        assert_eq!(
            payload["story_runtime_snapshot"]["foreshadow_payoff_plan"],
            json!(["回收暗号"])
        );
    }

    #[test]
    fn should_map_generation_context_access_denied_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            )
            .into_runtime_message(),
            "Chapter not found or access denied"
        );
    }

    #[test]
    fn should_map_generation_context_project_missing_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::ProjectNotFound.into_runtime_message(),
            "Project not found"
        );
    }

    #[test]
    fn should_keep_generation_context_internal_owner() {
        assert_eq!(
            LoadGenerationContextError::Internal("boom".to_string()).into_runtime_message(),
            "boom"
        );
    }

    #[test]
    fn should_keep_chapter_generation_runtime_context_owner_contract() {
        let context = ChapterGenerationRuntimeContext {
            chapter_model: build_chapter(),
            project_model: project::Model {
                id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                title: "Project".to_string(),
                description: Some("desc".to_string()),
                theme: None,
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
                default_story_creation_brief: None,
                default_quality_preset: None,
                default_quality_notes: None,
                created_at: Utc::now().naive_utc(),
                updated_at: None,
            },
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
        };

        assert_eq!(context.chapter_model.id, "chapter-1");
        assert_eq!(context.project_model.id, "project-1");
        assert_eq!(context.previous_chapter, None);
    }
}
