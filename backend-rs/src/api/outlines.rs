use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tracing::{debug, warn};
use uuid::Uuid;

mod continue_context_owner;
mod contract_prepare_owner;
mod plot_expansion_owner;

use self::continue_context_owner::{
    build_outline_continue_prompt_context, outline_continue_stage_instruction,
    OUTLINE_CONTINUE_RECENT_LIMIT,
};
use self::contract_prepare_owner::{
    prepare_outline_generate_contract, OutlineGenerateContractInput, OutlineGenerateContractMode,
};
use crate::ai::service::AIService;
use crate::api::wizard::OutlineRequest;
use crate::models::{chapter, outline, project};
use crate::services::auth::Claims;
use crate::services::chapter_service::ChapterService;
use crate::services::generation_contract_service::GenerationIntentKind;
use crate::services::generation_execution_audit_service::{
    build_generation_execution_audit, GenerationExecutionAuditV1,
};
use crate::services::outline_service::OutlineService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service;
use crate::services::wizard_service::{
    build_continue_outline_requirements, build_outline_content,
    build_outline_quality_guidance_bundle, build_outline_runtime_system_prompt,
    build_project_long_term_goal, clean_json_response, normalize_outline_items,
    OutlineRuntimeStage,
};
use crate::utils::sse::SseChannel;
use plot_expansion_owner::{
    create_tracked_plot_expansion_service, outline_expand_execution_intent_kind,
    OutlineExecutionAuditContext,
};

const OUTLINES_PROJECT_LIST_ROUTE: &str = "/outlines/project/{project_id}";
const OUTLINES_GENERATE_ROUTE: &str = "/outlines/generate";
const OUTLINES_GENERATE_STREAM_ROUTE: &str = "/outlines/generate-stream";
const OUTLINES_REORDER_ROUTE: &str = "/outlines/reorder";
const OUTLINES_BATCH_EXPAND_ROUTE: &str = "/outlines/batch-expand";
const OUTLINES_BATCH_EXPAND_STREAM_ROUTE: &str = "/outlines/batch-expand-stream";
const OUTLINES_LIST_CREATE_ROUTE: &str = "/outlines";
const OUTLINES_DETAIL_ROUTE: &str = "/outlines/{outline_id}";
const OUTLINES_EXPAND_ROUTE: &str = "/outlines/{outline_id}/expand";
const OUTLINES_EXPAND_STREAM_ROUTE: &str = "/outlines/{outline_id}/expand-stream";
const OUTLINES_CREATE_SINGLE_CHAPTER_ROUTE: &str = "/outlines/{outline_id}/create-single-chapter";
const OUTLINES_CHAPTERS_ROUTE: &str = "/outlines/{outline_id}/chapters";
const OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE: &str =
    "/outlines/{outline_id}/create-chapters-from-plans";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutlineGenerateMode {
    New,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
struct ContinueOutlineExecutionRequest {
    project_id: String,
    chapter_count: usize,
    narrative_perspective: Option<String>,
    requirements: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    compact_mode: Option<bool>,
    provider: Option<String>,
    model: Option<String>,
    story_direction: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct OutlineGenerateRouteRequest {
    pub project_id: String,
    #[serde(default = "default_outline_count")]
    pub chapter_count: usize,
    pub narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
    pub target_words: i32,
    pub requirements: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub compact_mode: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub mode: Option<String>,
    pub story_direction: Option<String>,
    pub keep_existing: Option<bool>,
    pub world_context: Option<Value>,
    pub characters_context: Option<Vec<Value>>,
}

fn default_outline_count() -> usize {
    3
}

fn default_target_words() -> i32 {
    100000
}

#[cfg(test)]
fn build_outlines_route_owner_contract() -> Value {
    json!({
        "owner": "outlines",
        "scope": "outlines_crud_generation_expansion_chapter_creation_route_group",
        "python_source_map": [
            "backend/migrator_app/models/outline.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/outlines.rs",
            "backend-rs/src/api/outlines/plot_expansion_owner.rs",
            "backend-rs/src/services/outline_service.rs",
            "backend-rs/src/services/wizard_service.rs",
            "backend-rs/src/services/wizard_service/outline_quality_owner.rs",
            "backend-rs/src/services/wizard_service/outline_requirement_owner.rs",
            "backend-rs/src/api/outlines/continue_context_owner.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "project_list": OUTLINES_PROJECT_LIST_ROUTE,
            "generate": OUTLINES_GENERATE_ROUTE,
            "generate_stream": OUTLINES_GENERATE_STREAM_ROUTE,
            "reorder": OUTLINES_REORDER_ROUTE,
            "batch_expand": OUTLINES_BATCH_EXPAND_ROUTE,
            "batch_expand_stream": OUTLINES_BATCH_EXPAND_STREAM_ROUTE,
            "list": OUTLINES_LIST_CREATE_ROUTE,
            "create": OUTLINES_LIST_CREATE_ROUTE,
            "detail": OUTLINES_DETAIL_ROUTE,
            "update": OUTLINES_DETAIL_ROUTE,
            "delete": OUTLINES_DETAIL_ROUTE,
            "expand": OUTLINES_EXPAND_ROUTE,
            "expand_stream": OUTLINES_EXPAND_STREAM_ROUTE,
            "create_single_chapter": OUTLINES_CREATE_SINGLE_CHAPTER_ROUTE,
            "chapters": OUTLINES_CHAPTERS_ROUTE,
            "create_chapters_from_plans": OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "list_outlines_by_project",
                "generate_outlines",
                "reorder_outlines",
                "batch_expand_outlines_compat",
                "create_outline",
                "list_outlines",
                "get_outline",
                "update_outline",
                "delete_outline",
                "expand_outline_compat",
                "create_single_chapter",
                "get_outline_chapters",
                "create_chapters_from_plans"
            ],
            "service_consumers": [
                "OutlineService::create",
                "OutlineService::list",
                "OutlineService::get",
                "OutlineService::update",
                "OutlineService::delete",
                "execute_outline_generate_route_request",
                "execute_outline_expand_request",
                "execute_outline_batch_expand_request"
            ],
            "stream_routes": [
                "generate_stream",
                "batch_expand_stream",
                "expand_stream"
            ],
            "chapter_creation_policy": {
                "one_to_one_only_for_create_single_chapter": true,
                "create_chapters_from_plans_uses_outline_order_context": true,
                "duplicate_single_chapter_maps_to_conflict": true
            },
            "compat_payload_policy": {
                "outline_payload_preserves_success_data_shape": true,
                "list_payload_preserves_data_items_total": true
            }
        },
        "readiness_evidence": [
            "outlines-project-list-auth-guard-rust",
            "outlines-list-auth-guard-rust",
            "outlines-generate-stream-auth-guard-rust",
            "outlines-batch-expand-stream-auth-guard-rust",
            "outlines-create-chapters-from-plans-auth-guard-rust",
            "outlines-setup-project-business-rust",
            "outlines-create-business-rust",
            "outlines-list-business-rust",
            "outlines-project-list-business-rust",
            "outlines-detail-business-rust",
            "outlines-update-business-rust",
            "outlines-reorder-business-rust",
            "outlines-chapters-empty-business-rust",
            "outlines-create-chapters-from-plans-business-rust",
            "outlines-chapters-created-business-rust",
            "outlines-delete-business-rust",
            "outlines-missing-detail-business-rust",
            "outlines-missing-project-list-business-rust",
            "outlines-create-single-chapter-mode-guard-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-outlines-business-owner",
            "business_probes": [
                "outlines-setup-project-business-rust",
                "outlines-create-business-rust",
                "outlines-list-business-rust",
                "outlines-project-list-business-rust",
                "outlines-detail-business-rust",
                "outlines-update-business-rust",
                "outlines-reorder-business-rust",
                "outlines-chapters-empty-business-rust",
                "outlines-create-chapters-from-plans-business-rust",
                "outlines-chapters-created-business-rust",
                "outlines-delete-business-rust",
                "outlines-missing-detail-business-rust",
                "outlines-missing-project-list-business-rust",
                "outlines-create-single-chapter-mode-guard-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-outlines-business-owner",
            "readiness_probe_count": 19,
            "business_probe_count": 14,
            "auth_guard_probe_count": 5,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "outlines route source-map shell deleted; remaining Python closeout work is limited to the outline model source-map contract",
        "migration_policy": "Outlines business smoke is covered by phase5-outlines-business-owner; the Python outlines route shell plus outline postprocess runtime facade have been physically deleted, while the remaining outline model source map stays as a separate closeout contract.",
        "validation_boundary": [
            "cargo test api::outlines",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-outlines-business-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_outline_model_source_map_replaced_by_migrator_and_test_support_fixtures",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "outlines_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_route_files_status": "outlines_route_source_map_deleted_remaining_outline_model_source_map_only",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "outline model source-map package still needs its own separate closeout round"
            ],
            "retired_manifest_fallbacks": [
                "outlines-project-list-auth-guard-python-fallback",
                "outlines-list-auth-guard-python-fallback",
                "outlines-generate-stream-auth-guard-python-fallback",
                "outlines-batch-expand-stream-auth-guard-python-fallback",
                "outlines-create-chapters-from-plans-auth-guard-python-fallback"
            ],
            "rollback_files": []
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct OutlineExpandRouteRequest {
    pub target_chapter_count: Option<i64>,
    pub expansion_strategy: Option<String>,
    pub auto_create_chapters: Option<bool>,
    pub enable_scene_analysis: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub batch_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct OutlineBatchExpandRouteRequest {
    pub project_id: Option<String>,
    pub chapters_per_outline: Option<i64>,
    pub expansion_strategy: Option<String>,
    pub auto_create_chapters: Option<bool>,
    pub enable_scene_analysis: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub outline_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineExpandExecutionRequest {
    pub outline_id: String,
    pub target_chapter_count: usize,
    pub expansion_strategy: String,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineBatchExpandExecutionRequest {
    pub project_id: String,
    pub chapters_per_outline: usize,
    pub expansion_strategy: String,
    pub auto_create_chapters: bool,
    pub enable_scene_analysis: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub outline_ids: Option<Vec<String>>,
}

pub(crate) fn build_outline_expand_execution_request(
    outline_id: impl Into<String>,
    payload: &Value,
) -> OutlineExpandExecutionRequest {
    OutlineExpandExecutionRequest {
        outline_id: outline_id.into(),
        target_chapter_count: payload
            .get("target_chapter_count")
            .and_then(Value::as_i64)
            .unwrap_or_default() as usize,
        expansion_strategy: payload
            .get("expansion_strategy")
            .and_then(Value::as_str)
            .unwrap_or("balanced")
            .to_string(),
        auto_create_chapters: payload
            .get("auto_create_chapters")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enable_scene_analysis: payload
            .get("enable_scene_analysis")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        provider: payload
            .get("provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        batch_size: payload
            .get("batch_size")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .unwrap_or(5) as usize,
    }
}

pub(crate) fn build_outline_expand_execution_request_from_route_request(
    outline_id: impl Into<String>,
    request: &OutlineExpandRouteRequest,
) -> OutlineExpandExecutionRequest {
    let mut payload = serde_json::Map::new();

    if let Some(value) = request.target_chapter_count {
        payload.insert(
            "target_chapter_count".to_string(),
            Value::Number(value.into()),
        );
    }
    if let Some(value) = request.expansion_strategy.as_ref() {
        payload.insert(
            "expansion_strategy".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = request.auto_create_chapters {
        payload.insert("auto_create_chapters".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.enable_scene_analysis {
        payload.insert("enable_scene_analysis".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.provider.as_ref() {
        payload.insert("provider".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.model.as_ref() {
        payload.insert("model".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.batch_size {
        payload.insert("batch_size".to_string(), Value::Number(value.into()));
    }

    build_outline_expand_execution_request(outline_id, &Value::Object(payload))
}

pub(crate) fn build_outline_batch_expand_execution_request(
    payload: &Value,
) -> OutlineBatchExpandExecutionRequest {
    OutlineBatchExpandExecutionRequest {
        project_id: payload
            .get("project_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        chapters_per_outline: payload
            .get("chapters_per_outline")
            .and_then(Value::as_i64)
            .unwrap_or_default() as usize,
        expansion_strategy: payload
            .get("expansion_strategy")
            .and_then(Value::as_str)
            .unwrap_or("balanced")
            .to_string(),
        auto_create_chapters: payload
            .get("auto_create_chapters")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enable_scene_analysis: payload
            .get("enable_scene_analysis")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        provider: payload
            .get("provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        outline_ids: payload
            .get("outline_ids")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            }),
    }
}

pub(crate) fn build_outline_batch_expand_execution_request_from_route_request(
    request: &OutlineBatchExpandRouteRequest,
) -> OutlineBatchExpandExecutionRequest {
    let mut payload = serde_json::Map::new();

    if let Some(value) = request.project_id.as_ref() {
        payload.insert("project_id".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.chapters_per_outline {
        payload.insert(
            "chapters_per_outline".to_string(),
            Value::Number(value.into()),
        );
    }
    if let Some(value) = request.expansion_strategy.as_ref() {
        payload.insert(
            "expansion_strategy".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = request.auto_create_chapters {
        payload.insert("auto_create_chapters".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.enable_scene_analysis {
        payload.insert("enable_scene_analysis".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.provider.as_ref() {
        payload.insert("provider".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.model.as_ref() {
        payload.insert("model".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.outline_ids.as_ref() {
        payload.insert(
            "outline_ids".to_string(),
            Value::Array(value.iter().cloned().map(Value::String).collect()),
        );
    }

    build_outline_batch_expand_execution_request(&Value::Object(payload))
}

pub(crate) fn outline_generate_request_to_wizard_request(
    project_id: String,
    chapter_count: usize,
    narrative_perspective: Option<String>,
    target_words: i32,
    requirements: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    compact_mode: Option<bool>,
    provider: Option<String>,
    model: Option<String>,
) -> OutlineRequest {
    OutlineRequest {
        project_id,
        chapter_count,
        narrative_perspective,
        target_words,
        requirements,
        creative_mode,
        story_focus,
        plot_stage,
        story_creation_brief,
        quality_preset,
        quality_notes,
        compact_mode,
        provider,
        model,
        user_id: None,
        enable_mcp: None,
        enable_web_research: None,
        web_research_query: None,
    }
}

pub(crate) async fn execute_outline_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    body: OutlineRequest,
) {
    let project_model = match project::Entity::find_by_id(&body.project_id).one(db).await {
        Ok(Some(model)) => model,
        Ok(None) => {
            channel.error("项目不存在", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载项目失败: {}", error), 500)
                .await;
            return;
        }
    };
    if project_model.user_id != user_id {
        channel.error("无权访问该项目", 403).await;
        return;
    }

    let prepared = match prepare_outline_generate_contract(
        &project_model,
        OutlineGenerateContractMode::New,
        OutlineGenerateContractInput {
            chapter_count: body.chapter_count,
            target_words: Some(body.target_words),
            narrative_perspective: body.narrative_perspective.as_deref(),
            requirements: body.requirements.as_deref(),
            creative_mode: body.creative_mode.as_deref(),
            story_focus: body.story_focus.as_deref(),
            plot_stage: body.plot_stage.as_deref(),
            story_creation_brief: body.story_creation_brief.as_deref(),
            quality_preset: body.quality_preset.as_deref(),
            quality_notes: body.quality_notes.as_deref(),
            compact_mode: body.compact_mode,
            story_direction: None,
        },
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            channel
                .error(&format!("准备大纲生成契约失败: {}", error), 500)
                .await;
            return;
        }
    };
    debug!(
        project_id = %body.project_id,
        input_digest = %prepared.snapshot.input_digest,
        "Prepared new outline generation contract"
    );
    let resolved = prepared.resolved;

    wizard_service::generate_outline(
        db,
        channel,
        user_id,
        &body.project_id,
        resolved.chapter_count,
        resolved.narrative_perspective.as_deref(),
        resolved.target_words,
        resolved.requirements.as_deref(),
        resolved.creative_mode.as_deref(),
        resolved.story_focus.as_deref(),
        resolved.plot_stage.as_deref(),
        resolved.story_creation_brief.as_deref(),
        resolved.quality_preset.as_deref(),
        resolved.quality_notes.as_deref(),
        resolved.compact_mode,
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
}

pub(crate) fn resolve_outline_generate_mode(
    requested_mode: Option<&str>,
    has_existing_outlines: bool,
) -> Result<OutlineGenerateMode, String> {
    let normalized = requested_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "auto".to_string());

    match normalized.as_str() {
        "auto" => {
            if has_existing_outlines {
                Ok(OutlineGenerateMode::Continue)
            } else {
                Ok(OutlineGenerateMode::New)
            }
        }
        "new" => Ok(OutlineGenerateMode::New),
        "continue" => {
            if has_existing_outlines {
                Ok(OutlineGenerateMode::Continue)
            } else {
                Err("没有可用的现有大纲，无法继续生成".to_string())
            }
        }
        _ => Err(format!("不支持的模式: {}", normalized)),
    }
}

pub(crate) async fn execute_outline_generate_route_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    request: &OutlineGenerateRouteRequest,
) {
    let project_model = match project::Entity::find_by_id(&request.project_id)
        .one(db)
        .await
    {
        Ok(Some(model)) => model,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载项目信息失败: {}", error), 500)
                .await;
            return;
        }
    };

    if project_model.user_id != user_id {
        channel.error("项目不存在或无权限", 404).await;
        return;
    }

    let existing_outlines = match OutlineService::list(db, &request.project_id, user_id).await {
        Ok(Some(items)) => items,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载大纲失败: {}", error), 500)
                .await;
            return;
        }
    };

    let mode =
        match resolve_outline_generate_mode(request.mode.as_deref(), !existing_outlines.is_empty())
        {
            Ok(mode) => mode,
            Err(error) => {
                channel.error(&error, 400).await;
                return;
            }
        };

    match mode {
        OutlineGenerateMode::New => {
            let wizard_request = outline_generate_request_to_wizard_request(
                request.project_id.clone(),
                request.chapter_count,
                request.narrative_perspective.clone(),
                request.target_words,
                request.requirements.clone(),
                request.creative_mode.clone(),
                request.story_focus.clone(),
                request.plot_stage.clone(),
                request.story_creation_brief.clone(),
                request.quality_preset.clone(),
                request.quality_notes.clone(),
                request.compact_mode,
                request.provider.clone(),
                request.model.clone(),
            );
            execute_outline_request(db, channel, user_id, wizard_request).await;
        }
        OutlineGenerateMode::Continue => {
            let prepared = match prepare_outline_generate_contract(
                &project_model,
                OutlineGenerateContractMode::Continue,
                OutlineGenerateContractInput {
                    chapter_count: request.chapter_count,
                    target_words: None,
                    narrative_perspective: request.narrative_perspective.as_deref(),
                    requirements: request.requirements.as_deref(),
                    creative_mode: request.creative_mode.as_deref(),
                    story_focus: request.story_focus.as_deref(),
                    plot_stage: request.plot_stage.as_deref(),
                    story_creation_brief: request.story_creation_brief.as_deref(),
                    quality_preset: request.quality_preset.as_deref(),
                    quality_notes: request.quality_notes.as_deref(),
                    compact_mode: request.compact_mode,
                    story_direction: request.story_direction.as_deref(),
                },
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    channel
                        .error(&format!("准备续写大纲契约失败: {}", error), 500)
                        .await;
                    return;
                }
            };
            debug!(
                project_id = %request.project_id,
                input_digest = %prepared.snapshot.input_digest,
                "Prepared continue outline generation contract"
            );
            let resolved = prepared.resolved;
            let continue_request = ContinueOutlineExecutionRequest {
                project_id: request.project_id.clone(),
                chapter_count: resolved.chapter_count,
                narrative_perspective: resolved.narrative_perspective,
                requirements: resolved.requirements,
                creative_mode: resolved.creative_mode,
                story_focus: resolved.story_focus,
                plot_stage: resolved.plot_stage,
                story_creation_brief: resolved.story_creation_brief,
                quality_preset: resolved.quality_preset,
                quality_notes: resolved.quality_notes,
                compact_mode: Some(resolved.compact_mode),
                provider: request.provider.clone(),
                model: request.model.clone(),
                story_direction: resolved.story_direction,
            };

            execute_continue_outline_request(
                db,
                channel,
                user_id,
                &project_model,
                &existing_outlines,
                &continue_request,
            )
            .await;
        }
    }
}

fn trimmed_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn build_outline_continue_system_prompt(
    project_model: &project::Model,
    chapter_count: usize,
) -> String {
    build_outline_runtime_system_prompt(
        project_model,
        chapter_count,
        OutlineRuntimeStage::Continuation,
    )
}

fn outline_model_to_result(outline_model: &outline::Model) -> Value {
    json!({
        "id": outline_model.id,
        "project_id": outline_model.project_id,
        "title": outline_model.title,
        "content": outline_model.content,
        "order_index": outline_model.order_index,
        "structure": outline_model.structure,
        "created_at": outline_model.created_at.and_utc().to_rfc3339(),
        "updated_at": outline_model.updated_at.map(|value| value.and_utc().to_rfc3339()),
    })
}

fn chapter_model_to_result(chapter_model: &chapter::Model) -> Value {
    json!({
        "id": chapter_model.id,
        "project_id": chapter_model.project_id,
        "title": chapter_model.title,
        "chapter_number": chapter_model.chapter_number,
        "summary": chapter_model.summary,
        "status": chapter_model.status,
        "outline_id": chapter_model.outline_id,
        "sub_index": chapter_model.sub_index,
    })
}

async fn execute_continue_outline_request(
    db: &DatabaseConnection,
    channel: &SseChannel,
    user_id: &str,
    project_model: &project::Model,
    existing_outlines: &[outline::Model],
    request: &ContinueOutlineExecutionRequest,
) {
    if request.chapter_count == 0 {
        channel.error("chapter_count 必须大于 0", 400).await;
        return;
    }

    let prepared_ai = match SettingsService::build_role_aware_ai_config(
        db,
        user_id,
        GenerationIntentKind::OutlineGenerate,
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await
    {
        Ok(config) => config,
        Err(error) => {
            channel.error(&format!("AI配置失败: {}", error), 500).await;
            return;
        }
    };
    let resolved_policy = prepared_ai.resolved_policy;
    let allow_model_fallback = prepared_ai.allow_model_fallback;
    let ai_service = AIService::new(prepared_ai.ai_config);
    let mut generation_execution_audit: Vec<GenerationExecutionAuditV1> = Vec::new();

    channel
        .progress("准备续写大纲提示词...", 5, "processing")
        .await;

    let template = match PromptTemplateService::system_template_info("OUTLINE_CONTINUE") {
        Some(template) => template,
        None => {
            channel.error("加载续写大纲模板失败", 500).await;
            return;
        }
    };

    let guidance_limit = request.chapter_count.max(OUTLINE_CONTINUE_RECENT_LIMIT);
    let quality_guidance_bundle = match build_outline_quality_guidance_bundle(
        db,
        &request.project_id,
        guidance_limit,
    )
    .await
    {
        Ok(bundle) => bundle,
        Err(error) => {
            warn!("Build outline-continue quality guidance failed: {}", error);
            Default::default()
        }
    };

    let last_chapter_number = existing_outlines
        .last()
        .and_then(|item| item.order_index)
        .unwrap_or(existing_outlines.len() as i32);
    let start_chapter = last_chapter_number + 1;
    let end_chapter = start_chapter + request.chapter_count as i32 - 1;
    let effective_plot_stage =
        trimmed_non_empty(request.plot_stage.as_deref()).unwrap_or("development");
    let stage_instruction = outline_continue_stage_instruction(effective_plot_stage);
    let narrative_perspective =
        trimmed_non_empty(request.narrative_perspective.as_deref()).unwrap_or("第三人称");
    let story_direction =
        trimmed_non_empty(request.story_direction.as_deref()).unwrap_or("自然延续");
    let prompt_context = match build_outline_continue_prompt_context(
        db,
        &request.project_id,
        existing_outlines,
        start_chapter,
        Some(story_direction),
        request.requirements.as_deref(),
    )
    .await
    {
        Ok(context) => context,
        Err(error) => {
            channel.error(&error, 500).await;
            return;
        }
    };

    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project_model.title.clone());
    params.insert(
        "theme".into(),
        project_model
            .theme
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "genre".into(),
        project_model.genre.clone().unwrap_or_else(|| "通用".into()),
    );
    params.insert(
        "narrative_perspective".into(),
        narrative_perspective.to_string(),
    );
    params.insert(
        "time_period".into(),
        project_model
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "location".into(),
        project_model
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "atmosphere".into(),
        project_model
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "rules".into(),
        project_model
            .world_rules
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert("recent_outlines".into(), prompt_context.recent_outlines);
    params.insert("characters_info".into(), prompt_context.characters_info);
    params.insert("chapter_count".into(), request.chapter_count.to_string());
    params.insert("start_chapter".into(), start_chapter.to_string());
    params.insert("end_chapter".into(), end_chapter.to_string());
    params.insert(
        "current_chapter_count".into(),
        existing_outlines.len().to_string(),
    );
    params.insert(
        "plot_stage_instruction".into(),
        stage_instruction.to_string(),
    );
    params.insert("story_direction".into(), story_direction.to_string());
    let project_long_term_goal = build_project_long_term_goal(
        project_model.theme.as_deref(),
        project_model.description.as_deref(),
        request.story_creation_brief.as_deref(),
        project_model
            .chapter_count
            .and_then(|value| usize::try_from(value).ok()),
        project_model
            .target_words
            .try_into()
            .ok()
            .filter(|value: &usize| *value > 0),
    );
    params.insert(
        "requirements".into(),
        build_continue_outline_requirements(
            request.requirements.as_deref(),
            request.chapter_count,
            request.creative_mode.as_deref(),
            request.story_focus.as_deref(),
            Some(effective_plot_stage),
            request.story_creation_brief.as_deref(),
            request.quality_preset.as_deref(),
            request.quality_notes.as_deref(),
            project_long_term_goal.as_deref(),
            Some(prompt_context.focus_names.as_slice()),
            Some(prompt_context.foreshadow_payoff_plan.as_slice()),
            Some(prompt_context.foreshadow_state_ledger.as_slice()),
            Some(prompt_context.character_state_ledger.as_slice()),
            Some(prompt_context.relationship_state_ledger.as_slice()),
            Some(prompt_context.organization_state_ledger.as_slice()),
            Some(prompt_context.career_state_ledger.as_slice()),
            Some(prompt_context.memory_guidance.as_str()),
            Some(quality_guidance_bundle.quality_repair_guidance.as_str()),
            Some(quality_guidance_bundle.quality_trend_guidance.as_str()),
            request.compact_mode.unwrap_or(true),
        ),
    );
    params.insert("mcp_references".into(), String::new());

    let prompt = match PromptTemplateService::format_prompt(&template.content, &params) {
        Ok(prompt) => prompt,
        Err(error) => {
            channel
                .error(&format!("提示词格式化失败: {}", error), 500)
                .await;
            return;
        }
    };
    let sys_prompt = build_outline_continue_system_prompt(project_model, request.chapter_count);

    channel
        .progress("AI正在续写大纲...", 10, "processing")
        .await;
    let progress = Mutex::new(10u32);
    let mut accumulated = String::new();
    let mut chunk_count = 0u64;

    let tracked_stream = ai_service.generate_text_stream_tracked(
        prompt.clone(),
        Some(sys_prompt.clone()),
        None,
        allow_model_fallback,
    );
    let mut rx = tracked_stream.stream;
    let execution_completion = tracked_stream.completion;
    while let Some(chunk_result) = rx.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let Some(reasoning) = chunk.reasoning_content {
                    channel.reasoning_chunk(&reasoning).await;
                }
                if let Some(text) = chunk.content {
                    accumulated.push_str(&text);
                    channel.chunk(&text).await;
                    chunk_count += 1;

                    if chunk_count % 10 == 0 {
                        let pct = (*progress.lock().await + 1).min(55);
                        channel
                            .progress(
                                &format!("续写大纲中... ({}字符)", accumulated.len()),
                                pct,
                                "processing",
                            )
                            .await;
                        *progress.lock().await = pct;
                    }
                }

                if chunk.done {
                    break;
                }
            }
            Err(error) => {
                channel
                    .progress(
                        &format!("⚠ 续写警告: {}", error),
                        *progress.lock().await,
                        "processing",
                    )
                    .await;
            }
        }
    }
    let primary_execution = match execution_completion.await {
        Ok(execution) => execution,
        Err(_) => {
            channel.error("续写大纲执行审计通道已关闭", 500).await;
            return;
        }
    };
    match build_generation_execution_audit(&resolved_policy, &primary_execution) {
        Ok(audit) => generation_execution_audit.push(audit),
        Err(error) => {
            channel
                .error(&format!("构建续写大纲执行审计失败: {}", error), 500)
                .await;
            return;
        }
    }

    channel
        .progress("解析续写大纲数据...", 55, "processing")
        .await;
    let cleaned = clean_json_response(&accumulated);
    let outline_data = if cleaned.trim().is_empty() {
        channel
            .progress("AI返回为空，自动重试...", 56, "processing")
            .await;
        let mut retry_acc = String::new();
        let tracked_retry = ai_service.generate_text_stream_tracked(
            prompt,
            Some(sys_prompt),
            None,
            allow_model_fallback,
        );
        let mut retry_rx = tracked_retry.stream;
        let retry_completion = tracked_retry.completion;
        while let Some(chunk_result) = retry_rx.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(reasoning) = chunk.reasoning_content {
                        channel.reasoning_chunk(&reasoning).await;
                    }
                    if let Some(text) = chunk.content {
                        retry_acc.push_str(&text);
                        channel.chunk(&text).await;
                    }
                    if chunk.done {
                        break;
                    }
                }
                Err(_) => {}
            }
        }
        let retry_execution = match retry_completion.await {
            Ok(execution) => execution,
            Err(_) => {
                channel.error("续写大纲重试执行审计通道已关闭", 500).await;
                return;
            }
        };
        match build_generation_execution_audit(&resolved_policy, &retry_execution) {
            Ok(audit) => generation_execution_audit.push(audit),
            Err(error) => {
                channel
                    .error(&format!("构建续写大纲重试执行审计失败: {}", error), 500)
                    .await;
                return;
            }
        }
        let retry_cleaned = clean_json_response(&retry_acc);
        if retry_cleaned.trim().is_empty() {
            channel
                .error("续写大纲生成失败（AI重试后仍返回为空）", 500)
                .await;
            return;
        }
        match serde_json::from_str::<Value>(&retry_cleaned) {
            Ok(data) => {
                channel
                    .progress("已自动修复返回格式，继续保存...", 58, "processing")
                    .await;
                normalize_outline_items(&data)
            }
            Err(error) => {
                channel
                    .error(&format!("续写大纲JSON解析失败（已重试）: {}", error), 500)
                    .await;
                return;
            }
        }
    } else {
        match serde_json::from_str::<Value>(&cleaned) {
            Ok(data) => normalize_outline_items(&data),
            Err(_error) => {
                channel
                    .progress("JSON解析失败，自动重试...", 56, "processing")
                    .await;
                let mut retry_acc = String::new();
                let tracked_retry = ai_service.generate_text_stream_tracked(
                    prompt,
                    Some(sys_prompt),
                    None,
                    allow_model_fallback,
                );
                let mut retry_rx = tracked_retry.stream;
                let retry_completion = tracked_retry.completion;
                while let Some(chunk_result) = retry_rx.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            if let Some(reasoning) = chunk.reasoning_content {
                                channel.reasoning_chunk(&reasoning).await;
                            }
                            if let Some(text) = chunk.content {
                                retry_acc.push_str(&text);
                                channel.chunk(&text).await;
                            }
                            if chunk.done {
                                break;
                            }
                        }
                        Err(_) => {}
                    }
                }
                let retry_execution = match retry_completion.await {
                    Ok(execution) => execution,
                    Err(_) => {
                        channel.error("续写大纲重试执行审计通道已关闭", 500).await;
                        return;
                    }
                };
                match build_generation_execution_audit(&resolved_policy, &retry_execution) {
                    Ok(audit) => generation_execution_audit.push(audit),
                    Err(error) => {
                        channel
                            .error(&format!("构建续写大纲重试执行审计失败: {}", error), 500)
                            .await;
                        return;
                    }
                }
                let retry_cleaned = clean_json_response(&retry_acc);
                if retry_cleaned.trim().is_empty() {
                    channel
                        .error("续写大纲生成失败（AI重试后仍返回为空）", 500)
                        .await;
                    return;
                }
                match serde_json::from_str::<Value>(&retry_cleaned) {
                    Ok(data) => {
                        channel
                            .progress("已自动修复返回格式，继续保存...", 58, "processing")
                            .await;
                        normalize_outline_items(&data)
                    }
                    Err(error) => {
                        channel
                            .error(&format!("续写大纲JSON解析失败（已重试）: {}", error), 500)
                            .await;
                        return;
                    }
                }
            }
        }
    };

    if outline_data.is_empty() {
        channel.error("续写大纲生成失败，AI返回为空", 500).await;
        return;
    }

    channel
        .progress("保存续写大纲到数据库...", 60, "processing")
        .await;

    let mut created_outlines = Vec::new();
    let mut created_chapters = Vec::new();
    for (index, item) in outline_data
        .iter()
        .take(request.chapter_count.min(outline_data.len()))
        .enumerate()
    {
        let fallback_number = start_chapter + index as i32;
        let chapter_number = item
            .get("chapter_number")
            .and_then(Value::as_i64)
            .map(|value| value as i32)
            .unwrap_or(fallback_number);
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("第{}章", chapter_number));
        let content = build_outline_content(item);
        let structure = serde_json::to_string(item).unwrap_or_default();

        let created_outline = match OutlineService::create(
            db,
            &request.project_id,
            user_id,
            &title,
            Some(&content),
            Some(chapter_number),
            Some(&structure),
        )
        .await
        {
            Ok(Some(model)) => model,
            Ok(None) => {
                channel.error("无权创建续写大纲", 403).await;
                return;
            }
            Err(error) => {
                channel
                    .error(&format!("保存续写大纲失败: {}", error), 500)
                    .await;
                return;
            }
        };

        if project_model.outline_mode == "one-to-one" {
            match ChapterService::create(
                db,
                &request.project_id,
                user_id,
                &title,
                chapter_number,
                None,
                Some(&content),
                Some("pending"),
                None,
                Some(1),
                None,
            )
            .await
            {
                Ok(Some(chapter_model)) => created_chapters.push(chapter_model),
                Ok(None) => {
                    channel.error("无权创建续写章节", 403).await;
                    return;
                }
                Err(error) => {
                    channel
                        .progress(&format!("⚠ 创建章节失败: {}", error), 80, "processing")
                        .await;
                }
            }
        }

        created_outlines.push(created_outline);
    }

    channel
        .progress(
            &format!("已续写{}个大纲节点", created_outlines.len()),
            78,
            "processing",
        )
        .await;

    if project_model.outline_mode == "one-to-one" {
        channel
            .progress(
                &format!("已自动创建{}个续写章节", created_chapters.len()),
                85,
                "processing",
            )
            .await;
    }

    let all_outlines = match OutlineService::list(db, &request.project_id, user_id).await {
        Ok(Some(items)) => items,
        Ok(None) => {
            channel.error("项目不存在或无权限", 404).await;
            return;
        }
        Err(error) => {
            channel
                .error(&format!("加载续写结果失败: {}", error), 500)
                .await;
            return;
        }
    };

    channel.progress("续写完成", 100, "success").await;
    channel
        .result(&json!({
            "message": format!(
                "续写完成！新增{}章，总计{}章",
                created_outlines.len(),
                all_outlines.len()
            ),
            "new_chapters": created_outlines.len(),
            "total_chapters": all_outlines.len(),
            "outline_count": all_outlines.len(),
            "chapter_count": created_chapters.len(),
            "outlines": all_outlines.iter().map(outline_model_to_result).collect::<Vec<_>>(),
            "chapters": created_chapters.iter().map(chapter_model_to_result).collect::<Vec<_>>(),
            "generation_execution_audit": generation_execution_audit,
        }))
        .await;
    channel.done().await;
}

pub(crate) async fn execute_outline_expand_request(
    db: &DatabaseConnection,
    user_id: &str,
    request: &OutlineExpandExecutionRequest,
) -> Result<Value, String> {
    let prepared = SettingsService::build_role_aware_ai_config(
        db,
        user_id,
        outline_expand_execution_intent_kind(),
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await?;
    let audit_context = OutlineExecutionAuditContext {
        resolved_policy: prepared.resolved_policy,
        allow_model_fallback: prepared.allow_model_fallback,
    };
    let ai_service = AIService::new(prepared.ai_config);
    let service = create_tracked_plot_expansion_service(&ai_service, &audit_context);

    service
        .expand_outline(
            db,
            user_id,
            &request.outline_id,
            request.target_chapter_count,
            &request.expansion_strategy,
            request.auto_create_chapters,
            request.enable_scene_analysis,
            request.provider.as_deref(),
            request.model.as_deref(),
            request.batch_size,
        )
        .await
}

pub(crate) async fn execute_outline_batch_expand_request(
    db: &DatabaseConnection,
    user_id: &str,
    request: &OutlineBatchExpandExecutionRequest,
) -> Result<Value, String> {
    let prepared = SettingsService::build_role_aware_ai_config(
        db,
        user_id,
        outline_expand_execution_intent_kind(),
        request.provider.as_deref(),
        request.model.as_deref(),
        None,
    )
    .await?;
    let audit_context = OutlineExecutionAuditContext {
        resolved_policy: prepared.resolved_policy,
        allow_model_fallback: prepared.allow_model_fallback,
    };
    let ai_service = AIService::new(prepared.ai_config);
    let service = create_tracked_plot_expansion_service(&ai_service, &audit_context);

    service
        .batch_expand_outlines(
            db,
            user_id,
            &request.project_id,
            request.chapters_per_outline,
            &request.expansion_strategy,
            request.auto_create_chapters,
            request.enable_scene_analysis,
            request.outline_ids.as_deref(),
            request.provider.as_deref(),
            request.model.as_deref(),
        )
        .await
}

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    title: String,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct OutlineReorderItem {
    id: String,
    order_index: i32,
}

#[derive(Deserialize)]
struct ReorderRequest {
    orders: Vec<OutlineReorderItem>,
}

fn compatible_outline_payload(outline: outline::Model) -> Value {
    let outline_value = serde_json::to_value(&outline).unwrap_or_else(|_| json!({}));
    match outline_value {
        Value::Object(mut map) => {
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), json!(outline));
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": outline
        }),
    }
}

async fn generate_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OutlineGenerateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (tx, mut rx) =
        mpsc::channel::<Result<axum::response::sse::Event, std::convert::Infallible>>(256);
    let result_capture: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let channel = SseChannel::with_result_capture(tx, result_capture.clone());
    let db_for_task = db.clone();
    let user_id = claims.sub.clone();
    let request = body.clone();

    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_outline_generate_route_request(&db_for_task, &channel, &user_id, &request).await;

    let _ = drain_handle.await;
    let result = result_capture.lock().await.clone().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "??????"})),
        )
    })?;

    let items = result
        .get("outlines")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let total = items.len();
    Ok(Json(json!({
        "success": true,
        "total": total,
        "items": items,
        "outlines": result.get("outlines").cloned().unwrap_or_else(|| json!([])),
        "chapters": result.get("chapters").cloned().unwrap_or_else(|| json!([])),
        "outline_count": result.get("outline_count").cloned().unwrap_or_else(|| json!(total)),
        "chapter_count": result.get("chapter_count").cloned().unwrap_or_else(|| json!(0)),
        "message": result.get("message").cloned().unwrap_or_else(|| json!("????")),
        "result": result,
    })))
}

async fn reorder_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ReorderRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if body.orders.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "????????"})),
        ));
    }

    let mut updated_count = 0usize;
    for order in body.orders {
        let Some(outline_model) = OutlineService::get(&db, &order.id, &claims.sub)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": e})),
                )
            })?
        else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "?????????"})),
            ));
        };

        let mut active: outline::ActiveModel = outline_model.into();
        active.order_index = Set(Some(order.order_index));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(&db).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": e.to_string()})),
            )
        })?;
        updated_count += 1;
    }

    Ok(Json(json!({
        "success": true,
        "message": "???????",
        "updated_outlines": updated_count,
        "updated_chapters": 0,
    })))
}

async fn expand_outline_compat(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<OutlineExpandRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(_outline_model) = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        ));
    };

    let request =
        build_outline_expand_execution_request_from_route_request(outline_id.clone(), &body);

    execute_outline_expand_request(&db, &claims.sub, &request)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })
}

async fn batch_expand_outlines_compat(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OutlineBatchExpandRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_outline_batch_expand_execution_request_from_route_request(&body);

    execute_outline_batch_expand_request(&db, &claims.sub, &request)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })
}

async fn create_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OutlineService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.title,
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok((
            StatusCode::CREATED,
            Json(compatible_outline_payload(outline)),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(outlines)) => Ok(Json(
            json!({"success": true, "data": outlines, "items": outlines, "total": outlines.len()}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::get(&db, &outline_id, &claims.sub).await {
        Ok(Some(outline)) => Ok(Json(compatible_outline_payload(outline))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::update(
        &db,
        &outline_id,
        &claims.sub,
        body.title.as_deref(),
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok(Json(compatible_outline_payload(outline))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::delete(&db, &outline_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "大纲已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn create_single_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "大纲不存在"}))))?;

    let proj = project::Entity::find_by_id(&ol.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "项目不存在"}))))?;

    if proj.outline_mode != "one-to-one" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "仅一对一模式支持从大纲直接创建章节"})),
        ));
    }

    let chapter_number = ol.order_index.unwrap_or(1);
    let sub_index = 1;

    // Check for duplicate
    let existing = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&ol.project_id))
        .filter(chapter::Column::ChapterNumber.eq(chapter_number))
        .filter(chapter::Column::SubIndex.eq(sub_index))
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"detail": format!("第{}章已存在", chapter_number)})),
        ));
    }

    let now = Utc::now().naive_utc();
    let content_str = ol.content.unwrap_or_default();
    let ch = chapter::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(ol.project_id.clone()),
        chapter_number: Set(chapter_number),
        title: Set(ol.title.clone()),
        content: Set(Some(String::new())),
        summary: Set(Some(content_str)),
        word_count: Set(0),
        status: Set("pending".to_string()),
        outline_id: Set(None), // traditional mode: no outline link
        sub_index: Set(sub_index),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    };

    let inserted = ch.insert(&db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    Ok(Json(json!({
        "message": "章节创建成功",
        "chapter": inserted,
    })))
}

async fn get_outline_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Verify outline belongs to user
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let ol = match ol {
        Some(o) => o,
        None => {
            return Ok(Json(json!({
                "has_chapters": false,
                "outline_id": outline_id,
                "outline_title": null,
                "chapter_count": 0,
                "chapters": [],
            })));
        }
    };

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::OutlineId.eq(&outline_id))
        .order_by_asc(chapter::Column::SubIndex)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let expansion_plans: Vec<Value> = chapters
        .iter()
        .filter_map(|c| {
            c.expansion_plan
                .as_ref()
                .and_then(|p| serde_json::from_str::<Value>(p).ok())
        })
        .collect();

    let has_chapters = !chapters.is_empty();
    Ok(Json(json!({
        "has_chapters": has_chapters,
        "outline_id": outline_id,
        "outline_title": ol.title,
        "chapter_count": chapters.len(),
        "chapters": chapters,
        "expansion_plans": if expansion_plans.is_empty() { json!(null) } else { json!(expansion_plans) },
    })))
}

#[derive(Deserialize, Serialize)]
struct ChapterPlan {
    sub_index: Option<i32>,
    title: String,
    plot_summary: Option<String>,
    key_events: Option<Vec<String>>,
    character_focus: Option<Vec<String>>,
    emotional_tone: Option<String>,
    narrative_goal: Option<String>,
    conflict_type: Option<String>,
    estimated_words: Option<i32>,
    scenes: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct CreateChaptersFromPlansRequest {
    #[serde(default, alias = "chapter_plans")]
    plans: Vec<ChapterPlan>,
}

async fn create_chapters_from_plans(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<CreateChaptersFromPlansRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "大纲不存在"}))))?;

    // Count existing chapters before this outline to determine starting chapter number
    let existing_count = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&ol.project_id))
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .len() as i32;

    // Also count chapters from earlier outlines
    let earlier_outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&ol.project_id))
        .order_by_asc(outline::Column::OrderIndex)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let mut start_chapter_num = 1i32;
    for eo in &earlier_outlines {
        let eo_chapters = chapter::Entity::find()
            .filter(chapter::Column::OutlineId.eq(&eo.id))
            .all(&db)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;
        start_chapter_num += eo_chapters.len() as i32;
        if eo.id == outline_id {
            break;
        }
    }

    // If no earlier outlines found via outline_id, use total count
    if start_chapter_num == 1 {
        start_chapter_num = existing_count + 1;
    }

    let mut created = Vec::new();
    let now = Utc::now().naive_utc();

    for (i, plan) in body.plans.iter().enumerate() {
        let chapter_number = start_chapter_num + i as i32;
        let sub_index = plan.sub_index.unwrap_or(i as i32 + 1);

        let expansion_plan = serde_json::to_string(plan).unwrap_or_default();

        let ch = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(ol.project_id.clone()),
            chapter_number: Set(chapter_number),
            title: Set(plan.title.clone()),
            content: Set(Some(String::new())),
            summary: Set(plan.plot_summary.clone()),
            word_count: Set(0),
            status: Set("pending".to_string()),
            outline_id: Set(Some(outline_id.clone())),
            sub_index: Set(sub_index),
            expansion_plan: Set(Some(expansion_plan)),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let inserted = ch.insert(&db).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

        created.push(inserted);
    }

    let created_chapters: Vec<Value> = created
        .iter()
        .map(|chapter| {
            json!({
                "id": chapter.id,
                "chapter_number": chapter.chapter_number,
                "title": chapter.title,
                "summary": chapter.summary,
                "outline_id": chapter.outline_id,
                "sub_index": chapter.sub_index,
                "status": chapter.status,
            })
        })
        .collect();

    Ok(Json(json!({
        "message": "??????",
        "outline_id": outline_id,
        "outline_title": ol.title,
        "chapters_created": created.len(),
        "created_chapters": created_chapters,
        "start_chapter_number": start_chapter_num,
        "chapters": created,
    })))
}

async fn list_outlines_by_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::list(&db, &project_id, &claims.sub).await {
        Ok(Some(outlines)) => Ok(Json(
            json!({"success": true, "data": outlines, "items": outlines, "total": outlines.len()}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(OUTLINES_PROJECT_LIST_ROUTE, get(list_outlines_by_project))
        .route(OUTLINES_GENERATE_ROUTE, post(generate_outlines))
        .route(OUTLINES_GENERATE_STREAM_ROUTE, post(generate_outlines))
        .route(OUTLINES_REORDER_ROUTE, post(reorder_outlines))
        .route(
            OUTLINES_BATCH_EXPAND_ROUTE,
            post(batch_expand_outlines_compat),
        )
        .route(
            OUTLINES_BATCH_EXPAND_STREAM_ROUTE,
            post(batch_expand_outlines_compat),
        )
        .route(
            OUTLINES_LIST_CREATE_ROUTE,
            post(create_outline).get(list_outlines),
        )
        .route(
            OUTLINES_DETAIL_ROUTE,
            get(get_outline).put(update_outline).delete(delete_outline),
        )
        .route(OUTLINES_EXPAND_ROUTE, post(expand_outline_compat))
        .route(OUTLINES_EXPAND_STREAM_ROUTE, post(expand_outline_compat))
        .route(
            OUTLINES_CREATE_SINGLE_CHAPTER_ROUTE,
            post(create_single_chapter),
        )
        .route(OUTLINES_CHAPTERS_ROUTE, get(get_outline_chapters))
        .route(
            OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE,
            post(create_chapters_from_plans),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        build_outline_continue_system_prompt, build_outlines_route_owner_contract,
        outline_generate_request_to_wizard_request, resolve_outline_generate_mode,
        OutlineGenerateMode, OutlineGenerateRouteRequest, OUTLINES_BATCH_EXPAND_ROUTE,
        OUTLINES_BATCH_EXPAND_STREAM_ROUTE, OUTLINES_CHAPTERS_ROUTE,
        OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE, OUTLINES_CREATE_SINGLE_CHAPTER_ROUTE,
        OUTLINES_DETAIL_ROUTE, OUTLINES_EXPAND_ROUTE, OUTLINES_EXPAND_STREAM_ROUTE,
        OUTLINES_GENERATE_ROUTE, OUTLINES_GENERATE_STREAM_ROUTE, OUTLINES_LIST_CREATE_ROUTE,
        OUTLINES_PROJECT_LIST_ROUTE, OUTLINES_REORDER_ROUTE,
    };
    use crate::api::outlines::continue_context_owner::build_recent_outlines_context;
    use crate::models::{outline, project};
    use chrono::NaiveDateTime;
    use serde_json::json;

    fn outline_model(
        id: &str,
        order_index: i32,
        title: &str,
        structure: Option<&str>,
    ) -> outline::Model {
        outline::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            content: Some("章节内容".to_string()),
            structure: structure.map(str::to_string),
            order_index: Some(order_index),
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        }
    }

    fn project_model() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试小说".to_string(),
            description: None,
            theme: Some("成长".to_string()),
            genre: Some("玄幻".to_string()),
            target_words: 100000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 4,
            outline_mode: "one-to-many".to_string(),
            world_time_period: Some("乱世末年".to_string()),
            world_location: Some("北境雪原".to_string()),
            world_atmosphere: Some("压抑肃杀".to_string()),
            world_rules: Some("灵力暴走会反噬经脉".to_string()),
            chapter_count: Some(100),
            narrative_perspective: Some("第三人称".to_string()),
            character_count: 4,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        }
    }

    #[test]
    fn should_publish_outlines_route_owner_contract() {
        let contract = build_outlines_route_owner_contract();

        assert_eq!(contract["owner"], "outlines");
        assert_eq!(
            contract["scope"],
            "outlines_crud_generation_expansion_chapter_creation_route_group"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/outline.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/outlines.rs"
        );
        assert_eq!(
            contract["route_contract"]["project_list"],
            OUTLINES_PROJECT_LIST_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["generate_stream"],
            OUTLINES_GENERATE_STREAM_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["create_chapters_from_plans"],
            OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][12],
            "create_chapters_from_plans"
        );
        assert_eq!(
            contract["readiness_evidence"][4],
            "outlines-create-chapters-from-plans-auth-guard-rust"
        );
        assert_eq!(contract["readiness_evidence"].as_array().unwrap().len(), 19);
        assert_eq!(
            contract["readiness_evidence"][18],
            "outlines-create-single-chapter-mode-guard-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-outlines-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("outlines business probes should be present");
        assert_eq!(business_probes.len(), 14);
        assert_eq!(
            contract["owner_profile"]["business_probes"][8],
            "outlines-create-chapters-from-plans-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            19
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            14
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            5
        );
        assert_eq!(contract["business_smoke_status"]["fixture_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "outlines route source-map shell deleted; remaining Python closeout work is limited to the outline model source-map contract"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-outlines-business-owner"));
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("physically deleted"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "outlines_route_runtime_registration_deleted_no_python_route_shell_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "outlines_route_source_map_deleted_remaining_outline_model_source_map_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_outline_model_source_map_replaced_by_migrator_and_test_support_fixtures"
        );
    }

    #[test]
    fn should_keep_outlines_route_group_paths_stable() {
        assert_eq!(
            OUTLINES_PROJECT_LIST_ROUTE,
            "/outlines/project/{project_id}"
        );
        assert_eq!(OUTLINES_GENERATE_ROUTE, "/outlines/generate");
        assert_eq!(OUTLINES_GENERATE_STREAM_ROUTE, "/outlines/generate-stream");
        assert_eq!(OUTLINES_REORDER_ROUTE, "/outlines/reorder");
        assert_eq!(OUTLINES_BATCH_EXPAND_ROUTE, "/outlines/batch-expand");
        assert_eq!(
            OUTLINES_BATCH_EXPAND_STREAM_ROUTE,
            "/outlines/batch-expand-stream"
        );
        assert_eq!(OUTLINES_LIST_CREATE_ROUTE, "/outlines");
        assert_eq!(OUTLINES_DETAIL_ROUTE, "/outlines/{outline_id}");
        assert_eq!(OUTLINES_EXPAND_ROUTE, "/outlines/{outline_id}/expand");
        assert_eq!(
            OUTLINES_EXPAND_STREAM_ROUTE,
            "/outlines/{outline_id}/expand-stream"
        );
        assert_eq!(
            OUTLINES_CREATE_SINGLE_CHAPTER_ROUTE,
            "/outlines/{outline_id}/create-single-chapter"
        );
        assert_eq!(OUTLINES_CHAPTERS_ROUTE, "/outlines/{outline_id}/chapters");
        assert_eq!(
            OUTLINES_CREATE_CHAPTERS_FROM_PLANS_ROUTE,
            "/outlines/{outline_id}/create-chapters-from-plans"
        );
    }

    #[test]
    fn outline_generate_mode_auto_follows_existing_outlines() {
        assert_eq!(
            resolve_outline_generate_mode(Some("auto"), false).unwrap(),
            OutlineGenerateMode::New
        );
        assert_eq!(
            resolve_outline_generate_mode(Some("auto"), true).unwrap(),
            OutlineGenerateMode::Continue
        );
    }

    #[test]
    fn outline_generate_mode_rejects_continue_without_existing_outlines() {
        let error = resolve_outline_generate_mode(Some("continue"), false).unwrap_err();
        assert!(error.contains("没有可用的现有大纲"));
    }

    #[test]
    fn recent_outlines_context_prefers_structure_summary_fields() {
        let outlines = vec![outline_model(
            "outline-1",
            3,
            "第三章",
            Some(
                r#"{
                    "summary":"主角在雨夜截住押送车队，逼出城门背后的内应名单。",
                    "key_points":["押送车队现身","截杀失败后反追踪"],
                    "characters":[{"name":"沈夜"},{"name":"顾寒舟"}],
                    "emotion":"紧绷",
                    "goal":"拿到名单"
                }"#,
            ),
        )];

        let context = build_recent_outlines_context(&outlines);
        assert!(context.contains("第3章《第三章》"));
        assert!(context.contains("概要：主角在雨夜截住押送车队"));
        assert!(context.contains("关键事件：押送车队现身"));
        assert!(context.contains("重点角色：沈夜、顾寒舟"));
        assert!(context.contains("叙事目标：拿到名单"));
    }

    #[test]
    fn continue_system_prompt_uses_shared_runtime_constraints() {
        let prompt = build_outline_continue_system_prompt(&project_model(), 4);

        assert!(prompt.contains("当前阶段：续写阶段"));
        assert!(prompt.contains("本轮目标章节数：4"));
        assert!(prompt.contains("世界规则：灵力暴走会反噬经脉"));
        assert!(prompt.contains("每章至少给一个可直接写成对白场景的冲突对话钩子"));
    }

    #[test]
    fn outline_generate_route_request_accepts_compact_mode_flag() {
        let request: OutlineGenerateRouteRequest = serde_json::from_value(json!({
            "project_id": "project-1",
            "chapter_count": 3,
            "target_words": 120000,
            "compact_mode": false
        }))
        .expect("deserialize route request");

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.chapter_count, 3);
        assert_eq!(request.compact_mode, Some(false));
    }

    #[test]
    fn outline_generate_adapter_keeps_outline_execution_inputs() {
        let request = outline_generate_request_to_wizard_request(
            "project-2".to_string(),
            8,
            Some("first_person".to_string()),
            120_000,
            Some("more twists".to_string()),
            Some("balanced".to_string()),
            Some("growth".to_string()),
            Some("midpoint".to_string()),
            Some("brief".to_string()),
            Some("high".to_string()),
            Some("notes".to_string()),
            Some(false),
            Some("openai".to_string()),
            Some("gpt-4.1".to_string()),
        );

        assert_eq!(request.project_id, "project-2");
        assert_eq!(request.chapter_count, 8);
        assert_eq!(request.target_words, 120_000);
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
        assert_eq!(request.compact_mode, Some(false));
        assert!(request.user_id.is_none());
    }
}
