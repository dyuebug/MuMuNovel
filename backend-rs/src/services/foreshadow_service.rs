use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::foreshadow;

fn model_to_value(f: &foreshadow::Model) -> Value {
    json!({
        "id": f.id,
        "project_id": f.project_id,
        "title": f.title,
        "content": f.content,
        "hint_text": f.hint_text,
        "resolution_text": f.resolution_text,
        "source_type": f.source_type,
        "source_memory_id": f.source_memory_id,
        "source_analysis_id": f.source_analysis_id,
        "plant_chapter_id": f.plant_chapter_id,
        "plant_chapter_number": f.plant_chapter_number,
        "target_resolve_chapter_id": f.target_resolve_chapter_id,
        "target_resolve_chapter_number": f.target_resolve_chapter_number,
        "actual_resolve_chapter_id": f.actual_resolve_chapter_id,
        "actual_resolve_chapter_number": f.actual_resolve_chapter_number,
        "status": f.status,
        "is_long_term": f.is_long_term,
        "importance": f.importance,
        "strength": f.strength,
        "subtlety": f.subtlety,
        "urgency": f.urgency,
        "related_characters": f.related_characters,
        "related_foreshadow_ids": f.related_foreshadow_ids,
        "tags": f.tags,
        "category": f.category,
        "notes": f.notes,
        "resolution_notes": f.resolution_notes,
        "auto_remind": f.auto_remind,
        "remind_before_chapters": f.remind_before_chapters,
        "include_in_context": f.include_in_context,
        "created_at": f.created_at.to_rfc3339(),
        "updated_at": f.updated_at.to_rfc3339(),
        "planted_at": f.planted_at.map(|t| t.to_rfc3339()),
        "resolved_at": f.resolved_at.map(|t| t.to_rfc3339()),
    })
}

fn compute_stats(items: &[foreshadow::Model]) -> Value {
    let mut total = 0i64;
    let mut pending = 0i64;
    let mut planted = 0i64;
    let mut resolved = 0i64;
    let mut partially_resolved = 0i64;
    let mut abandoned = 0i64;
    let mut long_term_count = 0i64;
    let mut overdue_count = 0i64;

    for f in items {
        total += 1;
        match f.status.as_str() {
            "pending" => pending += 1,
            "planted" => { planted += 1; if f.is_long_term { long_term_count += 1; } }
            "resolved" => resolved += 1,
            "partially_resolved" => partially_resolved += 1,
            "abandoned" => abandoned += 1,
            _ => {}
        }
        if f.urgency >= 2 { overdue_count += 1; }
    }

    json!({
        "total": total,
        "pending": pending,
        "planted": planted,
        "resolved": resolved,
        "partially_resolved": partially_resolved,
        "abandoned": abandoned,
        "long_term_count": long_term_count,
        "overdue_count": overdue_count,
    })
}

pub struct ForeshadowService;

impl ForeshadowService {
    pub async fn list_project(
        db: &DatabaseConnection,
        project_id: &str,
        status: Option<&str>,
        category: Option<&str>,
        source_type: Option<&str>,
        is_long_term: Option<bool>,
        page: Option<u64>,
        limit: Option<u64>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let mut query = foreshadow::Entity::find()
            .filter(foreshadow::Column::ProjectId.eq(project_id));

        if let Some(s) = status { query = query.filter(foreshadow::Column::Status.eq(s)); }
        if let Some(c) = category { query = query.filter(foreshadow::Column::Category.eq(c)); }
        if let Some(st) = source_type { query = query.filter(foreshadow::Column::SourceType.eq(st)); }
        if let Some(lt) = is_long_term { query = query.filter(foreshadow::Column::IsLongTerm.eq(lt)); }

        let all: Vec<foreshadow::Model> = query
            .clone()
            .order_by_desc(foreshadow::Column::CreatedAt)
            .all(db)
            .await?;

        let stats = compute_stats(&all);

        let limit = limit.unwrap_or(50) as usize;
        let page = page.unwrap_or(1) as usize;
        let skip = (page.saturating_sub(1)) * limit;

        let items: Vec<Value> = all.iter().skip(skip).take(limit).map(model_to_value).collect();

        Ok(json!({
            "total": all.len(),
            "items": items,
            "stats": stats,
        }))
    }

    pub async fn get_stats(
        db: &DatabaseConnection,
        project_id: &str,
        current_chapter: Option<i32>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let items = foreshadow::Entity::find()
            .filter(foreshadow::Column::ProjectId.eq(project_id))
            .all(db)
            .await?;

        let mut stats = compute_stats(&items);
        if let Some(ch) = current_chapter {
            let overdue = items.iter().filter(|f| {
                f.target_resolve_chapter_number.map_or(false, |t| t < ch)
                    && (f.status == "planted" || f.status == "pending")
            }).count();
            if let Some(obj) = stats.as_object_mut() {
                obj.insert("overdue_count".into(), json!(overdue));
            }
        }
        Ok(stats)
    }

    pub async fn get_context(
        db: &DatabaseConnection,
        project_id: &str,
        chapter_number: i32,
        include_pending: Option<bool>,
        include_overdue: Option<bool>,
        lookahead: Option<i32>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let all = foreshadow::Entity::find()
            .filter(foreshadow::Column::ProjectId.eq(project_id))
            .all(db)
            .await?;

        let lookahead = lookahead.unwrap_or(5);
        let inc_pending = include_pending.unwrap_or(true);
        let inc_overdue = include_overdue.unwrap_or(true);

        let pending_plant: Vec<Value> = if inc_pending {
            all.iter().filter(|f| f.status == "pending").map(model_to_value).collect()
        } else { vec![] };

        let pending_resolve: Vec<Value> = all.iter().filter(|f| {
            f.status == "planted"
                && f.target_resolve_chapter_number.map_or(false, |t| {
                    t >= chapter_number && t <= chapter_number + lookahead
                })
        }).map(model_to_value).collect();

        let overdue: Vec<Value> = if inc_overdue {
            all.iter().filter(|f| {
                f.target_resolve_chapter_number.map_or(false, |t| t < chapter_number)
                    && (f.status == "planted" || f.status == "pending")
            }).map(model_to_value).collect()
        } else { vec![] };

        let recently_planted: Vec<Value> = all.iter().filter(|f| {
            f.status == "planted"
                && f.plant_chapter_number.map_or(false, |p| {
                    p >= chapter_number.saturating_sub(3) && p < chapter_number
                })
        }).map(model_to_value).collect();

        let context_parts: Vec<String> = pending_resolve.iter()
            .filter_map(|f| {
                Some(format!("伏笔「{}」(第{}章): {}",
                    f.get("title")?.as_str()?,
                    f.get("target_resolve_chapter_number")?.as_i64()?,
                    f.get("content")?.as_str()?.chars().take(80).collect::<String>(),
                ))
            })
            .collect();

        Ok(json!({
            "chapter_number": chapter_number,
            "context_text": context_parts.join("\n"),
            "pending_plant": pending_plant,
            "pending_resolve": pending_resolve,
            "overdue": overdue,
            "recently_planted": recently_planted,
        }))
    }

    pub async fn list_pending_resolve(
        db: &DatabaseConnection,
        project_id: &str,
        current_chapter: i32,
        lookahead: Option<i32>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let lookahead = lookahead.unwrap_or(5);
        let all = foreshadow::Entity::find()
            .filter(foreshadow::Column::ProjectId.eq(project_id))
            .all(db)
            .await?;

        let items: Vec<Value> = all.iter().filter(|f| {
            f.status == "planted"
                && f.target_resolve_chapter_number.map_or(false, |t| {
                    t >= current_chapter && t <= current_chapter + lookahead
                })
        }).map(model_to_value).collect();

        Ok(json!({ "total": items.len(), "items": items }))
    }

    pub async fn get_one(
        db: &DatabaseConnection,
        foreshadow_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let f = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;
        Ok(model_to_value(&f))
    }

    pub async fn create(
        db: &DatabaseConnection,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let model = foreshadow::ActiveModel {
            id: Set(id.clone()),
            project_id: Set(body["project_id"].as_str().unwrap_or("").to_string()),
            title: Set(body["title"].as_str().unwrap_or("").to_string()),
            content: Set(body["content"].as_str().unwrap_or("").to_string()),
            hint_text: Set(body.get("hint_text").and_then(|v| v.as_str()).map(String::from)),
            resolution_text: Set(body.get("resolution_text").and_then(|v| v.as_str()).map(String::from)),
            source_type: Set("manual".to_string()),
            source_memory_id: Set(None),
            source_analysis_id: Set(None),
            plant_chapter_id: Set(None),
            plant_chapter_number: Set(body.get("plant_chapter_number").and_then(|v| v.as_i64()).map(|v| v as i32)),
            target_resolve_chapter_id: Set(None),
            target_resolve_chapter_number: Set(body.get("target_resolve_chapter_number").and_then(|v| v.as_i64()).map(|v| v as i32)),
            actual_resolve_chapter_id: Set(None),
            actual_resolve_chapter_number: Set(None),
            status: Set("pending".to_string()),
            is_long_term: Set(body.get("is_long_term").and_then(|v| v.as_bool()).unwrap_or(false)),
            importance: Set(body.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.5)),
            strength: Set(body.get("strength").and_then(|v| v.as_i64()).unwrap_or(5) as i32),
            subtlety: Set(body.get("subtlety").and_then(|v| v.as_i64()).unwrap_or(5) as i32),
            urgency: Set(0),
            related_characters: Set(body.get("related_characters").cloned()),
            related_foreshadow_ids: Set(None),
            tags: Set(body.get("tags").cloned()),
            category: Set(body.get("category").and_then(|v| v.as_str()).map(String::from)),
            notes: Set(body.get("notes").and_then(|v| v.as_str()).map(String::from)),
            resolution_notes: Set(body.get("resolution_notes").and_then(|v| v.as_str()).map(String::from)),
            auto_remind: Set(body.get("auto_remind").and_then(|v| v.as_bool()).unwrap_or(true)),
            remind_before_chapters: Set(body.get("remind_before_chapters").and_then(|v| v.as_i64()).unwrap_or(5) as i32),
            include_in_context: Set(body.get("include_in_context").and_then(|v| v.as_bool()).unwrap_or(true)),
            created_at: Set(now),
            updated_at: Set(now),
            planted_at: Set(None),
            resolved_at: Set(None),
        };

        let saved = model.insert(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn update(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let mut active: foreshadow::ActiveModel = existing.into();
        if let Some(v) = body.get("title").and_then(|v| v.as_str()) { active.title = Set(v.to_string()); }
        if let Some(v) = body.get("content").and_then(|v| v.as_str()) { active.content = Set(v.to_string()); }
        if let Some(v) = body.get("hint_text").and_then(|v| v.as_str()) { active.hint_text = Set(Some(v.to_string())); }
        if let Some(v) = body.get("resolution_text").and_then(|v| v.as_str()) { active.resolution_text = Set(Some(v.to_string())); }
        if let Some(v) = body.get("plant_chapter_number").and_then(|v| v.as_i64()) { active.plant_chapter_number = Set(Some(v as i32)); }
        if let Some(v) = body.get("target_resolve_chapter_number").and_then(|v| v.as_i64()) { active.target_resolve_chapter_number = Set(Some(v as i32)); }
        if let Some(v) = body.get("status").and_then(|v| v.as_str()) { active.status = Set(v.to_string()); }
        if let Some(v) = body.get("is_long_term").and_then(|v| v.as_bool()) { active.is_long_term = Set(v); }
        if let Some(v) = body.get("importance").and_then(|v| v.as_f64()) { active.importance = Set(v); }
        if let Some(v) = body.get("strength").and_then(|v| v.as_i64()) { active.strength = Set(v as i32); }
        if let Some(v) = body.get("subtlety").and_then(|v| v.as_i64()) { active.subtlety = Set(v as i32); }
        if let Some(v) = body.get("urgency").and_then(|v| v.as_i64()) { active.urgency = Set(v as i32); }
        if body.get("related_characters").is_some() { active.related_characters = Set(body.get("related_characters").cloned()); }
        if body.get("related_foreshadow_ids").is_some() { active.related_foreshadow_ids = Set(body.get("related_foreshadow_ids").cloned()); }
        if body.get("tags").is_some() { active.tags = Set(body.get("tags").cloned()); }
        if let Some(v) = body.get("category").and_then(|v| v.as_str()) { active.category = Set(Some(v.to_string())); }
        if let Some(v) = body.get("notes").and_then(|v| v.as_str()) { active.notes = Set(Some(v.to_string())); }
        if let Some(v) = body.get("resolution_notes").and_then(|v| v.as_str()) { active.resolution_notes = Set(Some(v.to_string())); }
        if let Some(v) = body.get("auto_remind").and_then(|v| v.as_bool()) { active.auto_remind = Set(v); }
        if let Some(v) = body.get("remind_before_chapters").and_then(|v| v.as_i64()) { active.remind_before_chapters = Set(v as i32); }
        if let Some(v) = body.get("include_in_context").and_then(|v| v.as_bool()) { active.include_in_context = Set(v); }
        active.updated_at = Set(Utc::now());

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn delete(
        db: &DatabaseConnection,
        foreshadow_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        foreshadow::Entity::delete_by_id(foreshadow_id).exec(db).await?;
        Ok(json!({"message": "伏笔已删除", "id": foreshadow_id}))
    }

    pub async fn plant(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let now = Utc::now();
        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set("planted".to_string());
        active.plant_chapter_id = Set(body.get("chapter_id").and_then(|v| v.as_str()).map(String::from));
        active.plant_chapter_number = Set(body.get("chapter_number").and_then(|v| v.as_i64()).map(|v| v as i32));
        if let Some(v) = body.get("hint_text").and_then(|v| v.as_str()) { active.hint_text = Set(Some(v.to_string())); }
        active.planted_at = Set(Some(now));
        active.updated_at = Set(now);

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn resolve(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let now = Utc::now();
        let is_partial = body.get("is_partial").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set(if is_partial { "partially_resolved".to_string() } else { "resolved".to_string() });
        active.actual_resolve_chapter_id = Set(body.get("chapter_id").and_then(|v| v.as_str()).map(String::from));
        active.actual_resolve_chapter_number = Set(body.get("chapter_number").and_then(|v| v.as_i64()).map(|v| v as i32));
        if let Some(v) = body.get("resolution_text").and_then(|v| v.as_str()) { active.resolution_text = Set(Some(v.to_string())); }
        active.resolved_at = Set(Some(now));
        active.updated_at = Set(now);

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn abandon(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        reason: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let now = Utc::now();
        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set("abandoned".to_string());
        if let Some(r) = reason { active.notes = Set(Some(format!("废弃原因: {}", r))); }
        active.updated_at = Set(now);

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn sync_from_analysis(
        _db: &DatabaseConnection,
        _project_id: &str,
        _body: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // AI-dependent feature: syncing from analysis results requires
        // chapter analysis pipeline. Return empty success for now.
        Ok(json!({
            "synced_count": 0,
            "skipped_count": 0,
            "new_foreshadows": [],
            "skipped_reasons": [],
        }))
    }
}
