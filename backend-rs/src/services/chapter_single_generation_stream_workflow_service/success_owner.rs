use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::models::chapter;
use crate::services::chapter_analysis_runtime_service::{
    analyze_generated_chapter_follow_up, prepare_chapter_analysis_execution,
};
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_history_persistence_service::persist_single_generation_candidate_draft_attempt;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::apply_generation_quality_runtime_context_from_current_quality;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::resolve_active_story_repair_payload_with_quality_fallback;
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_single_generation_result_lifecycle_service::{
    build_single_generation_followup_draft_result,
    single_generation_candidate_draft_lifecycle_view,
    update_latest_generated_chapter_history_quality_metrics,
};
use crate::utils::sse::{sse_done, sse_json, sse_result, SseProgress};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationStreamSuccessArtifacts {
    analysis_task_id: Option<String>,
    quality_metrics: Option<Value>,
    quality_gate_action: Option<String>,
    quality_gate_message: Option<String>,
    quality_gate_snapshot: Option<Value>,
    hard_gate_blocked: bool,
    story_runtime_contract: Option<Value>,
    followup_plan: SingleGenerationStreamAnalysisFollowupPlan,
    candidate_draft: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationStreamAnalysisFollowupPlan {
    completion_message: String,
    analysis_started_message: Option<String>,
}

impl SingleGenerationStreamAnalysisFollowupPlan {
    fn from_quality_gate(
        analysis_task_id: Option<&String>,
        quality_gate_action: Option<&str>,
    ) -> Self {
        let completion_message = match quality_gate_action {
            Some("retry") => "章节生成完成，已转入质量修复",
            Some("manual_review") => "章节生成完成，已转入人工复核",
            _ => "章节生成完成",
        }
        .to_string();
        let analysis_started_message = analysis_task_id.map(|_| {
            match quality_gate_action {
                Some("retry") => "质量修复分析任务已启动",
                Some("manual_review") => "人工复核分析任务已启动",
                _ => "章节分析任务已启动",
            }
            .to_string()
        });

        Self {
            completion_message,
            analysis_started_message,
        }
    }

    fn without_analysis() -> Self {
        Self {
            completion_message: "章节生成完成".to_string(),
            analysis_started_message: None,
        }
    }

    pub(crate) fn completion_message(&self) -> &str {
        self.completion_message.as_str()
    }

    pub(crate) fn analysis_started_message(&self) -> Option<&str> {
        self.analysis_started_message.as_deref()
    }
}

impl SingleGenerationStreamSuccessArtifacts {
    pub(crate) async fn from_generated_result(
        db: &DatabaseConnection,
        runtime_user_id: &str,
        target_word_count: i32,
        compat_options: &SingleChapterGenerationCompatOptions,
        enable_analysis: bool,
        result: &GeneratedChapterResult,
    ) -> Self {
        let story_runtime_contract = build_single_generation_stream_story_runtime_contract(
            result.chapter_number,
            target_word_count,
            compat_options,
        );
        let mut analysis = Self::from_quality_metrics(
            None,
            result.quality_metrics.clone(),
            story_runtime_contract.clone(),
        );
        let follow_up_analysis = Self::run_follow_up_analysis(
            db,
            runtime_user_id,
            result,
            enable_analysis,
            story_runtime_contract.clone(),
        )
        .await;
        if follow_up_analysis.quality_metrics.is_some() {
            analysis = follow_up_analysis;
        } else if follow_up_analysis.analysis_task_id.is_some() {
            analysis.analysis_task_id = follow_up_analysis.analysis_task_id;
            analysis.followup_plan = SingleGenerationStreamAnalysisFollowupPlan::from_quality_gate(
                analysis.analysis_task_id.as_ref(),
                analysis.quality_gate_action.as_deref(),
            );
        }
        if let Some(quality_metrics) = analysis.quality_metrics.as_ref() {
            let _ = update_latest_generated_chapter_history_quality_metrics(
                db,
                &result.chapter_id,
                &result.content,
                quality_metrics,
            )
            .await;
        }
        analysis.candidate_draft = resolve_single_generation_stream_candidate_draft(
            db,
            result,
            analysis.quality_gate_action.as_deref(),
            analysis.quality_metrics.as_ref(),
        )
        .await;

        analysis
    }

    pub(crate) fn from_quality_metrics(
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
        let followup_plan = SingleGenerationStreamAnalysisFollowupPlan::from_quality_gate(
            analysis_task_id.as_ref(),
            quality_gate_action.as_deref(),
        );

        Self {
            analysis_task_id,
            quality_metrics,
            quality_gate_action,
            quality_gate_message,
            quality_gate_snapshot,
            hard_gate_blocked,
            story_runtime_contract,
            followup_plan,
            candidate_draft: None,
        }
    }

    pub(crate) fn quality_metrics_event(&self, result: &GeneratedChapterResult) -> Option<Value> {
        let metrics = self.quality_metrics.as_ref()?.as_object()?.clone();
        let mut payload = serde_json::Map::from_iter([
            ("type".to_string(), json!("quality_metrics")),
            ("chapter_id".to_string(), json!(result.chapter_id)),
            ("chapter_number".to_string(), json!(result.chapter_number)),
        ]);
        payload.extend(metrics);
        Some(Value::Object(payload))
    }

    pub(crate) fn quality_gate_event(&self, result: &GeneratedChapterResult) -> Option<Value> {
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

    pub(crate) fn analysis_started_event(&self) -> Option<Value> {
        let task_id = self.analysis_task_id.as_deref()?;
        let message = self.followup_plan.analysis_started_message()?;

        Some(json!({
            "type": "analysis_started",
            "task_id": task_id,
            "message": message,
        }))
    }

    pub(crate) fn completion_message(&self) -> &str {
        self.followup_plan.completion_message()
    }

    pub(crate) fn response_payload(&self, result: &GeneratedChapterResult) -> Value {
        let saved_word_count = if result.saved_word_count > 0 {
            result.saved_word_count
        } else {
            result.word_count
        };
        let chapter_status = if result.chapter_status.trim().is_empty() {
            if result.content_applied {
                "completed"
            } else {
                "draft"
            }
        } else {
            result.chapter_status.as_str()
        };
        let mut payload = serde_json::Map::from_iter([
            ("chapter_id".to_string(), json!(result.chapter_id)),
            ("chapter_number".to_string(), json!(result.chapter_number)),
            ("title".to_string(), json!(result.title)),
            ("content".to_string(), json!(result.content)),
            ("word_count".to_string(), json!(result.word_count)),
            ("saved_word_count".to_string(), json!(saved_word_count)),
            ("chapter_status".to_string(), json!(chapter_status)),
            ("content_applied".to_string(), json!(result.content_applied)),
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
        if let Some(candidate_draft) = self
            .candidate_draft
            .as_ref()
            .or(result.candidate_draft.as_ref())
        {
            payload.insert("candidate_draft".to_string(), candidate_draft.clone());
        }
        if let Some(candidate_gateway_metadata) = result.candidate_gateway_metadata.as_ref() {
            payload.insert(
                "candidate_gateway".to_string(),
                candidate_gateway_metadata.clone(),
            );
        }

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

    pub(crate) fn ordered_success_event_payloads(
        &self,
        result: &GeneratedChapterResult,
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

    pub(crate) async fn emit_success(
        &self,
        result: &GeneratedChapterResult,
        tx: &mpsc::Sender<Result<Event, Infallible>>,
        tracker: &mut SseProgress,
    ) {
        for step in self.build_success_emission_plan(result) {
            match step {
                SingleGenerationStreamEmissionStep::Complete(message) => {
                    let _ = tx.send(Ok(tracker.complete(Some(&message)))).await;
                }
                SingleGenerationStreamEmissionStep::Payload(payload) => {
                    let event = match payload {
                        SingleGenerationStreamSuccessEventPayload::Json(payload) => {
                            sse_json(&payload)
                        }
                        SingleGenerationStreamSuccessEventPayload::Result(payload) => {
                            sse_result(&payload)
                        }
                    };
                    let _ = tx.send(Ok(event)).await;
                }
                SingleGenerationStreamEmissionStep::Done => {
                    let _ = tx.send(Ok(sse_done())).await;
                }
            };
        }
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
            followup_plan: SingleGenerationStreamAnalysisFollowupPlan::without_analysis(),
            candidate_draft: None,
        }
    }

    pub(crate) fn build_success_emission_plan(
        &self,
        result: &GeneratedChapterResult,
    ) -> Vec<SingleGenerationStreamEmissionStep> {
        let mut steps = Vec::new();
        steps.push(SingleGenerationStreamEmissionStep::Complete(
            self.completion_message().to_string(),
        ));
        steps.extend(
            self.ordered_success_event_payloads(result)
                .into_iter()
                .map(SingleGenerationStreamEmissionStep::Payload),
        );
        steps.push(SingleGenerationStreamEmissionStep::Done);
        steps
    }

    async fn run_follow_up_analysis(
        db: &DatabaseConnection,
        runtime_user_id: &str,
        generated: &GeneratedChapterResult,
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
}

async fn resolve_single_generation_stream_candidate_draft(
    db: &DatabaseConnection,
    result: &GeneratedChapterResult,
    quality_gate_action: Option<&str>,
    quality_metrics: Option<&Value>,
) -> Option<Value> {
    if let Some(candidate_draft) = result.candidate_draft.as_ref() {
        return Some(candidate_draft.clone());
    }
    if !matches!(quality_gate_action, Some("retry") | Some("manual_review")) {
        return None;
    }

    persist_single_generation_stream_followup_candidate_draft(
        db,
        result,
        quality_gate_action,
        quality_metrics,
    )
    .await
    .ok()
}

pub(crate) async fn persist_single_generation_stream_followup_candidate_draft(
    db: &DatabaseConnection,
    result: &GeneratedChapterResult,
    quality_gate_action: Option<&str>,
    quality_metrics: Option<&Value>,
) -> Result<Value, String> {
    let chapter_model = chapter::Entity::find_by_id(&result.chapter_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "chapter not found while saving follow-up candidate draft".to_string())?;

    let draft_result = build_single_generation_followup_draft_result(
        result,
        "draft",
        "manual_review",
        quality_gate_action,
        None,
        quality_metrics,
    );

    let previous_content = chapter_model.content.as_deref().unwrap_or_default();
    let previous_word_count = chapter_model.word_count;
    let draft_lifecycle_view = single_generation_candidate_draft_lifecycle_view(
        &chapter_model,
        &draft_result,
        previous_content,
        previous_word_count,
    );
    persist_single_generation_candidate_draft_attempt(db, draft_lifecycle_view).await
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingleGenerationStreamSuccessEventPayload {
    Json(Value),
    Result(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingleGenerationStreamEmissionStep {
    Complete(String),
    Payload(SingleGenerationStreamSuccessEventPayload),
    Done,
}

pub(crate) fn map_single_generation_stream_quality_gate_action(
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

pub(crate) fn build_single_generation_stream_story_runtime_contract(
    chapter_number: i32,
    target_word_count: i32,
    compat_options: &SingleChapterGenerationCompatOptions,
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

pub(crate) fn attach_single_generation_stream_story_runtime_contract(
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

fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
