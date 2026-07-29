use std::{collections::HashMap, fmt, future::Future};

use sea_orm::DatabaseConnection;
use serde::Serialize;
use tokio_stream::StreamExt;

use crate::{
    ai::service::AIService,
    services::{
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_service::ProjectService, prompt_template_service::PromptTemplateService,
        settings_service::SettingsService, wizard_service::clean_json_response,
    },
};

const MAX_WORLD_GENERATION_ATTEMPTS: u32 = 3;
const ESTIMATED_WORLD_OUTPUT_CHARS: usize = 3_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldGenerationFailurePolicy {
    ReturnError,
    UseCompatibilityPlaceholder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateWorldBuildingForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
    pub failure_policy: WorldGenerationFailurePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedWorldBuilding {
    pub time_period: Option<String>,
    pub location: Option<String>,
    pub atmosphere: Option<String>,
    pub rules: Option<String>,
    pub provider: String,
    pub model: String,
    pub attempts: u32,
    pub used_compatibility_placeholder: bool,
    pub content_digest: String,
}

impl GeneratedWorldBuilding {
    pub(crate) fn is_complete(&self) -> bool {
        [
            self.time_period.as_deref(),
            self.location.as_deref(),
            self.atmosphere.as_deref(),
            self.rules.as_deref(),
        ]
        .into_iter()
        .all(|value| value.is_some_and(|value| !value.trim().is_empty()))
            && !self.used_compatibility_placeholder
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorldGenerationProgress {
    pub message: String,
    pub progress: u32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WizardWorldGenerationError {
    Cancelled,
    ProjectNotFoundOrAccessDenied,
    ProjectRead,
    AiConfig,
    TemplateMissing,
    PromptFormat,
    Provider,
    EmptyResponse,
    InvalidResponse,
    Observer,
}

impl WizardWorldGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "world_generation_cancelled",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::AiConfig => "ai_config_failed",
            Self::TemplateMissing => "world_building_template_missing",
            Self::PromptFormat => "world_building_prompt_format_failed",
            Self::Provider => "world_generation_provider_failed",
            Self::EmptyResponse => "world_generation_empty_response",
            Self::InvalidResponse => "world_generation_invalid_response",
            Self::Observer => "world_generation_observer_failed",
        }
    }

    pub(crate) const fn user_message(&self) -> &'static str {
        match self {
            Self::Cancelled => "世界观生成已取消",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::AiConfig => "AI配置失败",
            Self::TemplateMissing => "WORLD_BUILDING模板未找到",
            Self::PromptFormat => "世界观提示词格式化失败",
            Self::Provider => "世界观模型调用失败",
            Self::EmptyResponse => "AI多次返回为空，请稍后重试",
            Self::InvalidResponse => "AI多次返回了无效的世界观数据，请稍后重试",
            Self::Observer => "世界观生成进度输出失败",
        }
    }
}

impl fmt::Display for WizardWorldGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for WizardWorldGenerationError {}

pub(crate) async fn generate_world_building_for_project<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateWorldBuildingForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    on_progress: P,
    on_content: C,
    on_reasoning: R,
) -> Result<GeneratedWorldBuilding, WizardWorldGenerationError>
where
    P: FnMut(WorldGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    generate_world_building_for_project_with_guidance(
        db,
        request,
        None,
        cancellation_token,
        on_progress,
        on_content,
        on_reasoning,
    )
    .await
}

pub(crate) async fn generate_world_building_for_project_with_guidance<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateWorldBuildingForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_progress: P,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedWorldBuilding, WizardWorldGenerationError>
where
    P: FnMut(WorldGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "加载项目信息...", 5, "processing").await?;

    let project = ProjectService::get(db, request.project_id, request.user_id)
        .await
        .map_err(|_| WizardWorldGenerationError::ProjectRead)?
        .ok_or(WizardWorldGenerationError::ProjectNotFoundOrAccessDenied)?;

    ensure_not_cancelled(cancellation_token)?;
    let ai_config = SettingsService::build_ai_config(
        db,
        request.user_id,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| WizardWorldGenerationError::AiConfig)?;
    let provider = ai_config.provider.clone();
    let model = ai_config.model.clone();
    let system_prompt = ai_config.system_prompt.clone();

    emit_progress(&mut on_progress, "准备AI提示词...", 15, "processing").await?;
    let template = PromptTemplateService::system_template_info("WORLD_BUILDING")
        .ok_or(WizardWorldGenerationError::TemplateMissing)?;
    let mut params = HashMap::new();
    params.insert("title".to_string(), project.title);
    params.insert(
        "theme".to_string(),
        project.theme.as_deref().unwrap_or("未设定").to_string(),
    );
    params.insert(
        "genre".to_string(),
        project.genre.as_deref().unwrap_or("通用").to_string(),
    );
    params.insert(
        "description".to_string(),
        project.description.unwrap_or_default(),
    );
    let prompt = PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(|_| WizardWorldGenerationError::PromptFormat)?;
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);
    let ai_service = AIService::new(ai_config);

    let mut last_failure = WizardWorldGenerationError::EmptyResponse;
    for attempt in 1..=MAX_WORLD_GENERATION_ATTEMPTS {
        ensure_not_cancelled(cancellation_token)?;
        if attempt > 1 {
            emit_progress(
                &mut on_progress,
                &format!(
                    "⚠ 重试中... ({}/{})",
                    attempt - 1,
                    MAX_WORLD_GENERATION_ATTEMPTS
                ),
                20,
                "processing",
            )
            .await?;
        }
        emit_progress(&mut on_progress, "重新生成世界观...", 20, "processing").await?;

        let mut accumulated_text = String::new();
        let mut chunk_count = 0u64;
        let mut provider_failed = false;
        let mut stream =
            ai_service.generate_text_stream(prompt.clone(), system_prompt.clone(), None);

        while let Some(chunk_result) = stream.next().await {
            ensure_not_cancelled(cancellation_token)?;
            match chunk_result {
                Ok(chunk) => {
                    if let Some(reasoning) =
                        chunk.reasoning_content.filter(|value| !value.is_empty())
                    {
                        on_reasoning(reasoning)
                            .await
                            .map_err(|_| WizardWorldGenerationError::Observer)?;
                    }
                    if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                        accumulated_text.push_str(&content);
                        on_content(content)
                            .await
                            .map_err(|_| WizardWorldGenerationError::Observer)?;
                        chunk_count += 1;
                        if chunk_count % 10 == 0 {
                            let char_bonus = (accumulated_text.chars().count() as f64
                                / ESTIMATED_WORLD_OUTPUT_CHARS as f64
                                * 60.0) as u32;
                            emit_progress(
                                &mut on_progress,
                                &format!(
                                    "重新生成世界观... ({}字符)",
                                    accumulated_text.chars().count()
                                ),
                                (20 + char_bonus).clamp(20, 80),
                                "processing",
                            )
                            .await?;
                        }
                    }
                    if chunk.done {
                        break;
                    }
                }
                Err(_) => {
                    provider_failed = true;
                }
            }
        }

        if accumulated_text.trim().is_empty() {
            last_failure = if provider_failed {
                WizardWorldGenerationError::Provider
            } else {
                WizardWorldGenerationError::EmptyResponse
            };
            continue;
        }

        emit_progress(&mut on_progress, "解析世界观数据...", 85, "processing").await?;
        match parse_generated_world_building(&accumulated_text, &provider, &model, attempt) {
            Ok(result) => return Ok(result),
            Err(error) => last_failure = error,
        }
    }

    match request.failure_policy {
        WorldGenerationFailurePolicy::ReturnError => Err(last_failure),
        WorldGenerationFailurePolicy::UseCompatibilityPlaceholder => Ok(compatibility_placeholder(
            provider,
            model,
            MAX_WORLD_GENERATION_ATTEMPTS,
        )),
    }
}

async fn emit_progress<P, PFut>(
    on_progress: &mut P,
    message: &str,
    progress: u32,
    status: &'static str,
) -> Result<(), WizardWorldGenerationError>
where
    P: FnMut(WorldGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
{
    on_progress(WorldGenerationProgress {
        message: message.to_string(),
        progress,
        status,
    })
    .await
    .map_err(|_| WizardWorldGenerationError::Observer)
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), WizardWorldGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(WizardWorldGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn parse_generated_world_building(
    raw_content: &str,
    provider: &str,
    model: &str,
    attempts: u32,
) -> Result<GeneratedWorldBuilding, WizardWorldGenerationError> {
    let cleaned = clean_json_response(raw_content);
    let data = serde_json::from_str::<serde_json::Value>(&cleaned)
        .map_err(|_| WizardWorldGenerationError::InvalidResponse)?;
    if !data.is_object() {
        return Err(WizardWorldGenerationError::InvalidResponse);
    }

    Ok(GeneratedWorldBuilding {
        time_period: optional_non_empty_string(&data, "time_period"),
        location: optional_non_empty_string(&data, "location"),
        atmosphere: optional_non_empty_string(&data, "atmosphere"),
        rules: optional_non_empty_string(&data, "rules"),
        provider: provider.to_string(),
        model: model.to_string(),
        attempts,
        used_compatibility_placeholder: false,
        content_digest: format!("{:x}", md5::compute(cleaned.as_bytes())),
    })
}

fn optional_non_empty_string(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn compatibility_placeholder(
    provider: String,
    model: String,
    attempts: u32,
) -> GeneratedWorldBuilding {
    const MESSAGE: &str = "AI多次返回为空，请稍后重试";
    GeneratedWorldBuilding {
        time_period: Some(MESSAGE.to_string()),
        location: Some(MESSAGE.to_string()),
        atmosphere: Some(MESSAGE.to_string()),
        rules: Some(MESSAGE.to_string()),
        provider,
        model,
        attempts,
        used_compatibility_placeholder: true,
        content_digest: format!("{:x}", md5::compute(MESSAGE.as_bytes())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compatibility_placeholder, parse_generated_world_building, WizardWorldGenerationError,
    };

    #[test]
    fn parses_fenced_world_json_into_typed_result() {
        let result = parse_generated_world_building(
            "```json\n{\"time_period\":\"星历42年\",\"location\":\"浮空城\",\"atmosphere\":\"压抑\",\"rules\":\"记忆可交易\"}\n```",
            "openai",
            "model-1",
            2,
        )
        .expect("valid world result");

        assert!(result.is_complete());
        assert_eq!(result.attempts, 2);
        assert_eq!(result.provider, "openai");
        assert!(!result.content_digest.is_empty());
    }

    #[test]
    fn partial_world_result_is_typed_but_not_complete() {
        let result =
            parse_generated_world_building(r#"{"time_period":"星历42年"}"#, "openai", "model-1", 1)
                .expect("partial object remains inspectable");

        assert!(!result.is_complete());
        assert!(result.location.is_none());
    }

    #[test]
    fn invalid_json_returns_stable_error_code() {
        let error = parse_generated_world_building("not-json", "openai", "model-1", 1)
            .expect_err("invalid response");

        assert_eq!(error, WizardWorldGenerationError::InvalidResponse);
        assert_eq!(error.code(), "world_generation_invalid_response");
    }

    #[test]
    fn compatibility_placeholder_can_never_be_persisted_as_complete_world() {
        let result = compatibility_placeholder("openai".into(), "model-1".into(), 3);

        assert!(result.used_compatibility_placeholder);
        assert!(!result.is_complete());
    }
}
