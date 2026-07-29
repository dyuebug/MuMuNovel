use std::{collections::HashMap, fmt, future::Future};

use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;
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

const MAX_CAREER_GENERATION_ATTEMPTS: u32 = 3;
const ESTIMATED_CAREER_OUTPUT_CHARS: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateCareerSystemForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct GeneratedCareerSystem {
    pub main_careers: Vec<GeneratedCareer>,
    pub sub_careers: Vec<GeneratedCareer>,
    pub provider: String,
    pub model: String,
    pub attempts: u32,
    pub content_digest: String,
}

impl GeneratedCareerSystem {
    pub(crate) fn is_complete(&self) -> bool {
        !self.main_careers.is_empty()
            && !self.sub_careers.is_empty()
            && self
                .main_careers
                .iter()
                .chain(&self.sub_careers)
                .all(GeneratedCareer::is_complete)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct GeneratedCareer {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub stages: Vec<GeneratedCareerStage>,
    pub max_stage: i32,
    pub requirements: Option<String>,
    pub special_abilities: Option<String>,
    pub worldview_rules: Option<String>,
    pub attribute_bonuses: Option<Value>,
}

impl GeneratedCareer {
    fn is_complete(&self) -> bool {
        self.max_stage > 0
            && self.stages.len() == self.max_stage as usize
            && self
                .stages
                .iter()
                .enumerate()
                .all(|(index, stage)| stage.level == index as i32 + 1 && !stage.name.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedCareerStage {
    pub level: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CareerGenerationProgress {
    pub message: String,
    pub progress: u32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WizardCareerGenerationError {
    Cancelled,
    ProjectNotFoundOrAccessDenied,
    ProjectRead,
    AiConfig,
    TemplateMissing,
    PromptFormat,
    Provider,
    EmptyResponse,
    InvalidResponse,
    IncompleteResponse,
    Observer,
}

impl WizardCareerGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "career_generation_cancelled",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::AiConfig => "ai_config_failed",
            Self::TemplateMissing => "career_system_template_missing",
            Self::PromptFormat => "career_system_prompt_format_failed",
            Self::Provider => "career_generation_provider_failed",
            Self::EmptyResponse => "career_generation_empty_response",
            Self::InvalidResponse => "career_generation_invalid_response",
            Self::IncompleteResponse => "career_generation_incomplete_response",
            Self::Observer => "career_generation_observer_failed",
        }
    }

    pub(crate) const fn user_message(&self) -> &'static str {
        match self {
            Self::Cancelled => "职业体系生成已取消",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::AiConfig => "AI配置失败",
            Self::TemplateMissing => "CAREER_SYSTEM_GENERATION模板未找到",
            Self::PromptFormat => "职业体系提示词格式化失败",
            Self::Provider => "职业体系模型调用失败",
            Self::EmptyResponse => "AI多次返回为空，请稍后重试",
            Self::InvalidResponse => "AI多次返回了无效的职业体系数据，请稍后重试",
            Self::IncompleteResponse => "AI多次返回了不完整的职业体系数据，请稍后重试",
            Self::Observer => "职业体系生成进度输出失败",
        }
    }
}

impl fmt::Display for WizardCareerGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for WizardCareerGenerationError {}

pub(crate) async fn generate_career_system_for_project<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateCareerSystemForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    on_progress: P,
    on_content: C,
    on_reasoning: R,
) -> Result<GeneratedCareerSystem, WizardCareerGenerationError>
where
    P: FnMut(CareerGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    generate_career_system_for_project_with_guidance(
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

pub(crate) async fn generate_career_system_for_project_with_guidance<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateCareerSystemForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_progress: P,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedCareerSystem, WizardCareerGenerationError>
where
    P: FnMut(CareerGenerationProgress) -> PFut,
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
        .map_err(|_| WizardCareerGenerationError::ProjectRead)?
        .ok_or(WizardCareerGenerationError::ProjectNotFoundOrAccessDenied)?;

    ensure_not_cancelled(cancellation_token)?;
    let ai_config = SettingsService::build_ai_config(
        db,
        request.user_id,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| WizardCareerGenerationError::AiConfig)?;
    let provider = ai_config.provider.clone();
    let model = ai_config.model.clone();
    let system_prompt = ai_config.system_prompt.clone();

    emit_progress(&mut on_progress, "准备AI提示词...", 15, "processing").await?;
    let template = PromptTemplateService::system_template_info("CAREER_SYSTEM_GENERATION")
        .ok_or(WizardCareerGenerationError::TemplateMissing)?;
    let mut params = HashMap::new();
    params.insert("title".to_string(), project.title);
    params.insert(
        "genre".to_string(),
        project.genre.unwrap_or_else(|| "通用".to_string()),
    );
    params.insert(
        "theme".to_string(),
        project.theme.unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "description".to_string(),
        project.description.unwrap_or_default(),
    );
    params.insert(
        "time_period".to_string(),
        project
            .world_time_period
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "location".to_string(),
        project
            .world_location
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "atmosphere".to_string(),
        project
            .world_atmosphere
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "rules".to_string(),
        project.world_rules.unwrap_or_else(|| "未设定".to_string()),
    );
    let prompt = PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(|_| WizardCareerGenerationError::PromptFormat)?;
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);
    let ai_service = AIService::new(ai_config);

    let mut last_failure = WizardCareerGenerationError::EmptyResponse;
    for attempt in 1..=MAX_CAREER_GENERATION_ATTEMPTS {
        ensure_not_cancelled(cancellation_token)?;
        if attempt > 1 {
            emit_progress(
                &mut on_progress,
                &format!(
                    "⚠ 重试中... ({}/{})",
                    attempt - 1,
                    MAX_CAREER_GENERATION_ATTEMPTS
                ),
                20,
                "processing",
            )
            .await?;
        }
        emit_progress(&mut on_progress, "生成职业体系中...", 20, "processing").await?;

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
                            .map_err(|_| WizardCareerGenerationError::Observer)?;
                    }
                    if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                        accumulated_text.push_str(&content);
                        on_content(content)
                            .await
                            .map_err(|_| WizardCareerGenerationError::Observer)?;
                        chunk_count += 1;
                        if chunk_count % 10 == 0 {
                            let char_bonus = (accumulated_text.chars().count() as f64
                                / ESTIMATED_CAREER_OUTPUT_CHARS as f64
                                * 60.0) as u32;
                            emit_progress(
                                &mut on_progress,
                                &format!(
                                    "生成职业体系中... ({}字符)",
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
                Err(_) => provider_failed = true,
            }
        }

        if accumulated_text.trim().is_empty() {
            last_failure = if provider_failed {
                WizardCareerGenerationError::Provider
            } else {
                WizardCareerGenerationError::EmptyResponse
            };
            continue;
        }

        emit_progress(&mut on_progress, "解析职业体系数据...", 85, "processing").await?;
        match parse_generated_career_system(&accumulated_text, &provider, &model, attempt) {
            Ok(result) => return Ok(result),
            Err(error) => last_failure = error,
        }
    }

    Err(last_failure)
}

async fn emit_progress<P, PFut>(
    on_progress: &mut P,
    message: &str,
    progress: u32,
    status: &'static str,
) -> Result<(), WizardCareerGenerationError>
where
    P: FnMut(CareerGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
{
    on_progress(CareerGenerationProgress {
        message: message.to_string(),
        progress,
        status,
    })
    .await
    .map_err(|_| WizardCareerGenerationError::Observer)
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), WizardCareerGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(WizardCareerGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn parse_generated_career_system(
    raw_content: &str,
    provider: &str,
    model: &str,
    attempts: u32,
) -> Result<GeneratedCareerSystem, WizardCareerGenerationError> {
    let cleaned = clean_json_response(raw_content);
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| WizardCareerGenerationError::InvalidResponse)?;
    let data = data
        .as_object()
        .ok_or(WizardCareerGenerationError::InvalidResponse)?;

    let main_careers = parse_careers(data.get("main_careers"))?;
    let sub_careers = parse_careers(data.get("sub_careers"))?;
    let result = GeneratedCareerSystem {
        main_careers,
        sub_careers,
        provider: provider.to_string(),
        model: model.to_string(),
        attempts,
        content_digest: format!("{:x}", md5::compute(cleaned.as_bytes())),
    };

    if result.is_complete() {
        Ok(result)
    } else {
        Err(WizardCareerGenerationError::IncompleteResponse)
    }
}

fn parse_careers(
    value: Option<&Value>,
) -> Result<Vec<GeneratedCareer>, WizardCareerGenerationError> {
    let careers = value
        .and_then(Value::as_array)
        .ok_or(WizardCareerGenerationError::IncompleteResponse)?;

    careers.iter().map(parse_career).collect()
}

fn parse_career(value: &Value) -> Result<GeneratedCareer, WizardCareerGenerationError> {
    let data = value
        .as_object()
        .ok_or(WizardCareerGenerationError::IncompleteResponse)?;
    let name = required_non_empty_string(data, "name")?;
    let max_stage = data
        .get("max_stage")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0 && *value <= i32::MAX as i64)
        .ok_or(WizardCareerGenerationError::IncompleteResponse)? as i32;
    let stages = data
        .get("stages")
        .and_then(Value::as_array)
        .ok_or(WizardCareerGenerationError::IncompleteResponse)?
        .iter()
        .map(parse_stage)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GeneratedCareer {
        name,
        description: optional_non_empty_string(data, "description"),
        category: optional_non_empty_string(data, "category"),
        stages,
        max_stage,
        requirements: optional_non_empty_string(data, "requirements"),
        special_abilities: optional_non_empty_string(data, "special_abilities"),
        worldview_rules: optional_non_empty_string(data, "worldview_rules"),
        attribute_bonuses: data
            .get("attribute_bonuses")
            .map(|value| {
                value
                    .as_object()
                    .map(|_| value.clone())
                    .ok_or(WizardCareerGenerationError::IncompleteResponse)
            })
            .transpose()?,
    })
}

fn parse_stage(value: &Value) -> Result<GeneratedCareerStage, WizardCareerGenerationError> {
    let data = value
        .as_object()
        .ok_or(WizardCareerGenerationError::IncompleteResponse)?;
    let level = data
        .get("level")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0 && *value <= i32::MAX as i64)
        .ok_or(WizardCareerGenerationError::IncompleteResponse)? as i32;

    Ok(GeneratedCareerStage {
        level,
        name: required_non_empty_string(data, "name")?,
        description: optional_non_empty_string(data, "description"),
    })
}

fn required_non_empty_string(
    data: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, WizardCareerGenerationError> {
    optional_non_empty_string(data, key).ok_or(WizardCareerGenerationError::IncompleteResponse)
}

fn optional_non_empty_string(data: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{parse_generated_career_system, WizardCareerGenerationError};

    const VALID_CAREER_SYSTEM: &str = r#"{
        "main_careers": [{
            "name": "星辰骑士",
            "description": "守护星门的战士",
            "category": "战斗系",
            "max_stage": 2,
            "stages": [
                {"level": 1, "name": "见习骑士", "description": "初识星辉"},
                {"level": 2, "name": "星门骑士", "description": "守护星门"}
            ],
            "requirements": "完成誓约",
            "special_abilities": "星辉斩",
            "worldview_rules": "遵守星门法则",
            "attribute_bonuses": {"strength": "+10%"}
        }],
        "sub_careers": [{
            "name": "星图师",
            "description": "绘制星图",
            "category": "辅助系",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "学徒", "description": "学习星图"}],
            "requirements": "识字",
            "special_abilities": "辨星"
        }]
    }"#;

    #[test]
    fn parses_fenced_career_json_into_typed_result() {
        let result = parse_generated_career_system(
            &format!("```json\n{VALID_CAREER_SYSTEM}\n```"),
            "openai",
            "model-1",
            2,
        )
        .expect("valid career system");

        assert!(result.is_complete());
        assert_eq!(result.provider, "openai");
        assert_eq!(result.model, "model-1");
        assert_eq!(result.attempts, 2);
        assert_eq!(result.main_careers[0].name, "星辰骑士");
        assert!(!result.content_digest.is_empty());
    }

    #[test]
    fn invalid_json_returns_stable_error_code() {
        let error = parse_generated_career_system("not-json", "openai", "model-1", 1)
            .expect_err("invalid response");

        assert_eq!(error, WizardCareerGenerationError::InvalidResponse);
        assert_eq!(error.code(), "career_generation_invalid_response");
    }

    #[test]
    fn stages_length_must_match_max_stage() {
        let invalid = VALID_CAREER_SYSTEM.replace("\"max_stage\": 2", "\"max_stage\": 3");
        let error = parse_generated_career_system(&invalid, "openai", "model-1", 1)
            .expect_err("incomplete career system");

        assert_eq!(error, WizardCareerGenerationError::IncompleteResponse);
    }

    #[test]
    fn career_stages_must_start_at_one_and_be_contiguous() {
        let invalid = VALID_CAREER_SYSTEM.replace("\"level\": 2", "\"level\": 3");
        let error = parse_generated_career_system(&invalid, "openai", "model-1", 1)
            .expect_err("invalid career stages");

        assert_eq!(error, WizardCareerGenerationError::IncompleteResponse);
    }

    #[test]
    fn each_career_group_must_have_at_least_one_entry() {
        let invalid = VALID_CAREER_SYSTEM.replace(
            r#""sub_careers": [{
            "name": "星图师",
            "description": "绘制星图",
            "category": "辅助系",
            "max_stage": 1,
            "stages": [{"level": 1, "name": "学徒", "description": "学习星图"}],
            "requirements": "识字",
            "special_abilities": "辨星"
        }]"#,
            r#""sub_careers": []"#,
        );
        let error = parse_generated_career_system(&invalid, "openai", "model-1", 1)
            .expect_err("empty sub-careers must be rejected");

        assert_eq!(error, WizardCareerGenerationError::IncompleteResponse);
    }
}
