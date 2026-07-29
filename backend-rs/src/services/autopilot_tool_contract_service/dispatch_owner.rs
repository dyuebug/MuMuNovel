use sea_orm::ConnectionTrait;
use serde_json::Value;

use crate::services::novel_workflow_service::{
    self, NovelWorkflowAuditContext, NovelWorkflowError,
};

use super::schema_owner::{
    parse_transition_project_workflow_args, AutopilotToolContractError,
    AutopilotToolExecutionResultV1, AutopilotToolName, AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutopilotToolConfirmation {
    Missing,
    ConfirmedByUser,
}

#[derive(Debug, Clone, Copy)]
pub struct AutopilotToolExecutionContext<'a> {
    pub actor_user_id: &'a str,
    pub confirmation: AutopilotToolConfirmation,
    /// 仅由内部后台任务边界注入的 canonical project scope；直接调用可保持无 scope。
    pub project_scope: Option<&'a str>,
}

pub async fn dispatch_autopilot_tool_call<C>(
    db: &C,
    context: AutopilotToolExecutionContext<'_>,
    tool_name: &str,
    arguments_json: &str,
) -> Result<AutopilotToolExecutionResultV1, AutopilotToolContractError>
where
    C: ConnectionTrait,
{
    let arguments = serde_json::from_str(arguments_json)
        .map_err(|_| AutopilotToolContractError::InvalidArguments)?;
    dispatch_autopilot_tool(db, context, tool_name, arguments).await
}

pub async fn dispatch_autopilot_tool<C>(
    db: &C,
    context: AutopilotToolExecutionContext<'_>,
    tool_name: &str,
    arguments: Value,
) -> Result<AutopilotToolExecutionResultV1, AutopilotToolContractError>
where
    C: ConnectionTrait,
{
    let tool = AutopilotToolName::parse(tool_name)?;
    let result = match tool {
        AutopilotToolName::TransitionProjectWorkflow => {
            let args = parse_transition_project_workflow_args(arguments)?;
            if context
                .project_scope
                .is_some_and(|project_scope| project_scope != args.project_id)
            {
                return Err(AutopilotToolContractError::ProjectScopeMismatch);
            }
            if tool.requires_confirmation()
                && context.confirmation != AutopilotToolConfirmation::ConfirmedByUser
            {
                return Err(AutopilotToolContractError::ConfirmationRequired);
            }

            let receipt = novel_workflow_service::transition_with_connection(
                db,
                &args.project_id,
                context.actor_user_id,
                args.expected_phase,
                args.target_phase,
                NovelWorkflowAuditContext {
                    reason: args.reason,
                    related_task_id: args.related_task_id,
                },
            )
            .await
            .map_err(map_workflow_error)?;
            AutopilotToolExecutionResultV1::transition_project_workflow(receipt)
        }
    };

    tracing::info!(
        event = "autopilot_tool_contract_execution",
        schema_version = AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION,
        tool_name = tool.as_str(),
        outcome = "succeeded",
        "autopilot tool contract execution completed"
    );
    Ok(result)
}

fn map_workflow_error(error: NovelWorkflowError) -> AutopilotToolContractError {
    match error {
        NovelWorkflowError::IllegalTransition { from, to } => {
            AutopilotToolContractError::InvalidTransition { from, to }
        }
        NovelWorkflowError::ReasonRequired { from, to } => {
            AutopilotToolContractError::ReasonRequired { from, to }
        }
        NovelWorkflowError::StaleExpectedPhase { expected, actual } => {
            AutopilotToolContractError::StaleExpectedPhase { expected, actual }
        }
        NovelWorkflowError::NotFoundOrAccessDenied => {
            AutopilotToolContractError::NotFoundOrAccessDenied
        }
        NovelWorkflowError::InvalidPhase { .. }
        | NovelWorkflowError::UnknownPersistedPhase { .. }
        | NovelWorkflowError::Internal(_) => AutopilotToolContractError::Internal,
    }
}
