//! G2 的固定、无敏感信息的 R7 Autopilot 测试样本。
//!
//! 该模块只在测试构建中注册；它不参与生产运行时 contract 或 payload 解析。

use serde_json::{json, Value};

pub(crate) const PROJECT_ID: &str = "g2-project-1";
pub(crate) const OWNER_ID: &str = "g2-owner-1";
pub(crate) const TASK_ID: &str = "g2-autopilot-task-1";
pub(crate) const TOOL_NAME: &str = "transition_project_workflow";
pub(crate) const EXPECTED_PHASE: &str = "foundation";
pub(crate) const TARGET_PHASE: &str = "world_building";
pub(crate) const TERMINAL_AUDIT_FAILURE_CODE: &str = "tool_execution_failed";

pub(crate) fn confirmed_transition_payload(project_id: &str) -> Value {
    json!({
        "tool_name": TOOL_NAME,
        "arguments": format!(
            r#"{{"project_id":"{project_id}","expected_phase":"{EXPECTED_PHASE}","target_phase":"{TARGET_PHASE}"}}"#
        ),
        "confirmed_by_user": true,
    })
}
