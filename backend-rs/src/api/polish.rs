use std::collections::HashMap;

use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::generation_history;
use crate::services::auth::Claims;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;

const POLISH_TEXT_ROUTE: &str = "/polish";
const POLISH_BATCH_ROUTE: &str = "/polish/batch";

struct PolishService;

const FOCUS_INSTRUCTIONS: &[(&str, &str)] = &[
    (
        "balanced",
        "- 平衡处理叙事、对话、情绪和节奏，整体降低模板腔。",
    ),
    (
        "dialogue",
        "- 优先处理人物对白，让说话方式更像真人，保住角色区分度。",
    ),
    (
        "pacing",
        "- 优先处理叙事节奏，减少拖沓解释，强化场面推进和段落落点。",
    ),
    (
        "emotion",
        "- 优先处理情绪表达，让反应更具体，少空泛感慨和统一抒情。",
    ),
    ("hook", "- 优先处理开场与结尾牵引，保住追读钩子和信息差。"),
];

#[cfg(test)]
fn build_polish_route_owner_contract() -> Value {
    json!({
        "owner": "polish",
        "rust_owner": "backend-rs/src/api/polish.rs",
        "routes": {
            "polish_text": POLISH_TEXT_ROUTE,
            "polish_batch": POLISH_BATCH_ROUTE
        },
        "methods": {
            "polish_text": ["POST"],
            "polish_batch": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/api/polish.rs",
            "backend-rs/src/services/prompt_template_service.rs",
            "backend-rs/src/services/settings_service.rs",
            "backend-rs/src/ai/service.rs",
            "backend-rs/src/ai/config.rs",
            "backend-rs/src/models/generation_history.rs"
        ],
        "readiness_probes": [
            "polish-text-auth-guard-rust",
            "polish-batch-auth-guard-rust",
            "polish-configure-mock-openai-business-rust",
            "polish-text-business-rust",
            "polish-batch-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-polish-business-owner",
            "business_probes": [
                "polish-configure-mock-openai-business-rust",
                "polish-text-business-rust",
                "polish-batch-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-polish-business-owner",
            "readiness_probe_count": 5,
            "business_probe_count": 3,
            "auth_guard_probe_count": 2,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "source_map_files": [
            "backend/migrator_app/models/generation_history.py"
        ],
        "next_cutover_gate": "explicit polish generation_history source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Polish route business smoke is covered by phase5-polish-business-owner; the Python route shell and schema shell are no longer part of the surviving Python API surface, the legacy ai_service compatibility shim has already been physically deleted, and the route-facing AI_DENOISING prompt template definition/render path now lives in Rust PromptTemplateService plus bundled system_templates_data.json. The old Python generation-history runtime-store service file is also physically deleted, and the remaining closeout work is now limited to the shared generation_history model source-map contract under one explicit freeze/delete/repoint approval with same-round rollback policy.",
        "behavior_contract": {
            "text_alias": "original_text accepts the legacy text alias",
            "empty_text_error": "original_text or text empty returns 400",
            "batch_empty_error": "empty batch texts return 400",
            "project_history": "single polish writes generation_history when project_id is present; batch keeps legacy no-history behavior"
        },
        "rollback_boundary": {
            "source_map_policy": "polish_route_source_map_deleted_remaining_generation_history_model_only",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "polish_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_route_files_status": "polish_route_source_map_deleted_remaining_generation_history_model_only",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust polish route group has dedicated phase5-polish-business-owner probes for mock OpenAI configuration, text polish, and batch polish behavior; the Python route shell and schema shell have been removed from the surviving Python API surface, the legacy ai_service compatibility shim has already been deleted, the route-facing AI_DENOISING prompt template definition/render path already lives in Rust PromptTemplateService plus bundled system_templates_data.json, the old Python generation-history runtime-store service file is physically deleted, and the remaining source map is now limited to the shared generation_history model file.",
            "rollback_files": []
        }
    })
}

impl PolishService {
    fn build_history_prompt(original_text: &str) -> String {
        let preview = original_text.chars().take(100).collect::<String>();
        format!("原文: {}...", preview)
    }

    fn focus_instruction(mode: &str) -> &'static str {
        FOCUS_INSTRUCTIONS
            .iter()
            .find(|(candidate, _)| *candidate == mode)
            .map(|(_, instruction)| *instruction)
            .unwrap_or(FOCUS_INSTRUCTIONS[0].1)
    }

    fn build_runtime_blocks(
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
    ) -> HashMap<String, String> {
        let focus = Self::focus_instruction(focus_mode).to_string();

        let structure = vec![
            "- 尽量保留原文的情节顺序和信息密度，不要重写成另一种故事。".to_string(),
            if preserve_paragraphs {
                "- 保留原段落边界和段间呼吸感，除非原文断段明显影响阅读。".to_string()
            } else {
                "- 允许按节奏重新切分段落，但不要打散原有事件顺序。".to_string()
            },
            if retain_hooks {
                "- 保留段尾和章尾的悬念、动作牵引或情绪悬置，不要抹平成总结句。".to_string()
            } else {
                "- 可以适度重写尾句，但仍要保住阅读牵引力。".to_string()
            },
        ]
        .join("\n");

        let style_hint = style.unwrap_or("").trim();
        let style_block = if !style_hint.is_empty() {
            format!("【额外风格偏好】\n- {}", style_hint)
        } else {
            "【额外风格偏好】\n- 无额外补充，按自然中文网文表达处理。".to_string()
        };

        let mut blocks = HashMap::new();
        blocks.insert("focus_instruction".to_string(), focus);
        blocks.insert("structure_instruction".to_string(), structure);
        blocks.insert("style_hint_block".to_string(), style_block);
        blocks
    }

    async fn record_polish_history(
        db: &DatabaseConnection,
        project_id: i64,
        original_text: &str,
        polished_text: &str,
        model_name: Option<&str>,
    ) -> Result<(), String> {
        generation_history::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            chapter_id: Set(None),
            prompt: Set(Some(Self::build_history_prompt(original_text))),
            generated_content: Set(Some(polished_text.to_string())),
            model: Set(Some(model_name.unwrap_or("default").to_string())),
            tokens_used: Set(None),
            generation_time: Set(None),
            created_at: Set(Some(Utc::now().naive_utc())),
        }
        .insert(db)
        .await
        .map_err(|error| format!("记录AI去味历史失败: {}", error))?;

        Ok(())
    }

    async fn polish_text(
        db: &DatabaseConnection,
        user_id: &str,
        project_id: Option<i64>,
        original_text: &str,
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<Value, String> {
        let mut params =
            Self::build_runtime_blocks(style, focus_mode, preserve_paragraphs, retain_hooks);
        params.insert("original_text".to_string(), original_text.to_string());

        let template = PromptTemplateService::system_template_info("AI_DENOISING")
            .ok_or("AI_DENOISING 模板不存在")?;
        let prompt = PromptTemplateService::format_prompt(&template.content, &params)?;

        let config = SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            temperature_override,
        )
        .await?;
        let service = AIService::new(config);
        let response = service.generate_text(&prompt, None, None).await?;
        let polished_text = response.content;

        if let Some(project_id) = project_id {
            Self::record_polish_history(
                db,
                project_id,
                original_text,
                &polished_text,
                model_override,
            )
            .await?;
        }

        Ok(json!({
            "original_text": original_text,
            "polished_text": polished_text,
            "word_count_before": original_text.chars().count(),
            "word_count_after": polished_text.chars().count(),
        }))
    }

    async fn polish_batch(
        db: &DatabaseConnection,
        user_id: &str,
        texts: &[String],
        style: Option<&str>,
        focus_mode: &str,
        preserve_paragraphs: bool,
        retain_hooks: bool,
        provider_override: Option<&str>,
        model_override: Option<&str>,
        temperature_override: Option<f64>,
    ) -> Result<Value, String> {
        let runtime_blocks =
            Self::build_runtime_blocks(style, focus_mode, preserve_paragraphs, retain_hooks);

        let template = PromptTemplateService::system_template_info("AI_DENOISING")
            .ok_or("AI_DENOISING 模板不存在")?;

        let config = SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            temperature_override,
        )
        .await?;
        let service = AIService::new(config);

        let mut results = Vec::new();
        for (index, text) in texts.iter().enumerate() {
            let mut params = runtime_blocks.clone();
            params.insert("original_text".to_string(), text.clone());

            let prompt = PromptTemplateService::format_prompt(&template.content, &params)?;
            let response = service.generate_text(&prompt, None, None).await?;

            results.push(json!({
                "index": index,
                "original": text,
                "polished": response.content,
                "word_count_before": text.chars().count(),
                "word_count_after": response.content.chars().count(),
            }));
        }

        Ok(json!({
            "total": results.len(),
            "results": results,
        }))
    }
}

fn default_focus_mode() -> String {
    "balanced".into()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct PolishRequest {
    #[serde(alias = "text")]
    original_text: String,
    project_id: Option<i64>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    style: Option<String>,
    #[serde(default = "default_focus_mode")]
    focus_mode: String,
    #[serde(default = "default_true")]
    preserve_paragraphs: bool,
    #[serde(default = "default_true")]
    retain_hooks: bool,
}

#[derive(Deserialize)]
struct PolishBatchRequest {
    texts: Vec<String>,
    #[allow(dead_code)]
    project_id: Option<i64>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
    style: Option<String>,
    #[serde(default = "default_focus_mode")]
    focus_mode: String,
    #[serde(default = "default_true")]
    preserve_paragraphs: bool,
    #[serde(default = "default_true")]
    retain_hooks: bool,
}

async fn polish_text(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PolishRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let original = body.original_text.trim().to_string();
    if original.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "original_text 或 text 不能为空"})),
        ));
    }

    match PolishService::polish_text(
        &db,
        &claims.sub,
        body.project_id,
        &original,
        body.style.as_deref(),
        &body.focus_mode,
        body.preserve_paragraphs,
        body.retain_hooks,
        body.provider.as_deref(),
        body.model.as_deref(),
        body.temperature,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("AI去味失败: {}", e)})),
        )),
    }
}

async fn polish_batch(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PolishBatchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let normalized: Vec<String> = body
        .texts
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if normalized.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "texts 不能为空"})),
        ));
    }

    match PolishService::polish_batch(
        &db,
        &claims.sub,
        &normalized,
        body.style.as_deref(),
        &body.focus_mode,
        body.preserve_paragraphs,
        body.retain_hooks,
        body.provider.as_deref(),
        body.model.as_deref(),
        body.temperature,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("批量AI去味失败: {}", e)})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(POLISH_TEXT_ROUTE, post(polish_text))
        .route(POLISH_BATCH_ROUTE, post(polish_batch))
}

#[cfg(test)]
mod tests {
    use super::{
        build_polish_route_owner_contract, PolishService, POLISH_BATCH_ROUTE, POLISH_TEXT_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_polish_route_owner_contract() {
        let contract = build_polish_route_owner_contract();

        assert_eq!(contract["owner"], json!("polish"));
        assert_eq!(
            contract["rust_owner"],
            json!("backend-rs/src/api/polish.rs")
        );
        assert_eq!(contract["routes"]["polish_text"], json!(POLISH_TEXT_ROUTE));
        assert_eq!(
            contract["routes"]["polish_batch"],
            json!(POLISH_BATCH_ROUTE)
        );
        assert_eq!(contract["methods"]["polish_text"], json!(["POST"]));
        assert_eq!(contract["methods"]["polish_batch"], json!(["POST"]));
        assert_eq!(contract["service_owners"].as_array().map(Vec::len), Some(6));
        assert_eq!(
            contract["readiness_probes"].as_array().map(Vec::len),
            Some(5)
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .and_then(|probes| probes.last()),
            Some(&json!("polish-batch-business-rust"))
        );
        assert_eq!(
            contract["source_map_files"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(
            contract["source_map_files"][0],
            json!("backend/migrator_app/models/generation_history.py")
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            json!("phase5-polish-business-owner")
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be present");
        assert_eq!(business_probes.len(), 3);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "polish-text-business-rust"));
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            json!("covered_by_dedicated_rust_owner_profile")
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(5)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(3)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            json!("explicit polish generation_history source-map freeze/delete/repoint approval with same-round rollback policy")
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-polish-business-owner"));
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("PromptTemplateService"));
        assert!(contract["behavior_contract"]["project_history"]
            .as_str()
            .unwrap_or_default()
            .contains("generation_history"));
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
            json!("polish_route_runtime_registration_deleted_no_python_route_shell_remains")
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            json!("polish_route_source_map_deleted_remaining_generation_history_model_only")
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }

    #[test]
    fn should_keep_polish_route_group_paths_stable() {
        let contract = build_polish_route_owner_contract();

        assert_eq!(
            contract["routes"],
            json!({
                "polish_text": POLISH_TEXT_ROUTE,
                "polish_batch": POLISH_BATCH_ROUTE
            })
        );
    }

    #[test]
    fn should_build_python_compatible_polish_history_prompt() {
        let original = "一二三四五六七八九十".repeat(12);
        let prompt = PolishService::build_history_prompt(&original);

        assert!(prompt.starts_with("原文: "));
        assert!(prompt.ends_with("..."));
        assert_eq!(
            prompt
                .trim_start_matches("原文: ")
                .trim_end_matches("...")
                .chars()
                .count(),
            100
        );
    }

    #[test]
    fn should_keep_unknown_focus_mode_on_balanced_instruction() {
        assert_eq!(
            PolishService::focus_instruction("unknown"),
            PolishService::focus_instruction("balanced")
        );
    }
}
