use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ai::types::{ToolDef, ToolFunction};
use crate::services::novel_workflow_service::{NovelWorkflowPhase, NovelWorkflowTransitionReceipt};

pub const AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION: &str = "autopilot-tool-contract/v1";

// 这是 provider 可见 schema 的稳定公共枚举；tests 会与 workflow owner 的 canonical enum 对齐，
// 但不会在此复制 workflow transition matrix 或持久化规则。
const PUBLIC_WORKFLOW_PHASE_VALUES: [&str; 9] = [
    "inspiration",
    "foundation",
    "world_building",
    "character_design",
    "outline",
    "writing",
    "reviewing",
    "polishing",
    "completed",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotToolName {
    TransitionProjectWorkflow,
}

impl AutopilotToolName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransitionProjectWorkflow => "transition_project_workflow",
        }
    }

    pub const fn requires_confirmation(self) -> bool {
        match self {
            Self::TransitionProjectWorkflow => true,
        }
    }

    pub fn parse(value: &str) -> Result<Self, AutopilotToolContractError> {
        match value {
            "transition_project_workflow" => Ok(Self::TransitionProjectWorkflow),
            _ => Err(AutopilotToolContractError::UnknownTool),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutopilotToolContractError {
    UnknownTool,
    InvalidArguments,
    ConfirmationRequired,
    ProjectScopeMismatch,
    NotFoundOrAccessDenied,
    StaleExpectedPhase {
        expected: NovelWorkflowPhase,
        actual: NovelWorkflowPhase,
    },
    InvalidTransition {
        from: NovelWorkflowPhase,
        to: NovelWorkflowPhase,
    },
    ReasonRequired {
        from: NovelWorkflowPhase,
        to: NovelWorkflowPhase,
    },
    Internal,
}

impl fmt::Display for AutopilotToolContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool => formatter.write_str("unknown autopilot tool"),
            Self::InvalidArguments => formatter.write_str("invalid autopilot tool arguments"),
            Self::ConfirmationRequired => {
                formatter.write_str("user confirmation is required for this autopilot tool")
            }
            Self::ProjectScopeMismatch => {
                formatter.write_str("autopilot task project scope does not match tool arguments")
            }
            Self::NotFoundOrAccessDenied => {
                formatter.write_str("project not found or access denied")
            }
            Self::StaleExpectedPhase { expected, actual } => write!(
                formatter,
                "stale expected workflow phase: expected {expected}, actual {actual}"
            ),
            Self::InvalidTransition { from, to } => {
                write!(formatter, "invalid workflow transition: {from} -> {to}")
            }
            Self::ReasonRequired { from, to } => {
                write!(
                    formatter,
                    "workflow transition reason is required: {from} -> {to}"
                )
            }
            Self::Internal => formatter.write_str("autopilot tool execution failed"),
        }
    }
}

impl std::error::Error for AutopilotToolContractError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutopilotToolExecutionResultV1 {
    pub schema_version: &'static str,
    pub tool_name: AutopilotToolName,
    pub receipt: NovelWorkflowTransitionReceipt,
}

impl AutopilotToolExecutionResultV1 {
    pub fn transition_project_workflow(receipt: NovelWorkflowTransitionReceipt) -> Self {
        Self {
            schema_version: AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION,
            tool_name: AutopilotToolName::TransitionProjectWorkflow,
            receipt,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionProjectWorkflowArgs {
    pub project_id: String,
    pub expected_phase: NovelWorkflowPhase,
    pub target_phase: NovelWorkflowPhase,
    pub reason: Option<String>,
    pub related_task_id: Option<String>,
}

pub fn parse_transition_project_workflow_args(
    arguments: Value,
) -> Result<TransitionProjectWorkflowArgs, AutopilotToolContractError> {
    if !arguments.is_object() {
        return Err(AutopilotToolContractError::InvalidArguments);
    }

    let mut args: TransitionProjectWorkflowArgs = serde_json::from_value(arguments)
        .map_err(|_| AutopilotToolContractError::InvalidArguments)?;
    args.project_id = args.project_id.trim().to_string();
    if args.project_id.is_empty() {
        return Err(AutopilotToolContractError::InvalidArguments);
    }

    Ok(args)
}

pub fn autopilot_tool_definitions() -> Vec<ToolDef> {
    vec![ToolDef {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: AutopilotToolName::TransitionProjectWorkflow
                .as_str()
                .to_string(),
            description: "在用户已确认后，按期望阶段以 CAS 方式推进项目工作流。".to_string(),
            parameters: transition_project_workflow_input_schema(),
        },
    }]
}

fn transition_project_workflow_input_schema() -> Value {
    let phase_values = PUBLIC_WORKFLOW_PHASE_VALUES.to_vec();

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id", "expected_phase", "target_phase"],
        "properties": {
            "project_id": {"type": "string", "minLength": 1},
            "expected_phase": {"type": "string", "enum": phase_values},
            "target_phase": {"type": "string", "enum": phase_values},
            "reason": {"type": "string"},
            "related_task_id": {"type": "string"}
        }
    })
}
