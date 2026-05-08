use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};

use crate::models::{project_default_style, writing_style};

const PRESET_DEFAULTS: &[(&str, &str, &str)] = &[
    (
        "natural",
        "自然风格",
        "使用自然流畅的语言，平实而富有感染力地进行叙述。注重情感的真实表达和细节的生动描绘。",
    ),
    (
        "classical",
        "古典风格",
        "采用典雅庄重的语言风格，注重词藻的华丽和修辞的运用。适合历史、玄幻等题材。",
    ),
    (
        "modern",
        "现代风格",
        "使用简洁明快的现代语言，节奏紧凑，适合都市、言情等题材。强调对话和内心独白。",
    ),
    (
        "literary",
        "文学风格",
        "追求语言的艺术性和哲理性，善用比喻、象征等修辞手法。适合文学性较强的作品。",
    ),
    (
        "humorous",
        "幽默风格",
        "以轻松诙谐的语言为主，善用夸张、反讽等手法。适合喜剧、讽刺类作品。",
    ),
];

fn style_to_value(s: &writing_style::Model, is_default: bool) -> Value {
    json!({
        "id": s.id,
        "user_id": s.user_id,
        "name": s.name,
        "style_type": s.style_type,
        "preset_id": s.preset_id,
        "description": s.description,
        "prompt_content": s.prompt_content,
        "is_default": is_default,
        "order_index": s.order_index,
        "created_at": s.created_at.and_utc().to_rfc3339(),
        "updated_at": s.updated_at.and_utc().to_rfc3339(),
    })
}

pub struct WritingStyleService;

impl WritingStyleService {
    pub async fn list_presets(
        db: &DatabaseConnection,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.is_null())
            .all(db)
            .await?;

        let items: Vec<Value> = styles
            .iter()
            .map(|s| {
                json!({
                    "id": s.preset_id.clone().unwrap_or_else(|| s.id.to_string()),
                    "name": s.name,
                    "description": s.description,
                    "prompt_content": s.prompt_content,
                })
            })
            .collect();

        Ok(json!(items))
    }

    pub async fn list_user_styles(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let preset_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.is_null())
            .all(db)
            .await?;
        let user_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        let items: Vec<Value> = preset_styles
            .iter()
            .chain(user_styles.iter())
            .map(|s| style_to_value(s, false))
            .collect();

        Ok(json!({ "styles": items, "total": items.len() }))
    }

    pub async fn list_project_styles(
        db: &DatabaseConnection,
        user_id: &str,
        project_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let preset_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.is_null())
            .all(db)
            .await?;
        let user_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        let default_style = project_default_style::Entity::find()
            .filter(project_default_style::Column::ProjectId.eq(project_id))
            .one(db)
            .await?;

        let default_style_id = default_style.map(|ds| ds.style_id);

        let items: Vec<Value> = preset_styles
            .iter()
            .chain(user_styles.iter())
            .map(|s| style_to_value(s, Some(s.id) == default_style_id))
            .collect();

        Ok(json!({ "styles": items, "total": items.len() }))
    }

    pub async fn create_style(
        db: &DatabaseConnection,
        user_id: &str,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now().naive_utc();
        let name = body
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Custom Style");
        let style_type = body
            .get("style_type")
            .and_then(|v| v.as_str())
            .unwrap_or("custom");
        let prompt_content = body
            .get("prompt_content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let model = writing_style::ActiveModel {
            user_id: Set(Some(user_id.to_string())),
            name: Set(name.to_string()),
            style_type: Set(style_type.to_string()),
            preset_id: Set(body
                .get("preset_id")
                .and_then(|v| v.as_str())
                .map(String::from)),
            description: Set(body
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from)),
            prompt_content: Set(prompt_content.to_string()),
            order_index: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let saved = model.insert(db).await?;
        Ok(style_to_value(&saved, false))
    }

    pub async fn update_style(
        db: &DatabaseConnection,
        user_id: &str,
        style_id: i32,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("style not found")?;

        if existing.user_id.as_deref() != Some(user_id) {
            return Err("not your style".into());
        }

        let mut active: writing_style::ActiveModel = existing.into();
        if let Some(v) = body.get("name").and_then(|v| v.as_str()) {
            active.name = Set(v.to_string());
        }
        if let Some(v) = body.get("description").and_then(|v| v.as_str()) {
            active.description = Set(Some(v.to_string()));
        }
        if let Some(v) = body.get("prompt_content").and_then(|v| v.as_str()) {
            active.prompt_content = Set(v.to_string());
        }
        if let Some(v) = body.get("order_index").and_then(|v| v.as_i64()) {
            active.order_index = Set(v as i32);
        }
        active.updated_at = Set(Utc::now().naive_utc());

        let saved = active.update(db).await?;
        Ok(style_to_value(&saved, false))
    }

    pub async fn delete_style(
        db: &DatabaseConnection,
        user_id: &str,
        style_id: i32,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("style not found")?;

        if existing.user_id.as_deref() != Some(user_id) {
            return Err("not your style".into());
        }

        project_default_style::Entity::delete_many()
            .filter(project_default_style::Column::StyleId.eq(style_id))
            .exec(db)
            .await?;

        writing_style::Entity::delete_by_id(style_id)
            .exec(db)
            .await?;
        Ok(json!({"message": "风格已删除"}))
    }

    pub async fn set_default_style(
        db: &DatabaseConnection,
        user_id: &str,
        style_id: i32,
        project_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let style = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("style not found")?;

        // Verify ownership: style must belong to user or be a preset (null user_id)
        if style.user_id.is_some() && style.user_id.as_deref() != Some(user_id) {
            return Err("not your style".into());
        }

        let now = Utc::now().naive_utc();

        // Upsert: delete existing default for this project, then insert new
        let existing_default = project_default_style::Entity::find()
            .filter(project_default_style::Column::ProjectId.eq(project_id))
            .one(db)
            .await?;

        if let Some(ed) = existing_default {
            project_default_style::Entity::delete_by_id(ed.id)
                .exec(db)
                .await?;
        }

        let pd = project_default_style::ActiveModel {
            project_id: Set(project_id.to_string()),
            style_id: Set(style_id),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        pd.insert(db).await?;

        Ok(style_to_value(&style, true))
    }

    pub async fn initialize_defaults(
        db: &DatabaseConnection,
        _project_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now().naive_utc();
        let mut styles: Vec<Value> = vec![];

        for (preset_id, name, description) in PRESET_DEFAULTS {
            // Check if already exists
            let existing = writing_style::Entity::find()
                .filter(writing_style::Column::PresetId.eq(*preset_id))
                .filter(writing_style::Column::UserId.is_null())
                .one(db)
                .await?;

            if existing.is_none() {
                let prompt_content = format!(
                    "你是一位精通{}的作家。请按照以下风格进行创作：\n\n{}",
                    name, description
                );

                let model = writing_style::ActiveModel {
                    user_id: Set(None),
                    name: Set(name.to_string()),
                    style_type: Set("preset".to_string()),
                    preset_id: Set(Some(preset_id.to_string())),
                    description: Set(Some(description.to_string())),
                    prompt_content: Set(prompt_content),
                    order_index: Set(0),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                };
                let saved = model.insert(db).await?;
                styles.push(style_to_value(&saved, false));
            }
        }

        // Also return user's existing styles
        Ok(json!({
            "styles": styles,
            "total": styles.len(),
            "message": if styles.is_empty() { "默认风格已存在" } else { "默认风格已初始化" }
        }))
    }
}
