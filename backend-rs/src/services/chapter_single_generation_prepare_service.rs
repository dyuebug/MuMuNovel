use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::chapter_batch_generation_task_model_service::build_batch_generation_task_active_model;
use crate::services::chapter_generation_execution_config_service::prepare_generation_execution_config_with_provider_payload;
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::settings_service::SettingsService;
use crate::services::chapter_single_generation_runtime_checkpoint_service::{
    build_single_generation_runtime_checkpoint_for_stage, SingleGenerationSnapshotStage,
};

use super::chapter_batch_generation_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationRequest {
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
    pub(crate) enable_analysis: Option<bool>,
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Option<Vec<String>>,
    pub(crate) story_preserve_strengths: Option<Vec<String>>,
}

impl SingleChapterGenerationRequest {
    pub(crate) fn from_route_payload(
        style_id: Option<i32>,
        target_word_count: Option<i32>,
        model: Option<String>,
        enable_analysis: Option<bool>,
        enable_mcp: Option<bool>,
        enable_web_research: Option<bool>,
        web_research_query: Option<String>,
        narrative_perspective: Option<String>,
        creative_mode: Option<String>,
        story_focus: Option<String>,
        plot_stage: Option<String>,
        story_creation_brief: Option<String>,
        quality_preset: Option<String>,
        quality_notes: Option<String>,
        story_repair_summary: Option<String>,
        story_repair_targets: Option<Vec<String>>,
        story_preserve_strengths: Option<Vec<String>>,
    ) -> Self {
        Self {
            style_id,
            target_word_count,
            model,
            enable_analysis,
            enable_mcp,
            enable_web_research,
            web_research_query,
            narrative_perspective,
            creative_mode,
            story_focus,
            plot_stage,
            story_creation_brief,
            quality_preset,
            quality_notes,
            story_repair_summary,
            story_repair_targets,
            story_preserve_strengths,
        }
    }

    pub(crate) fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: self.enable_analysis.unwrap_or(true),
            enable_mcp: self.enable_mcp.unwrap_or(true),
            web_research_enabled: normalize_single_generation_web_research_enabled(
                self.enable_web_research,
                web_research_default,
            ),
            web_research_query: self.web_research_query.clone(),
            narrative_perspective: self.narrative_perspective.clone(),
            creative_mode: self.creative_mode.clone(),
            story_focus: self.story_focus.clone(),
            plot_stage: self.plot_stage.clone(),
            story_creation_brief: self.story_creation_brief.clone(),
            quality_preset: self.quality_preset.clone(),
            quality_notes: self.quality_notes.clone(),
            story_repair_summary: self.story_repair_summary.clone(),
            story_repair_targets: self.story_repair_targets.clone().unwrap_or_default(),
            story_preserve_strengths: self
                .story_preserve_strengths
                .clone()
                .unwrap_or_default(),
        }
    }
}

fn normalize_single_generation_web_research_enabled(
    enabled: Option<bool>,
    default_enabled: bool,
) -> bool {
    enabled.unwrap_or(default_enabled)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SingleChapterGenerationCompatOptions {
    pub(crate) style_id: Option<i32>,
    pub(crate) enable_analysis: bool,
    pub(crate) enable_mcp: bool,
    pub(crate) web_research_enabled: bool,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
}

impl SingleChapterGenerationCompatOptions {
    pub(crate) fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    pub(crate) fn enable_analysis(&self) -> bool {
        self.enable_analysis
    }

    pub(crate) fn enable_mcp(&self) -> bool {
        self.enable_mcp
    }

    pub(crate) fn web_research_enabled(&self) -> bool {
        self.web_research_enabled
    }

    pub(crate) fn web_research_query(&self) -> Option<&str> {
        self.web_research_query.as_deref()
    }

    pub(crate) fn narrative_perspective(&self) -> &str {
        self.narrative_perspective.as_deref().unwrap_or_default()
    }

    pub(crate) fn creative_mode(&self) -> &str {
        self.creative_mode.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_focus(&self) -> &str {
        self.story_focus.as_deref().unwrap_or_default()
    }

    pub(crate) fn plot_stage(&self) -> &str {
        self.plot_stage.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_creation_brief(&self) -> &str {
        self.story_creation_brief.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_preset(&self) -> &str {
        self.quality_preset.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_notes(&self) -> &str {
        self.quality_notes.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_summary(&self) -> &str {
        self.story_repair_summary.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_targets(&self) -> &[String] {
        &self.story_repair_targets
    }

    pub(crate) fn story_preserve_strengths(&self) -> &[String] {
        &self.story_preserve_strengths
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationExecutionInput {
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) execution_config:
        crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
}

#[derive(Debug)]
pub(crate) struct SingleChapterGenerationTarget {
    pub(crate) project_id: String,
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
}

impl SingleChapterGenerationTarget {
    pub(crate) fn pending_checkpoint(&self) -> Value {
        build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Pending,
            &self.chapter_id,
            Some(self.chapter_number),
            None,
        )
    }

    pub(crate) fn background_response_payload(
        &self,
        task_id: &str,
        estimated_minutes: i32,
    ) -> Value {
        json!({
            "task_id": task_id,
            "chapter_id": self.chapter_id,
            "status": "pending",
            "message": "单章后台生成任务已创建",
            "estimated_time_minutes": estimated_minutes,
            "active_story_repair_payload": null,
        })
    }

    pub(crate) fn background_task_active_model(
        &self,
        task_id: String,
        user_id: String,
        target_word_count: i32,
        now: chrono::NaiveDateTime,
    ) -> crate::models::batch_generation_task::ActiveModel {
        build_batch_generation_task_active_model(
            task_id,
            self.project_id.clone(),
            user_id,
            self.chapter_number,
            1,
            json!([{
                "id": self.chapter_id,
                "chapter_number": self.chapter_number,
                "title": self.title,
            }]),
            None,
            target_word_count,
            false,
            1,
            Some(self.chapter_id.clone()),
            Some(self.chapter_number),
            0,
            now,
        )
    }
}

#[derive(Debug)]
pub(crate) enum PrepareSingleChapterGenerationRequestError {
    Chapter(LoadAccessibleChapterForGenerationError),
    Config(String),
    Internal(String),
}

pub(crate) async fn prepare_single_chapter_generation_request(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: &SingleChapterGenerationRequest,
) -> Result<
    (
        SingleChapterGenerationTarget,
        SingleChapterGenerationExecutionInput,
    ),
    PrepareSingleChapterGenerationRequestError,
> {
    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| PrepareSingleChapterGenerationRequestError::Config(error.to_string()))?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Chapter)?;
    let chapter_target = SingleChapterGenerationTarget {
        project_id: chapter_model.project_id,
        chapter_id: chapter_model.id,
        chapter_number: chapter_model.chapter_number,
        title: chapter_model.title,
    };
    let provider_payload =
        build_single_chapter_research_provider_payload(
            db,
            user_id,
            &chapter_target,
            &compat_options,
        )
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Config)?;
    let execution_config =
        prepare_generation_execution_config_with_provider_payload(
            db,
            user_id,
            request.model.as_deref(),
            provider_payload,
        )
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Config)?;

    Ok((
        chapter_target,
        SingleChapterGenerationExecutionInput {
            target_word_count: normalize_chapter_generation_target_word_count(
                request.target_word_count,
            ),
            compat_options,
            execution_config,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
        SingleChapterGenerationRequest,
        SingleChapterGenerationTarget,
    };
    use crate::models::chapter;
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn should_normalize_single_chapter_generation_target_word_count() {
        assert_eq!(normalize_chapter_generation_target_word_count(None), 3000);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(normalize_chapter_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_load_single_chapter_generation_target_from_request() {
        let request = SingleChapterGenerationRequest {
            style_id: None,
            target_word_count: Some(1800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        };

        assert_eq!(
            normalize_chapter_generation_target_word_count(request.target_word_count),
            1800
        );
    }

    #[test]
    fn should_keep_single_chapter_generation_execution_input_contract() {
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: SingleChapterGenerationCompatOptions {
                style_id: None,
                enable_analysis: true,
                enable_mcp: true,
                web_research_enabled: false,
                web_research_query: None,
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
            },
        };

        assert_eq!(execution_input.target_word_count, 2600);
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .external_assets,
            "[]"
        );
    }

    #[test]
    fn should_keep_single_chapter_generation_target_projection_contract() {
        let chapter_model = chapter::Model {
            id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
            content: Some("content".to_string()),
            summary: Some("summary".to_string()),
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        };

        let target = SingleChapterGenerationTarget {
            project_id: chapter_model.project_id.clone(),
            chapter_id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        };

        assert_eq!(target.project_id, "project-1");
        assert_eq!(target.chapter_id, "chapter-7");
        assert_eq!(target.chapter_number, 7);
        assert_eq!(target.title, "Seven");
    }

    #[test]
    fn should_build_single_chapter_generation_target_payloads_from_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = target.pending_checkpoint();
        let response_payload = target.background_response_payload("task-1", 2);
        let active_model = target.background_task_active_model(
            "task-1".to_string(),
            "user-1".to_string(),
            2600,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
        assert_eq!(active_model.target_word_count, sea_orm::Set(2600));
        assert_eq!(active_model.chapter_ids, sea_orm::Set(json!([{
            "id": "chapter-7",
            "chapter_number": 7,
            "title": "Seven",
        }])));
        assert_eq!(
            active_model.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_build_single_chapter_generation_background_parts_from_target_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = target.pending_checkpoint();
        let response_payload = target.background_response_payload("task-1", 2);
        let task = target.background_task_active_model(
            "task-1".to_string(),
            "user-1".to_string(),
            2600,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(
            response_payload["estimated_time_minutes"],
            2
        );
        assert_eq!(task.target_word_count, sea_orm::Set(2600));
        assert_eq!(
            task.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_build_single_chapter_generation_request_parts_from_owner() {
        let request = SingleChapterGenerationRequest::from_route_payload(
            Some(7),
            Some(2200),
            Some("gpt-test".to_string()),
            Some(true),
            Some(true),
            Some(true),
            Some("hero backstory".to_string()),
            Some("third_person".to_string()),
            Some("balanced".to_string()),
            Some("advance_plot".to_string()),
            Some("development".to_string()),
            Some("brief".to_string()),
            Some("balanced".to_string()),
            Some("notes".to_string()),
            Some("repair".to_string()),
            Some(vec!["target-a".to_string()]),
            Some(vec!["strength-a".to_string()]),
        );
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-8".to_string(),
            chapter_number: 8,
            title: "Eight".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2200,
            compat_options: request.compat_options_with_web_research_default(false),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
            },
        };

        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
        assert_eq!(request.enable_analysis, Some(true));
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(true));
        assert_eq!(
            request.web_research_query.as_deref(),
            Some("hero backstory")
        );
        assert_eq!(
            request.narrative_perspective.as_deref(),
            Some("third_person")
        );
        assert_eq!(request.creative_mode.as_deref(), Some("balanced"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(request.story_creation_brief.as_deref(), Some("brief"));
        assert_eq!(request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(request.story_repair_summary.as_deref(), Some("repair"));
        assert_eq!(
            request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
        assert_eq!(chapter_target.chapter_id, "chapter-8");
        assert_eq!(execution_input.target_word_count, 2200);
        assert_eq!(execution_input.compat_options.style_id(), Some(7));
        assert!(execution_input.compat_options.enable_analysis());
        assert!(execution_input.compat_options.enable_mcp());
        assert!(execution_input.compat_options.web_research_enabled());
        assert_eq!(
            execution_input.compat_options.web_research_query(),
            Some("hero backstory")
        );
        assert_eq!(
            execution_input.compat_options.narrative_perspective(),
            "third_person"
        );
        assert_eq!(execution_input.compat_options.creative_mode(), "balanced");
        assert_eq!(execution_input.compat_options.story_focus(), "advance_plot");
        assert_eq!(execution_input.compat_options.plot_stage(), "development");
        assert_eq!(
            execution_input.compat_options.story_creation_brief(),
            "brief"
        );
        assert_eq!(execution_input.compat_options.quality_preset(), "balanced");
        assert_eq!(execution_input.compat_options.quality_notes(), "notes");
        assert_eq!(
            execution_input.compat_options.story_repair_summary(),
            "repair"
        );
        assert_eq!(
            execution_input.compat_options.story_repair_targets(),
            &["target-a".to_string()]
        );
        assert_eq!(
            execution_input.compat_options.story_preserve_strengths(),
            &["strength-a".to_string()]
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .research_query,
            ""
        );
    }

    #[test]
    fn should_normalize_single_chapter_generation_compat_options_from_request_owner() {
        let request = SingleChapterGenerationRequest::from_route_payload(
            Some(9),
            Some(2800),
            None,
            None,
            None,
            None,
            None,
            None,
            Some("hook".to_string()),
            Some("reveal_mystery".to_string()),
            None,
            None,
            Some("immersive".to_string()),
            None,
            None,
            None,
            None,
        );

        let compat = request.compat_options_with_web_research_default(false);

        assert_eq!(compat.style_id(), Some(9));
        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
        assert_eq!(compat.web_research_query(), None);
        assert_eq!(compat.creative_mode(), "hook");
        assert_eq!(compat.story_focus(), "reveal_mystery");
        assert_eq!(compat.quality_preset(), "immersive");
        assert_eq!(compat.story_repair_targets(), &[] as &[String]);
        assert_eq!(compat.story_preserve_strengths(), &[] as &[String]);
    }

    #[test]
    fn should_fallback_to_settings_default_for_single_generation_web_research() {
        let request = SingleChapterGenerationRequest::from_route_payload(
            None,
            Some(2800),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let compat = request.compat_options_with_web_research_default(true);

        assert!(compat.web_research_enabled());
    }
}
