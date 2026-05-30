use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::foreshadow;
use crate::services::foreshadow_request_service::{
    CreateForeshadowRequest, PlantForeshadowRequest, ResolveForeshadowRequest,
    SyncForeshadowFromAnalysisRequest, UpdateForeshadowRequest,
};

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
        "created_at": f.created_at.and_utc().to_rfc3339(),
        "updated_at": f.updated_at.and_utc().to_rfc3339(),
        "planted_at": f.planted_at.map(|t| t.and_utc().to_rfc3339()),
        "resolved_at": f.resolved_at.map(|t| t.and_utc().to_rfc3339()),
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
            "planted" => {
                planted += 1;
                if f.is_long_term {
                    long_term_count += 1;
                }
            }
            "resolved" => resolved += 1,
            "partially_resolved" => partially_resolved += 1,
            "abandoned" => abandoned += 1,
            _ => {}
        }
        if f.urgency >= 2 {
            overdue_count += 1;
        }
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

fn normalize_analysis_foreshadow_key(chapter_id: &str, content: &str) -> String {
    let normalized = content
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .take(48)
        .collect::<String>();
    format!("analysis:{}:{}", chapter_id, normalized)
}

fn default_analysis_foreshadow_title(content: &str) -> String {
    let preview = content.trim().chars().take(50).collect::<String>();
    if content.trim().chars().count() > 50 {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn analysis_item_title(item: &Value) -> String {
    item.get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            default_analysis_foreshadow_title(
                item.get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
}

async fn sync_foreshadows_from_analysis_payload(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    chapter_number: i32,
    analysis_foreshadows: &[Value],
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now().naive_utc();
    let mut planted_count = 0_i64;
    let mut resolved_count = 0_i64;
    let mut created_ids = Vec::new();
    let mut updated_ids = Vec::new();
    let mut skipped_reasons = Vec::new();

    for item in analysis_foreshadows {
        let item_type = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("planted");

        match item_type {
            "resolved" => {
                let Some(reference_id) = item
                    .get("reference_foreshadow_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    skipped_reasons.push(format!(
                        "skip resolved foreshadow without reference id: {}",
                        analysis_item_title(item)
                    ));
                    continue;
                };

                let Some(existing) = foreshadow::Entity::find_by_id(reference_id)
                    .filter(foreshadow::Column::ProjectId.eq(project_id))
                    .one(db)
                    .await?
                else {
                    skipped_reasons.push(format!(
                        "skip resolved foreshadow without matching record: {}",
                        analysis_item_title(item)
                    ));
                    continue;
                };

                if existing.status == "resolved"
                    && existing.actual_resolve_chapter_number == Some(chapter_number)
                {
                    continue;
                }
                if existing.status == "resolved" {
                    skipped_reasons.push(format!(
                        "skip already resolved foreshadow: {}",
                        existing.title
                    ));
                    continue;
                }
                if existing.status != "planted" {
                    skipped_reasons.push(format!(
                        "skip foreshadow in non-planted state: {} ({})",
                        existing.title, existing.status
                    ));
                    continue;
                }

                let mut active: foreshadow::ActiveModel = existing.into();
                active.status = Set("resolved".to_string());
                active.actual_resolve_chapter_id = Set(Some(chapter_id.to_string()));
                active.actual_resolve_chapter_number = Set(Some(chapter_number));
                active.resolution_text = Set(
                    item.get("content")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string),
                );
                active.resolved_at = Set(Some(now));
                active.updated_at = Set(now);
                let saved = active.update(db).await?;
                resolved_count += 1;
                updated_ids.push(saved.id);
            }
            "planted" => {
                let Some(content) = item
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    skipped_reasons.push("skip planted foreshadow with empty content".to_string());
                    continue;
                };

                let stable_source_memory_id = normalize_analysis_foreshadow_key(chapter_id, content);
                let title = analysis_item_title(item);
                let existing = foreshadow::Entity::find()
                    .filter(foreshadow::Column::ProjectId.eq(project_id))
                    .filter(foreshadow::Column::SourceType.eq("analysis"))
                    .filter(
                        foreshadow::Column::SourceMemoryId
                            .eq(stable_source_memory_id.clone())
                            .or(
                                foreshadow::Column::PlantChapterId
                                    .eq(chapter_id)
                                    .and(foreshadow::Column::Title.eq(title.clone())),
                            ),
                    )
                    .one(db)
                    .await?;

                if let Some(existing) = existing {
                    let mut active: foreshadow::ActiveModel = existing.into();
                    active.title = Set(title.clone());
                    active.content = Set(content.to_string());
                    active.hint_text = Set(
                        item.get("keyword")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToString::to_string),
                    );
                    active.source_memory_id = Set(Some(stable_source_memory_id));
                    active.strength = Set(
                        item.get("strength")
                            .and_then(Value::as_i64)
                            .unwrap_or(5)
                            .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    );
                    active.subtlety = Set(
                        item.get("subtlety")
                            .and_then(Value::as_i64)
                            .unwrap_or(5)
                            .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    );
                    active.category = Set(
                        item.get("category")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToString::to_string),
                    );
                    active.related_characters = Set(item.get("related_characters").cloned());
                    active.is_long_term =
                        Set(item.get("is_long_term").and_then(Value::as_bool).unwrap_or(false));
                    active.target_resolve_chapter_number = Set(
                        item.get("estimated_resolve_chapter")
                            .and_then(Value::as_i64)
                            .map(|value| value.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
                    );
                    active.updated_at = Set(now);
                    let saved = active.update(db).await?;
                    updated_ids.push(saved.id);
                } else {
                    let id = Uuid::new_v4().to_string();
                    let saved = foreshadow::ActiveModel {
                        id: Set(id.clone()),
                        project_id: Set(project_id.to_string()),
                        title: Set(title),
                        content: Set(content.to_string()),
                        hint_text: Set(
                            item.get("keyword")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(ToString::to_string),
                        ),
                        resolution_text: Set(None),
                        source_type: Set("analysis".to_string()),
                        source_memory_id: Set(Some(stable_source_memory_id)),
                        source_analysis_id: Set(None),
                        plant_chapter_id: Set(Some(chapter_id.to_string())),
                        plant_chapter_number: Set(Some(chapter_number)),
                        target_resolve_chapter_id: Set(None),
                        target_resolve_chapter_number: Set(
                            item.get("estimated_resolve_chapter")
                                .and_then(Value::as_i64)
                                .map(|value| value.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
                        ),
                        actual_resolve_chapter_id: Set(None),
                        actual_resolve_chapter_number: Set(None),
                        status: Set("planted".to_string()),
                        is_long_term: Set(
                            item.get("is_long_term").and_then(Value::as_bool).unwrap_or(false),
                        ),
                        importance: Set(
                            (item.get("strength").and_then(Value::as_f64).unwrap_or(5.0) / 10.0)
                                .min(1.0),
                        ),
                        strength: Set(
                            item.get("strength")
                                .and_then(Value::as_i64)
                                .unwrap_or(5)
                                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                        ),
                        subtlety: Set(
                            item.get("subtlety")
                                .and_then(Value::as_i64)
                                .unwrap_or(5)
                                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                        ),
                        urgency: Set(0),
                        related_characters: Set(item.get("related_characters").cloned()),
                        related_foreshadow_ids: Set(None),
                        tags: Set(None),
                        category: Set(
                            item.get("category")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(ToString::to_string),
                        ),
                        notes: Set(None),
                        resolution_notes: Set(None),
                        auto_remind: Set(true),
                        remind_before_chapters: Set(5),
                        include_in_context: Set(true),
                        created_at: Set(now),
                        updated_at: Set(now),
                        planted_at: Set(Some(now)),
                        resolved_at: Set(None),
                    }
                    .insert(db)
                    .await?;
                    planted_count += 1;
                    created_ids.push(saved.id);
                }
            }
            _ => {
                skipped_reasons.push(format!(
                    "skip unknown foreshadow type: {}",
                    analysis_item_title(item)
                ));
            }
        }
    }

    Ok(json!({
        "synced_count": planted_count + resolved_count,
        "planted_count": planted_count,
        "resolved_count": resolved_count,
        "created_count": created_ids.len(),
        "updated_ids": updated_ids,
        "created_ids": created_ids,
        "skipped_count": skipped_reasons.len(),
        "skipped_reasons": skipped_reasons,
    }))
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
        let mut query =
            foreshadow::Entity::find().filter(foreshadow::Column::ProjectId.eq(project_id));

        if let Some(s) = status {
            query = query.filter(foreshadow::Column::Status.eq(s));
        }
        if let Some(c) = category {
            query = query.filter(foreshadow::Column::Category.eq(c));
        }
        if let Some(st) = source_type {
            query = query.filter(foreshadow::Column::SourceType.eq(st));
        }
        if let Some(lt) = is_long_term {
            query = query.filter(foreshadow::Column::IsLongTerm.eq(lt));
        }

        let all: Vec<foreshadow::Model> = query
            .clone()
            .order_by_desc(foreshadow::Column::CreatedAt)
            .all(db)
            .await?;

        let stats = compute_stats(&all);

        let limit = limit.unwrap_or(50) as usize;
        let page = page.unwrap_or(1) as usize;
        let skip = (page.saturating_sub(1)) * limit;

        let items: Vec<Value> = all
            .iter()
            .skip(skip)
            .take(limit)
            .map(model_to_value)
            .collect();

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
            let overdue = items
                .iter()
                .filter(|f| {
                    f.target_resolve_chapter_number.map_or(false, |t| t < ch)
                        && (f.status == "planted" || f.status == "pending")
                })
                .count();
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
            all.iter()
                .filter(|f| f.status == "pending")
                .map(model_to_value)
                .collect()
        } else {
            vec![]
        };

        let pending_resolve: Vec<Value> = all
            .iter()
            .filter(|f| {
                f.status == "planted"
                    && f.target_resolve_chapter_number.map_or(false, |t| {
                        t >= chapter_number && t <= chapter_number + lookahead
                    })
            })
            .map(model_to_value)
            .collect();

        let overdue: Vec<Value> = if inc_overdue {
            all.iter()
                .filter(|f| {
                    f.target_resolve_chapter_number
                        .map_or(false, |t| t < chapter_number)
                        && (f.status == "planted" || f.status == "pending")
                })
                .map(model_to_value)
                .collect()
        } else {
            vec![]
        };

        let recently_planted: Vec<Value> = all
            .iter()
            .filter(|f| {
                f.status == "planted"
                    && f.plant_chapter_number.map_or(false, |p| {
                        p >= chapter_number.saturating_sub(3) && p < chapter_number
                    })
            })
            .map(model_to_value)
            .collect();

        let context_parts: Vec<String> = pending_resolve
            .iter()
            .filter_map(|f| {
                Some(format!(
                    "伏笔「{}」(第{}章): {}",
                    f.get("title")?.as_str()?,
                    f.get("target_resolve_chapter_number")?.as_i64()?,
                    f.get("content")?
                        .as_str()?
                        .chars()
                        .take(80)
                        .collect::<String>(),
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

        let items: Vec<Value> = all
            .iter()
            .filter(|f| {
                f.status == "planted"
                    && f.target_resolve_chapter_number.map_or(false, |t| {
                        t >= current_chapter && t <= current_chapter + lookahead
                    })
            })
            .map(model_to_value)
            .collect();

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
        request: &CreateForeshadowRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now().naive_utc();
        let id = Uuid::new_v4().to_string();

        let model = foreshadow::ActiveModel {
            id: Set(id.clone()),
            project_id: Set(request.project_id().to_string()),
            title: Set(request.title().to_string()),
            content: Set(request.content().to_string()),
            hint_text: Set(request.hint_text().map(ToString::to_string)),
            resolution_text: Set(request.resolution_text().map(ToString::to_string)),
            source_type: Set("manual".to_string()),
            source_memory_id: Set(None),
            source_analysis_id: Set(None),
            plant_chapter_id: Set(None),
            plant_chapter_number: Set(request.plant_chapter_number()),
            target_resolve_chapter_id: Set(None),
            target_resolve_chapter_number: Set(request.target_resolve_chapter_number()),
            actual_resolve_chapter_id: Set(None),
            actual_resolve_chapter_number: Set(None),
            status: Set("pending".to_string()),
            is_long_term: Set(request.is_long_term()),
            importance: Set(request.importance()),
            strength: Set(request.strength()),
            subtlety: Set(request.subtlety()),
            urgency: Set(0),
            related_characters: Set(request.related_characters().cloned()),
            related_foreshadow_ids: Set(None),
            tags: Set(request.tags().cloned()),
            category: Set(request.category().map(ToString::to_string)),
            notes: Set(request.notes().map(ToString::to_string)),
            resolution_notes: Set(request.resolution_notes().map(ToString::to_string)),
            auto_remind: Set(request.auto_remind()),
            remind_before_chapters: Set(request.remind_before_chapters()),
            include_in_context: Set(request.include_in_context()),
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
        request: &UpdateForeshadowRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let mut active: foreshadow::ActiveModel = existing.into();
        if let Some(value) = request.title() {
            active.title = Set(value.to_string());
        }
        if let Some(value) = request.content() {
            active.content = Set(value.to_string());
        }
        if let Some(value) = request.hint_text() {
            active.hint_text = Set(Some(value.to_string()));
        }
        if let Some(value) = request.resolution_text() {
            active.resolution_text = Set(Some(value.to_string()));
        }
        if let Some(value) = request.plant_chapter_number() {
            active.plant_chapter_number = Set(Some(value));
        }
        if let Some(value) = request.target_resolve_chapter_number() {
            active.target_resolve_chapter_number = Set(Some(value));
        }
        if let Some(value) = request.status() {
            active.status = Set(value.to_string());
        }
        if let Some(value) = request.is_long_term() {
            active.is_long_term = Set(value);
        }
        if let Some(value) = request.importance() {
            active.importance = Set(value);
        }
        if let Some(value) = request.strength() {
            active.strength = Set(value);
        }
        if let Some(value) = request.subtlety() {
            active.subtlety = Set(value);
        }
        if let Some(value) = request.urgency() {
            active.urgency = Set(value);
        }
        if request.related_characters_present() {
            active.related_characters = Set(request.related_characters().cloned());
        }
        if request.related_foreshadow_ids_present() {
            active.related_foreshadow_ids = Set(request.related_foreshadow_ids().cloned());
        }
        if request.tags_present() {
            active.tags = Set(request.tags().cloned());
        }
        if let Some(value) = request.category() {
            active.category = Set(Some(value.to_string()));
        }
        if let Some(value) = request.notes() {
            active.notes = Set(Some(value.to_string()));
        }
        if let Some(value) = request.resolution_notes() {
            active.resolution_notes = Set(Some(value.to_string()));
        }
        if let Some(value) = request.auto_remind() {
            active.auto_remind = Set(value);
        }
        if let Some(value) = request.remind_before_chapters() {
            active.remind_before_chapters = Set(value);
        }
        if let Some(value) = request.include_in_context() {
            active.include_in_context = Set(value);
        }
        active.updated_at = Set(Utc::now().naive_utc());

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn delete(
        db: &DatabaseConnection,
        foreshadow_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        foreshadow::Entity::delete_by_id(foreshadow_id)
            .exec(db)
            .await?;
        Ok(json!({"message": "伏笔已删除", "id": foreshadow_id}))
    }

    pub async fn plant(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        request: &PlantForeshadowRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let now = Utc::now().naive_utc();
        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set("planted".to_string());
        active.plant_chapter_id = Set(request.chapter_id().map(ToString::to_string));
        active.plant_chapter_number = Set(request.chapter_number());
        if let Some(value) = request.hint_text() {
            active.hint_text = Set(Some(value.to_string()));
        }
        active.planted_at = Set(Some(now));
        active.updated_at = Set(now);

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn resolve(
        db: &DatabaseConnection,
        foreshadow_id: &str,
        request: &ResolveForeshadowRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = foreshadow::Entity::find_by_id(foreshadow_id)
            .one(db)
            .await?
            .ok_or("foreshadow not found")?;

        let now = Utc::now().naive_utc();
        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set(if request.is_partial() {
            "partially_resolved".to_string()
        } else {
            "resolved".to_string()
        });
        active.actual_resolve_chapter_id = Set(request.chapter_id().map(ToString::to_string));
        active.actual_resolve_chapter_number = Set(request.chapter_number());
        if let Some(value) = request.resolution_text() {
            active.resolution_text = Set(Some(value.to_string()));
        }
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

        let now = Utc::now().naive_utc();
        let mut active: foreshadow::ActiveModel = existing.into();
        active.status = Set("abandoned".to_string());
        if let Some(r) = reason {
            active.notes = Set(Some(format!("废弃原因: {}", r)));
        }
        active.updated_at = Set(now);

        let saved = active.update(db).await?;
        Ok(model_to_value(&saved))
    }

    pub async fn sync_from_analysis(
        db: &DatabaseConnection,
        project_id: &str,
        request: &SyncForeshadowFromAnalysisRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let chapter_id = request
            .body()
            .get("chapter_id")
            .and_then(Value::as_str)
            .ok_or("chapter_id is required")?;
        let chapter_number = request
            .body()
            .get("chapter_number")
            .and_then(Value::as_i64)
            .map(|value| value.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            .ok_or("chapter_number is required")?;
        let foreshadows = request
            .body()
            .get("analysis_foreshadows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        sync_foreshadows_from_analysis_payload(
            db,
            project_id,
            chapter_id,
            chapter_number,
            &foreshadows,
        )
        .await
    }
}
