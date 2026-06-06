use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;

const BATCH_REQUEST_RUNTIME_STATE_KEY: &str = "batch_request_runtime_state";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BatchGenerationRequestRuntimeState {
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) model_override: Option<String>,
}

impl BatchGenerationRequestRuntimeState {
    pub(crate) fn new(
        compat_options: SingleChapterGenerationCompatOptions,
        model_override: Option<String>,
    ) -> Self {
        Self {
            compat_options,
            model_override,
        }
    }

    pub(crate) fn active_story_repair_payload_with_scope(&self, scope: &str) -> Option<Value> {
        let summary = self.compat_options.story_repair_summary().trim();
        let repair_targets = self
            .compat_options
            .story_repair_targets()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let preserve_strengths = self
            .compat_options
            .story_preserve_strengths()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if summary.is_empty() && repair_targets.is_empty() && preserve_strengths.is_empty() {
            return None;
        }

        Some(json!({
            "summary": if summary.is_empty() { Value::Null } else { json!(summary) },
            "repair_targets": repair_targets,
            "preserve_strengths": preserve_strengths,
            "focus_areas": Vec::<String>::new(),
            "weakest_metric_key": Value::Null,
            "weakest_metric_label": Value::Null,
            "weakest_metric_value": Value::Null,
            "quality_gate": Value::Null,
            "quality_gate_status": Value::Null,
            "quality_gate_decision": Value::Null,
            "quality_gate_label": Value::Null,
            "quality_gate_summary": Value::Null,
            "quality_gate_failed_metrics": Vec::<String>::new(),
            "source": "manual_request",
            "source_label": "Manual request",
            "scope": scope,
            "updated_at": Value::Null,
        }))
    }
}

pub(crate) fn batch_generation_request_runtime_state_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        BATCH_REQUEST_RUNTIME_STATE_KEY.to_string(),
        json!(request_runtime_state),
    )]);
    if let Some(active_story_repair_payload) =
        request_runtime_state.active_story_repair_payload_with_scope("batch")
    {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }

    Value::Object(payload)
}

pub(crate) fn parse_batch_generation_request_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationRequestRuntimeState {
    workflow_runtime_state
        .and_then(|state| state.get(BATCH_REQUEST_RUNTIME_STATE_KEY).cloned())
        .and_then(|value| serde_json::from_value::<BatchGenerationRequestRuntimeState>(value).ok())
        .unwrap_or_default()
}

pub(crate) fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
        .cloned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        active_story_repair_payload_from_runtime_state,
        batch_generation_request_runtime_state_payload,
        parse_batch_generation_request_runtime_state, BatchGenerationRequestRuntimeState,
    };
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;

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
    fn should_build_batch_request_runtime_state_payload_from_owner() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("强化旧伏笔".to_string()),
                story_repair_targets: vec!["伏笔".to_string()],
                story_preserve_strengths: vec!["氛围".to_string()],
                ..empty_compat_options()
            },
            Some("gpt-4.1".to_string()),
        );

        let payload = batch_generation_request_runtime_state_payload(&runtime_state);

        assert_eq!(
            payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(payload["active_story_repair_payload"]["scope"], "batch");
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"][0],
            "伏笔"
        );
    }

    #[test]
    fn should_parse_batch_request_runtime_state_from_runtime_payload() {
        let payload = json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "style_id": 7,
                    "enable_analysis": true,
                    "enable_mcp": false,
                    "web_research_enabled": true,
                    "web_research_query": "旧都城",
                    "narrative_perspective": null,
                    "creative_mode": null,
                    "story_focus": null,
                    "plot_stage": null,
                    "story_creation_brief": null,
                    "quality_preset": null,
                    "quality_notes": null,
                    "story_repair_summary": "强化冲突",
                    "story_repair_targets": ["冲突"],
                    "story_preserve_strengths": ["节奏"]
                },
                "model_override": "gpt-4.1"
            }
        });

        let runtime_state = parse_batch_generation_request_runtime_state(Some(&payload));

        assert_eq!(runtime_state.model_override.as_deref(), Some("gpt-4.1"));
        assert_eq!(
            runtime_state.compat_options.story_repair_summary.as_deref(),
            Some("强化冲突")
        );
        assert_eq!(
            runtime_state.compat_options.story_preserve_strengths,
            vec!["节奏".to_string()]
        );
    }

    #[test]
    fn should_extract_active_story_repair_payload_from_runtime_payload() {
        let payload = json!({
            "active_story_repair_payload": {
                "scope": "chapter",
                "summary": "修复节奏"
            }
        });

        let repair_payload =
            active_story_repair_payload_from_runtime_state(Some(&payload)).expect("repair payload");

        assert_eq!(repair_payload["scope"], "chapter");
        assert_eq!(repair_payload["summary"], "修复节奏");
    }
}
