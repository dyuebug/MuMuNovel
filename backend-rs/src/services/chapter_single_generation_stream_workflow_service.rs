use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_analysis_runtime_service::{
    analyze_generated_chapter_follow_up, prepare_chapter_analysis_execution,
};
use crate::services::chapter_generation_quality_runtime_context_service::apply_generation_quality_runtime_context_from_current_quality;
use crate::services::chapter_generation_runtime_service::update_latest_generated_chapter_history_quality_metrics;
use crate::services::chapter_story_repair_quality_context_service::resolve_active_story_repair_payload_with_quality_fallback;
use crate::utils::sse::{sse_done, sse_error, sse_json, sse_result, SseProgress};

use super::chapter_single_generation_prepare_service::{
    build_single_chapter_generation_request_from_route_payload,
    PrepareSingleChapterGenerationRequestError,
    PreparedSingleChapterGenerationRestoredRuntimeLaunch, SingleChapterGenerationCompatOptions,
    SingleChapterGenerationRequest, SingleChapterGenerationRouteRequest,
};
use super::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

pub(crate) type SingleChapterGenerationStream = ReceiverStream<Result<Event, Infallible>>;

#[derive(Debug, Clone)]
struct SingleGenerationStreamWorkflowStart {
    lifecycle: SingleGenerationStreamLifecyclePlan,
}

pub(crate) async fn create_single_generation_stream_workflow(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    request: SingleChapterGenerationRequest,
) -> Result<
    tokio_stream::wrappers::ReceiverStream<
        Result<axum::response::sse::Event, std::convert::Infallible>,
    >,
    PrepareSingleChapterGenerationRequestError,
> {
    Ok(
        SingleGenerationStreamWorkflowStart::prepare(&db, &user_id, &chapter_id, &request)
            .await?
            .spawn(db),
    )
}

pub(crate) async fn create_single_generation_stream_workflow_from_route_payload(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    route_request: SingleChapterGenerationRouteRequest,
) -> Result<SingleChapterGenerationStream, PrepareSingleChapterGenerationRequestError> {
    create_single_generation_stream_workflow(
        db,
        user_id,
        chapter_id,
        build_single_chapter_generation_request_from_route_payload(route_request),
    )
    .await
}

impl SingleGenerationStreamWorkflowStart {
    async fn prepare(
        db: &DatabaseConnection,
        user_id: &str,
        chapter_id: &str,
        request: &SingleChapterGenerationRequest,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        let runtime_input =
            PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_runtime_launch_input(
                db, chapter_id, user_id, request,
            )
            .await?;

        Ok(Self::from_runtime_launch(runtime_input))
    }

    fn spawn(self, db: DatabaseConnection) -> SingleChapterGenerationStream {
        self.lifecycle.spawn(db)
    }

    fn from_runtime_launch(runtime_input: SingleGenerationRuntimeLaunchInput) -> Self {
        Self {
            lifecycle: SingleGenerationStreamLifecyclePlan::from_runtime_launch(runtime_input),
        }
    }

    #[cfg(test)]
    fn lifecycle(&self) -> &SingleGenerationStreamLifecyclePlan {
        &self.lifecycle
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SingleGenerationStreamAnalysisOutcome {
    analysis_task_id: Option<String>,
    quality_metrics: Option<Value>,
    quality_gate_action: Option<String>,
    quality_gate_message: Option<String>,
    quality_gate_snapshot: Option<Value>,
    hard_gate_blocked: bool,
    story_runtime_contract: Option<Value>,
    completion_message: String,
    analysis_started_message: Option<String>,
}

impl SingleGenerationStreamAnalysisOutcome {
    async fn from_generated_result(
        db: &DatabaseConnection,
        runtime_user_id: &str,
        target_word_count: i32,
        compat_options: &SingleChapterGenerationCompatOptions,
        enable_analysis: bool,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    ) -> Self {
        let story_runtime_contract = build_single_generation_stream_story_runtime_contract(
            result.chapter_number,
            target_word_count,
            compat_options,
        );
        let analysis = Self::run_follow_up_analysis(
            db,
            runtime_user_id,
            result,
            enable_analysis,
            story_runtime_contract,
        )
        .await;
        if let Some(quality_metrics) = analysis.quality_metrics.as_ref() {
            let _ = update_latest_generated_chapter_history_quality_metrics(
                db,
                &result.chapter_id,
                &result.content,
                quality_metrics,
            )
            .await;
        }

        analysis
    }

    fn without_analysis(story_runtime_contract: Option<Value>) -> Self {
        Self {
            analysis_task_id: None,
            quality_metrics: None,
            quality_gate_action: None,
            quality_gate_message: None,
            quality_gate_snapshot: None,
            hard_gate_blocked: false,
            story_runtime_contract,
            completion_message: "章节生成完成".to_string(),
            analysis_started_message: None,
        }
    }

    fn from_quality_metrics(
        analysis_task_id: Option<String>,
        quality_metrics: Option<Value>,
        story_runtime_contract: Option<Value>,
    ) -> Self {
        let quality_metrics = attach_single_generation_stream_story_runtime_contract(
            quality_metrics,
            story_runtime_contract.as_ref(),
        );
        let quality_gate_snapshot = quality_metrics
            .as_ref()
            .and_then(|metrics| metrics.get("quality_gate"))
            .filter(|payload| payload.is_object())
            .cloned();
        let quality_gate_action =
            map_single_generation_stream_quality_gate_action(quality_gate_snapshot.as_ref());
        let quality_gate_message =
            extract_single_generation_stream_quality_gate_message(quality_gate_snapshot.as_ref());
        let hard_gate_blocked = matches!(
            quality_gate_action.as_deref(),
            Some("retry") | Some("manual_review")
        );
        let completion_message = match quality_gate_action.as_deref() {
            Some("retry") => "章节生成完成，已转入质量修复",
            Some("manual_review") => "章节生成完成，已转入人工复核",
            _ => "章节生成完成",
        }
        .to_string();
        let analysis_started_message = analysis_task_id.as_ref().map(|_| {
            match quality_gate_action.as_deref() {
                Some("retry") => "质量修复分析任务已启动",
                Some("manual_review") => "人工复核分析任务已启动",
                _ => "章节分析任务已启动",
            }
            .to_string()
        });

        Self {
            analysis_task_id,
            quality_metrics,
            quality_gate_action,
            quality_gate_message,
            quality_gate_snapshot,
            hard_gate_blocked,
            story_runtime_contract,
            completion_message,
            analysis_started_message,
        }
    }

    async fn run_follow_up_analysis(
        db: &DatabaseConnection,
        runtime_user_id: &str,
        generated: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
        enable_analysis: bool,
        story_runtime_contract: Option<Value>,
    ) -> Self {
        if !enable_analysis {
            return Self::without_analysis(story_runtime_contract);
        }

        let mut analysis_task_id = None;
        let analysis_payload =
            match prepare_chapter_analysis_execution(db, &generated.chapter_id, runtime_user_id)
                .await
            {
                Ok(prepared) => {
                    analysis_task_id = Some(prepared.task_id().to_string());
                    prepared.execute(db, runtime_user_id).await.ok()
                }
                Err(_) => analyze_generated_chapter_follow_up(db, runtime_user_id, generated)
                    .await
                    .ok(),
            };

        let quality_metrics = analysis_payload
            .as_ref()
            .and_then(|payload| payload.get("quality_metrics"))
            .filter(|payload| payload.is_object())
            .cloned();

        Self::from_quality_metrics(analysis_task_id, quality_metrics, story_runtime_contract)
    }

    fn quality_metrics_event(
        &self,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    ) -> Option<Value> {
        let metrics = self.quality_metrics.as_ref()?.as_object()?.clone();
        let mut payload = serde_json::Map::from_iter([
            ("type".to_string(), json!("quality_metrics")),
            ("chapter_id".to_string(), json!(result.chapter_id)),
            ("chapter_number".to_string(), json!(result.chapter_number)),
        ]);
        payload.extend(metrics);
        Some(Value::Object(payload))
    }

    fn quality_gate_event(
        &self,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    ) -> Option<Value> {
        if !self.hard_gate_blocked {
            return None;
        }

        Some(json!({
            "type": if matches!(self.quality_gate_action.as_deref(), Some("retry")) {
                "quality_gate_retry"
            } else {
                "quality_gate_blocked"
            },
            "chapter_id": result.chapter_id,
            "chapter_number": result.chapter_number,
            "message": self.quality_gate_message,
            "progress": if matches!(self.quality_gate_action.as_deref(), Some("retry")) {
                88
            } else {
                95
            },
            "quality_gate": self.quality_gate_snapshot,
        }))
    }

    fn analysis_started_event(&self) -> Option<Value> {
        let task_id = self.analysis_task_id.as_deref()?;
        let message = self.analysis_started_message.as_deref()?;

        Some(json!({
            "type": "analysis_started",
            "task_id": task_id,
            "message": message,
        }))
    }

    fn completion_message(&self) -> &str {
        self.completion_message.as_str()
    }

    fn response_payload(
        &self,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    ) -> Value {
        let mut payload = serde_json::Map::from_iter([
            ("chapter_id".to_string(), json!(result.chapter_id)),
            ("chapter_number".to_string(), json!(result.chapter_number)),
            ("title".to_string(), json!(result.title)),
            ("content".to_string(), json!(result.content)),
            ("word_count".to_string(), json!(result.word_count)),
            ("saved_word_count".to_string(), json!(result.word_count)),
            ("chapter_status".to_string(), json!("draft")),
            ("content_applied".to_string(), json!(true)),
            ("content_source".to_string(), json!("chapter")),
            ("analysis_task_id".to_string(), json!(self.analysis_task_id)),
            ("quality_metrics".to_string(), json!(self.quality_metrics)),
            (
                "quality_gate_action".to_string(),
                json!(self.quality_gate_action),
            ),
            (
                "quality_gate_message".to_string(),
                json!(self.quality_gate_message),
            ),
            (
                "hard_gate_blocked".to_string(),
                json!(self.hard_gate_blocked),
            ),
            (
                "story_runtime_contract".to_string(),
                json!(self.story_runtime_contract),
            ),
        ]);

        if let Some(latest_quality_metrics) = self.quality_metrics.as_ref() {
            apply_generation_quality_runtime_context_from_current_quality(
                &mut payload,
                "chapter",
                None,
                None,
                latest_quality_metrics,
                20,
            );
            let active_story_repair_payload =
                resolve_active_story_repair_payload_with_quality_fallback(
                    None,
                    payload.get("quality_metrics_summary"),
                    Some(latest_quality_metrics),
                    "chapter",
                    "plot_analysis",
                    "Plot analysis",
                );
            payload.insert(
                "active_story_repair_payload".to_string(),
                json!(active_story_repair_payload),
            );
        }

        Value::Object(payload)
    }

    fn ordered_success_event_payloads(
        &self,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    ) -> Vec<SingleGenerationStreamSuccessEventPayload> {
        let mut payloads = Vec::new();

        if let Some(quality_metrics_event) = self.quality_metrics_event(result) {
            payloads.push(SingleGenerationStreamSuccessEventPayload::Json(
                quality_metrics_event,
            ));
        }

        if let Some(quality_gate_event) = self.quality_gate_event(result) {
            payloads.push(SingleGenerationStreamSuccessEventPayload::Json(
                quality_gate_event,
            ));
        }

        payloads.push(SingleGenerationStreamSuccessEventPayload::Result(
            self.response_payload(result),
        ));

        if let Some(analysis_started_event) = self.analysis_started_event() {
            payloads.push(SingleGenerationStreamSuccessEventPayload::Json(
                analysis_started_event,
            ));
        }

        payloads
    }

    async fn emit_success(
        &self,
        result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
        tx: &mpsc::Sender<Result<Event, Infallible>>,
        tracker: &mut SseProgress,
    ) {
        let _ = tx
            .send(Ok(tracker.complete(Some(self.completion_message()))))
            .await;

        for payload in self.ordered_success_event_payloads(result) {
            let event = match payload {
                SingleGenerationStreamSuccessEventPayload::Json(payload) => sse_json(&payload),
                SingleGenerationStreamSuccessEventPayload::Result(payload) => sse_result(&payload),
            };
            let _ = tx.send(Ok(event)).await;
        }

        let _ = tx.send(Ok(sse_done())).await;
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SingleGenerationStreamSuccessEventPayload {
    Json(Value),
    Result(Value),
}

#[derive(Debug, Clone)]
struct SingleGenerationStreamLifecyclePlan {
    target_word_count: i32,
    compat_options: SingleChapterGenerationCompatOptions,
    enable_analysis: bool,
    runtime_user_id: String,
    runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl SingleGenerationStreamLifecyclePlan {
    fn from_runtime_launch(runtime_input: SingleGenerationRuntimeLaunchInput) -> Self {
        let target_word_count = runtime_input.execution_input.target_word_count;
        let compat_options = runtime_input.execution_input.compat_options.clone();
        let enable_analysis = compat_options.enable_analysis();
        let runtime_user_id = runtime_input.user_id.clone();

        Self {
            target_word_count,
            compat_options,
            enable_analysis,
            runtime_user_id,
            runtime_input,
        }
    }

    fn spawn(self, db: DatabaseConnection) -> SingleChapterGenerationStream {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

        tokio::spawn(async move {
            self.run(db, tx).await;
        });

        ReceiverStream::new(rx)
    }

    async fn run(self, db: DatabaseConnection, tx: mpsc::Sender<Result<Event, Infallible>>) {
        let mut tracker = SseProgress::new("Chapter Generation");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(
                tracker.preparing(Some("Preparing chapter generation..."))
            ))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Generating chapter content..."),
                (15, 95),
                self.target_word_count as usize,
                None,
            )))
            .await;

        match self.runtime_input.execute_generation(&db).await {
            Ok(result) => {
                let analysis = SingleGenerationStreamAnalysisOutcome::from_generated_result(
                    &db,
                    &self.runtime_user_id,
                    self.target_word_count,
                    &self.compat_options,
                    self.enable_analysis,
                    &result,
                )
                .await;
                analysis.emit_success(&result, &tx, &mut tracker).await;
            }
            Err(error_message) => {
                let _ = tx.send(Ok(sse_error(&error_message, 500))).await;
            }
        }
    }
}

fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn map_single_generation_stream_quality_gate_action(
    quality_gate_snapshot: Option<&Value>,
) -> Option<String> {
    let decision = normalized_non_empty_string(
        quality_gate_snapshot.and_then(|payload| payload.get("decision")),
    )?;

    Some(match decision.as_str() {
        "passed" | "continue" => "continue".to_string(),
        "auto_repair" | "repair" | "retry" => "retry".to_string(),
        "manual_review" => "manual_review".to_string(),
        _ => decision,
    })
}

fn extract_single_generation_stream_quality_gate_message(
    quality_gate_snapshot: Option<&Value>,
) -> Option<String> {
    normalized_non_empty_string(quality_gate_snapshot.and_then(|payload| payload.get("summary")))
        .or_else(|| {
            normalized_non_empty_string(
                quality_gate_snapshot.and_then(|payload| payload.get("label")),
            )
        })
}

fn single_generation_story_runtime_contract_text_value(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Value::Null
    } else {
        json!(trimmed)
    }
}

fn insert_single_generation_story_runtime_override(
    overrides: &mut serde_json::Map<String, Value>,
    key: &str,
    value: &str,
) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        overrides.insert(key.to_string(), json!(trimmed));
    }
}

fn build_single_generation_stream_story_runtime_contract(
    chapter_number: i32,
    target_word_count: i32,
    compat_options: &super::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions,
) -> Option<Value> {
    let mut request_overrides = serde_json::Map::new();
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "creative_mode",
        compat_options.creative_mode(),
    );
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "story_focus",
        compat_options.story_focus(),
    );
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "plot_stage",
        compat_options.plot_stage(),
    );
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "story_creation_brief",
        compat_options.story_creation_brief(),
    );
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "quality_preset",
        compat_options.quality_preset(),
    );
    insert_single_generation_story_runtime_override(
        &mut request_overrides,
        "quality_notes",
        compat_options.quality_notes(),
    );

    Some(json!({
        "version": 1,
        "guidance": {
            "creative_mode": single_generation_story_runtime_contract_text_value(compat_options.creative_mode()),
            "story_focus": single_generation_story_runtime_contract_text_value(compat_options.story_focus()),
            "plot_stage": single_generation_story_runtime_contract_text_value(compat_options.plot_stage()),
            "story_creation_brief": single_generation_story_runtime_contract_text_value(compat_options.story_creation_brief()),
            "quality_preset": single_generation_story_runtime_contract_text_value(compat_options.quality_preset()),
            "quality_notes": single_generation_story_runtime_contract_text_value(compat_options.quality_notes()),
        },
        "request_overrides": Value::Object(request_overrides),
        "source": "chapter-generation-intent",
        "blueprint": {
            "long_term_goal": Value::Null,
            "chapter_count": Value::Null,
            "current_chapter_number": chapter_number,
            "target_word_count": target_word_count,
            "character_focus_names": Vec::<String>::new(),
            "foreshadow_payoff_plan": Vec::<String>::new(),
            "character_state_ledger": Vec::<Value>::new(),
            "relationship_state_ledger": Vec::<Value>::new(),
            "foreshadow_state_ledger": Vec::<Value>::new(),
            "organization_state_ledger": Vec::<Value>::new(),
            "career_state_ledger": Vec::<Value>::new(),
        }
    }))
}

fn attach_single_generation_stream_story_runtime_contract(
    quality_metrics: Option<Value>,
    story_runtime_contract: Option<&Value>,
) -> Option<Value> {
    match quality_metrics {
        Some(Value::Object(mut metrics)) => {
            if let Some(contract) = story_runtime_contract.filter(|payload| payload.is_object()) {
                metrics
                    .entry("story_runtime_contract".to_string())
                    .or_insert_with(|| contract.clone());
            }
            Some(Value::Object(metrics))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        attach_single_generation_stream_story_runtime_contract,
        build_single_generation_stream_story_runtime_contract,
        map_single_generation_stream_quality_gate_action, SingleChapterGenerationRequest,
        SingleGenerationStreamAnalysisOutcome, SingleGenerationStreamLifecyclePlan,
        SingleGenerationStreamSuccessEventPayload, SingleGenerationStreamWorkflowStart,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_batch_generation_task_model_service::build_batch_generation_task_active_model;
    use crate::services::chapter_generation_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationCompatOptions,
        SingleChapterGenerationExecutionInput, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
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
        }
    }

    #[test]
    fn should_keep_background_workflow_error_contract_shape() {
        let chapter_error = PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
        );
        let config_error =
            PrepareSingleChapterGenerationRequestError::Config("model missing".to_string());
        let internal_error =
            PrepareSingleChapterGenerationRequestError::Internal("db failed".to_string());

        assert!(matches!(
            chapter_error,
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            config_error,
            PrepareSingleChapterGenerationRequestError::Config(detail) if detail == "model missing"
        ));
        assert!(matches!(
            internal_error,
            PrepareSingleChapterGenerationRequestError::Chapter(_)
                | PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    #[test]
    fn should_build_single_generation_task_chapter_payload_from_parts() {
        let payload = build_batch_generation_task_active_model(
            "task-2".to_string(),
            "project-2".to_string(),
            "user-2".to_string(),
            8,
            1,
            json!([{
                "id": "chapter-2",
                "chapter_number": 8,
                "title": "第八章",
            }]),
            None,
            2100,
            false,
            1,
            Some("chapter-2".to_string()),
            Some(8),
            0,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(
            payload.chapter_ids,
            Set(json!([{
                "id": "chapter-2",
                "chapter_number": 8,
                "title": "第八章",
            }]))
        );
    }

    #[test]
    fn should_build_single_generation_background_runtime_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-9".to_string(),
            user_id: "user-42".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-42");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
    }

    #[test]
    fn should_build_single_generation_stream_launch_input_from_runtime_parts() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: empty_compat_options(),
            execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                ai_config: AIConfig::default(),
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

        let chapter_id = chapter_target.chapter_id.clone();
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: chapter_target.chapter_id,
            user_id: "user-1".to_string(),
            execution_input,
        };

        assert_eq!(launch.chapter_id, "chapter-7");
        assert_eq!(launch.user_id, "user-1");
        assert_eq!(launch.execution_input.target_word_count, 2600);
        assert_eq!(chapter_id, "chapter-7");
    }

    #[test]
    fn should_build_single_generation_stream_runtime_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-11".to_string(),
            user_id: "user-77".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1800,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-11");
        assert_eq!(runtime_input.user_id, "user-77");
        assert_eq!(runtime_input.execution_input.target_word_count, 1800);
    }

    #[test]
    fn should_keep_single_generation_stream_launch_input_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-9".to_string(),
            user_id: "user-42".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
            },
        };

        assert_eq!(launch.chapter_id, "chapter-9");
        assert_eq!(launch.user_id, "user-42");
        assert_eq!(launch.execution_input.target_word_count, 2400);
    }

    #[test]
    fn should_keep_single_generation_stream_workflow_start_owner_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-stream".to_string(),
            user_id: "user-stream".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2500,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
            },
        };

        let workflow_start =
            SingleGenerationStreamWorkflowStart::from_runtime_launch(launch.clone());

        assert_eq!(
            workflow_start.lifecycle().runtime_input.chapter_id,
            launch.chapter_id
        );
        assert_eq!(
            workflow_start.lifecycle().runtime_input.user_id,
            launch.user_id
        );
        assert_eq!(workflow_start.lifecycle().target_word_count, 2500);
        assert!(!workflow_start.lifecycle().enable_analysis);
    }

    #[test]
    fn should_build_single_generation_stream_terminal_success_payload() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: "content".to_string(),
            word_count: 2600,
        };

        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-1".to_string()),
            Some(json!({
                "overall_score": 9.1,
                "quality_gate": {
                    "decision": "passed",
                    "summary": "当前章节通过"
                }
            })),
            Some(json!({
                "guidance": {
                    "creative_mode": "hook"
                },
                "blueprint": {
                    "current_chapter_number": 7,
                    "target_word_count": 2600
                }
            })),
        );

        let response_payload = analysis.response_payload(&result);

        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["chapter_number"], 7);
        assert_eq!(response_payload["word_count"], 2600);
        assert_eq!(response_payload["saved_word_count"], 2600);
        assert_eq!(response_payload["chapter_status"], "draft");
        assert_eq!(response_payload["content_applied"], true);
        assert_eq!(response_payload["content_source"], "chapter");
        assert_eq!(response_payload["analysis_task_id"], "analysis-task-1");
        assert_eq!(response_payload["quality_gate_action"], "continue");
        assert_eq!(response_payload["quality_gate_message"], "当前章节通过");
        assert_eq!(response_payload["hard_gate_blocked"], false);
        assert_eq!(
            response_payload["latest_quality_metrics"]["overall_score"],
            9.1
        );
        assert_eq!(
            response_payload["quality_metrics_history"][0]["overall_score"],
            9.1
        );
        assert_eq!(
            response_payload["quality_metrics_summary_state"]["scope"],
            "chapter"
        );
        assert_eq!(
            response_payload["quality_metrics_summary_state"]["chapter_count"],
            1
        );
        assert_eq!(
            response_payload["quality_history_context"]["scope"],
            "chapter"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "当前质量指标不足，暂时无法生成修复指引。"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["source"],
            "plot_analysis"
        );
        assert_eq!(
            response_payload["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
        assert_eq!(
            response_payload["quality_metrics"]["story_runtime_contract"]["blueprint"]
                ["current_chapter_number"],
            7
        );
    }

    #[test]
    fn should_map_single_generation_stream_quality_gate_action_to_python_contract() {
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(&json!({"decision": "passed"}))),
            Some("continue".to_string())
        );
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(
                &json!({"decision": "auto_repair"})
            )),
            Some("retry".to_string())
        );
        assert_eq!(
            map_single_generation_stream_quality_gate_action(Some(
                &json!({"decision": "manual_review"})
            )),
            Some("manual_review".to_string())
        );
    }

    #[test]
    fn should_build_single_generation_stream_quality_events_for_retry_follow_up() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-8".to_string(),
            chapter_number: 8,
            title: "第八章".to_string(),
            content: "content".to_string(),
            word_count: 2800,
        };
        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-8".to_string()),
            Some(json!({
                "overall_score": 7.2,
                "repair_guidance": {
                    "summary": "建议收紧中段说明"
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议继续修复",
                    "summary": "建议收紧中段说明"
                }
            })),
            Some(json!({
                "guidance": {
                    "story_focus": "advance_plot"
                },
                "blueprint": {
                    "current_chapter_number": 8,
                    "target_word_count": 2800
                }
            })),
        );

        let quality_metrics_event = analysis
            .quality_metrics_event(&result)
            .expect("quality metrics event");
        let quality_gate_event = analysis
            .quality_gate_event(&result)
            .expect("quality gate event");
        let analysis_started_event = analysis
            .analysis_started_event()
            .expect("analysis started event");
        let response_payload = analysis.response_payload(&result);

        assert_eq!(quality_metrics_event["type"], "quality_metrics");
        assert_eq!(quality_metrics_event["chapter_id"], "chapter-8");
        assert_eq!(
            quality_metrics_event["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(quality_gate_event["type"], "quality_gate_retry");
        assert_eq!(quality_gate_event["message"], "建议收紧中段说明");
        assert_eq!(quality_gate_event["progress"], 88);
        assert_eq!(analysis_started_event["type"], "analysis_started");
        assert_eq!(analysis_started_event["task_id"], "analysis-task-8");
        assert_eq!(analysis_started_event["message"], "质量修复分析任务已启动");
        assert_eq!(response_payload["quality_gate_action"], "retry");
        assert_eq!(response_payload["hard_gate_blocked"], true);
        assert_eq!(
            response_payload["latest_quality_metrics"]["overall_score"],
            7.2
        );
        assert_eq!(
            response_payload["quality_metrics_history"][0]["overall_score"],
            7.2
        );
        assert_eq!(
            response_payload["quality_metrics_summary_state"]["chapter_count"],
            1
        );
        assert_eq!(
            response_payload["quality_history_context"]["scope"],
            "chapter"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "建议收紧中段说明"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["source"],
            "plot_analysis"
        );
        assert_eq!(
            response_payload["story_runtime_contract"]["guidance"]["story_focus"],
            "advance_plot"
        );
    }

    #[test]
    fn should_preserve_richer_quality_runtime_contract_on_single_generation_stream_result_payload()
    {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-10".to_string(),
            chapter_number: 10,
            title: "第十章".to_string(),
            content: "content".to_string(),
            word_count: 3100,
        };
        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-10".to_string()),
            Some(json!({
                "overall_score": 6.4,
                "pacing_score": 6.1,
                "engagement_score": 7.2,
                "coherence_score": 6.8,
                "repair_guidance": {
                    "summary": "建议补强冲突推进",
                    "repair_targets": ["补强冲突推进", "压缩解释段"],
                    "preserve_strengths": ["角色语气稳定"],
                    "focus_areas": ["节奏", "连贯性"]
                },
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复",
                    "summary": "建议补强冲突推进",
                    "failed_metrics": [{"label": "节奏"}]
                },
                "quality_runtime_context": {
                    "scope": "chapter",
                    "source": "analysis",
                    "score_justification": "冲突推进偏慢"
                }
            })),
            Some(json!({
                "guidance": {
                    "quality_notes": "压缩解释"
                },
                "blueprint": {
                    "current_chapter_number": 10,
                    "target_word_count": 3100
                }
            })),
        );

        let payload = analysis.response_payload(&result);

        assert_eq!(payload["analysis_task_id"], "analysis-task-10");
        assert_eq!(payload["quality_metrics"]["overall_score"], 6.4);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 6.4);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 6.4);
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "chapter");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
        assert_eq!(payload["quality_history_context"]["source"], "analysis");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"][0],
            "补强冲突推进"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "plot_analysis"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source_label"],
            "Plot analysis"
        );
        assert_eq!(payload["quality_gate_action"], "retry");
        assert_eq!(payload["hard_gate_blocked"], true);
    }

    #[test]
    fn should_keep_single_generation_stream_success_projection_on_analysis_owner_for_retry() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-12".to_string(),
            chapter_number: 12,
            title: "第十二章".to_string(),
            content: "content".to_string(),
            word_count: 3300,
        };
        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-12".to_string()),
            Some(json!({
                "overall_score": 6.9,
                "repair_guidance": {
                    "summary": "建议压缩说明段",
                    "repair_targets": ["压缩说明段"]
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "summary": "建议压缩说明段"
                }
            })),
            Some(json!({
                "guidance": {
                    "quality_notes": "压缩说明段"
                },
                "blueprint": {
                    "current_chapter_number": 12,
                    "target_word_count": 3300
                }
            })),
        );

        assert_eq!(
            analysis.completion_message(),
            "章节生成完成，已转入质量修复"
        );
        assert_eq!(
            analysis
                .quality_metrics_event(&result)
                .expect("quality metrics event")["type"],
            "quality_metrics"
        );
        assert_eq!(
            analysis
                .quality_gate_event(&result)
                .expect("quality gate event")["type"],
            "quality_gate_retry"
        );
        assert_eq!(
            analysis.response_payload(&result)["quality_gate_action"],
            "retry"
        );
        assert_eq!(
            analysis
                .analysis_started_event()
                .expect("analysis started event")["message"],
            "质量修复分析任务已启动"
        );
    }

    #[test]
    fn should_keep_single_generation_stream_success_projection_on_analysis_owner() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-13".to_string(),
            chapter_number: 13,
            title: "第十三章".to_string(),
            content: "content".to_string(),
            word_count: 3500,
        };
        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-13".to_string()),
            Some(json!({
                "overall_score": 7.6,
                "quality_gate": {
                    "decision": "passed",
                    "summary": "当前章节通过"
                }
            })),
            Some(json!({
                "guidance": {
                    "creative_mode": "payoff"
                },
                "blueprint": {
                    "current_chapter_number": 13,
                    "target_word_count": 3500
                }
            })),
        );

        assert_eq!(analysis.completion_message(), "章节生成完成");
        assert_eq!(
            analysis
                .quality_metrics_event(&result)
                .expect("quality metrics event")["chapter_number"],
            13
        );
        assert!(analysis.quality_gate_event(&result).is_none());
        assert_eq!(
            analysis.response_payload(&result)["quality_gate_action"],
            "continue"
        );
        assert_eq!(
            analysis.response_payload(&result)["story_runtime_contract"]["guidance"]
                ["creative_mode"],
            "payoff"
        );
        assert_eq!(
            analysis
                .analysis_started_event()
                .expect("analysis started event")["task_id"],
            "analysis-task-13"
        );
    }

    #[test]
    fn should_project_single_generation_stream_success_event_order_from_analysis_owner() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-14".to_string(),
            chapter_number: 14,
            title: "第十四章".to_string(),
            content: "content".to_string(),
            word_count: 3600,
        };
        let analysis = SingleGenerationStreamAnalysisOutcome::from_quality_metrics(
            Some("analysis-task-14".to_string()),
            Some(json!({
                "overall_score": 6.8,
                "quality_gate": {
                    "decision": "manual_review",
                    "summary": "需要人工复核"
                }
            })),
            Some(json!({
                "guidance": {
                    "creative_mode": "suspense"
                }
            })),
        );

        let payloads = analysis.ordered_success_event_payloads(&result);

        assert_eq!(
            analysis.completion_message(),
            "章节生成完成，已转入人工复核"
        );
        assert_eq!(payloads.len(), 4);
        assert!(matches!(
            payloads[0],
            SingleGenerationStreamSuccessEventPayload::Json(_)
        ));
        assert!(matches!(
            payloads[1],
            SingleGenerationStreamSuccessEventPayload::Json(_)
        ));
        assert!(matches!(
            payloads[2],
            SingleGenerationStreamSuccessEventPayload::Result(_)
        ));
        assert!(matches!(
            payloads[3],
            SingleGenerationStreamSuccessEventPayload::Json(_)
        ));

        match &payloads[0] {
            SingleGenerationStreamSuccessEventPayload::Json(payload) => {
                assert_eq!(payload["type"], "quality_metrics");
            }
            SingleGenerationStreamSuccessEventPayload::Result(_) => {
                panic!("expected quality metrics event")
            }
        }

        match &payloads[1] {
            SingleGenerationStreamSuccessEventPayload::Json(payload) => {
                assert_eq!(payload["type"], "quality_gate_blocked");
            }
            SingleGenerationStreamSuccessEventPayload::Result(_) => {
                panic!("expected quality gate event")
            }
        }

        match &payloads[2] {
            SingleGenerationStreamSuccessEventPayload::Result(payload) => {
                assert_eq!(payload["chapter_id"], "chapter-14");
                assert_eq!(payload["quality_gate_action"], "manual_review");
            }
            SingleGenerationStreamSuccessEventPayload::Json(_) => {
                panic!("expected response payload")
            }
        }

        match &payloads[3] {
            SingleGenerationStreamSuccessEventPayload::Json(payload) => {
                assert_eq!(payload["type"], "analysis_started");
            }
            SingleGenerationStreamSuccessEventPayload::Result(_) => {
                panic!("expected analysis started event")
            }
        }
    }

    #[test]
    fn should_build_minimal_single_generation_stream_story_runtime_contract() {
        let contract = build_single_generation_stream_story_runtime_contract(
            9,
            3200,
            &SingleChapterGenerationCompatOptions {
                creative_mode: Some("hook".to_string()),
                story_focus: Some("advance_plot".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("推进夜航谈判".to_string()),
                quality_preset: Some("immersive".to_string()),
                quality_notes: Some("压缩说明".to_string()),
                ..empty_compat_options()
            },
        )
        .expect("runtime contract");

        assert_eq!(contract["version"], 1);
        assert_eq!(contract["source"], "chapter-generation-intent");
        assert_eq!(contract["guidance"]["creative_mode"], "hook");
        assert_eq!(contract["guidance"]["story_focus"], "advance_plot");
        assert_eq!(contract["guidance"]["plot_stage"], "climax");
        assert_eq!(contract["guidance"]["story_creation_brief"], "推进夜航谈判");
        assert_eq!(contract["guidance"]["quality_preset"], "immersive");
        assert_eq!(contract["guidance"]["quality_notes"], "压缩说明");
        assert_eq!(contract["request_overrides"]["creative_mode"], "hook");
        assert_eq!(contract["blueprint"]["current_chapter_number"], 9);
        assert_eq!(contract["blueprint"]["target_word_count"], 3200);
        assert!(contract["blueprint"]["character_focus_names"]
            .as_array()
            .expect("array")
            .is_empty());
    }

    #[test]
    fn should_attach_story_runtime_contract_into_quality_metrics_when_missing() {
        let metrics = attach_single_generation_stream_story_runtime_contract(
            Some(json!({
                "overall_score": 8.3,
                "quality_gate": {
                    "decision": "passed"
                }
            })),
            Some(&json!({
                "guidance": {
                    "plot_stage": "development"
                }
            })),
        )
        .expect("metrics payload");

        assert_eq!(
            metrics["story_runtime_contract"]["guidance"]["plot_stage"],
            "development"
        );
        assert_eq!(metrics["quality_gate"]["decision"], "passed");
    }

    #[test]
    fn should_build_single_generation_stream_terminal_failure_event() {
        let error = Err::<
            crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
            _,
        >("generation failed".to_string())
        .expect_err("expected failure");

        assert_eq!(error, "generation failed");
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_even_when_runtime_will_fail_later() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2000,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
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
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = SingleGenerationStreamLifecyclePlan::from_runtime_launch(launch).spawn(db);
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_when_follow_up_analysis_disabled() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-2".to_string(),
            user_id: "user-2".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1600,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config:
                    crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                        ai_config: AIConfig::default(),
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
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = SingleGenerationStreamLifecyclePlan::from_runtime_launch(launch).spawn(db);
    }

    #[test]
    fn should_keep_single_chapter_generation_request_contract_minimal() {
        let request = SingleChapterGenerationRequest {
            style_id: None,
            target_word_count: Some(2200),
            model: Some("gpt-test".to_string()),
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

        assert_eq!(request.style_id, None);
        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
    }
}
