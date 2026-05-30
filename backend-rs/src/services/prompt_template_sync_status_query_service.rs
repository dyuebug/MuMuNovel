use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::prompt_template_service::PromptTemplateService;

fn build_prompt_template_sync_status_response(items: Vec<Value>, managed_only: bool) -> Value {
    json!({
        "total": items.len(),
        "managed_only": managed_only,
        "items": items,
    })
}

fn select_prompt_template_sync_status_keys(managed_only: bool) -> Vec<String> {
    if managed_only {
        PromptTemplateService::managed_keys().to_vec()
    } else {
        PromptTemplateService::all_system_templates()
            .iter()
            .map(|template| template.template_key.clone())
            .collect()
    }
}

pub async fn load_prompt_template_sync_status_payload(
    db: &DatabaseConnection,
    user_id: &str,
    managed_only: bool,
) -> Result<Value, String> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(db, user_id).await?;

    let template_keys = select_prompt_template_sync_status_keys(managed_only);
    let mut items = Vec::with_capacity(template_keys.len());

    for key in &template_keys {
        let user_template = PromptTemplateService::find_user_template(db, user_id, key).await?;
        items.push(PromptTemplateService::build_sync_status(
            key,
            user_template.as_ref(),
        ));
    }

    Ok(build_prompt_template_sync_status_response(
        items,
        managed_only,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        build_prompt_template_sync_status_response, select_prompt_template_sync_status_keys,
    };
    use serde_json::json;

    #[test]
    fn build_prompt_template_sync_status_response_keeps_existing_shell() {
        let payload = build_prompt_template_sync_status_response(
            vec![json!({
                "template_key": "chapter_generate",
                "sync_status": "system_default",
            })],
            true,
        );

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["managed_only"], true);
        assert_eq!(payload["items"][0]["template_key"], "chapter_generate");
        assert_eq!(payload["items"][0]["sync_status"], "system_default");
    }

    #[test]
    fn select_prompt_template_sync_status_keys_uses_managed_filter_when_enabled() {
        let managed_keys = select_prompt_template_sync_status_keys(true);
        let all_keys = select_prompt_template_sync_status_keys(false);

        assert!(!managed_keys.is_empty());
        assert!(all_keys.len() >= managed_keys.len());
        assert!(managed_keys.iter().all(|key| all_keys.contains(key)));
    }
}
