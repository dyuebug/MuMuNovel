use crate::ai::config::AIConfig;
use crate::models::{chapter, generation_history, project};
use crate::services::chapter_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_contract_prepare_service::build_chapter_story_packet_contract;
use crate::services::chapter_generation_execution_contract_service::PreparedRoleModelPolicyContext;
use crate::services::chapter_generation_history_payload_service::payload_owner::generated_history_runtime_snapshot_from_payload;
use crate::services::chapter_generation_history_persistence_service::persist_single_generation_generated_result_with_contract_and_audit;
use crate::services::chapter_generation_prompt_service::{
    build_previous_chapter_prompt_context, resolve_prompt_preference,
    ChapterGenerationPromptOverrides, PreviousChapterPromptContext, PromptContextProviderPayload,
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
    execute_single_generation_candidate_runtime_tracked_with_guidance,
    execute_single_generation_candidate_runtime_with_guidance, GeneratedChapterResult,
};
use crate::services::generation_contract_service::{
    apply_generation_intent_overrides, build_generation_contract_snapshot, fill_missing_continuity,
    merge_generation_contract_runtime_snapshot, merge_story_packet_layers,
    story_packet_to_legacy_flat_value, GenerationContractSnapshotV1, GenerationCreativeOverrides,
    GenerationIntentKind, GenerationIntentOverrides, GenerationIntentV1, GenerationTarget,
    GenerationTargetKind, StoryPacketFactLayer, StoryPacketSource, StoryPacketSourceKind,
    StoryPacketV1,
};
use crate::services::generation_execution_audit_service::{
    build_generation_execution_audit, GenerationExecutionAuditV1,
};
use crate::services::wizard_service::build_project_long_term_goal;
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
    pub(crate) story_packet: StoryPacketV1,
}

impl ChapterGenerationRuntimeContext {
    async fn persist_generated_result(
        self,
        db: &DatabaseConnection,
        prompt: String,
        result: GeneratedChapterResult,
        generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
        generation_execution_audit: Option<&GenerationExecutionAuditV1>,
    ) -> Result<GeneratedChapterResult, String> {
        persist_single_generation_generated_result_with_contract_and_audit(
            db,
            &self.chapter_model,
            prompt,
            result,
            generation_contract_snapshot,
            generation_execution_audit,
        )
        .await
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
        self.generate_and_persist_with_optional_contract(
            db,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
            None,
            None,
        )
        .await
    }

    pub(crate) async fn generate_and_persist_single_with_candidate_route_gateway(
        self,
        db: &DatabaseConnection,
        task_id: Option<&str>,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        let snapshot =
            self.build_single_generation_contract_snapshot(target_word_count, overrides)?;
        if let Some(task_id) = task_id {
            let mut runtime_state = Value::Null;
            merge_generation_contract_runtime_snapshot(&mut runtime_state, &snapshot)
                .map_err(|error| error.to_string())?;
            crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::upsert_chapter_generation_runtime_snapshot(
                db,
                task_id,
                runtime_state,
                Utc::now().naive_utc(),
            )
            .await?;
        }

        self.generate_and_persist_with_optional_contract(
            db,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
            Some(snapshot),
            role_policy_context,
        )
        .await
    }

    fn build_single_generation_contract_snapshot(
        &self,
        target_word_count: i32,
        overrides: &ChapterGenerationPromptOverrides,
    ) -> Result<GenerationContractSnapshotV1, String> {
        let request_target_word_count = positive_u32(target_word_count);
        let mut story_packet = self.story_packet.clone();
        if let Some(target_word_count) = request_target_word_count {
            story_packet.target_word_count = Some(target_word_count);
            append_story_packet_source(
                &mut story_packet,
                StoryPacketSource {
                    kind: StoryPacketSourceKind::LegacyRequestAdapter,
                    reference: Some("single_generation_target_word_count".to_owned()),
                },
            );
        }

        let target = story_packet.target.clone();
        let mut intent = GenerationIntentV1::new(GenerationIntentKind::ChapterGenerate, target);
        apply_generation_intent_overrides(
            &mut intent,
            build_chapter_generation_intent_overrides(
                &self.project_model,
                request_target_word_count,
                overrides,
                "single_generation_active_route",
            ),
        );
        build_generation_contract_snapshot(story_packet, intent).map_err(|error| error.to_string())
    }

    pub(crate) async fn generate_candidate_only(
        &self,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        self.generate_candidate_only_with_guidance(
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            None,
            gateway_config,
            role_policy_context,
        )
        .await
    }

    pub(crate) async fn generate_candidate_only_with_guidance(
        &self,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        additional_guidance: Option<&str>,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        let generation_contract_snapshot =
            self.build_single_generation_contract_snapshot(target_word_count, overrides)?;
        let (_, result, _, _) = self
            .generate_candidate_with_optional_contract(
                ai_config,
                target_word_count,
                provider_payload,
                overrides,
                additional_guidance,
                gateway_config,
                Some(generation_contract_snapshot),
                role_policy_context,
            )
            .await?;
        Ok(result)
    }

    pub(crate) async fn generate_candidate_only_with_contract(
        &self,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        generation_contract_snapshot: GenerationContractSnapshotV1,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        self.generate_candidate_only_with_contract_and_guidance(
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            None,
            gateway_config,
            generation_contract_snapshot,
            role_policy_context,
        )
        .await
    }

    pub(crate) async fn generate_candidate_only_with_contract_and_guidance(
        &self,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        additional_guidance: Option<&str>,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        generation_contract_snapshot: GenerationContractSnapshotV1,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        let (_, result, _, _) = self
            .generate_candidate_with_optional_contract(
                ai_config,
                target_word_count,
                provider_payload,
                overrides,
                additional_guidance,
                gateway_config,
                Some(generation_contract_snapshot),
                role_policy_context,
            )
            .await?;
        Ok(result)
    }

    async fn generate_candidate_with_optional_contract(
        &self,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        additional_guidance: Option<&str>,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        generation_contract_snapshot: Option<GenerationContractSnapshotV1>,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<
        (
            String,
            GeneratedChapterResult,
            Option<GenerationContractSnapshotV1>,
            Option<GenerationExecutionAuditV1>,
        ),
        String,
    > {
        let legacy_story_packet = generation_contract_snapshot
            .as_ref()
            .map(|snapshot| story_packet_to_legacy_flat_value(&snapshot.story_packet))
            .unwrap_or_else(|| story_packet_to_legacy_flat_value(&self.story_packet));
        let execution_context =
            crate::services::chapter_generation_runtime_service::SingleGenerationCandidateRuntimeExecutionContext {
                project_model: self.project_model.clone(),
                chapter_model: self.chapter_model.clone(),
                previous_chapter_exists: self.previous_chapter.is_some(),
                previous_chapter_prompt_context: self.previous_chapter_prompt_context.clone(),
                story_packet: legacy_story_packet,
                generation_contract_snapshot,
            };
        let (prompt, result, generation_execution_audit) =
            if let Some(role_policy_context) = role_policy_context {
                let (prompt, result, execution) =
                    execute_single_generation_candidate_runtime_tracked_with_guidance(
                        &execution_context,
                        ai_config,
                        target_word_count,
                        provider_payload,
                        overrides,
                        additional_guidance,
                        gateway_config,
                        role_policy_context.allow_model_fallback,
                    )
                    .await?;
                let audit = execution
                    .as_ref()
                    .map(|execution| {
                        build_generation_execution_audit(
                            &role_policy_context.resolved_policy,
                            execution,
                        )
                    })
                    .transpose()
                    .map_err(|error| error.to_string())?;
                (prompt, result, audit)
            } else {
                let (prompt, result) = execute_single_generation_candidate_runtime_with_guidance(
                    &execution_context,
                    ai_config,
                    target_word_count,
                    provider_payload,
                    overrides,
                    additional_guidance,
                    gateway_config,
                )
                .await?;
                (prompt, result, None)
            };
        Ok((
            prompt,
            result,
            execution_context.generation_contract_snapshot,
            generation_execution_audit,
        ))
    }

    async fn generate_and_persist_with_optional_contract(
        self,
        db: &DatabaseConnection,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
        generation_contract_snapshot: Option<GenerationContractSnapshotV1>,
        role_policy_context: Option<PreparedRoleModelPolicyContext>,
    ) -> Result<GeneratedChapterResult, String> {
        let (prompt, result, generation_contract_snapshot, generation_execution_audit) = self
            .generate_candidate_with_optional_contract(
                ai_config,
                target_word_count,
                provider_payload,
                overrides,
                None,
                gateway_config,
                generation_contract_snapshot,
                role_policy_context,
            )
            .await?;
        self.persist_generated_result(
            db,
            prompt,
            result,
            generation_contract_snapshot.as_ref(),
            generation_execution_audit.as_ref(),
        )
        .await
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
    let story_packet = build_single_generation_story_packet_contract(
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

fn build_single_generation_story_packet_contract(
    project_model: &project::Model,
    chapter_model: &chapter::Model,
    previous_story_runtime_snapshot: Option<&Value>,
    project_continuity_ledger: Option<&ProjectContinuityLedger>,
) -> StoryPacketV1 {
    build_chapter_story_packet_contract(
        project_model,
        chapter_model,
        previous_story_runtime_snapshot,
        project_continuity_ledger.map(ProjectContinuityLedger::to_story_continuity_snapshot),
    )
}

pub(crate) fn build_batch_generation_contract_snapshot(
    project_model: &project::Model,
    chapter_ids: Vec<String>,
    start_chapter_number: i32,
    normalized_target_word_count: i32,
    target_word_count_overridden: bool,
    overrides: &ChapterGenerationPromptOverrides,
    project_continuity_ledger: Option<&ProjectContinuityLedger>,
) -> Result<GenerationContractSnapshotV1, String> {
    let target_word_count = positive_u32(normalized_target_word_count);
    let target = GenerationTarget::chapter_batch(project_model.id.clone(), chapter_ids);
    let mut system_defaults = StoryPacketV1::new(project_model.id.clone(), target.clone());
    system_defaults.sources.push(StoryPacketSource {
        kind: StoryPacketSourceKind::SystemDefaults,
        reference: None,
    });

    let authoritative_facts = StoryPacketFactLayer {
        sources: vec![StoryPacketSource {
            kind: StoryPacketSourceKind::AuthoritativeDatabase,
            reference: Some(format!("project:{}/chapter-batch", project_model.id)),
        }],
        current_chapter_number: positive_u32(start_chapter_number),
        chapter_count: project_model.chapter_count.and_then(positive_u32),
        target_word_count,
        story_long_term_goal: build_project_long_term_goal(
            project_model.theme.as_deref(),
            project_model.description.as_deref(),
            project_model.default_story_creation_brief.as_deref(),
            project_model
                .chapter_count
                .and_then(|value| usize::try_from(value).ok()),
            usize::try_from(project_model.target_words).ok(),
        ),
        compatibility_metadata: BTreeMap::from([(
            "legacy_source".to_owned(),
            json!("batch_generation_create"),
        )]),
        ..StoryPacketFactLayer::default()
    };
    let mut story_packet = merge_story_packet_layers(system_defaults, authoritative_facts, None);
    if target_word_count_overridden {
        append_story_packet_source(
            &mut story_packet,
            StoryPacketSource {
                kind: StoryPacketSourceKind::LegacyRequestAdapter,
                reference: Some("batch_generation_target_word_count".to_owned()),
            },
        );
    }
    if let Some(project_continuity_ledger) = project_continuity_ledger {
        fill_missing_continuity(
            &mut story_packet.continuity,
            project_continuity_ledger.to_story_continuity_snapshot(),
        );
    }

    let mut intent = GenerationIntentV1::new(GenerationIntentKind::BatchChapterGenerate, target);
    apply_generation_intent_overrides(
        &mut intent,
        build_chapter_generation_intent_overrides(
            project_model,
            target_word_count,
            overrides,
            "batch_generation_create",
        ),
    );
    build_generation_contract_snapshot(story_packet, intent).map_err(|error| error.to_string())
}

#[cfg(test)]
fn build_single_generation_story_packet(
    project_model: &project::Model,
    chapter_model: &chapter::Model,
    previous_story_runtime_snapshot: Option<&Value>,
    project_continuity_ledger: Option<&ProjectContinuityLedger>,
) -> Value {
    story_packet_to_legacy_flat_value(&build_single_generation_story_packet_contract(
        project_model,
        chapter_model,
        previous_story_runtime_snapshot,
        project_continuity_ledger,
    ))
}

fn positive_u32(value: i32) -> Option<u32> {
    u32::try_from(value).ok().filter(|value| *value > 0)
}

fn append_story_packet_source(packet: &mut StoryPacketV1, source: StoryPacketSource) {
    if !packet.sources.contains(&source) {
        packet.sources.push(source);
    }
}

pub(crate) fn build_chapter_generation_intent_overrides(
    project_model: &project::Model,
    target_word_count: Option<u32>,
    overrides: &ChapterGenerationPromptOverrides,
    legacy_mode: &str,
) -> GenerationIntentOverrides {
    let narrative_style = resolved_prompt_value(
        overrides.narrative_perspective.as_deref(),
        project_model.narrative_perspective.as_deref(),
    );
    let creative_mode = resolved_prompt_value(
        overrides.creative_mode.as_deref(),
        project_model.default_creative_mode.as_deref(),
    );
    let story_focus = resolved_prompt_value(
        overrides.story_focus.as_deref(),
        project_model.default_story_focus.as_deref(),
    );
    let plot_stage = resolved_prompt_value(
        overrides.plot_stage.as_deref(),
        project_model.default_plot_stage.as_deref(),
    );
    let story_creation_brief = resolved_prompt_value(
        overrides.story_creation_brief.as_deref(),
        project_model.default_story_creation_brief.as_deref(),
    );
    let quality_preset = resolved_prompt_value(
        overrides.quality_preset.as_deref(),
        project_model.default_quality_preset.as_deref(),
    );
    let quality_notes = resolved_prompt_value(
        overrides.quality_notes.as_deref(),
        project_model.default_quality_notes.as_deref(),
    );

    let mut opaque_overrides = BTreeMap::new();
    insert_optional_override(
        &mut opaque_overrides,
        "creative_mode",
        creative_mode.as_ref(),
    );
    insert_optional_override(&mut opaque_overrides, "story_focus", story_focus.as_ref());
    insert_optional_override(&mut opaque_overrides, "plot_stage", plot_stage.as_ref());
    insert_optional_override(
        &mut opaque_overrides,
        "story_creation_brief",
        story_creation_brief.as_ref(),
    );
    insert_optional_override(
        &mut opaque_overrides,
        "quality_preset",
        quality_preset.as_ref(),
    );
    insert_optional_override(
        &mut opaque_overrides,
        "quality_notes",
        quality_notes.as_ref(),
    );
    if overrides.web_research_enabled {
        opaque_overrides.insert("web_research_enabled".to_owned(), Value::Bool(true));
        insert_optional_override(
            &mut opaque_overrides,
            "web_research_query",
            overrides.web_research_query.as_ref(),
        );
    }
    insert_optional_override(
        &mut opaque_overrides,
        "story_repair_summary",
        overrides.story_repair_summary.as_ref(),
    );
    insert_string_array_override(
        &mut opaque_overrides,
        "story_repair_targets",
        &overrides.story_repair_targets,
    );
    insert_string_array_override(
        &mut opaque_overrides,
        "story_preserve_strengths",
        &overrides.story_preserve_strengths,
    );

    GenerationIntentOverrides {
        target_word_count,
        creative_overrides: GenerationCreativeOverrides {
            narrative_style,
            creative_direction: story_creation_brief,
            story_direction: story_focus,
            quality_requirements: quality_notes,
            extra_constraints: Vec::new(),
            opaque_overrides,
        },
        compatibility_metadata: BTreeMap::from([("legacy_mode".to_owned(), json!(legacy_mode))]),
        ..GenerationIntentOverrides::default()
    }
}

fn resolved_prompt_value(
    override_value: Option<&str>,
    project_default: Option<&str>,
) -> Option<String> {
    let value = resolve_prompt_preference(override_value, project_default);
    (!value.trim().is_empty()).then_some(value)
}

fn insert_optional_override(
    overrides: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<&String>,
) {
    if let Some(value) = value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        overrides.insert(key.to_owned(), Value::String(value.to_owned()));
    }
}

fn insert_string_array_override(
    overrides: &mut BTreeMap<String, Value>,
    key: &str,
    values: &[String],
) {
    let values = values
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Value::String(value.to_owned()))
        .collect::<Vec<_>>();
    if !values.is_empty() {
        overrides.insert(key.to_owned(), Value::Array(values));
    }
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

fn build_batch_chapter_attempt_contract_snapshot(
    context: &ChapterGenerationRuntimeContext,
    batch_snapshot: &GenerationContractSnapshotV1,
    target_word_count: i32,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<Option<GenerationContractSnapshotV1>, String> {
    let batch_story_target = &batch_snapshot.story_packet.target;
    let batch_intent_target = &batch_snapshot.generation_intent.target;
    let project_id = &context.project_model.id;
    let chapter_id = &context.chapter_model.id;
    let has_expected_batch_semantics = batch_snapshot.generation_intent.kind
        == GenerationIntentKind::BatchChapterGenerate
        && batch_story_target.kind == GenerationTargetKind::ChapterBatch
        && batch_intent_target.kind == GenerationTargetKind::ChapterBatch
        && batch_snapshot.story_packet.project_id == *project_id
        && batch_story_target.project_id == *project_id
        && batch_intent_target.project_id == *project_id
        && batch_story_target.chapter_ids == batch_intent_target.chapter_ids
        && batch_story_target
            .chapter_ids
            .iter()
            .any(|id| id == chapter_id);
    if !has_expected_batch_semantics {
        return Ok(None);
    }

    let story_packet = batch_snapshot.story_packet.clone();
    let target = GenerationTarget::chapter(project_id.clone(), chapter_id.clone());
    let mut intent = GenerationIntentV1::new(GenerationIntentKind::ChapterGenerate, target);
    apply_generation_intent_overrides(
        &mut intent,
        build_chapter_generation_intent_overrides(
            &context.project_model,
            positive_u32(target_word_count),
            overrides,
            "batch_generation_chapter_attempt",
        ),
    );
    build_generation_contract_snapshot(story_packet, intent)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(crate) async fn generate_and_persist_batch_chapter_content_with_candidate_route_gateway(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    ai_config: AIConfig,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    batch_generation_contract_snapshot: Option<&GenerationContractSnapshotV1>,
    role_policy_context: Option<PreparedRoleModelPolicyContext>,
) -> Result<GeneratedChapterResult, String> {
    let context = load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?;
    let attempt_contract_snapshot = match batch_generation_contract_snapshot {
        Some(snapshot) => build_batch_chapter_attempt_contract_snapshot(
            &context,
            snapshot,
            target_word_count,
            overrides,
        )?,
        None => None,
    };

    context
        .generate_and_persist_with_optional_contract(
            db,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
            attempt_contract_snapshot,
            role_policy_context,
        )
        .await
}

pub(crate) async fn generate_and_persist_single_chapter_content_with_candidate_route_gateway(
    db: &DatabaseConnection,
    task_id: Option<&str>,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    ai_config: AIConfig,
    gateway_config: ChapterCandidateRouteGatewayConfig,
    role_policy_context: Option<PreparedRoleModelPolicyContext>,
) -> Result<GeneratedChapterResult, String> {
    load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?
        .generate_and_persist_single_with_candidate_route_gateway(
            db,
            task_id,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
            role_policy_context,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        build_batch_chapter_attempt_contract_snapshot, build_batch_generation_contract_snapshot,
        build_single_generation_story_packet, build_single_generation_story_packet_contract,
        load_generation_context, ChapterGenerationRuntimeContext,
    };
    use crate::models::{
        career, chapter, character, character_career, generation_history, organization,
        plot_analysis, project, relationship, story_memory,
    };
    use crate::services::chapter_generation_prompt_service::{
        build_previous_chapter_prompt_context, ChapterGenerationPromptOverrides,
    };
    use crate::services::chapter_generation_runtime_service::story_continuity_ledger_owner::{
        ProjectContinuityLedger, ProjectContinuityLedgerEntry,
    };
    use crate::services::generation_contract_service::{
        story_packet_to_legacy_flat_value, validate_generation_contract_snapshot,
        GenerationIntentKind, GenerationTargetKind, StoryPacketSourceKind,
        GENERATION_CONTRACT_SCHEMA_VERSION,
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

    #[test]
    fn should_build_typed_single_generation_contract_with_request_target_word_count() {
        let project_model = build_project();
        let chapter_model = build_chapter();
        let context = ChapterGenerationRuntimeContext {
            chapter_model: chapter_model.clone(),
            project_model: project_model.clone(),
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet: build_single_generation_story_packet_contract(
                &project_model,
                &chapter_model,
                None,
                None,
            ),
        };

        let snapshot = context
            .build_single_generation_contract_snapshot(
                3_200,
                &ChapterGenerationPromptOverrides::default(),
            )
            .expect("build typed single generation contract");

        assert_eq!(snapshot.story_packet.target_word_count, Some(3_200));
        assert_eq!(snapshot.generation_intent.target_word_count, Some(3_200));
        assert_eq!(
            snapshot.generation_intent.kind,
            GenerationIntentKind::ChapterGenerate
        );
        assert!(snapshot.story_packet.sources.iter().any(|source| {
            source.kind == StoryPacketSourceKind::LegacyRequestAdapter
                && source.reference.as_deref() == Some("single_generation_target_word_count")
        }));
    }

    #[test]
    fn should_map_prompt_overrides_to_typed_intent_without_mutating_story_facts() {
        let project_model = build_project();
        let chapter_model = build_chapter();
        let story_packet = build_single_generation_story_packet_contract(
            &project_model,
            &chapter_model,
            None,
            None,
        );
        let expected_story_packet = story_packet.clone();
        let context = ChapterGenerationRuntimeContext {
            chapter_model,
            project_model,
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet,
        };
        let overrides = ChapterGenerationPromptOverrides {
            narrative_perspective: Some("第一人称".to_owned()),
            creative_mode: Some("沉浸式".to_owned()),
            story_focus: Some("推进主线冲突".to_owned()),
            plot_stage: Some("升级".to_owned()),
            story_creation_brief: Some("放大选择代价".to_owned()),
            quality_preset: Some("strict".to_owned()),
            quality_notes: Some("避免信息重复".to_owned()),
            web_research_enabled: true,
            web_research_query: Some("宋代夜禁".to_owned()),
            story_repair_summary: Some("修复人物动机".to_owned()),
            story_repair_targets: vec!["动机".to_owned()],
            story_preserve_strengths: vec!["氛围".to_owned()],
        };

        let snapshot = context
            .build_single_generation_contract_snapshot(0, &overrides)
            .expect("build typed intent overrides");
        let creative = &snapshot.generation_intent.creative_overrides;

        assert_eq!(snapshot.story_packet, expected_story_packet);
        assert_eq!(creative.narrative_style.as_deref(), Some("第一人称"));
        assert_eq!(creative.creative_direction.as_deref(), Some("放大选择代价"));
        assert_eq!(creative.story_direction.as_deref(), Some("推进主线冲突"));
        assert_eq!(
            creative.quality_requirements.as_deref(),
            Some("避免信息重复")
        );
        assert_eq!(creative.opaque_overrides["creative_mode"], "沉浸式");
        assert_eq!(creative.opaque_overrides["plot_stage"], "升级");
        assert_eq!(creative.opaque_overrides["quality_preset"], "strict");
        assert_eq!(creative.opaque_overrides["web_research_enabled"], true);
        assert_eq!(creative.opaque_overrides["web_research_query"], "宋代夜禁");
        assert_eq!(
            creative.opaque_overrides["story_repair_summary"],
            "修复人物动机"
        );
        assert_eq!(creative.opaque_overrides["story_repair_targets"][0], "动机");
        assert_eq!(
            creative.opaque_overrides["story_preserve_strengths"][0],
            "氛围"
        );
    }

    #[test]
    fn should_build_batch_generation_contract_with_ordered_targets_and_valid_digest() {
        let chapter_ids = vec![
            "chapter-3".to_owned(),
            "chapter-4".to_owned(),
            "chapter-5".to_owned(),
        ];
        let snapshot = build_batch_generation_contract_snapshot(
            &build_project(),
            chapter_ids.clone(),
            3,
            2_800,
            true,
            &ChapterGenerationPromptOverrides::default(),
            None,
        )
        .expect("build batch generation contract");

        assert_eq!(snapshot.schema_version, GENERATION_CONTRACT_SCHEMA_VERSION);
        assert_eq!(
            snapshot.story_packet.target.kind,
            GenerationTargetKind::ChapterBatch
        );
        assert_eq!(snapshot.story_packet.target.chapter_ids, chapter_ids);
        assert_eq!(snapshot.story_packet.current_chapter_number, Some(3));
        assert_eq!(snapshot.story_packet.chapter_count, Some(12));
        assert_eq!(snapshot.story_packet.target_word_count, Some(2_800));
        assert_eq!(
            snapshot.story_packet.compatibility_metadata["legacy_source"],
            "batch_generation_create"
        );
        assert!(snapshot.story_packet.sources.iter().any(|source| {
            source.kind == StoryPacketSourceKind::LegacyRequestAdapter
                && source.reference.as_deref() == Some("batch_generation_target_word_count")
        }));
        assert_eq!(
            snapshot.generation_intent.kind,
            GenerationIntentKind::BatchChapterGenerate
        );
        assert_eq!(
            snapshot.generation_intent.target.kind,
            GenerationTargetKind::ChapterBatch
        );
        assert_eq!(
            snapshot.generation_intent.target.chapter_ids,
            snapshot.story_packet.target.chapter_ids
        );
        assert_eq!(snapshot.generation_intent.target_word_count, Some(2_800));
        assert_eq!(
            snapshot.generation_intent.compatibility_metadata["legacy_mode"],
            "batch_generation_create"
        );
        validate_generation_contract_snapshot(&snapshot).expect("validate batch contract digest");
    }

    #[test]
    fn should_derive_distinct_chapter_intents_from_stable_batch_story_packet() {
        let project_model = build_project();
        let batch_snapshot = build_batch_generation_contract_snapshot(
            &project_model,
            vec!["chapter-3".to_owned(), "chapter-4".to_owned()],
            3,
            3_200,
            false,
            &ChapterGenerationPromptOverrides::default(),
            None,
        )
        .expect("build batch contract");
        let mut first_chapter = build_chapter();
        first_chapter.id = "chapter-3".to_owned();
        first_chapter.chapter_number = 3;
        let mut second_chapter = build_chapter();
        second_chapter.id = "chapter-4".to_owned();
        second_chapter.chapter_number = 4;
        let first_context = ChapterGenerationRuntimeContext {
            chapter_model: first_chapter,
            project_model: project_model.clone(),
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet: batch_snapshot.story_packet.clone(),
        };
        let second_context = ChapterGenerationRuntimeContext {
            chapter_model: second_chapter,
            project_model,
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet: batch_snapshot.story_packet.clone(),
        };

        let first_attempt = build_batch_chapter_attempt_contract_snapshot(
            &first_context,
            &batch_snapshot,
            3_200,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("build first chapter attempt")
        .expect("first chapter belongs to batch");
        let second_attempt = build_batch_chapter_attempt_contract_snapshot(
            &second_context,
            &batch_snapshot,
            3_200,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("build second chapter attempt")
        .expect("second chapter belongs to batch");

        assert_eq!(first_attempt.story_packet, batch_snapshot.story_packet);
        assert_eq!(second_attempt.story_packet, batch_snapshot.story_packet);
        assert_eq!(
            first_attempt.generation_intent.kind,
            GenerationIntentKind::ChapterGenerate
        );
        assert_eq!(
            second_attempt.generation_intent.kind,
            GenerationIntentKind::ChapterGenerate
        );
        assert_eq!(
            first_attempt.generation_intent.target.chapter_id.as_deref(),
            Some("chapter-3")
        );
        assert_eq!(
            second_attempt
                .generation_intent
                .target
                .chapter_id
                .as_deref(),
            Some("chapter-4")
        );
        assert_ne!(first_attempt.input_digest, second_attempt.input_digest);
        assert_eq!(
            first_attempt.generation_intent.compatibility_metadata["legacy_mode"],
            "batch_generation_chapter_attempt"
        );
        assert_eq!(
            second_attempt.generation_intent.compatibility_metadata["legacy_mode"],
            "batch_generation_chapter_attempt"
        );
        validate_generation_contract_snapshot(&first_attempt)
            .expect("validate first chapter attempt digest");
        validate_generation_contract_snapshot(&second_attempt)
            .expect("validate second chapter attempt digest");
    }

    #[test]
    fn should_fallback_when_chapter_attempt_does_not_match_batch_contract() {
        let project_model = build_project();
        let batch_snapshot = build_batch_generation_contract_snapshot(
            &project_model,
            vec!["chapter-3".to_owned(), "chapter-4".to_owned()],
            3,
            3_200,
            false,
            &ChapterGenerationPromptOverrides::default(),
            None,
        )
        .expect("build batch contract");
        let mut unrelated_chapter = build_chapter();
        unrelated_chapter.id = "chapter-outside-batch".to_owned();
        let unrelated_context = ChapterGenerationRuntimeContext {
            chapter_model: unrelated_chapter,
            project_model,
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
            story_packet: batch_snapshot.story_packet.clone(),
        };

        let attempt = build_batch_chapter_attempt_contract_snapshot(
            &unrelated_context,
            &batch_snapshot,
            3_200,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("semantic mismatch should not be an error");

        assert!(attempt.is_none());
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
        let legacy_packet = story_packet_to_legacy_flat_value(&context.story_packet);
        assert_eq!(legacy_packet["source"], "single_generation_active_route");
        assert_eq!(legacy_packet["project_id"], "project-1");
        assert_eq!(legacy_packet["chapter_id"], "chapter-current");

        assert_eq!(
            legacy_packet["character_state_ledger"][0]["label"],
            "快照角色"
        );
        assert_eq!(
            legacy_packet["relationship_state_ledger"][0]["label"],
            "林河/白露"
        );
        assert_eq!(
            legacy_packet["relationship_state_ledger"][0]["summary"],
            "盟友; 互相隐瞒代价"
        );
        assert_eq!(
            legacy_packet["foreshadow_state_ledger"][0]["label"],
            "断裂的铜钥匙"
        );
        assert_eq!(
            legacy_packet["organization_state_ledger"][0]["label"],
            "白塔"
        );
        assert_eq!(
            legacy_packet["career_state_ledger"][0]["summary"],
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
