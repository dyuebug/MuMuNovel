use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::{chapter, character, outline, project};
use crate::services::outline_service::OutlineService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::wizard_service::clean_json_response;

const DEFAULT_ESTIMATED_WORDS: i32 = 3000;

fn truncate_text(text: &str, limit: usize) -> String {
    let normalized = text.trim();
    if normalized.len() <= limit {
        normalized.to_string()
    } else {
        normalized[..limit].trim_end().to_string()
    }
}

fn value_to_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|raw| raw.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn infer_ending_type(narrative_goal: &str, index: usize) -> String {
    if narrative_goal.contains("?") && narrative_goal.contains("?") {
        "??".to_string()
    } else if narrative_goal.contains("?") && narrative_goal.contains("?") {
        "??".to_string()
    } else if narrative_goal.contains("??") || narrative_goal.contains("??") {
        "????".to_string()
    } else if narrative_goal.contains("??") {
        "????".to_string()
    } else if narrative_goal.contains("??") || narrative_goal.contains("??") {
        "????".to_string()
    } else {
        format!("????-{}", index)
    }
}

fn normalize_plan_value(mut plan: Map<String, Value>, outline_id: &str, index: usize) -> Value {
    let narrative_goal = plan
        .get("narrative_goal")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let title = plan
        .get("title")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("第{}章", index));

    let plot_summary = plan
        .get("plot_summary")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    let mut key_events = value_to_string_list(plan.get("key_events"));
    if key_events.is_empty() {
        key_events.push(format!("章节{}核心事件", index));
    }

    let character_focus = value_to_string_list(plan.get("character_focus"));
    let emotional_tone = plan
        .get("emotional_tone")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());

    let conflict_type = plan
        .get("conflict_type")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());

    let ending_type = plan
        .get("ending_type")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| infer_ending_type(&narrative_goal, index));

    let estimated_words = plan
        .get("estimated_words")
        .and_then(|value| value.as_i64())
        .map(|value| value as i32)
        .unwrap_or(DEFAULT_ESTIMATED_WORDS);

    plan.insert(
        "outline_id".to_string(),
        Value::String(outline_id.to_string()),
    );
    plan.insert("sub_index".to_string(), Value::Number(index.into()));
    plan.insert("title".to_string(), Value::String(title));
    plan.insert("plot_summary".to_string(), Value::String(plot_summary));
    plan.insert(
        "key_events".to_string(),
        Value::Array(key_events.into_iter().map(Value::String).collect()),
    );
    plan.insert(
        "character_focus".to_string(),
        Value::Array(character_focus.into_iter().map(Value::String).collect()),
    );
    plan.insert("emotional_tone".to_string(), Value::String(emotional_tone));
    plan.insert("narrative_goal".to_string(), Value::String(narrative_goal));
    plan.insert("conflict_type".to_string(), Value::String(conflict_type));
    plan.insert("ending_type".to_string(), Value::String(ending_type));
    plan.insert(
        "estimated_words".to_string(),
        Value::Number(serde_json::Number::from(estimated_words.max(0))),
    );

    if !plan.contains_key("scenes") {
        plan.insert("scenes".to_string(), Value::Null);
    }

    Value::Object(plan)
}

fn fallback_plan(outline_id: &str, outline_title: &str, summary: &str) -> Value {
    let mut plan = Map::new();
    plan.insert(
        "outline_id".to_string(),
        Value::String(outline_id.to_string()),
    );
    plan.insert("sub_index".to_string(), Value::Number(1.into()));
    plan.insert(
        "title".to_string(),
        Value::String(format!("{}-章节1", outline_title)),
    );
    plan.insert(
        "plot_summary".to_string(),
        Value::String(summary.chars().take(500).collect()),
    );
    plan.insert(
        "key_events".to_string(),
        Value::Array(vec![Value::String("解析失败".to_string())]),
    );
    plan.insert("character_focus".to_string(), Value::Array(vec![]));
    plan.insert(
        "emotional_tone".to_string(),
        Value::String("未知".to_string()),
    );
    plan.insert(
        "narrative_goal".to_string(),
        Value::String("需要重新生成".to_string()),
    );
    plan.insert(
        "conflict_type".to_string(),
        Value::String("未知".to_string()),
    );
    plan.insert("ending_type".to_string(), Value::String("未知".to_string()));
    plan.insert(
        "estimated_words".to_string(),
        Value::Number(serde_json::Number::from(DEFAULT_ESTIMATED_WORDS)),
    );
    Value::Object(plan)
}

fn build_characters_info(characters: &[character::Model]) -> String {
    let lines: Vec<String> = characters
        .iter()
        .map(|character| {
            let kind = if character.is_organization {
                "组织"
            } else {
                "角色"
            };
            let role_type = character.role_type.as_deref().unwrap_or("未知");
            let personality = character
                .personality
                .as_deref()
                .map(|value| truncate_text(value, 100))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "暂无描述".to_string());
            format!(
                "- {} ({}, {}): {}",
                character.name, kind, role_type, personality
            )
        })
        .collect();

    if lines.is_empty() {
        "暂无角色".to_string()
    } else {
        lines.join("\n")
    }
}

async fn build_outline_context(
    db: &DatabaseConnection,
    outline_model: &outline::Model,
) -> Result<String, String> {
    let prev_outline = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&outline_model.project_id))
        .filter(outline::Column::OrderIndex.lt(outline_model.order_index.unwrap_or_default()))
        .order_by_desc(outline::Column::OrderIndex)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;

    let next_outline = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&outline_model.project_id))
        .filter(outline::Column::OrderIndex.gt(outline_model.order_index.unwrap_or_default()))
        .order_by_asc(outline::Column::OrderIndex)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;

    let mut context = String::new();
    if let Some(prev) = prev_outline {
        let content = prev.content.as_deref().unwrap_or("");
        context.push_str(&format!(
            "【前一节】{}: {}...\n\n",
            prev.title,
            truncate_text(content, 200)
        ));
    }
    if let Some(next) = next_outline {
        let content = next.content.as_deref().unwrap_or("");
        context.push_str(&format!(
            "【后一节】{}: {}...\n",
            next.title,
            truncate_text(content, 200)
        ));
    }

    Ok(if context.is_empty() {
        "（无前后文）".to_string()
    } else {
        context
    })
}

fn build_prompt_fields(
    project_model: &project::Model,
    outline_model: &outline::Model,
    characters_info: &str,
    context_info: &str,
    expansion_strategy: &str,
    target_chapter_count: usize,
    previous_context: Option<&str>,
    start_index: Option<usize>,
    end_index: Option<usize>,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("project_title".into(), project_model.title.clone());
    params.insert(
        "project_genre".into(),
        project_model
            .genre
            .clone()
            .unwrap_or_else(|| "通用".to_string()),
    );
    params.insert(
        "project_theme".into(),
        project_model
            .theme
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_narrative_perspective".into(),
        project_model
            .narrative_perspective
            .clone()
            .unwrap_or_else(|| "第三人称".to_string()),
    );
    params.insert(
        "project_world_time_period".into(),
        project_model
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_world_location".into(),
        project_model
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_world_atmosphere".into(),
        project_model
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert("characters_info".into(), characters_info.to_string());
    params.insert(
        "outline_order_index".into(),
        outline_model.order_index.unwrap_or_default().to_string(),
    );
    params.insert("outline_title".into(), outline_model.title.clone());
    params.insert(
        "outline_content".into(),
        outline_model.content.clone().unwrap_or_default(),
    );
    params.insert("context_info".into(), context_info.to_string());
    params.insert(
        "strategy_instruction".into(),
        expansion_strategy.to_string(),
    );
    params.insert(
        "target_chapter_count".into(),
        target_chapter_count.to_string(),
    );
    params.insert("scene_instruction".into(), String::new());
    params.insert("scene_field".into(), String::new());
    if let Some(previous_context) = previous_context {
        params.insert("previous_context".into(), previous_context.to_string());
    }
    if let Some(start_index) = start_index {
        params.insert("start_index".into(), start_index.to_string());
    }
    if let Some(end_index) = end_index {
        params.insert("end_index".into(), end_index.to_string());
    }
    params
}

fn parse_chapter_plans(ai_response: &str, outline_id: &str, outline_title: &str) -> Vec<Value> {
    let cleaned = clean_json_response(ai_response);
    let parsed = serde_json::from_str::<Value>(&cleaned).ok();

    let raw_plans = match parsed {
        Some(Value::Array(items)) => items,
        Some(Value::Object(map)) => map
            .get("chapter_plans")
            .and_then(|value| value.as_array())
            .cloned()
            .or_else(|| map.get("plans").and_then(|value| value.as_array()).cloned())
            .unwrap_or_else(|| vec![Value::Object(map)]),
        Some(other) => vec![other],
        None => vec![fallback_plan(outline_id, outline_title, ai_response)],
    };

    raw_plans
        .into_iter()
        .enumerate()
        .map(|(index, value)| match value {
            Value::Object(map) => normalize_plan_value(map, outline_id, index + 1),
            _ => fallback_plan(outline_id, outline_title, ai_response),
        })
        .collect()
}

fn parse_string_array(value: Option<&Value>) -> Vec<String> {
    value_to_string_list(value)
}

pub(super) struct PlotExpansionService<'a> {
    ai_service: &'a AIService,
}

impl<'a> PlotExpansionService<'a> {
    fn new(ai_service: &'a AIService) -> Self {
        Self { ai_service }
    }

    async fn load_outline_and_project(
        &self,
        db: &DatabaseConnection,
        user_id: &str,
        outline_id: &str,
    ) -> Result<(outline::Model, project::Model), String> {
        let outline_model = OutlineService::get(db, outline_id, user_id)
            .await?
            .ok_or_else(|| "大纲不存在或无权限".to_string())?;

        let project_model = project::Entity::find_by_id(&outline_model.project_id)
            .one(db)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "项目不存在".to_string())?;

        if project_model.user_id != user_id {
            return Err("项目不存在或无权限".to_string());
        }

        Ok((outline_model, project_model))
    }

    async fn load_characters(
        &self,
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Vec<character::Model>, String> {
        character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .order_by_asc(character::Column::CreatedAt)
            .all(db)
            .await
            .map_err(|e| e.to_string())
    }

    async fn analyze_outline_for_chapters(
        &self,
        db: &DatabaseConnection,
        outline_model: &outline::Model,
        project_model: &project::Model,
        target_chapter_count: usize,
        expansion_strategy: &str,
        enable_scene_analysis: bool,
        _provider: Option<&str>,
        _model: Option<&str>,
        batch_size: usize,
    ) -> Result<Vec<Value>, String> {
        let characters = self.load_characters(db, &project_model.id).await?;
        let characters_info = build_characters_info(&characters);
        let context_info = build_outline_context(db, outline_model).await?;
        let mut chapter_plans = Vec::new();

        let single_batch = target_chapter_count <= batch_size;
        let total_batches = if single_batch {
            1
        } else {
            (target_chapter_count + batch_size - 1) / batch_size
        };

        let mut used_key_events: Vec<String> = Vec::new();

        for batch_num in 0..total_batches {
            let remaining_chapters = target_chapter_count.saturating_sub(chapter_plans.len());
            if remaining_chapters == 0 {
                break;
            }
            let current_batch_size = remaining_chapters.min(batch_size);
            let current_start_index = chapter_plans.len() + 1;
            let current_end_index = current_start_index + current_batch_size - 1;

            let previous_context = if chapter_plans.is_empty() {
                String::new()
            } else {
                let previous_summaries: Vec<String> = chapter_plans
                    .iter()
                    .map(|plan: &Value| {
                        let title = plan
                            .get("title")
                            .and_then(|value: &Value| value.as_str())
                            .unwrap_or("?????");
                        let summary = plan
                            .get("plot_summary")
                            .and_then(|value: &Value| value.as_str())
                            .unwrap_or("");
                        let key_events = parse_string_array(plan.get("key_events"));
                        let ending_type = plan
                            .get("ending_type")
                            .and_then(|value: &Value| value.as_str())
                            .unwrap_or("??");
                        format!(
                            "?{}??{}?:\n  - ???{}\n  - ?????{}\n  - ?????{}",
                            plan.get("sub_index")
                                .and_then(|value: &Value| value.as_i64())
                                .unwrap_or(0),
                            title,
                            truncate_text(summary, 150),
                            if key_events.is_empty() {
                                "?".to_string()
                            } else {
                                key_events[..key_events.len().min(3)].join("?")
                            },
                            ending_type,
                        )
                    })
                    .collect();
                let used_events = if used_key_events.is_empty() {
                    "暂无".to_string()
                } else {
                    used_key_events
                        .iter()
                        .rev()
                        .take(20)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("、")
                };
                format!(
                    "【🔴 已生成章节完整信息（必须参考以确保差异化）】\n{}\n\n【🔴 已使用的关键事件（本批次不可重复使用）】\n{}\n\n【🔴 差异化强制要求】\n⚠️ 当前是第{}-{}节（共{}节中的第{}批）\n⚠️ 每个新章节必须有完全不同的：\n   1. 开场场景（不同地点/时间/人物状态）\n   2. 核心事件（不与已生成章节的关键事件重复）\n   3. 结尾悬念（不同类型的钩子）\n⚠️ 新章节的key_events不得与上面【已使用的关键事件】中的任何事件相同或相似",
                    previous_summaries.join("\n\n"),
                    used_events,
                    current_start_index,
                    current_end_index,
                    target_chapter_count,
                    batch_num + 1,
                )
            };

            let mut params = build_prompt_fields(
                project_model,
                outline_model,
                &characters_info,
                &context_info,
                expansion_strategy,
                current_batch_size,
                if previous_context.is_empty() {
                    None
                } else {
                    Some(previous_context.as_str())
                },
                Some(current_start_index),
                Some(current_end_index),
            );
            let template_key = if single_batch && total_batches == 1 {
                "OUTLINE_EXPAND_SINGLE"
            } else {
                "OUTLINE_EXPAND_MULTI"
            };
            params.insert(
                "enable_scene_analysis".into(),
                enable_scene_analysis.to_string(),
            );
            params.insert(
                "target_chapter_count".into(),
                current_batch_size.to_string(),
            );
            let template = PromptTemplateService::system_template_info(template_key)
                .ok_or_else(|| format!("找不到提示词模板: {}", template_key))?;
            let prompt = PromptTemplateService::format_prompt(&template.content, &params)?;
            let response = self
                .ai_service
                .generate_text(&prompt, None, None)
                .await
                .map_err(|e| format!("AI调用失败: {}", e))?;
            let batch_plans =
                parse_chapter_plans(&response.content, &outline_model.id, &outline_model.title);

            for (offset, plan) in batch_plans.into_iter().enumerate() {
                if let Some(plan_obj) = plan.as_object() {
                    if let Some(events) = plan_obj
                        .get("key_events")
                        .and_then(|value| value.as_array())
                    {
                        for event in events.iter().filter_map(|value| value.as_str()) {
                            let trimmed = event.trim();
                            if !trimmed.is_empty() {
                                used_key_events.push(trimmed.to_string());
                            }
                        }
                    }
                }
                let mut normalized = plan;
                if let Some(map) = normalized.as_object_mut() {
                    map.insert(
                        "sub_index".to_string(),
                        Value::Number((current_start_index + offset).into()),
                    );
                }
                chapter_plans.push(normalized);
            }
        }

        Ok(chapter_plans)
    }

    async fn create_chapters_from_plans(
        &self,
        outline_id: &str,
        project_id: &str,
        chapter_plans: &[Value],
        db: &DatabaseConnection,
        start_chapter_number: Option<i32>,
    ) -> Result<Vec<chapter::Model>, String> {
        let start_chapter_number = if let Some(value) = start_chapter_number {
            value
        } else {
            let current_outline = outline::Entity::find_by_id(outline_id)
                .one(db)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("大纲 {} 不存在", outline_id))?;

            let prev_outlines = outline::Entity::find()
                .filter(outline::Column::ProjectId.eq(project_id))
                .filter(
                    outline::Column::OrderIndex.lt(current_outline.order_index.unwrap_or_default()),
                )
                .order_by_asc(outline::Column::OrderIndex)
                .all(db)
                .await
                .map_err(|e| e.to_string())?;

            let mut total_prev_chapters = 0i32;
            for prev_outline in prev_outlines {
                let chapters = chapter::Entity::find()
                    .filter(chapter::Column::ProjectId.eq(project_id))
                    .filter(chapter::Column::OutlineId.eq(&prev_outline.id))
                    .all(db)
                    .await
                    .map_err(|e| e.to_string())?;
                total_prev_chapters += chapters.len() as i32;
            }
            total_prev_chapters + 1
        };

        let mut created = Vec::new();
        let now = Utc::now().naive_utc();

        for (index, plan) in chapter_plans.iter().enumerate() {
            let chapter_number = start_chapter_number + index as i32;
            let sub_index = plan
                .get("sub_index")
                .and_then(|value| value.as_i64())
                .map(|value| value as i32)
                .unwrap_or(index as i32 + 1);
            let title = plan
                .get("title")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("第{}章", chapter_number));
            let summary = plan
                .get("plot_summary")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            let expansion_plan = serde_json::to_string(plan).unwrap_or_else(|_| "{}".to_string());

            let inserted = chapter::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                project_id: Set(project_id.to_string()),
                chapter_number: Set(chapter_number),
                title: Set(title),
                content: Set(Some(String::new())),
                summary: Set(Some(summary)),
                word_count: Set(0),
                status: Set("draft".to_string()),
                outline_id: Set(Some(outline_id.to_string())),
                sub_index: Set(sub_index),
                expansion_plan: Set(Some(expansion_plan)),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(db)
            .await
            .map_err(|e| e.to_string())?;

            created.push(inserted);
        }

        self.renumber_subsequent_chapters(db, project_id, outline_id)
            .await?;

        Ok(created)
    }

    async fn renumber_subsequent_chapters(
        &self,
        db: &DatabaseConnection,
        project_id: &str,
        current_outline_id: &str,
    ) -> Result<(), String> {
        let current_outline = outline::Entity::find_by_id(current_outline_id)
            .one(db)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("大纲 {} 不存在", current_outline_id))?;
        let current_order_index = current_outline.order_index.unwrap_or_default();

        let prev_outlines = outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .filter(outline::Column::OrderIndex.lt(current_order_index))
            .order_by_asc(outline::Column::OrderIndex)
            .all(db)
            .await
            .map_err(|e| e.to_string())?;

        let mut current_chapter_number = 1i32;
        for prev_outline in prev_outlines {
            let chapters = chapter::Entity::find()
                .filter(chapter::Column::ProjectId.eq(project_id))
                .filter(chapter::Column::OutlineId.eq(&prev_outline.id))
                .all(db)
                .await
                .map_err(|e| e.to_string())?;
            current_chapter_number += chapters.len() as i32;
        }

        let subsequent_outlines = outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .filter(outline::Column::OrderIndex.gte(current_order_index))
            .order_by_asc(outline::Column::OrderIndex)
            .all(db)
            .await
            .map_err(|e| e.to_string())?;

        for outline_model in subsequent_outlines {
            let mut chapters = chapter::Entity::find()
                .filter(chapter::Column::ProjectId.eq(project_id))
                .filter(chapter::Column::OutlineId.eq(&outline_model.id))
                .order_by_asc(chapter::Column::SubIndex)
                .all(db)
                .await
                .map_err(|e| e.to_string())?;

            for chapter_model in chapters.iter_mut() {
                if chapter_model.chapter_number != current_chapter_number {
                    let mut active: chapter::ActiveModel = chapter_model.clone().into();
                    active.chapter_number = Set(current_chapter_number);
                    active.updated_at = Set(Some(Utc::now().naive_utc()));
                    active.update(db).await.map_err(|e| e.to_string())?;
                    chapter_model.chapter_number = current_chapter_number;
                }
                current_chapter_number += 1;
            }
        }

        Ok(())
    }

    pub(super) async fn expand_outline(
        &self,
        db: &DatabaseConnection,
        user_id: &str,
        outline_id: &str,
        target_chapter_count: usize,
        expansion_strategy: &str,
        auto_create_chapters: bool,
        enable_scene_analysis: bool,
        provider: Option<&str>,
        model: Option<&str>,
        batch_size: usize,
    ) -> Result<Value, String> {
        let (outline_model, project_model) = self
            .load_outline_and_project(db, user_id, outline_id)
            .await?;

        let chapter_plans = self
            .analyze_outline_for_chapters(
                db,
                &outline_model,
                &project_model,
                target_chapter_count,
                expansion_strategy,
                enable_scene_analysis,
                provider,
                model,
                batch_size,
            )
            .await?;

        let created_chapters = if auto_create_chapters {
            Some(
                self.create_chapters_from_plans(
                    outline_id,
                    &project_model.id,
                    &chapter_plans,
                    db,
                    None,
                )
                .await?,
            )
        } else {
            None
        };

        let created_chapters_value = created_chapters.map(|chapters| {
            Value::Array(
                chapters
                    .into_iter()
                    .map(|chapter| {
                        json!({
                            "id": chapter.id,
                            "chapter_number": chapter.chapter_number,
                            "title": chapter.title,
                            "summary": chapter.summary,
                            "outline_id": chapter.outline_id,
                            "sub_index": chapter.sub_index,
                            "status": chapter.status,
                        })
                    })
                    .collect(),
            )
        });

        Ok(json!({
            "success": true,
            "message": "大纲展开完成",
            "outline_id": outline_model.id,
            "outline_title": outline_model.title,
            "target_chapter_count": target_chapter_count,
            "actual_chapter_count": chapter_plans.len(),
            "expansion_strategy": expansion_strategy,
            "enable_scene_analysis": enable_scene_analysis,
            "auto_create_chapters": auto_create_chapters,
            "chapter_plans": chapter_plans,
            "created_chapters": created_chapters_value.unwrap_or(Value::Null),
        }))
    }

    pub(super) async fn batch_expand_outlines(
        &self,
        db: &DatabaseConnection,
        user_id: &str,
        project_id: &str,
        chapters_per_outline: usize,
        expansion_strategy: &str,
        auto_create_chapters: bool,
        enable_scene_analysis: bool,
        outline_ids: Option<&[String]>,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<Value, String> {
        let project_model = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("项目 {} 不存在", project_id))?;
        if project_model.user_id != user_id {
            return Err("项目不存在或无权限".to_string());
        }

        let outlines = if let Some(outline_ids) = outline_ids {
            outline::Entity::find()
                .filter(outline::Column::ProjectId.eq(project_id))
                .filter(outline::Column::Id.is_in(outline_ids.iter().cloned()))
                .order_by_asc(outline::Column::OrderIndex)
                .all(db)
                .await
                .map_err(|e| e.to_string())?
        } else {
            outline::Entity::find()
                .filter(outline::Column::ProjectId.eq(project_id))
                .order_by_asc(outline::Column::OrderIndex)
                .all(db)
                .await
                .map_err(|e| e.to_string())?
        };

        if outlines.is_empty() {
            return Ok(json!({
                "success": true,
                "message": "没有找到要展开的大纲",
                "project_id": project_id,
                "chapters_per_outline": chapters_per_outline,
                "expansion_strategy": expansion_strategy,
                "enable_scene_analysis": enable_scene_analysis,
                "auto_create_chapters": auto_create_chapters,
                "total_outlines_expanded": 0,
                "total_chapters_created": 0,
                "skipped_count": 0,
                "skipped_outlines": [],
                "expansion_results": [],
            }));
        }

        let mut expansion_results = Vec::new();
        let mut skipped_outlines = Vec::new();
        let mut total_chapters_created = 0usize;

        for outline_model in outlines {
            let existing_chapter = chapter::Entity::find()
                .filter(chapter::Column::OutlineId.eq(&outline_model.id))
                .one(db)
                .await
                .map_err(|e| e.to_string())?;

            if existing_chapter.is_some() {
                skipped_outlines.push(json!({
                    "outline_id": outline_model.id,
                    "outline_title": outline_model.title,
                    "reason": "已展开",
                }));
                continue;
            }

            match self
                .analyze_outline_for_chapters(
                    db,
                    &outline_model,
                    &project_model,
                    chapters_per_outline,
                    expansion_strategy,
                    enable_scene_analysis,
                    provider,
                    model,
                    5,
                )
                .await
            {
                Ok(chapter_plans) => {
                    let created_chapters_value = if auto_create_chapters {
                        let created = self
                            .create_chapters_from_plans(
                                &outline_model.id,
                                &project_model.id,
                                &chapter_plans,
                                db,
                                None,
                            )
                            .await?;
                        total_chapters_created += created.len();
                        Value::Array(
                            created
                                .into_iter()
                                .map(|chapter| {
                                    json!({
                                        "id": chapter.id,
                                        "chapter_number": chapter.chapter_number,
                                        "title": chapter.title,
                                        "summary": chapter.summary,
                                        "outline_id": chapter.outline_id,
                                        "sub_index": chapter.sub_index,
                                        "status": chapter.status,
                                    })
                                })
                                .collect(),
                        )
                    } else {
                        Value::Null
                    };

                    expansion_results.push(json!({
                        "outline_id": outline_model.id,
                        "outline_title": outline_model.title,
                        "target_chapter_count": chapters_per_outline,
                        "actual_chapter_count": chapter_plans.len(),
                        "expansion_strategy": expansion_strategy,
                        "chapter_plans": chapter_plans,
                        "created_chapters": created_chapters_value,
                    }));
                }
                Err(error) => {
                    expansion_results.push(json!({
                        "outline_id": outline_model.id,
                        "outline_title": outline_model.title,
                        "target_chapter_count": chapters_per_outline,
                        "actual_chapter_count": 0,
                        "expansion_strategy": expansion_strategy,
                        "chapter_plans": [],
                        "created_chapters": Value::Null,
                        "error": error,
                    }));
                }
            }
        }

        Ok(json!({
            "success": true,
            "message": "批量展开完成",
            "project_id": project_id,
            "chapters_per_outline": chapters_per_outline,
            "expansion_strategy": expansion_strategy,
            "enable_scene_analysis": enable_scene_analysis,
            "auto_create_chapters": auto_create_chapters,
            "total_outlines_expanded": expansion_results.len(),
            "total_chapters_created": total_chapters_created,
            "skipped_count": skipped_outlines.len(),
            "skipped_outlines": skipped_outlines,
            "expansion_results": expansion_results,
        }))
    }
}

pub(super) fn create_plot_expansion_service(ai_service: &AIService) -> PlotExpansionService<'_> {
    PlotExpansionService::new(ai_service)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::json;

    use crate::models::{character, project};

    use super::{
        build_characters_info, fallback_plan, infer_ending_type, normalize_plan_value,
        parse_chapter_plans, truncate_text, value_to_string_list, DEFAULT_ESTIMATED_WORDS,
    };

    #[test]
    fn truncate_text_trims_and_respects_limit() {
        assert_eq!(truncate_text("  hello  ", 10), "hello");
        assert_eq!(truncate_text("abcdef", 4), "abcd");
    }

    #[test]
    fn value_to_string_list_filters_empty_entries() {
        let input = json!([" a ", "", "b", null, 1]);
        assert_eq!(value_to_string_list(Some(&input)), vec!["a", "b"]);
    }

    #[test]
    fn infer_ending_type_returns_fallback_label() {
        assert_eq!(infer_ending_type("普通结尾", 3), "????-3");
    }

    #[test]
    fn normalize_plan_value_backfills_required_fields() {
        let plan = json!({
            "title": " 第一章 ",
            "plot_summary": " 摘要 ",
            "key_events": [" 事件1 ", ""],
            "character_focus": [" 主角 "],
            "estimated_words": -10
        });

        let normalized =
            normalize_plan_value(plan.as_object().cloned().expect("object"), "outline-1", 2);

        assert_eq!(normalized["outline_id"], "outline-1");
        assert_eq!(normalized["sub_index"], 2);
        assert_eq!(normalized["title"], "第一章");
        assert_eq!(normalized["plot_summary"], "摘要");
        assert_eq!(normalized["character_focus"][0], "主角");
        assert_eq!(normalized["estimated_words"], 0);
        assert!(normalized.get("scenes").is_some());
    }

    #[test]
    fn fallback_plan_preserves_outline_identity() {
        let plan = fallback_plan("outline-1", "大纲标题", "摘要内容");

        assert_eq!(plan["outline_id"], "outline-1");
        assert_eq!(plan["sub_index"], 1);
        assert_eq!(plan["estimated_words"], DEFAULT_ESTIMATED_WORDS);
    }

    #[test]
    fn build_characters_info_formats_roles_and_orgs() {
        let characters = vec![
            character::Model {
                id: "char-1".to_string(),
                project_id: "project-1".to_string(),
                name: "沈夜".to_string(),
                age: None,
                gender: None,
                is_organization: false,
                role_type: Some("主角".to_string()),
                personality: Some("冷静谨慎".to_string()),
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: None,
                state_updated_chapter: None,
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: NaiveDateTime::parse_from_str(
                    "1970-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                updated_at: None,
            },
            character::Model {
                id: "char-2".to_string(),
                project_id: "project-1".to_string(),
                name: "北境军".to_string(),
                age: None,
                gender: None,
                is_organization: true,
                role_type: Some("势力".to_string()),
                personality: None,
                background: None,
                appearance: None,
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: None,
                state_updated_chapter: None,
                main_career_id: None,
                main_career_stage: None,
                sub_careers: None,
                avatar_url: None,
                traits: None,
                created_at: NaiveDateTime::parse_from_str(
                    "1970-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                updated_at: None,
            },
        ];

        let formatted = build_characters_info(&characters);
        assert!(formatted.contains("沈夜 (角色, 主角): 冷静谨慎"));
        assert!(formatted.contains("北境军 (组织, 势力): 暂无描述"));
    }

    #[test]
    fn parse_chapter_plans_uses_array_payloads() {
        let plans = parse_chapter_plans(
            r#"[{"title":"第一章","plot_summary":"摘要"}]"#,
            "outline-1",
            "测试大纲",
        );

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0]["outline_id"], "outline-1");
        assert_eq!(plans[0]["title"], "第一章");
    }

    #[test]
    fn parse_chapter_plans_uses_object_wrapped_arrays() {
        let plans = parse_chapter_plans(
            r#"{"chapter_plans":[{"title":"第一章","plot_summary":"摘要"}]}"#,
            "outline-1",
            "测试大纲",
        );

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0]["sub_index"], 1);
    }

    #[test]
    fn parse_chapter_plans_falls_back_on_invalid_json() {
        let plans = parse_chapter_plans("not-json", "outline-1", "测试大纲");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0]["key_events"][0], "解析失败");
    }

    #[test]
    fn parse_chapter_plans_accepts_plans_alias() {
        let plans = parse_chapter_plans(
            r#"{"plans":[{"title":"第一章","plot_summary":"摘要"}]}"#,
            "outline-1",
            "测试大纲",
        );

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0]["plot_summary"], "摘要");
    }

    #[test]
    fn empty_character_list_uses_default_text() {
        assert_eq!(build_characters_info(&[]), "暂无角色");
    }

    #[test]
    fn fallback_plan_truncates_summary() {
        let summary = "a".repeat(600);
        let plan = fallback_plan("outline-1", "测试大纲", &summary);

        assert_eq!(plan["plot_summary"].as_str().unwrap().len(), 500);
    }

    #[test]
    fn normalize_plan_value_provides_default_key_events() {
        let plan = json!({
            "title": "第一章",
            "plot_summary": "摘要"
        });
        let normalized =
            normalize_plan_value(plan.as_object().cloned().expect("object"), "outline-1", 4);

        assert_eq!(normalized["key_events"][0], "章节4核心事件");
    }

    #[test]
    fn value_to_string_list_defaults_to_empty() {
        let value = json!({"key": "value"});
        let values = value_to_string_list(Some(&value));
        assert!(values.is_empty());
    }

    #[test]
    fn normalize_plan_value_preserves_positive_estimated_words() {
        let plan = json!({
            "title": "第一章",
            "plot_summary": "摘要",
            "estimated_words": 4200
        });
        let normalized =
            normalize_plan_value(plan.as_object().cloned().expect("object"), "outline-1", 1);
        assert_eq!(normalized["estimated_words"], 4200);
    }

    #[test]
    fn parse_chapter_plans_wraps_non_object_items() {
        let plans = parse_chapter_plans(r#"[1]"#, "outline-1", "测试大纲");
        assert_eq!(plans[0]["outline_id"], "outline-1");
    }

    #[test]
    fn project_model_fixture_shape_is_stable() {
        let project = project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试小说".to_string(),
            description: None,
            theme: Some("成长".to_string()),
            genre: Some("玄幻".to_string()),
            target_words: 100000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 4,
            outline_mode: "one-to-many".to_string(),
            world_time_period: Some("乱世末年".to_string()),
            world_location: Some("北境雪原".to_string()),
            world_atmosphere: Some("压抑肃杀".to_string()),
            world_rules: Some("灵力暴走会反噬经脉".to_string()),
            chapter_count: Some(100),
            narrative_perspective: Some("第三人称".to_string()),
            character_count: 4,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            updated_at: None,
        };
        assert_eq!(project.genre.as_deref(), Some("玄幻"));
    }
}
