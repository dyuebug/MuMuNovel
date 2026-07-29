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

const MAX_FOUNDATION_GENERATION_ATTEMPTS: u32 = 3;
const ESTIMATED_FOUNDATION_OUTPUT_CHARS: usize = 1_500;
const DEFAULT_NARRATIVE_PERSPECTIVE: &str = "第三人称";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateFoundationForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedFoundation {
    pub title: String,
    pub description: String,
    pub theme: String,
    pub genre: Vec<String>,
    pub narrative_perspective: String,
    pub provider: String,
    pub model: String,
    pub attempts: u32,
    pub content_digest: String,
}

impl GeneratedFoundation {
    pub(crate) fn is_complete(&self) -> bool {
        !self.title.trim().is_empty()
            && !self.description.trim().is_empty()
            && !self.theme.trim().is_empty()
            && !self.genre.is_empty()
            && self.genre.iter().all(|item| !item.trim().is_empty())
            && !self.narrative_perspective.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FoundationGenerationProgress {
    pub message: String,
    pub progress: u32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoundationGenerationError {
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

impl FoundationGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "foundation_generation_cancelled",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::AiConfig => "ai_config_failed",
            Self::TemplateMissing => "foundation_template_missing",
            Self::PromptFormat => "foundation_prompt_format_failed",
            Self::Provider => "foundation_generation_provider_failed",
            Self::EmptyResponse => "foundation_generation_empty_response",
            Self::InvalidResponse => "foundation_generation_invalid_response",
            Self::IncompleteResponse => "foundation_generation_incomplete_response",
            Self::Observer => "foundation_generation_observer_failed",
        }
    }

    pub(crate) const fn user_message(&self) -> &'static str {
        match self {
            Self::Cancelled => "基础设定生成已取消",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::AiConfig => "AI配置失败",
            Self::TemplateMissing => "INSPIRATION_QUICK_COMPLETE模板未找到",
            Self::PromptFormat => "基础设定提示词格式化失败",
            Self::Provider => "基础设定模型调用失败",
            Self::EmptyResponse => "AI多次返回为空，请稍后重试",
            Self::InvalidResponse => "AI多次返回了无效的基础设定数据，请稍后重试",
            Self::IncompleteResponse => "AI多次返回了不完整的基础设定数据，请稍后重试",
            Self::Observer => "基础设定生成进度输出失败",
        }
    }
}

impl fmt::Display for FoundationGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FoundationGenerationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingFoundation {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Vec<String>,
    narrative_perspective: Option<String>,
}

pub(crate) async fn generate_foundation_for_project<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateFoundationForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    on_progress: P,
    on_content: C,
    on_reasoning: R,
) -> Result<GeneratedFoundation, FoundationGenerationError>
where
    P: FnMut(FoundationGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    generate_foundation_for_project_with_guidance(
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

pub(crate) async fn generate_foundation_for_project_with_guidance<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateFoundationForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_progress: P,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedFoundation, FoundationGenerationError>
where
    P: FnMut(FoundationGenerationProgress) -> PFut,
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
        .map_err(|_| FoundationGenerationError::ProjectRead)?
        .ok_or(FoundationGenerationError::ProjectNotFoundOrAccessDenied)?;
    let existing = ExistingFoundation {
        title: non_empty(project.title),
        description: project.description.and_then(non_empty),
        theme: project.theme.and_then(non_empty),
        genre: normalize_genre_text(project.genre.as_deref().unwrap_or_default()),
        narrative_perspective: project.narrative_perspective.and_then(non_empty),
    };

    ensure_not_cancelled(cancellation_token)?;
    let ai_config = SettingsService::build_ai_config(
        db,
        request.user_id,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| FoundationGenerationError::AiConfig)?;
    let provider = ai_config.provider.clone();
    let model = ai_config.model.clone();
    let configured_system_prompt = ai_config.system_prompt.clone();

    emit_progress(&mut on_progress, "准备AI提示词...", 15, "processing").await?;
    let template = PromptTemplateService::system_template_info("INSPIRATION_QUICK_COMPLETE")
        .ok_or(FoundationGenerationError::TemplateMissing)?;
    let mut params = HashMap::new();
    params.insert("existing".to_string(), build_existing_text(&existing));
    let mut prompt = PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(|_| FoundationGenerationError::PromptFormat)?;
    prompt.push_str(
        "\n\n请在不偏离现有信息的前提下补全缺失字段，只返回JSON。\n\
         必须返回 title、description、theme、genre、narrative_perspective；\
         genre 必须是非空字符串数组。不要输出流程说明、提示词或自我评价。",
    );
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);
    let ai_service = AIService::new(ai_config);

    let mut last_failure = FoundationGenerationError::EmptyResponse;
    for attempt in 1..=MAX_FOUNDATION_GENERATION_ATTEMPTS {
        ensure_not_cancelled(cancellation_token)?;
        if attempt > 1 {
            emit_progress(
                &mut on_progress,
                &format!(
                    "⚠ 重试中... ({}/{})",
                    attempt - 1,
                    MAX_FOUNDATION_GENERATION_ATTEMPTS
                ),
                20,
                "processing",
            )
            .await?;
        }
        emit_progress(&mut on_progress, "生成基础设定中...", 20, "processing").await?;

        let mut accumulated_text = String::new();
        let mut chunk_count = 0u64;
        let mut provider_failed = false;
        let mut stream =
            ai_service.generate_text_stream(prompt.clone(), configured_system_prompt.clone(), None);

        while let Some(chunk_result) = stream.next().await {
            ensure_not_cancelled(cancellation_token)?;
            match chunk_result {
                Ok(chunk) => {
                    if let Some(reasoning) =
                        chunk.reasoning_content.filter(|value| !value.is_empty())
                    {
                        on_reasoning(reasoning)
                            .await
                            .map_err(|_| FoundationGenerationError::Observer)?;
                    }
                    if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                        accumulated_text.push_str(&content);
                        on_content(content)
                            .await
                            .map_err(|_| FoundationGenerationError::Observer)?;
                        chunk_count += 1;
                        if chunk_count % 10 == 0 {
                            let char_bonus = (accumulated_text.chars().count() as f64
                                / ESTIMATED_FOUNDATION_OUTPUT_CHARS as f64
                                * 60.0) as u32;
                            emit_progress(
                                &mut on_progress,
                                &format!(
                                    "生成基础设定中... ({}字符)",
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
                FoundationGenerationError::Provider
            } else {
                FoundationGenerationError::EmptyResponse
            };
            continue;
        }

        ensure_not_cancelled(cancellation_token)?;
        emit_progress(&mut on_progress, "解析基础设定数据...", 85, "processing").await?;
        match parse_generated_foundation(&accumulated_text, &existing, &provider, &model, attempt) {
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
) -> Result<(), FoundationGenerationError>
where
    P: FnMut(FoundationGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
{
    on_progress(FoundationGenerationProgress {
        message: message.to_string(),
        progress,
        status,
    })
    .await
    .map_err(|_| FoundationGenerationError::Observer)
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), FoundationGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(FoundationGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn build_existing_text(existing: &ExistingFoundation) -> String {
    let mut parts = Vec::new();
    if let Some(title) = existing.title.as_deref() {
        parts.push(format!("- 书名：{title}"));
    }
    if let Some(description) = existing.description.as_deref() {
        parts.push(format!("- 简介：{description}"));
    }
    if let Some(theme) = existing.theme.as_deref() {
        parts.push(format!("- 主题：{theme}"));
    }
    if !existing.genre.is_empty() {
        parts.push(format!("- 类型：{}", existing.genre.join(", ")));
    }
    if let Some(perspective) = existing.narrative_perspective.as_deref() {
        parts.push(format!("- 叙事视角：{perspective}"));
    }
    if parts.is_empty() {
        "暂无信息".to_string()
    } else {
        parts.join("\n")
    }
}

fn parse_generated_foundation(
    raw_content: &str,
    existing: &ExistingFoundation,
    provider: &str,
    model: &str,
    attempts: u32,
) -> Result<GeneratedFoundation, FoundationGenerationError> {
    let cleaned = clean_json_response(raw_content);
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| FoundationGenerationError::InvalidResponse)?;
    if !data.is_object() {
        return Err(FoundationGenerationError::InvalidResponse);
    }

    let generated_genre = normalize_genre_value(data.get("genre"));
    let result = GeneratedFoundation {
        title: existing
            .title
            .clone()
            .or_else(|| optional_non_empty_string(&data, "title"))
            .unwrap_or_default(),
        description: existing
            .description
            .clone()
            .or_else(|| optional_non_empty_string(&data, "description"))
            .unwrap_or_default(),
        theme: existing
            .theme
            .clone()
            .or_else(|| optional_non_empty_string(&data, "theme"))
            .unwrap_or_default(),
        genre: if existing.genre.is_empty() {
            generated_genre
        } else {
            existing.genre.clone()
        },
        narrative_perspective: existing
            .narrative_perspective
            .clone()
            .or_else(|| optional_non_empty_string(&data, "narrative_perspective"))
            .unwrap_or_else(|| DEFAULT_NARRATIVE_PERSPECTIVE.to_string()),
        provider: provider.to_string(),
        model: model.to_string(),
        attempts,
        content_digest: format!("{:x}", md5::compute(cleaned.as_bytes())),
    };

    if result.is_complete() {
        Ok(result)
    } else {
        Err(FoundationGenerationError::IncompleteResponse)
    }
}

fn optional_non_empty_string(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .and_then(|value| non_empty(value.to_string()))
}

fn normalize_genre_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .flat_map(normalize_genre_text)
            .collect(),
        Some(Value::String(value)) => normalize_genre_text(value),
        _ => Vec::new(),
    }
}

fn normalize_genre_text(value: &str) -> Vec<String> {
    value
        .split(|character: char| matches!(character, '，' | ',' | '、' | '/' | '|' | '｜'))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .fold(Vec::new(), |mut items, item| {
            if !items.contains(&item) {
                items.push(item);
            }
            items
        })
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.len() == value.len() {
        Some(value)
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_existing_text, parse_generated_foundation, ExistingFoundation,
        FoundationGenerationError,
    };

    fn empty_existing() -> ExistingFoundation {
        ExistingFoundation {
            title: None,
            description: None,
            theme: None,
            genre: Vec::new(),
            narrative_perspective: None,
        }
    }

    #[test]
    fn parses_fenced_foundation_json_into_typed_result() {
        let result = parse_generated_foundation(
            r#"```json
            {
              "title":"雾钟封港",
              "description":"旧港封锁后，少女必须在天亮前查清父亲失踪的真相。",
              "theme":"真相与守护的代价",
              "genre":["悬疑","都市/情报博弈"],
              "narrative_perspective":"第三人称"
            }
            ```"#,
            &empty_existing(),
            "openai",
            "model-1",
            2,
        )
        .expect("foundation should parse");

        assert!(result.is_complete());
        assert_eq!(result.title, "雾钟封港");
        assert_eq!(result.genre, vec!["悬疑", "都市", "情报博弈"]);
        assert_eq!(result.attempts, 2);
        assert_eq!(result.provider, "openai");
        assert!(!result.content_digest.is_empty());
    }

    #[test]
    fn preserves_non_empty_manual_foundation_fields() {
        let existing = ExistingFoundation {
            title: Some("人工书名".to_string()),
            description: Some("人工简介".to_string()),
            theme: Some("人工主题".to_string()),
            genre: vec!["人工类型".to_string()],
            narrative_perspective: Some("第一人称".to_string()),
        };
        let result = parse_generated_foundation(
            r#"{
              "title":"模型书名",
              "description":"模型简介",
              "theme":"模型主题",
              "genre":["模型类型"],
              "narrative_perspective":"第三人称"
            }"#,
            &existing,
            "openai",
            "model-1",
            1,
        )
        .expect("foundation should parse");

        assert_eq!(result.title, "人工书名");
        assert_eq!(result.description, "人工简介");
        assert_eq!(result.theme, "人工主题");
        assert_eq!(result.genre, vec!["人工类型"]);
        assert_eq!(result.narrative_perspective, "第一人称");
    }

    #[test]
    fn rejects_incomplete_foundation_response() {
        let error = parse_generated_foundation(
            r#"{"title":"只有书名"}"#,
            &empty_existing(),
            "openai",
            "model-1",
            1,
        )
        .expect_err("incomplete foundation must fail");

        assert_eq!(error, FoundationGenerationError::IncompleteResponse);
    }

    #[test]
    fn rejects_non_json_response() {
        let error =
            parse_generated_foundation("not-json", &empty_existing(), "openai", "model-1", 1)
                .expect_err("invalid foundation must fail");

        assert_eq!(error, FoundationGenerationError::InvalidResponse);
    }

    #[test]
    fn existing_text_does_not_add_empty_fields() {
        let existing = ExistingFoundation {
            title: Some("人工书名".to_string()),
            description: None,
            theme: None,
            genre: vec!["玄幻".to_string(), "成长".to_string()],
            narrative_perspective: None,
        };

        assert_eq!(
            build_existing_text(&existing),
            "- 书名：人工书名\n- 类型：玄幻, 成长"
        );
    }
}
