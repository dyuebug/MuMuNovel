use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{prompt_submission, prompt_workshop_item, prompt_workshop_like, writing_style};

const WORKSHOP_SERVER_MODE: &str = "server";

fn is_workshop_server(_cfg: &crate::config::AppConfig) -> bool {
    std::env::var("WORKSHOP_MODE")
        .unwrap_or_else(|_| "client".to_string())
        .to_lowercase()
        == WORKSHOP_SERVER_MODE
}

fn instance_id() -> String {
    std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())
}

fn user_identifier(instance_id: &str, user_id: &str) -> String {
    format!("{}:{}", instance_id, user_id)
}

fn item_to_dict(item: &prompt_workshop_item::Model, is_liked: bool) -> Value {
    json!({
        "id": item.id,
        "name": item.name,
        "description": item.description,
        "prompt_content": item.prompt_content,
        "category": item.category,
        "tags": item.tags.as_ref().and_then(|t| serde_json::from_str::<Vec<String>>(t).ok()),
        "author_name": item.author_name,
        "is_official": item.is_official,
        "download_count": item.download_count,
        "like_count": item.like_count,
        "is_liked": is_liked,
        "created_at": item.created_at.and_utc().to_rfc3339(),
    })
}

fn submission_to_dict(s: &prompt_submission::Model) -> Value {
    json!({
        "id": s.id,
        "name": s.name,
        "description": s.description,
        "prompt_content": s.prompt_content,
        "category": s.category,
        "tags": s.tags.as_ref().and_then(|t| serde_json::from_str::<Vec<String>>(t).ok()),
        "author_display_name": s.author_display_name,
        "is_anonymous": s.is_anonymous,
        "status": s.status,
        "review_note": s.review_note,
        "reviewed_at": s.reviewed_at.map(|t| t.and_utc().to_rfc3339()),
        "created_at": s.created_at.and_utc().to_rfc3339(),
        "source_instance": s.source_instance,
        "submitter_name": s.submitter_name,
    })
}

fn workshop_categories() -> Value {
    json!([
        {"id": "general", "name": "通用", "count": 0},
        {"id": "fantasy", "name": "玄幻/仙侠", "count": 0},
        {"id": "martial", "name": "武侠", "count": 0},
        {"id": "romance", "name": "言情", "count": 0},
        {"id": "scifi", "name": "科幻", "count": 0},
        {"id": "horror", "name": "悬疑/惊悚", "count": 0},
        {"id": "history", "name": "历史", "count": 0},
        {"id": "urban", "name": "都市", "count": 0},
        {"id": "game", "name": "游戏/电竞", "count": 0},
        {"id": "other", "name": "其他", "count": 0},
    ])
}

pub struct PromptWorkshopService;

impl PromptWorkshopService {
    // ==================== 公开 API ====================

    pub async fn get_status(cfg: &crate::config::AppConfig) -> Value {
        let mut result = json!({
            "mode": std::env::var("WORKSHOP_MODE").unwrap_or_else(|_| "client".to_string()),
            "instance_id": instance_id(),
        });
        if !is_workshop_server(cfg) {
            result["cloud_url"] = json!(std::env::var("WORKSHOP_CLOUD_URL")
                .unwrap_or_else(|_| "https://mumuverse.space:1566".to_string()));
            result["cloud_connected"] = json!(null);
        }
        result
    }

    pub async fn get_items(
        db: &DatabaseConnection,
        category: Option<&str>,
        search: Option<&str>,
        _tags: Option<&str>,
        sort: &str,
        page: u64,
        limit: u64,
        user_identifier: Option<&str>,
    ) -> Result<Value, String> {
        use prompt_workshop_item::{Column as C, Entity};

        let mut query = Entity::find().filter(C::Status.eq("active"));
        let mut count_query = Entity::find().filter(C::Status.eq("active"));

        if let Some(c) = category {
            query = query.filter(C::Category.eq(c));
            count_query = count_query.filter(C::Category.eq(c));
        }
        if let Some(s) = search {
            let sf = format!("%{}%", s);
            query = query.filter(C::Name.like(&sf).or(C::Description.like(&sf)));
            count_query = count_query.filter(C::Name.like(&sf).or(C::Description.like(&sf)));
        }

        query = match sort {
            "popular" => query.order_by_desc(C::LikeCount),
            "downloads" => query.order_by_desc(C::DownloadCount),
            _ => query.order_by_desc(C::CreatedAt),
        };

        let total = count_query.count(db).await.map_err(|e| format!("{}", e))?;
        let items = query
            .offset((page.saturating_sub(1) * limit) as u64)
            .limit(limit)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let liked_ids: std::collections::HashSet<String> = if let Some(uid) = user_identifier {
            prompt_workshop_like::Entity::find()
                .filter(prompt_workshop_like::Column::UserIdentifier.eq(uid))
                .all(db)
                .await
                .map(|likes| likes.into_iter().map(|l| l.workshop_item_id).collect())
                .unwrap_or_default()
        } else {
            std::collections::HashSet::new()
        };

        // Category stats
        let mut cat_stats: Vec<Value> = Vec::new();
        let all_active = Entity::find()
            .filter(C::Status.eq("active"))
            .all(db)
            .await
            .unwrap_or_default();
        let mut cat_count: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for item in &all_active {
            *cat_count.entry(item.category.clone()).or_default() += 1;
        }
        for cat in workshop_categories().as_array().unwrap() {
            let cid = cat["id"].as_str().unwrap_or("");
            let count = cat_count.get(cid).copied().unwrap_or(0);
            cat_stats.push(json!({"id": cid, "name": cat["name"], "count": count}));
        }

        Ok(json!({
            "success": true,
            "data": {
                "total": total,
                "page": page,
                "limit": limit,
                "items": items.iter().map(|item| item_to_dict(item, liked_ids.contains(&item.id))).collect::<Vec<_>>(),
                "categories": cat_stats,
            }
        }))
    }

    pub async fn get_item(
        db: &DatabaseConnection,
        item_id: &str,
        _user_identifier: Option<&str>,
    ) -> Result<Option<Value>, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .filter(prompt_workshop_item::Column::Status.eq("active"))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(item) = item else {
            return Ok(None);
        };
        Ok(Some(
            json!({"success": true, "data": item_to_dict(&item, false)}),
        ))
    }

    pub async fn import_item(
        db: &DatabaseConnection,
        item_id: &str,
        custom_name: Option<&str>,
        user_id: &str,
    ) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };

        // Increment download count
        let mut active_item: prompt_workshop_item::ActiveModel = item.clone().into();
        active_item.download_count = Set(item.download_count + 1);
        active_item.update(db).await.map_err(|e| format!("{}", e))?;

        // Count user's existing styles
        let count = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .count(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let new_style = writing_style::ActiveModel {
            user_id: Set(Some(user_id.to_string())),
            name: Set(custom_name.unwrap_or(&item.name).to_string()),
            style_type: Set("custom".to_string()),
            description: Set(Some(format!(
                "从提示词工坊导入: {}",
                item.description.as_deref().unwrap_or("")
            ))),
            prompt_content: Set(item.prompt_content.clone()),
            order_index: Set(count as i32 + 1),
            ..Default::default()
        };

        let inserted = new_style.insert(db).await.map_err(|e| format!("{}", e))?;

        Ok(json!({
            "success": true,
            "message": "导入成功",
            "writing_style": {
                "id": inserted.id,
                "name": inserted.name,
                "style_type": inserted.style_type,
                "prompt_content": inserted.prompt_content,
            }
        }))
    }

    pub async fn toggle_like(
        db: &DatabaseConnection,
        item_id: &str,
        user_identifier: &str,
    ) -> Result<Value, String> {
        let existing = prompt_workshop_like::Entity::find()
            .filter(prompt_workshop_like::Column::UserIdentifier.eq(user_identifier))
            .filter(prompt_workshop_like::Column::WorkshopItemId.eq(item_id))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };

        let mut active_item: prompt_workshop_item::ActiveModel = item.into();
        let liked: bool;

        if let Some(like) = existing {
            prompt_workshop_like::Entity::delete_by_id(&like.id)
                .exec(db)
                .await
                .map_err(|e| format!("{}", e))?;
            active_item.like_count = Set(std::cmp::max(0, active_item.like_count.unwrap() - 1));
            liked = false;
        } else {
            prompt_workshop_like::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                user_identifier: Set(user_identifier.to_string()),
                workshop_item_id: Set(item_id.to_string()),
                created_at: Set(Utc::now().naive_utc()),
            }
            .insert(db)
            .await
            .map_err(|e| format!("{}", e))?;
            active_item.like_count = Set(active_item.like_count.unwrap() + 1);
            liked = true;
        }

        let updated = active_item.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({"success": true, "liked": liked, "like_count": updated.like_count}))
    }

    pub async fn record_download(db: &DatabaseConnection, item_id: &str) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };
        let mut active: prompt_workshop_item::ActiveModel = item.into();
        let new_count = active.download_count.unwrap() + 1;
        active.download_count = Set(new_count);
        active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({"success": true, "download_count": new_count}))
    }

    pub async fn submit_prompt(
        db: &DatabaseConnection,
        user_identifier: &str,
        submitter_name: &str,
        name: &str,
        description: Option<&str>,
        prompt_content: &str,
        category: &str,
        tags: Option<&str>,
        author_display_name: Option<&str>,
        is_anonymous: bool,
        source_instance: &str,
    ) -> Result<Value, String> {
        let now = Utc::now().naive_utc();
        let submission = prompt_submission::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            submitter_id: Set(user_identifier.to_string()),
            submitter_name: Set(Some(submitter_name.to_string())),
            source_instance: Set(source_instance.to_string()),
            name: Set(name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            prompt_content: Set(prompt_content.to_string()),
            category: Set(category.to_string()),
            tags: Set(tags.map(|s| s.to_string())),
            author_display_name: Set(Some(
                author_display_name.unwrap_or(submitter_name).to_string(),
            )),
            is_anonymous: Set(is_anonymous),
            status: Set("pending".to_string()),
            reviewer_id: Set(None),
            review_note: Set(None),
            reviewed_at: Set(None),
            workshop_item_id: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        let inserted = submission.insert(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({
            "success": true,
            "message": "提交成功，等待管理员审核",
            "submission": {
                "id": inserted.id,
                "status": inserted.status,
                "created_at": inserted.created_at.and_utc().to_rfc3339(),
            }
        }))
    }

    pub async fn get_my_submissions(
        db: &DatabaseConnection,
        user_identifier: &str,
        status_filter: Option<&str>,
    ) -> Result<Value, String> {
        let mut query = prompt_submission::Entity::find()
            .filter(prompt_submission::Column::SubmitterId.eq(user_identifier));
        if let Some(s) = status_filter {
            query = query.filter(prompt_submission::Column::Status.eq(s));
        }
        query = query.order_by_desc(prompt_submission::Column::CreatedAt);
        let submissions = query.all(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({
            "success": true,
            "data": {
                "total": submissions.len(),
                "items": submissions.iter().map(|s| submission_to_dict(s)).collect::<Vec<_>>(),
            }
        }))
    }

    pub async fn withdraw_submission(
        db: &DatabaseConnection,
        submission_id: &str,
        user_identifier: &str,
        force: bool,
    ) -> Result<Value, String> {
        let submission = prompt_submission::Entity::find()
            .filter(prompt_submission::Column::Id.eq(submission_id))
            .filter(prompt_submission::Column::SubmitterId.eq(user_identifier))
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(submission) = submission else {
            return Err("提交记录不存在".to_string());
        };
        if submission.status != "pending" && !force {
            return Err("只能撤回待审核的提交，删除已审核记录请使用 force 参数".to_string());
        }
        prompt_submission::Entity::delete_by_id(&submission.id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(if submission.status == "pending" {
            json!({"success": true, "message": "撤回成功"})
        } else {
            json!({"success": true, "message": "删除成功"})
        })
    }

    // ==================== 管理员 API ====================

    pub async fn admin_get_submissions(
        db: &DatabaseConnection,
        status_filter: Option<&str>,
        source: Option<&str>,
        page: u64,
        limit: u64,
    ) -> Result<Value, String> {
        use prompt_submission::{Column as C, Entity};

        let mut query = Entity::find();
        let mut count_query = Entity::find();

        if let Some(s) = status_filter {
            if s != "all" {
                query = query.filter(C::Status.eq(s));
                count_query = count_query.filter(C::Status.eq(s));
            }
        }
        if let Some(src) = source {
            query = query.filter(C::SourceInstance.eq(src));
            count_query = count_query.filter(C::SourceInstance.eq(src));
        }

        let total = count_query.count(db).await.map_err(|e| format!("{}", e))?;
        let pending_count = Entity::find()
            .filter(C::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|e| format!("{}", e))?;

        let submissions = query
            .order_by_desc(C::CreatedAt)
            .offset((page.saturating_sub(1) * limit) as u64)
            .limit(limit)
            .all(db)
            .await
            .map_err(|e| format!("{}", e))?;

        Ok(json!({
            "success": true,
            "data": {
                "total": total,
                "pending_count": pending_count,
                "page": page,
                "limit": limit,
                "items": submissions.iter().map(|s| submission_to_dict(s)).collect::<Vec<_>>(),
            }
        }))
    }

    pub async fn admin_review_submission(
        db: &DatabaseConnection,
        submission_id: &str,
        action: &str,
        review_note: Option<&str>,
        category: Option<&str>,
        tags: Option<&str>,
        reviewer_id: &str,
    ) -> Result<Value, String> {
        let submission = prompt_submission::Entity::find_by_id(submission_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(submission) = submission else {
            return Err("提交记录不存在".to_string());
        };
        if submission.status != "pending" {
            return Err("该提交已被审核".to_string());
        }

        let now = Utc::now().naive_utc();
        let mut active_sub: prompt_submission::ActiveModel = submission.clone().into();

        if action == "approve" {
            let new_item = prompt_workshop_item::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                name: Set(submission.name.clone()),
                description: Set(submission.description.clone()),
                prompt_content: Set(submission.prompt_content.clone()),
                category: Set(category.unwrap_or(&submission.category).to_string()),
                tags: Set(tags.map(|s| s.to_string()).or(submission.tags.clone())),
                author_id: Set(if submission.is_anonymous {
                    None
                } else {
                    Some(submission.submitter_id.clone())
                }),
                author_name: Set(if submission.is_anonymous {
                    None
                } else {
                    submission.author_display_name.clone()
                }),
                source_instance: Set(Some(submission.source_instance.clone())),
                is_official: Set(false),
                download_count: Set(0),
                like_count: Set(0),
                status: Set("active".to_string()),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            };
            let inserted = new_item.insert(db).await.map_err(|e| format!("{}", e))?;

            active_sub.status = Set("approved".to_string());
            active_sub.workshop_item_id = Set(Some(inserted.id.clone()));
            active_sub.reviewer_id = Set(Some(reviewer_id.to_string()));
            active_sub.review_note = Set(review_note.map(|s| s.to_string()));
            active_sub.reviewed_at = Set(Some(now));
            active_sub.updated_at = Set(Some(now));
            active_sub.update(db).await.map_err(|e| format!("{}", e))?;

            Ok(json!({
                "success": true,
                "message": "已通过审核并发布",
                "workshop_item": item_to_dict(&inserted, false),
            }))
        } else {
            active_sub.status = Set("rejected".to_string());
            active_sub.reviewer_id = Set(Some(reviewer_id.to_string()));
            active_sub.review_note = Set(review_note.map(|s| s.to_string()));
            active_sub.reviewed_at = Set(Some(now));
            active_sub.updated_at = Set(Some(now));
            let updated = active_sub.update(db).await.map_err(|e| format!("{}", e))?;
            Ok(json!({
                "success": true,
                "message": "已拒绝",
                "submission": submission_to_dict(&updated),
            }))
        }
    }

    pub async fn admin_create_item(
        db: &DatabaseConnection,
        name: &str,
        description: Option<&str>,
        prompt_content: &str,
        category: &str,
        tags: Option<&str>,
    ) -> Result<Value, String> {
        let now = Utc::now().naive_utc();
        let item = prompt_workshop_item::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            prompt_content: Set(prompt_content.to_string()),
            category: Set(category.to_string()),
            tags: Set(tags.map(|s| s.to_string())),
            author_id: Set(None),
            author_name: Set(Some("官方".to_string())),
            source_instance: Set(None),
            is_official: Set(true),
            download_count: Set(0),
            like_count: Set(0),
            status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        let inserted = item.insert(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({"success": true, "item": item_to_dict(&inserted, false)}))
    }

    pub async fn admin_update_item(
        db: &DatabaseConnection,
        item_id: &str,
        updates: Value,
    ) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };
        let mut active: prompt_workshop_item::ActiveModel = item.into();

        if let Some(v) = updates.get("name").and_then(|v| v.as_str()) {
            active.name = Set(v.to_string());
        }
        if let Some(v) = updates.get("description").and_then(|v| v.as_str()) {
            active.description = Set(Some(v.to_string()));
        }
        if let Some(v) = updates.get("prompt_content").and_then(|v| v.as_str()) {
            active.prompt_content = Set(v.to_string());
        }
        if let Some(v) = updates.get("category").and_then(|v| v.as_str()) {
            active.category = Set(v.to_string());
        }
        if let Some(v) = updates.get("tags") {
            active.tags = Set(Some(v.to_string()));
        }
        if let Some(v) = updates.get("status").and_then(|v| v.as_str()) {
            active.status = Set(v.to_string());
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));

        let updated = active.update(db).await.map_err(|e| format!("{}", e))?;
        Ok(json!({"success": true, "item": item_to_dict(&updated, false)}))
    }

    pub async fn admin_delete_item(
        db: &DatabaseConnection,
        item_id: &str,
    ) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| format!("{}", e))?;
        if item.is_none() {
            return Err("提示词不存在".to_string());
        }
        prompt_workshop_item::Entity::delete_by_id(item_id)
            .exec(db)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(json!({"success": true, "message": "删除成功"}))
    }

    pub async fn admin_get_stats(db: &DatabaseConnection) -> Result<Value, String> {
        use prompt_workshop_item::Column as C;
        use prompt_workshop_item::Entity;

        let total_items = Entity::find()
            .filter(C::Status.eq("active"))
            .count(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let total_official = Entity::find()
            .filter(C::Status.eq("active"))
            .filter(C::IsOfficial.eq(true))
            .count(db)
            .await
            .map_err(|e| format!("{}", e))?;
        let total_pending = prompt_submission::Entity::find()
            .filter(prompt_submission::Column::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|e| format!("{}", e))?;

        // Sum of download_count and like_count
        let all_items = Entity::find().all(db).await.map_err(|e| format!("{}", e))?;
        let total_downloads: i64 = all_items.iter().map(|i| i.download_count as i64).sum();
        let total_likes: i64 = all_items.iter().map(|i| i.like_count as i64).sum();

        Ok(json!({
            "success": true,
            "data": {
                "total_items": total_items,
                "total_official": total_official,
                "total_pending": total_pending,
                "total_downloads": total_downloads,
                "total_likes": total_likes,
            }
        }))
    }

    // ==================== Helper ====================

    pub fn check_workshop_server(cfg: &crate::config::AppConfig) -> bool {
        is_workshop_server(cfg)
    }

    pub fn get_user_identifier(user_id: &str) -> String {
        user_identifier(&instance_id(), user_id)
    }
}
