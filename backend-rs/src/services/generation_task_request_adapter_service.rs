use serde_json::{json, Value};

use crate::api::characters::GenerateCharacterRequest;
use crate::api::organizations::GenerateOrganizationRequest;

pub fn adapt_character_generation_task_request(
    project_id: &str,
    payload: &Value,
) -> Result<GenerateCharacterRequest, String> {
    serde_json::from_value::<GenerateCharacterRequest>(json!({
        "project_id": project_id,
        "name": payload.get("name").cloned(),
        "role_type": payload.get("role_type").cloned(),
        "background": payload.get("background").cloned(),
        "requirements": payload.get("requirements").cloned(),
        "provider": payload.get("provider").cloned(),
        "model": payload.get("model").cloned(),
    }))
    .map_err(|error| format!("无效的角色生成参数: {}", error))
}

pub fn adapt_organization_generation_task_request(
    project_id: &str,
    payload: &Value,
) -> Result<GenerateOrganizationRequest, String> {
    serde_json::from_value::<GenerateOrganizationRequest>(json!({
        "project_id": project_id,
        "name": payload.get("name").cloned(),
        "organization_type": payload.get("organization_type").cloned(),
        "background": payload.get("background").cloned(),
        "requirements": payload.get("requirements").cloned(),
        "provider": payload.get("provider").cloned(),
        "model": payload.get("model").cloned(),
    }))
    .map_err(|error| format!("无效的组织生成参数: {}", error))
}

#[cfg(test)]
mod tests {
    use super::{
        adapt_character_generation_task_request, adapt_organization_generation_task_request,
    };
    use serde_json::json;

    #[test]
    fn character_generation_task_adapter_keeps_existing_payload_contract() {
        adapt_character_generation_task_request(
            "project-1",
            &json!({
                "name": "阿青",
                "role_type": "supporting",
                "background": "来自边城",
                "requirements": "要有反差感",
                "provider": "openai",
                "model": "gpt-4o-mini"
            }),
        )
        .expect("character request should adapt");
    }

    #[test]
    fn organization_generation_task_adapter_keeps_existing_payload_contract() {
        adapt_organization_generation_task_request(
            "project-2",
            &json!({
                "name": "玄霜盟",
                "organization_type": "门派",
                "background": "北境旧盟",
                "requirements": "要有资源约束",
                "provider": "openai",
                "model": "gpt-4.1"
            }),
        )
        .expect("organization request should adapt");
    }
}
