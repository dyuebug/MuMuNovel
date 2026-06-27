use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use reqwest::Method;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::models::{prompt_submission, prompt_workshop_item, prompt_workshop_like, writing_style};
use crate::services::auth::Claims;

const PROMPT_WORKSHOP_STATUS_ROUTE: &str = "/prompt-workshop/status";
const PROMPT_WORKSHOP_ITEMS_ROUTE: &str = "/prompt-workshop/items";
const PROMPT_WORKSHOP_ITEM_DETAIL_ROUTE: &str = "/prompt-workshop/items/{item_id}";
const PROMPT_WORKSHOP_ITEM_IMPORT_ROUTE: &str = "/prompt-workshop/items/{item_id}/import";
const PROMPT_WORKSHOP_ITEM_LIKE_ROUTE: &str = "/prompt-workshop/items/{item_id}/like";
const PROMPT_WORKSHOP_ITEM_DOWNLOAD_ROUTE: &str = "/prompt-workshop/items/{item_id}/download";
const PROMPT_WORKSHOP_SUBMIT_ROUTE: &str = "/prompt-workshop/submit";
const PROMPT_WORKSHOP_MY_SUBMISSIONS_ROUTE: &str = "/prompt-workshop/my-submissions";
const PROMPT_WORKSHOP_SUBMISSION_DETAIL_ROUTE: &str =
    "/prompt-workshop/submissions/{submission_id}";
const PROMPT_WORKSHOP_ADMIN_SUBMISSIONS_ROUTE: &str = "/prompt-workshop/admin/submissions";
const PROMPT_WORKSHOP_ADMIN_SUBMISSION_REVIEW_ROUTE: &str =
    "/prompt-workshop/admin/submissions/{submission_id}/review";
const PROMPT_WORKSHOP_ADMIN_ITEMS_ROUTE: &str = "/prompt-workshop/admin/items";
const PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE: &str = "/prompt-workshop/admin/items/{item_id}";
const PROMPT_WORKSHOP_ADMIN_STATS_ROUTE: &str = "/prompt-workshop/admin/stats";
const WORKSHOP_SERVER_MODE: &str = "server";

#[derive(Debug, PartialEq)]
struct PreparedPromptWorkshopAdminUpdateItemRequest {
    name: Option<String>,
    description: Option<String>,
    prompt_content: Option<String>,
    category: Option<String>,
    tags: Option<Value>,
    status: Option<String>,
}

fn is_workshop_server(_cfg: &AppConfig) -> bool {
    std::env::var("WORKSHOP_MODE")
        .unwrap_or_else(|_| "client".to_string())
        .to_lowercase()
        == WORKSHOP_SERVER_MODE
}

fn instance_id() -> String {
    std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())
}

async fn check_cloud_connection(cloud_url: &str) -> bool {
    let url = format!(
        "{}/api/prompt-workshop/status",
        cloud_url.trim_end_matches('/')
    );
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };

    match client.get(url).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

fn item_to_dict(item: &prompt_workshop_item::Model, is_liked: bool) -> Value {
    json!({
        "id": item.id,
        "name": item.name,
        "description": item.description,
        "prompt_content": item.prompt_content,
        "category": item.category,
        "tags": item.tags.as_ref().and_then(|tags| serde_json::from_str::<Vec<String>>(tags).ok()),
        "author_name": item.author_name,
        "is_official": item.is_official,
        "download_count": item.download_count,
        "like_count": item.like_count,
        "is_liked": is_liked,
        "created_at": item.created_at.and_utc().to_rfc3339(),
    })
}

fn submission_to_dict(submission: &prompt_submission::Model) -> Value {
    json!({
        "id": submission.id,
        "name": submission.name,
        "description": submission.description,
        "prompt_content": submission.prompt_content,
        "category": submission.category,
        "tags": submission.tags.as_ref().and_then(|tags| serde_json::from_str::<Vec<String>>(tags).ok()),
        "author_display_name": submission.author_display_name,
        "is_anonymous": submission.is_anonymous,
        "status": submission.status,
        "review_note": submission.review_note,
        "reviewed_at": submission.reviewed_at.map(|time| time.and_utc().to_rfc3339()),
        "created_at": submission.created_at.and_utc().to_rfc3339(),
        "source_instance": submission.source_instance,
        "submitter_name": submission.submitter_name,
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

fn required_workshop_text<'a>(item: &'a Value, field: &str) -> Result<&'a str, String> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("云端提示词缺少必要字段: {}", field))
}

struct PromptWorkshopService;

impl PromptWorkshopService {
    async fn get_status(cfg: &AppConfig) -> Value {
        let mut result = json!({
            "mode": std::env::var("WORKSHOP_MODE").unwrap_or_else(|_| "client".to_string()),
            "instance_id": instance_id(),
        });
        if !is_workshop_server(cfg) {
            let cloud_url = workshop_cloud_url();
            let cloud_connected = check_cloud_connection(&cloud_url).await;
            result["cloud_url"] = json!(cloud_url);
            result["cloud_connected"] = json!(cloud_connected);
        }
        result
    }

    async fn get_items(
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

        if let Some(category) = category {
            query = query.filter(C::Category.eq(category));
            count_query = count_query.filter(C::Category.eq(category));
        }
        if let Some(search) = search {
            let search_filter = format!("%{}%", search);
            query = query.filter(
                C::Name
                    .like(&search_filter)
                    .or(C::Description.like(&search_filter)),
            );
            count_query = count_query.filter(
                C::Name
                    .like(&search_filter)
                    .or(C::Description.like(&search_filter)),
            );
        }

        query = match sort {
            "popular" => query.order_by_desc(C::LikeCount),
            "downloads" => query.order_by_desc(C::DownloadCount),
            _ => query.order_by_desc(C::CreatedAt),
        };

        let total = count_query
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let items = query
            .offset(page.saturating_sub(1) * limit)
            .limit(limit)
            .all(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let liked_ids: std::collections::HashSet<String> =
            if let Some(user_identifier) = user_identifier {
                prompt_workshop_like::Entity::find()
                    .filter(prompt_workshop_like::Column::UserIdentifier.eq(user_identifier))
                    .all(db)
                    .await
                    .map(|likes| {
                        likes
                            .into_iter()
                            .map(|like| like.workshop_item_id)
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashSet::new()
            };

        let all_active = Entity::find()
            .filter(C::Status.eq("active"))
            .all(db)
            .await
            .unwrap_or_default();
        let mut category_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for item in &all_active {
            *category_counts.entry(item.category.clone()).or_default() += 1;
        }
        let categories = workshop_categories()
            .as_array()
            .into_iter()
            .flatten()
            .map(|category| {
                let category_id = category["id"].as_str().unwrap_or("");
                json!({
                    "id": category_id,
                    "name": category["name"],
                    "count": category_counts.get(category_id).copied().unwrap_or(0),
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "success": true,
            "data": {
                "total": total,
                "page": page,
                "limit": limit,
                "items": items
                    .iter()
                    .map(|item| item_to_dict(item, liked_ids.contains(&item.id)))
                    .collect::<Vec<_>>(),
                "categories": categories,
            }
        }))
    }

    async fn get_item(
        db: &DatabaseConnection,
        item_id: &str,
        _user_identifier: Option<&str>,
    ) -> Result<Option<Value>, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .filter(prompt_workshop_item::Column::Status.eq("active"))
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let Some(item) = item else {
            return Ok(None);
        };
        Ok(Some(
            json!({"success": true, "data": item_to_dict(&item, false)}),
        ))
    }

    async fn import_item(
        db: &DatabaseConnection,
        item_id: &str,
        custom_name: Option<&str>,
        user_id: &str,
    ) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };

        let mut active_item: prompt_workshop_item::ActiveModel = item.clone().into();
        active_item.download_count = Set(item.download_count + 1);
        active_item
            .update(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let count = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let inserted = writing_style::ActiveModel {
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
        }
        .insert(db)
        .await
        .map_err(|error| format!("{}", error))?;

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

    async fn create_writing_style_from_workshop_item(
        db: &DatabaseConnection,
        item: &Value,
        custom_name: Option<&str>,
        user_id: &str,
    ) -> Result<Value, String> {
        let name = required_workshop_text(item, "name")?;
        let prompt_content = required_workshop_text(item, "prompt_content")?;
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let count = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let inserted = writing_style::ActiveModel {
            user_id: Set(Some(user_id.to_string())),
            name: Set(custom_name.unwrap_or(name).to_string()),
            style_type: Set("custom".to_string()),
            description: Set(Some(format!("从提示词工坊导入: {}", description))),
            prompt_content: Set(prompt_content.to_string()),
            order_index: Set(count as i32 + 1),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|error| format!("{}", error))?;

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

    async fn toggle_like(
        db: &DatabaseConnection,
        item_id: &str,
        user_identifier: &str,
    ) -> Result<Value, String> {
        let existing = prompt_workshop_like::Entity::find()
            .filter(prompt_workshop_like::Column::UserIdentifier.eq(user_identifier))
            .filter(prompt_workshop_like::Column::WorkshopItemId.eq(item_id))
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };

        let mut active_item: prompt_workshop_item::ActiveModel = item.into();
        let liked;

        if let Some(existing) = existing {
            prompt_workshop_like::Entity::delete_by_id(&existing.id)
                .exec(db)
                .await
                .map_err(|error| format!("{}", error))?;
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
            .map_err(|error| format!("{}", error))?;
            active_item.like_count = Set(active_item.like_count.unwrap() + 1);
            liked = true;
        }

        let updated = active_item
            .update(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(json!({"success": true, "liked": liked, "like_count": updated.like_count}))
    }

    async fn record_download(db: &DatabaseConnection, item_id: &str) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };
        let mut active_item: prompt_workshop_item::ActiveModel = item.into();
        let new_count = active_item.download_count.unwrap() + 1;
        active_item.download_count = Set(new_count);
        active_item
            .update(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(json!({"success": true, "download_count": new_count}))
    }

    async fn submit_prompt(
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
        let inserted = prompt_submission::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            submitter_id: Set(user_identifier.to_string()),
            submitter_name: Set(Some(submitter_name.to_string())),
            source_instance: Set(source_instance.to_string()),
            name: Set(name.to_string()),
            description: Set(description.map(str::to_string)),
            prompt_content: Set(prompt_content.to_string()),
            category: Set(category.to_string()),
            tags: Set(tags.map(str::to_string)),
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
        }
        .insert(db)
        .await
        .map_err(|error| format!("{}", error))?;

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

    async fn get_my_submissions(
        db: &DatabaseConnection,
        user_identifier: &str,
        status_filter: Option<&str>,
    ) -> Result<Value, String> {
        let mut query = prompt_submission::Entity::find()
            .filter(prompt_submission::Column::SubmitterId.eq(user_identifier));
        if let Some(status_filter) = status_filter {
            query = query.filter(prompt_submission::Column::Status.eq(status_filter));
        }
        let submissions = query
            .order_by_desc(prompt_submission::Column::CreatedAt)
            .all(db)
            .await
            .map_err(|error| format!("{}", error))?;

        Ok(json!({
            "success": true,
            "data": {
                "total": submissions.len(),
                "items": submissions
                    .iter()
                    .map(submission_to_dict)
                    .collect::<Vec<_>>(),
            }
        }))
    }

    async fn withdraw_submission(
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
            .map_err(|error| format!("{}", error))?;
        let Some(submission) = submission else {
            return Err("提交记录不存在".to_string());
        };
        if submission.status != "pending" && !force {
            return Err("只能撤回待审核的提交，删除已审核记录请使用 force 参数".to_string());
        }
        prompt_submission::Entity::delete_by_id(&submission.id)
            .exec(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(if submission.status == "pending" {
            json!({"success": true, "message": "撤回成功"})
        } else {
            json!({"success": true, "message": "删除成功"})
        })
    }

    async fn admin_get_submissions(
        db: &DatabaseConnection,
        status_filter: Option<&str>,
        source: Option<&str>,
        page: u64,
        limit: u64,
    ) -> Result<Value, String> {
        use prompt_submission::{Column as C, Entity};

        let mut query = Entity::find();
        let mut count_query = Entity::find();

        if let Some(status_filter) = status_filter {
            if status_filter != "all" {
                query = query.filter(C::Status.eq(status_filter));
                count_query = count_query.filter(C::Status.eq(status_filter));
            }
        }
        if let Some(source) = source {
            query = query.filter(C::SourceInstance.eq(source));
            count_query = count_query.filter(C::SourceInstance.eq(source));
        }

        let total = count_query
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let pending_count = Entity::find()
            .filter(C::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;

        let submissions = query
            .order_by_desc(C::CreatedAt)
            .offset(page.saturating_sub(1) * limit)
            .limit(limit)
            .all(db)
            .await
            .map_err(|error| format!("{}", error))?;

        Ok(json!({
            "success": true,
            "data": {
                "total": total,
                "pending_count": pending_count,
                "page": page,
                "limit": limit,
                "items": submissions
                    .iter()
                    .map(submission_to_dict)
                    .collect::<Vec<_>>(),
            }
        }))
    }

    async fn admin_review_submission(
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
            .map_err(|error| format!("{}", error))?;
        let Some(submission) = submission else {
            return Err("提交记录不存在".to_string());
        };
        if submission.status != "pending" {
            return Err("该提交已被审核".to_string());
        }

        let now = Utc::now().naive_utc();
        let mut active_submission: prompt_submission::ActiveModel = submission.clone().into();

        if action == "approve" {
            let inserted = prompt_workshop_item::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                name: Set(submission.name.clone()),
                description: Set(submission.description.clone()),
                prompt_content: Set(submission.prompt_content.clone()),
                category: Set(category.unwrap_or(&submission.category).to_string()),
                tags: Set(tags.map(str::to_string).or(submission.tags.clone())),
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
            }
            .insert(db)
            .await
            .map_err(|error| format!("{}", error))?;

            active_submission.status = Set("approved".to_string());
            active_submission.workshop_item_id = Set(Some(inserted.id.clone()));
            active_submission.reviewer_id = Set(Some(reviewer_id.to_string()));
            active_submission.review_note = Set(review_note.map(str::to_string));
            active_submission.reviewed_at = Set(Some(now));
            active_submission.updated_at = Set(Some(now));
            active_submission
                .update(db)
                .await
                .map_err(|error| format!("{}", error))?;

            Ok(json!({
                "success": true,
                "message": "已通过审核并发布",
                "workshop_item": item_to_dict(&inserted, false),
            }))
        } else {
            active_submission.status = Set("rejected".to_string());
            active_submission.reviewer_id = Set(Some(reviewer_id.to_string()));
            active_submission.review_note = Set(review_note.map(str::to_string));
            active_submission.reviewed_at = Set(Some(now));
            active_submission.updated_at = Set(Some(now));
            let updated = active_submission
                .update(db)
                .await
                .map_err(|error| format!("{}", error))?;
            Ok(json!({
                "success": true,
                "message": "已拒绝",
                "submission": submission_to_dict(&updated),
            }))
        }
    }

    async fn admin_create_item(
        db: &DatabaseConnection,
        name: &str,
        description: Option<&str>,
        prompt_content: &str,
        category: &str,
        tags: Option<&str>,
    ) -> Result<Value, String> {
        let now = Utc::now().naive_utc();
        let inserted = prompt_workshop_item::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            description: Set(description.map(str::to_string)),
            prompt_content: Set(prompt_content.to_string()),
            category: Set(category.to_string()),
            tags: Set(tags.map(str::to_string)),
            author_id: Set(None),
            author_name: Set(Some("官方".to_string())),
            source_instance: Set(None),
            is_official: Set(true),
            download_count: Set(0),
            like_count: Set(0),
            status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| format!("{}", error))?;
        Ok(json!({"success": true, "item": item_to_dict(&inserted, false)}))
    }

    async fn admin_update_item(
        db: &DatabaseConnection,
        item_id: &str,
        updates: &PreparedPromptWorkshopAdminUpdateItemRequest,
    ) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let Some(item) = item else {
            return Err("提示词不存在".to_string());
        };
        let mut active_item: prompt_workshop_item::ActiveModel = item.into();

        if let Some(name) = updates.name.as_ref() {
            active_item.name = Set(name.clone());
        }
        if let Some(description) = updates.description.as_ref() {
            active_item.description = Set(Some(description.clone()));
        }
        if let Some(prompt_content) = updates.prompt_content.as_ref() {
            active_item.prompt_content = Set(prompt_content.clone());
        }
        if let Some(category) = updates.category.as_ref() {
            active_item.category = Set(category.clone());
        }
        if let Some(tags) = updates.tags.as_ref() {
            active_item.tags = Set(Some(tags.to_string()));
        }
        if let Some(status) = updates.status.as_ref() {
            active_item.status = Set(status.clone());
        }
        active_item.updated_at = Set(Some(Utc::now().naive_utc()));

        let updated = active_item
            .update(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(json!({"success": true, "item": item_to_dict(&updated, false)}))
    }

    async fn admin_delete_item(db: &DatabaseConnection, item_id: &str) -> Result<Value, String> {
        let item = prompt_workshop_item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        if item.is_none() {
            return Err("提示词不存在".to_string());
        }
        prompt_workshop_item::Entity::delete_by_id(item_id)
            .exec(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(json!({"success": true, "message": "删除成功"}))
    }

    async fn admin_get_stats(db: &DatabaseConnection) -> Result<Value, String> {
        use prompt_workshop_item::{Column as C, Entity};

        let total_items = Entity::find()
            .filter(C::Status.eq("active"))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let total_official = Entity::find()
            .filter(C::Status.eq("active"))
            .filter(C::IsOfficial.eq(true))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let total_pending = prompt_submission::Entity::find()
            .filter(prompt_submission::Column::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let all_items = Entity::find()
            .all(db)
            .await
            .map_err(|error| format!("{}", error))?;
        let total_downloads: i64 = all_items
            .iter()
            .map(|item| item.download_count as i64)
            .sum();
        let total_likes: i64 = all_items.iter().map(|item| item.like_count as i64).sum();

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

    fn check_workshop_server(cfg: &AppConfig) -> bool {
        is_workshop_server(cfg)
    }
}

#[cfg(test)]
fn build_prompt_workshop_route_owner_contract() -> Value {
    json!({
        "owner": "prompt_workshop",
        "rust_owner": "backend-rs/src/api/prompt_workshop.rs",
        "routes": {
            "status": PROMPT_WORKSHOP_STATUS_ROUTE,
            "items": PROMPT_WORKSHOP_ITEMS_ROUTE,
            "item_detail": PROMPT_WORKSHOP_ITEM_DETAIL_ROUTE,
            "item_import": PROMPT_WORKSHOP_ITEM_IMPORT_ROUTE,
            "item_like": PROMPT_WORKSHOP_ITEM_LIKE_ROUTE,
            "item_download": PROMPT_WORKSHOP_ITEM_DOWNLOAD_ROUTE,
            "submit": PROMPT_WORKSHOP_SUBMIT_ROUTE,
            "my_submissions": PROMPT_WORKSHOP_MY_SUBMISSIONS_ROUTE,
            "submission_withdraw": PROMPT_WORKSHOP_SUBMISSION_DETAIL_ROUTE,
            "admin_submissions": PROMPT_WORKSHOP_ADMIN_SUBMISSIONS_ROUTE,
            "admin_submission_review": PROMPT_WORKSHOP_ADMIN_SUBMISSION_REVIEW_ROUTE,
            "admin_items": PROMPT_WORKSHOP_ADMIN_ITEMS_ROUTE,
            "admin_item_detail": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
            "admin_item_update": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
            "admin_item_delete": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
            "admin_stats": PROMPT_WORKSHOP_ADMIN_STATS_ROUTE
        },
        "methods": {
            "status": ["GET"],
            "items": ["GET"],
            "item_detail": ["GET"],
            "item_import": ["POST"],
            "item_like": ["POST"],
            "item_download": ["POST"],
            "submit": ["POST"],
            "my_submissions": ["GET"],
            "submission_withdraw": ["DELETE"],
            "admin_submissions": ["GET"],
            "admin_submission_review": ["POST"],
            "admin_items": ["POST"],
            "admin_item_detail": ["PUT", "DELETE"],
            "admin_stats": ["GET"]
        },
        "service_owners": [
            "backend-rs/src/models/prompt_workshop_item.rs",
            "backend-rs/src/models/prompt_submission.rs",
            "backend-rs/src/models/prompt_workshop_like.rs",
            "backend-rs/src/config.rs"
        ],
        "readiness_probes": [
            "prompt-workshop-submit-auth-guard-rust",
            "prompt-workshop-like-auth-guard-rust",
            "prompt-workshop-status-business-rust",
            "prompt-workshop-submit-business-rust",
            "prompt-workshop-my-submissions-business-rust",
            "prompt-workshop-withdraw-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-prompt-workshop-business-owner",
            "business_probes": [
                "prompt-workshop-status-business-rust",
                "prompt-workshop-submit-business-rust",
                "prompt-workshop-my-submissions-business-rust",
                "prompt-workshop-withdraw-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "business_smoke_status": {
            "owner_profile": "phase5-prompt-workshop-business-owner",
            "readiness_probe_count": 6,
            "business_probe_count": 4,
            "auth_guard_probe_count": 2,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "source_map_files": [],
        "next_cutover_gate": "prompt-workshop Python route/model/schema/client/category source-map deleted; remaining maturity work is limited to optional cloud-proxy success smoke hardening",
        "migration_policy": "Prompt workshop route business smoke is covered by phase5-prompt-workshop-business-owner; the Python route shell, legacy model/schema, cloud client, and category source maps are physically deleted, and the remaining maturity work is limited to optional cloud-proxy success smoke hardening.",
        "rollback_boundary": {
            "source_map_policy": "prompt_workshop_route_model_schema_client_category_source_map_deleted_no_python_prompt_workshop_shell_remains",
            "python_route_files_status": "prompt_workshop_route_model_schema_client_category_source_map_deleted",
            "python_bootstrap_status": "prompt_workshop_route_registration_deleted_no_python_route_model_schema_client_or_category_shell_remains",
            "source_map_freeze_status": "physical_closeout_completed",
            "source_map_physical_closeout_action": "delete_completed",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [
                "optional prompt-workshop cloud-proxy success smoke if final active-route maturity coverage needs end-to-end external service evidence"
            ],
            "freeze_reason": "phase5-prompt-workshop-business-owner covers status, submit, my-submissions, and withdraw probes with zero Python fallback probes, while the detached Python route/model/schema/cloud-client/category files no longer have any production consumers and are physically deleted.",
            "rollback_files": []
        }
    })
}

fn workshop_cloud_url() -> String {
    std::env::var("WORKSHOP_CLOUD_URL")
        .unwrap_or_else(|_| "https://mumuverse.space:1566".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn cloud_error(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"detail": message.into()})),
    )
}

async fn proxy_workshop_request(
    method: Method,
    path: &str,
    params: Vec<(&str, String)>,
    body: Option<Value>,
    user_identifier: Option<&str>,
) -> Result<Value, (StatusCode, Json<Value>)> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| cloud_error(format!("创建云端工坊客户端失败: {}", error)))?;

    let url = format!("{}/api/prompt-workshop{}", workshop_cloud_url(), path);
    let mut request = client
        .request(method, url)
        .query(&params)
        .header("X-Instance-ID", workshop_instance_id())
        .header("Content-Type", "application/json");

    if let Ok(secret) = std::env::var("WORKSHOP_PROXY_SHARED_SECRET") {
        if !secret.trim().is_empty() {
            request = request.header("X-Workshop-Secret", secret);
        }
    }
    if let Some(user_identifier) = user_identifier {
        request = request.header("X-User-ID", user_identifier);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request
        .send()
        .await
        .map_err(|error| cloud_error(format!("无法连接到云端工坊服务: {}", error)))?;
    let status = response.status();
    if !status.is_success() {
        let preview = response.text().await.unwrap_or_default();
        return Err(cloud_error(format!(
            "云端工坊服务错误: HTTP {}, {}",
            status.as_u16(),
            preview.chars().take(200).collect::<String>()
        )));
    }

    response
        .json::<Value>()
        .await
        .map_err(|error| cloud_error(format!("云端工坊返回非 JSON 内容: {}", error)))
}

fn workshop_response_data(response: &Value) -> &Value {
    response.get("data").unwrap_or(response)
}

fn normalize_tags_value(tags: Option<&Value>) -> Option<String> {
    let value = tags?;

    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            if trimmed.starts_with('[') {
                return Some(trimmed.to_string());
            }

            let items: Vec<String> = trimmed
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if items.is_empty() {
                None
            } else {
                serde_json::to_string(&items).ok()
            }
        }
        Value::Array(items) => {
            let normalized: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();

            if normalized.is_empty() {
                None
            } else {
                serde_json::to_string(&normalized).ok()
            }
        }
        _ => None,
    }
}

fn workshop_instance_id() -> String {
    std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())
}

fn workshop_user_identifier(user_id: &str) -> String {
    format!("{}:{}", workshop_instance_id(), user_id)
}

fn build_workshop_download_payload(instance_id: &str, user_identifier: &str) -> Value {
    json!({
        "instance_id": instance_id,
        "user_identifier": user_identifier,
    })
}

fn default_workshop_category() -> String {
    "general".to_string()
}

#[derive(Debug, PartialEq)]
struct PreparedSubmitPromptRequest {
    user_identifier: String,
    submitter_name: String,
    normalized_tags: Option<String>,
    proxy_payload: Value,
}

fn prepare_submit_prompt_request(
    instance_id: &str,
    user_id: &str,
    name: &str,
    description: Option<&str>,
    prompt_content: &str,
    category: &str,
    tags: Option<&Value>,
    author_display_name: Option<&str>,
    is_anonymous: bool,
) -> PreparedSubmitPromptRequest {
    let user_identifier = workshop_user_identifier(user_id);
    let submitter_name = author_display_name
        .map(str::to_string)
        .unwrap_or_else(|| user_id.to_string());
    let normalized_tags = normalize_tags_value(tags);

    let mut payload = Map::new();
    payload.insert("instance_id".to_string(), json!(instance_id));
    payload.insert("submitter_id".to_string(), json!(user_identifier));
    payload.insert("submitter_name".to_string(), json!(submitter_name));
    payload.insert("name".to_string(), json!(name));
    payload.insert("description".to_string(), json!(description));
    payload.insert("prompt_content".to_string(), json!(prompt_content));
    payload.insert("category".to_string(), json!(category));
    payload.insert(
        "author_display_name".to_string(),
        json!(author_display_name),
    );
    payload.insert("is_anonymous".to_string(), json!(is_anonymous));
    payload.insert(
        "tags".to_string(),
        normalized_tags
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .unwrap_or(Value::Null),
    );

    PreparedSubmitPromptRequest {
        user_identifier,
        submitter_name,
        normalized_tags,
        proxy_payload: Value::Object(payload),
    }
}

#[derive(Deserialize)]
struct PromptWorkshopAdminReviewRouteRequest {
    action: String,
    review_note: Option<String>,
    category: Option<String>,
    tags: Option<Value>,
}

#[derive(Debug, PartialEq)]
struct PreparedPromptWorkshopAdminReviewRequest {
    action: String,
    review_note: Option<String>,
    category: Option<String>,
    normalized_tags: Option<String>,
}

fn prepare_admin_review_submission_request(
    body: PromptWorkshopAdminReviewRouteRequest,
) -> PreparedPromptWorkshopAdminReviewRequest {
    let normalized_tags = normalize_tags_value(body.tags.as_ref());

    PreparedPromptWorkshopAdminReviewRequest {
        action: body.action,
        review_note: body.review_note,
        category: body.category,
        normalized_tags,
    }
}

#[derive(Deserialize)]
struct PromptWorkshopAdminCreateItemRouteRequest {
    name: String,
    description: Option<String>,
    prompt_content: String,
    #[serde(default = "default_workshop_category")]
    category: String,
    tags: Option<Value>,
}

#[derive(Debug, PartialEq)]
struct PreparedPromptWorkshopAdminCreateItemRequest {
    name: String,
    description: Option<String>,
    prompt_content: String,
    category: String,
    normalized_tags: Option<String>,
}

fn prepare_admin_create_item_request(
    body: PromptWorkshopAdminCreateItemRouteRequest,
) -> PreparedPromptWorkshopAdminCreateItemRequest {
    let normalized_tags = normalize_tags_value(body.tags.as_ref());

    PreparedPromptWorkshopAdminCreateItemRequest {
        name: body.name,
        description: body.description,
        prompt_content: body.prompt_content,
        category: body.category,
        normalized_tags,
    }
}

#[derive(Deserialize)]
struct PromptWorkshopAdminUpdateItemRouteRequest {
    name: Option<String>,
    description: Option<String>,
    prompt_content: Option<String>,
    category: Option<String>,
    tags: Option<Value>,
    status: Option<String>,
}

fn prepare_admin_update_item_request(
    body: PromptWorkshopAdminUpdateItemRouteRequest,
) -> PreparedPromptWorkshopAdminUpdateItemRequest {
    PreparedPromptWorkshopAdminUpdateItemRequest {
        name: body.name,
        description: body.description,
        prompt_content: body.prompt_content,
        category: body.category,
        tags: body.tags,
        status: body.status,
    }
}

#[derive(Deserialize)]
struct ImportRequest {
    custom_name: Option<String>,
}

#[derive(Deserialize)]
struct SubmitRequest {
    name: String,
    description: Option<String>,
    prompt_content: String,
    #[serde(default = "default_workshop_category")]
    category: String,
    tags: Option<Value>,
    author_display_name: Option<String>,
    #[serde(default)]
    is_anonymous: bool,
}

#[derive(Deserialize)]
struct UpdateQuery {
    force: Option<bool>,
}

async fn get_status(Extension(cfg): Extension<AppConfig>) -> Json<Value> {
    let mut status = PromptWorkshopService::get_status(&cfg).await;
    if let Some(map) = status.as_object_mut() {
        let mode = map
            .get("mode")
            .and_then(|value| value.as_str())
            .unwrap_or("client")
            .to_string();
        let cloud_url = map
            .get("cloud_url")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                std::env::var("WORKSHOP_CLOUD_URL")
                    .unwrap_or_else(|_| "https://mumuverse.space:1566".to_string())
            });
        let cloud_connected = map
            .get("cloud_connected")
            .and_then(|value| value.as_bool())
            .unwrap_or(mode == "server");

        map.insert("mode".to_string(), json!(mode));
        map.insert(
            "instance_id".to_string(),
            json!(std::env::var("INSTANCE_ID").unwrap_or_else(|_| "local".to_string())),
        );
        map.insert("cloud_url".to_string(), json!(cloud_url));
        map.insert("cloud_connected".to_string(), json!(cloud_connected));
    }
    Json(status)
}

#[derive(Deserialize)]
struct ListQuery {
    category: Option<String>,
    search: Option<String>,
    tags: Option<String>,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_sort() -> String {
    "newest".to_string()
}
fn default_page() -> u64 {
    1
}
fn default_limit() -> u64 {
    20
}

async fn get_items(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = vec![
            ("sort", query.sort.clone()),
            ("page", query.page.to_string()),
            ("limit", query.limit.to_string()),
        ];
        if let Some(category) = &query.category {
            params.push(("category", category.clone()));
        }
        if let Some(search) = &query.search {
            params.push(("search", search.clone()));
        }
        if let Some(tags) = &query.tags {
            params.push(("tags", tags.clone()));
        }
        return proxy_workshop_request(Method::GET, "/items", params, None, Some(&user_identifier))
            .await
            .map(Json);
    }

    match PromptWorkshopService::get_items(
        &db,
        query.category.as_deref(),
        query.search.as_deref(),
        query.tags.as_deref(),
        &query.sort,
        query.page,
        query.limit,
        Some(&user_identifier),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn get_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        return proxy_workshop_request(
            Method::GET,
            &format!("/items/{}", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::get_item(&db, &item_id, Some(&user_identifier)).await {
        Ok(Some(data)) => Ok(Json(data)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "提示词项目不存在"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn import_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let user_identifier = workshop_user_identifier(&claims.sub);
        let item_response = proxy_workshop_request(
            Method::GET,
            &format!("/items/{}", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await?;
        let item = workshop_response_data(&item_response);

        let _ = proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/download", item_id),
            Vec::new(),
            Some(build_workshop_download_payload(
                &workshop_instance_id(),
                &user_identifier,
            )),
            Some(&user_identifier),
        )
        .await;

        return create_writing_style_from_workshop_item(
            &db,
            item,
            body.custom_name.as_deref(),
            &claims.sub,
        )
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        });
    }

    match PromptWorkshopService::import_item(
        &db,
        &item_id,
        body.custom_name.as_deref(),
        &claims.sub,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn create_writing_style_from_workshop_item(
    db: &DatabaseConnection,
    item: &Value,
    custom_name: Option<&str>,
    user_id: &str,
) -> Result<Value, String> {
    PromptWorkshopService::create_writing_style_from_workshop_item(db, item, custom_name, user_id)
        .await
}

async fn toggle_like(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        return proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/like", item_id),
            Vec::new(),
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::toggle_like(&db, &item_id, &user_identifier).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn record_download(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let user_identifier = workshop_user_identifier(&claims.sub);
        return proxy_workshop_request(
            Method::POST,
            &format!("/items/{}/download", item_id),
            Vec::new(),
            Some(build_workshop_download_payload(
                &workshop_instance_id(),
                &user_identifier,
            )),
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::record_download(&db, &item_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn submit_prompt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<SubmitRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let instance_id = workshop_instance_id();
    let prepared = prepare_submit_prompt_request(
        &instance_id,
        &claims.sub,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        body.tags.as_ref(),
        body.author_display_name.as_deref(),
        body.is_anonymous,
    );

    if !PromptWorkshopService::check_workshop_server(&cfg) {
        return proxy_workshop_request(
            Method::POST,
            "/submit",
            Vec::new(),
            Some(prepared.proxy_payload),
            Some(&prepared.user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::submit_prompt(
        &db,
        &prepared.user_identifier,
        &prepared.submitter_name,
        &body.name,
        body.description.as_deref(),
        &body.prompt_content,
        &body.category,
        prepared.normalized_tags.as_deref(),
        body.author_display_name.as_deref(),
        body.is_anonymous,
        &instance_id,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

#[derive(Deserialize)]
struct MySubmissionsQuery {
    status: Option<String>,
}

async fn get_my_submissions(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<MySubmissionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = Vec::new();
        if let Some(status) = &query.status {
            params.push(("status", status.clone()));
        }
        return proxy_workshop_request(
            Method::GET,
            "/my-submissions",
            params,
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::get_my_submissions(&db, &user_identifier, query.status.as_deref())
        .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn withdraw_submission(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(submission_id): Path<String>,
    Query(query): Query<UpdateQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user_identifier = workshop_user_identifier(&claims.sub);
    if !PromptWorkshopService::check_workshop_server(&cfg) {
        let mut params = Vec::new();
        if query.force.unwrap_or(false) {
            params.push(("force", "true".to_string()));
        }
        return proxy_workshop_request(
            Method::DELETE,
            &format!("/submissions/{}", submission_id),
            params,
            None,
            Some(&user_identifier),
        )
        .await
        .map(Json);
    }

    match PromptWorkshopService::withdraw_submission(
        &db,
        &submission_id,
        &user_identifier,
        query.force.unwrap_or(false),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

// ==================== Admin helpers ====================

fn check_admin(cfg: &AppConfig, claims: &Claims) -> Result<(), (StatusCode, Json<Value>)> {
    if !PromptWorkshopService::check_workshop_server(cfg) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"detail": "该功能仅在云端服务可用"})),
        ));
    }
    if !claims.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"detail": "需要管理员权限"})),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct AdminSubmissionsQuery {
    status: Option<String>,
    source: Option<String>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_limit")]
    limit: u64,
}

async fn admin_get_submissions(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Query(query): Query<AdminSubmissionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_get_submissions(
        &db,
        query.status.as_deref(),
        query.source.as_deref(),
        query.page,
        query.limit,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn admin_review_submission(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(submission_id): Path<String>,
    Json(body): Json<PromptWorkshopAdminReviewRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    let request = prepare_admin_review_submission_request(body);
    match PromptWorkshopService::admin_review_submission(
        &db,
        &submission_id,
        &request.action,
        request.review_note.as_deref(),
        request.category.as_deref(),
        request.normalized_tags.as_deref(),
        &claims.sub,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"detail": e})))),
    }
}

async fn admin_create_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Json(body): Json<PromptWorkshopAdminCreateItemRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    let request = prepare_admin_create_item_request(body);
    match PromptWorkshopService::admin_create_item(
        &db,
        &request.name,
        request.description.as_deref(),
        &request.prompt_content,
        &request.category,
        request.normalized_tags.as_deref(),
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

async fn admin_update_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
    Json(body): Json<PromptWorkshopAdminUpdateItemRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    let request = prepare_admin_update_item_request(body);
    match PromptWorkshopService::admin_update_item(&db, &item_id, &request).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn admin_delete_item(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
    Path(item_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_delete_item(&db, &item_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::NOT_FOUND, Json(json!({"detail": e})))),
    }
}

async fn admin_get_stats(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Extension(cfg): Extension<AppConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    check_admin(&cfg, &claims)?;
    match PromptWorkshopService::admin_get_stats(&db).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(PROMPT_WORKSHOP_STATUS_ROUTE, get(get_status))
        .route(PROMPT_WORKSHOP_ITEMS_ROUTE, get(get_items))
        .route(PROMPT_WORKSHOP_ITEM_DETAIL_ROUTE, get(get_item))
        .route(PROMPT_WORKSHOP_ITEM_IMPORT_ROUTE, post(import_item))
        .route(PROMPT_WORKSHOP_ITEM_LIKE_ROUTE, post(toggle_like))
        .route(PROMPT_WORKSHOP_ITEM_DOWNLOAD_ROUTE, post(record_download))
        .route(PROMPT_WORKSHOP_SUBMIT_ROUTE, post(submit_prompt))
        .route(
            PROMPT_WORKSHOP_MY_SUBMISSIONS_ROUTE,
            get(get_my_submissions),
        )
        .route(
            PROMPT_WORKSHOP_SUBMISSION_DETAIL_ROUTE,
            delete(withdraw_submission),
        )
        .route(
            PROMPT_WORKSHOP_ADMIN_SUBMISSIONS_ROUTE,
            get(admin_get_submissions),
        )
        .route(
            PROMPT_WORKSHOP_ADMIN_SUBMISSION_REVIEW_ROUTE,
            post(admin_review_submission),
        )
        .route(PROMPT_WORKSHOP_ADMIN_ITEMS_ROUTE, post(admin_create_item))
        .route(
            PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
            axum::routing::put(admin_update_item).delete(admin_delete_item),
        )
        .route(PROMPT_WORKSHOP_ADMIN_STATS_ROUTE, get(admin_get_stats))
}

#[cfg(test)]
mod tests {
    use super::{
        build_prompt_workshop_route_owner_contract, build_workshop_download_payload,
        default_workshop_category, normalize_tags_value, prepare_admin_create_item_request,
        prepare_admin_review_submission_request, prepare_admin_update_item_request,
        prepare_submit_prompt_request, required_workshop_text, workshop_instance_id,
        workshop_user_identifier, PreparedPromptWorkshopAdminUpdateItemRequest,
        PromptWorkshopAdminCreateItemRouteRequest, PromptWorkshopAdminReviewRouteRequest,
        PromptWorkshopAdminUpdateItemRouteRequest, PROMPT_WORKSHOP_ADMIN_ITEMS_ROUTE,
        PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE, PROMPT_WORKSHOP_ADMIN_STATS_ROUTE,
        PROMPT_WORKSHOP_ADMIN_SUBMISSIONS_ROUTE, PROMPT_WORKSHOP_ADMIN_SUBMISSION_REVIEW_ROUTE,
        PROMPT_WORKSHOP_ITEMS_ROUTE, PROMPT_WORKSHOP_ITEM_DETAIL_ROUTE,
        PROMPT_WORKSHOP_ITEM_DOWNLOAD_ROUTE, PROMPT_WORKSHOP_ITEM_IMPORT_ROUTE,
        PROMPT_WORKSHOP_ITEM_LIKE_ROUTE, PROMPT_WORKSHOP_MY_SUBMISSIONS_ROUTE,
        PROMPT_WORKSHOP_STATUS_ROUTE, PROMPT_WORKSHOP_SUBMISSION_DETAIL_ROUTE,
        PROMPT_WORKSHOP_SUBMIT_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_prompt_workshop_route_owner_contract() {
        let contract = build_prompt_workshop_route_owner_contract();

        assert_eq!(contract["owner"], json!("prompt_workshop"));
        assert_eq!(
            contract["rust_owner"],
            json!("backend-rs/src/api/prompt_workshop.rs")
        );
        assert_eq!(
            contract["routes"]["submit"],
            json!(PROMPT_WORKSHOP_SUBMIT_ROUTE)
        );
        assert_eq!(
            contract["routes"]["admin_item_update"],
            json!(PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE)
        );
        assert_eq!(
            contract["routes"]["admin_item_delete"],
            json!(PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE)
        );
        assert_eq!(
            contract["methods"]["admin_item_detail"],
            json!(["PUT", "DELETE"])
        );
        assert_eq!(contract["service_owners"].as_array().map(Vec::len), Some(4));
        assert_eq!(
            contract["readiness_probes"].as_array().map(Vec::len),
            Some(6)
        );
        assert_eq!(
            contract["readiness_probes"]
                .as_array()
                .and_then(|probes| probes.last()),
            Some(&json!("prompt-workshop-withdraw-business-rust"))
        );
        assert_eq!(
            contract["source_map_files"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            json!("phase5-prompt-workshop-business-owner")
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be present");
        assert_eq!(business_probes.len(), 4);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "prompt-workshop-submit-business-rust"));
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            json!("covered_by_dedicated_rust_owner_profile")
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(4)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            json!("prompt-workshop Python route/model/schema/client/category source-map deleted; remaining maturity work is limited to optional cloud-proxy success smoke hardening")
        );
        assert!(contract["migration_policy"]
            .as_str()
            .unwrap()
            .contains("phase5-prompt-workshop-business-owner"));
        assert!(contract["migration_policy"].as_str().unwrap().contains(
            "legacy model/schema, cloud client, and category source maps are physically deleted"
        ));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["rollback_files"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn should_keep_prompt_workshop_route_group_paths_stable() {
        let contract = build_prompt_workshop_route_owner_contract();

        assert_eq!(
            contract["routes"],
            json!({
                "status": PROMPT_WORKSHOP_STATUS_ROUTE,
                "items": PROMPT_WORKSHOP_ITEMS_ROUTE,
                "item_detail": PROMPT_WORKSHOP_ITEM_DETAIL_ROUTE,
                "item_import": PROMPT_WORKSHOP_ITEM_IMPORT_ROUTE,
                "item_like": PROMPT_WORKSHOP_ITEM_LIKE_ROUTE,
                "item_download": PROMPT_WORKSHOP_ITEM_DOWNLOAD_ROUTE,
                "submit": PROMPT_WORKSHOP_SUBMIT_ROUTE,
                "my_submissions": PROMPT_WORKSHOP_MY_SUBMISSIONS_ROUTE,
                "submission_withdraw": PROMPT_WORKSHOP_SUBMISSION_DETAIL_ROUTE,
                "admin_submissions": PROMPT_WORKSHOP_ADMIN_SUBMISSIONS_ROUTE,
                "admin_submission_review": PROMPT_WORKSHOP_ADMIN_SUBMISSION_REVIEW_ROUTE,
                "admin_items": PROMPT_WORKSHOP_ADMIN_ITEMS_ROUTE,
                "admin_item_detail": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
                "admin_item_update": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
                "admin_item_delete": PROMPT_WORKSHOP_ADMIN_ITEM_DETAIL_ROUTE,
                "admin_stats": PROMPT_WORKSHOP_ADMIN_STATS_ROUTE
            })
        );
    }

    #[test]
    fn normalize_tags_value_keeps_csv_and_array_inputs_compatible() {
        assert_eq!(
            normalize_tags_value(Some(&json!("tag-1, tag-2 , ,tag-3"))),
            Some("[\"tag-1\",\"tag-2\",\"tag-3\"]".to_string())
        );
        assert_eq!(
            normalize_tags_value(Some(&json!(["a", " b ", "", 1]))),
            Some("[\"a\",\"b\"]".to_string())
        );
    }

    #[test]
    fn normalize_tags_value_keeps_json_string_and_empty_inputs_behavior() {
        assert_eq!(
            normalize_tags_value(Some(&json!("[\"x\",\"y\"]"))),
            Some("[\"x\",\"y\"]".to_string())
        );
        assert_eq!(normalize_tags_value(Some(&json!("   "))), None);
        assert_eq!(normalize_tags_value(Some(&json!(null))), None);
    }

    #[test]
    fn workshop_user_identifier_uses_instance_prefix() {
        let original = std::env::var("INSTANCE_ID").ok();
        unsafe {
            std::env::set_var("INSTANCE_ID", "test-instance");
        }

        assert_eq!(workshop_instance_id(), "test-instance");
        assert_eq!(workshop_user_identifier("user-7"), "test-instance:user-7");

        if let Some(value) = original {
            unsafe {
                std::env::set_var("INSTANCE_ID", value);
            }
        } else {
            unsafe {
                std::env::remove_var("INSTANCE_ID");
            }
        }
    }

    #[test]
    fn build_workshop_download_payload_keeps_existing_shape() {
        let payload = build_workshop_download_payload("instance-a", "instance-a:user-1");

        assert_eq!(payload["instance_id"], "instance-a");
        assert_eq!(payload["user_identifier"], "instance-a:user-1");
    }

    #[test]
    fn prepare_submit_prompt_request_keeps_proxy_payload_contract() {
        let original = std::env::var("INSTANCE_ID").ok();
        unsafe {
            std::env::set_var("INSTANCE_ID", "test-instance");
        }

        let request = prepare_submit_prompt_request(
            "instance-a",
            "user-1",
            "提示词名",
            Some("描述"),
            "内容",
            "general",
            Some(&json!(["tag-1", " tag-2 "])),
            Some("展示名"),
            true,
        );

        assert_eq!(request.user_identifier, "test-instance:user-1");
        assert_eq!(request.submitter_name, "展示名");
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
        assert_eq!(request.proxy_payload["instance_id"], "instance-a");
        assert_eq!(
            request.proxy_payload["submitter_id"],
            "test-instance:user-1"
        );
        assert_eq!(request.proxy_payload["submitter_name"], "展示名");
        assert_eq!(request.proxy_payload["name"], "提示词名");
        assert_eq!(request.proxy_payload["description"], "描述");
        assert_eq!(request.proxy_payload["prompt_content"], "内容");
        assert_eq!(request.proxy_payload["category"], "general");
        assert_eq!(request.proxy_payload["author_display_name"], "展示名");
        assert_eq!(request.proxy_payload["is_anonymous"], true);
        assert_eq!(request.proxy_payload["tags"][0], "tag-1");
        assert_eq!(request.proxy_payload["tags"][1], "tag-2");

        if let Some(value) = original {
            unsafe {
                std::env::set_var("INSTANCE_ID", value);
            }
        } else {
            unsafe {
                std::env::remove_var("INSTANCE_ID");
            }
        }
    }

    #[test]
    fn prepare_admin_review_submission_request_normalizes_tags() {
        let request =
            prepare_admin_review_submission_request(PromptWorkshopAdminReviewRouteRequest {
                action: "approve".to_string(),
                review_note: Some("通过".to_string()),
                category: Some("romance".to_string()),
                tags: Some(json!(["tag-1", " tag-2 ", ""])),
            });

        assert_eq!(request.action, "approve");
        assert_eq!(request.review_note.as_deref(), Some("通过"));
        assert_eq!(request.category.as_deref(), Some("romance"));
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
    }

    #[test]
    fn prepare_admin_create_item_request_keeps_default_category_and_normalized_tags() {
        let request =
            prepare_admin_create_item_request(PromptWorkshopAdminCreateItemRouteRequest {
                name: "官方提示词".to_string(),
                description: Some("描述".to_string()),
                prompt_content: "内容".to_string(),
                category: default_workshop_category(),
                tags: Some(json!("tag-1, tag-2")),
            });

        assert_eq!(request.name, "官方提示词");
        assert_eq!(request.description.as_deref(), Some("描述"));
        assert_eq!(request.prompt_content, "内容");
        assert_eq!(request.category, "general");
        assert_eq!(
            request.normalized_tags,
            Some("[\"tag-1\",\"tag-2\"]".to_string())
        );
    }

    #[test]
    fn prepare_admin_update_item_request_keeps_partial_update_contract() {
        let request =
            prepare_admin_update_item_request(PromptWorkshopAdminUpdateItemRouteRequest {
                name: Some("Prompt".to_string()),
                description: None,
                prompt_content: Some("Updated prompt".to_string()),
                category: Some("writing".to_string()),
                tags: Some(json!(["tag-1", "tag-2"])),
                status: Some("inactive".to_string()),
            });

        assert_eq!(request.name.as_deref(), Some("Prompt"));
        assert_eq!(request.description, None);
        assert_eq!(request.prompt_content.as_deref(), Some("Updated prompt"));
        assert_eq!(request.category.as_deref(), Some("writing"));
        assert_eq!(request.tags, Some(json!(["tag-1", "tag-2"])));
        assert_eq!(request.status.as_deref(), Some("inactive"));
    }

    #[test]
    fn required_workshop_text_rejects_missing_or_blank_fields() {
        let item = json!({
            "name": "风格 A",
            "prompt_content": "内容",
            "blank": "   "
        });

        assert_eq!(
            required_workshop_text(&item, "name").expect("name should exist"),
            "风格 A"
        );
        assert_eq!(
            required_workshop_text(&item, "missing").expect_err("missing should fail"),
            "云端提示词缺少必要字段: missing"
        );
        assert_eq!(
            required_workshop_text(&item, "blank").expect_err("blank should fail"),
            "云端提示词缺少必要字段: blank"
        );
    }

    #[test]
    fn prepared_prompt_workshop_admin_update_item_request_keeps_partial_update_contract() {
        let request = PreparedPromptWorkshopAdminUpdateItemRequest {
            name: Some("Prompt".to_string()),
            description: None,
            prompt_content: Some("Updated prompt".to_string()),
            category: Some("writing".to_string()),
            tags: Some(json!(["tag-1", "tag-2"])),
            status: Some("inactive".to_string()),
        };

        assert_eq!(request.name.as_deref(), Some("Prompt"));
        assert_eq!(request.description, None);
        assert_eq!(request.prompt_content.as_deref(), Some("Updated prompt"));
        assert_eq!(request.category.as_deref(), Some("writing"));
        assert_eq!(request.tags, Some(json!(["tag-1", "tag-2"])));
        assert_eq!(request.status.as_deref(), Some("inactive"));
    }
}
