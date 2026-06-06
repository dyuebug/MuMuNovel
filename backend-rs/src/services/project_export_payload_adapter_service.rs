use std::collections::HashMap;

use chrono::{NaiveDateTime, Utc};
use serde_json::{json, Value};

use crate::models::{chapter, writing_style};
use crate::services::project_export_query_service::{ProjectExportContext, ProjectExportOptions};

fn format_export_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn format_optional_export_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(format_export_datetime)
}

fn current_export_time() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

pub fn build_project_export_data_payload(
    context: &ProjectExportContext,
    options: &ProjectExportOptions,
) -> Value {
    let outline_title_mapping = context
        .outlines
        .iter()
        .map(|outline| (outline.id.clone(), outline.title.clone()))
        .collect::<HashMap<_, _>>();
    let chapter_title_mapping = context
        .chapters
        .iter()
        .map(|chapter| (chapter.id.clone(), chapter.title.clone()))
        .collect::<HashMap<_, _>>();
    let character_name_mapping = context
        .characters
        .iter()
        .map(|character| (character.id.clone(), character.name.clone()))
        .collect::<HashMap<_, _>>();
    let organization_by_id = context
        .organizations
        .iter()
        .map(|organization| (organization.id.clone(), organization))
        .collect::<HashMap<_, _>>();
    let character_by_id = context
        .characters
        .iter()
        .map(|character| (character.id.clone(), character))
        .collect::<HashMap<_, _>>();
    let career_name_mapping = context
        .careers
        .iter()
        .map(|career| (career.id.clone(), career.name.clone()))
        .collect::<HashMap<_, _>>();
    let style_name_mapping = context
        .writing_styles
        .iter()
        .map(|style| (style.id, style.name.clone()))
        .collect::<HashMap<_, _>>();

    json!({
        "version": "1.1.0",
        "export_time": current_export_time(),
        "project": build_project_payload(&context.project),
        "chapters": context
            .chapters
            .iter()
            .map(|chapter| build_chapter_payload(chapter, &outline_title_mapping))
            .collect::<Vec<_>>(),
        "characters": context
            .characters
            .iter()
            .map(build_character_payload)
            .collect::<Vec<_>>(),
        "outlines": context
            .outlines
            .iter()
            .map(build_outline_payload)
            .collect::<Vec<_>>(),
        "relationships": context
            .relationships
            .iter()
            .filter_map(|relationship| build_relationship_payload(relationship, &character_name_mapping))
            .collect::<Vec<_>>(),
        "organizations": context
            .organizations
            .iter()
            .filter_map(|organization| build_organization_payload(organization, &organization_by_id, &character_by_id))
            .collect::<Vec<_>>(),
        "organization_members": context
            .organization_members
            .iter()
            .filter_map(|member| {
                build_organization_member_payload(member, &organization_by_id, &character_by_id)
            })
            .collect::<Vec<_>>(),
        "writing_styles": if options.include_writing_styles {
            context
                .writing_styles
                .iter()
                .filter(|style| style.user_id.as_ref() == Some(&context.project.user_id))
                .map(build_writing_style_payload)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "generation_history": if options.include_generation_history {
            context
                .generation_history
                .iter()
                .map(|history| build_generation_history_payload(history, &chapter_title_mapping))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "careers": if options.include_careers {
            context
                .careers
                .iter()
                .map(build_career_payload)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "character_careers": if options.include_careers {
            context
                .character_careers
                .iter()
                .filter_map(|item| {
                    build_character_career_payload(item, &character_name_mapping, &career_name_mapping)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "story_memories": if options.include_memories {
            context
                .story_memories
                .iter()
                .map(|memory| build_story_memory_payload(memory, &chapter_title_mapping, &character_name_mapping))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "plot_analysis": if options.include_plot_analysis {
            context
                .plot_analysis
                .iter()
                .filter_map(|analysis| build_plot_analysis_payload(analysis, &chapter_title_mapping))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "project_default_style": build_project_default_style_payload(
            context.project_default_style_style.as_ref(),
            &style_name_mapping,
            context.project_default_style.as_ref().map(|item| item.style_id),
        ),
    })
}

fn build_project_payload(project: &crate::models::project::Model) -> Value {
    json!({
        "title": project.title,
        "description": project.description,
        "theme": project.theme,
        "genre": project.genre,
        "target_words": project.target_words,
        "current_words": project.current_words,
        "status": project.status,
        "world_time_period": project.world_time_period,
        "world_location": project.world_location,
        "world_atmosphere": project.world_atmosphere,
        "world_rules": project.world_rules,
        "chapter_count": project.chapter_count,
        "narrative_perspective": project.narrative_perspective,
        "character_count": project.character_count,
        "outline_mode": project.outline_mode,
        "user_id": project.user_id,
        "created_at": format_export_datetime(project.created_at),
        "wizard_status": project.wizard_status,
        "wizard_step": project.wizard_step,
        "default_creative_mode": project.default_creative_mode,
        "default_story_focus": project.default_story_focus,
        "default_plot_stage": project.default_plot_stage,
        "default_story_creation_brief": project.default_story_creation_brief,
        "default_quality_preset": project.default_quality_preset,
        "default_quality_notes": project.default_quality_notes,
    })
}

fn build_chapter_payload(
    chapter: &crate::models::chapter::Model,
    outline_title_mapping: &HashMap<String, String>,
) -> Value {
    json!({
        "title": chapter.title,
        "content": chapter.content,
        "summary": chapter.summary,
        "chapter_number": chapter.chapter_number,
        "word_count": chapter.word_count,
        "status": chapter.status,
        "created_at": format_export_datetime(chapter.created_at),
        "outline_title": chapter
            .outline_id
            .as_ref()
            .and_then(|outline_id| outline_title_mapping.get(outline_id))
            .cloned(),
        "sub_index": chapter.sub_index,
        "expansion_plan": parse_json_text(chapter.expansion_plan.as_deref()),
    })
}

fn build_character_payload(character: &crate::models::character::Model) -> Value {
    json!({
        "name": character.name,
        "age": character.age,
        "gender": character.gender,
        "is_organization": character.is_organization,
        "role_type": character.role_type,
        "personality": character.personality,
        "background": character.background,
        "appearance": character.appearance,
        "traits": parse_json_text(character.traits.as_deref()),
        "organization_type": character.organization_type,
        "organization_purpose": character.organization_purpose,
        "avatar_url": character.avatar_url,
        "main_career_id": character.main_career_id,
        "main_career_stage": character.main_career_stage,
        "sub_careers": character.sub_careers,
        "created_at": format_export_datetime(character.created_at),
    })
}

fn build_outline_payload(outline: &crate::models::outline::Model) -> Value {
    json!({
        "title": outline.title,
        "content": outline.content,
        "structure": outline.structure,
        "order_index": outline.order_index,
        "created_at": format_export_datetime(outline.created_at),
    })
}

fn build_relationship_payload(
    relationship: &crate::models::relationship::Model,
    character_name_mapping: &HashMap<String, String>,
) -> Option<Value> {
    let source_name = character_name_mapping
        .get(&relationship.character_from_id)
        .cloned()?;
    let target_name = character_name_mapping
        .get(&relationship.character_to_id)
        .cloned()?;

    Some(json!({
        "source_name": source_name,
        "target_name": target_name,
        "relationship_name": relationship.relationship_name,
        "intimacy_level": relationship.intimacy_level,
        "status": relationship.status,
        "description": relationship.description,
        "started_at": relationship.started_at,
    }))
}

fn build_organization_payload(
    organization: &crate::models::organization::Model,
    organization_by_id: &HashMap<String, &crate::models::organization::Model>,
    character_by_id: &HashMap<String, &crate::models::character::Model>,
) -> Option<Value> {
    let org_character = character_by_id.get(&organization.character_id)?;
    let parent_org_name = organization
        .parent_org_id
        .as_ref()
        .and_then(|parent_id| organization_by_id.get(parent_id))
        .and_then(|parent_org| character_by_id.get(&parent_org.character_id))
        .map(|character| character.name.clone());

    Some(json!({
        "character_name": org_character.name,
        "parent_org_name": parent_org_name,
        "power_level": organization.power_level,
        "member_count": organization.member_count,
        "location": organization.location,
        "motto": organization.motto,
        "color": organization.color,
    }))
}

fn build_organization_member_payload(
    member: &crate::models::organization_member::Model,
    organization_by_id: &HashMap<String, &crate::models::organization::Model>,
    character_by_id: &HashMap<String, &crate::models::character::Model>,
) -> Option<Value> {
    let organization = organization_by_id.get(&member.organization_id)?;
    let organization_character = character_by_id.get(&organization.character_id)?;
    let member_character = character_by_id.get(&member.character_id)?;

    Some(json!({
        "organization_name": organization_character.name,
        "character_name": member_character.name,
        "position": member.position,
        "rank": member.rank,
        "status": member.status,
        "joined_at": member.joined_at,
        "loyalty": member.loyalty,
        "contribution": member.contribution,
        "notes": member.notes,
    }))
}

fn build_writing_style_payload(style: &crate::models::writing_style::Model) -> Value {
    json!({
        "name": style.name,
        "style_type": style.style_type,
        "preset_id": style.preset_id,
        "description": style.description,
        "prompt_content": style.prompt_content,
        "order_index": style.order_index,
    })
}

fn build_generation_history_payload(
    history: &crate::models::generation_history::Model,
    chapter_title_mapping: &HashMap<String, String>,
) -> Value {
    json!({
        "chapter_title": history
            .chapter_id
            .as_ref()
            .and_then(|chapter_id| chapter_title_mapping.get(chapter_id))
            .cloned(),
        "prompt": history.prompt,
        "generated_content": history.generated_content,
        "model": history.model,
        "tokens_used": history.tokens_used,
        "generation_time": history.generation_time,
        "created_at": format_optional_export_datetime(history.created_at),
    })
}

fn build_career_payload(career: &crate::models::career::Model) -> Value {
    json!({
        "name": career.name,
        "type": career.career_type,
        "description": career.description,
        "category": career.category,
        "stages": career.stages,
        "max_stage": career.max_stage,
        "requirements": career.requirements,
        "special_abilities": career.special_abilities,
        "worldview_rules": career.worldview_rules,
        "attribute_bonuses": career.attribute_bonuses,
        "source": career.source,
        "created_at": format_export_datetime(career.created_at),
    })
}

fn build_character_career_payload(
    item: &crate::models::character_career::Model,
    character_name_mapping: &HashMap<String, String>,
    career_name_mapping: &HashMap<String, String>,
) -> Option<Value> {
    Some(json!({
        "character_name": character_name_mapping.get(&item.character_id)?.clone(),
        "career_name": career_name_mapping.get(&item.career_id)?.clone(),
        "career_type": item.career_type,
        "current_stage": item.current_stage,
        "stage_progress": item.stage_progress.unwrap_or(0),
        "started_at": item.started_at,
        "reached_current_stage_at": item.reached_current_stage_at,
        "notes": item.notes,
    }))
}

fn build_story_memory_payload(
    memory: &crate::models::story_memory::Model,
    chapter_title_mapping: &HashMap<String, String>,
    character_name_mapping: &HashMap<String, String>,
) -> Value {
    let related_characters = memory.related_characters.as_ref().and_then(|value| {
        value.as_array().map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|character_id| {
                    character_name_mapping
                        .get(character_id)
                        .cloned()
                        .unwrap_or_else(|| character_id.to_string())
                })
                .collect::<Vec<_>>()
        })
    });

    json!({
        "chapter_title": memory
            .chapter_id
            .as_ref()
            .and_then(|chapter_id| chapter_title_mapping.get(chapter_id))
            .cloned(),
        "memory_type": memory.memory_type,
        "title": memory.title,
        "content": memory.content,
        "full_context": memory.full_context,
        "related_characters": related_characters,
        "related_locations": memory.related_locations,
        "tags": memory.tags,
        "importance_score": memory.importance_score.unwrap_or(0.5),
        "story_timeline": memory.story_timeline,
        "chapter_position": memory.chapter_position,
        "text_length": memory.text_length,
        "is_foreshadow": memory.is_foreshadow,
        "foreshadow_strength": memory.foreshadow_strength,
        "created_at": format_optional_export_datetime(memory.created_at),
    })
}

fn build_plot_analysis_payload(
    analysis: &crate::models::plot_analysis::Model,
    chapter_title_mapping: &HashMap<String, String>,
) -> Option<Value> {
    let chapter_title = chapter_title_mapping.get(&analysis.chapter_id).cloned()?;

    Some(json!({
        "chapter_title": chapter_title,
        "plot_stage": analysis.plot_stage,
        "conflict_level": analysis.conflict_level,
        "conflict_types": analysis.conflict_types,
        "emotional_tone": analysis.emotional_tone,
        "emotional_intensity": analysis.emotional_intensity,
        "emotional_curve": analysis.emotional_curve,
        "hooks": analysis.hooks,
        "hooks_count": analysis.hooks_count,
        "hooks_avg_strength": analysis.hooks_avg_strength,
        "foreshadows": analysis.foreshadows,
        "foreshadows_planted": analysis.foreshadows_planted,
        "foreshadows_resolved": analysis.foreshadows_resolved,
        "plot_points": analysis.plot_points,
        "plot_points_count": analysis.plot_points_count,
        "character_states": analysis.character_states,
        "scenes": analysis.scenes,
        "pacing": analysis.pacing,
        "overall_quality_score": analysis.overall_quality_score,
        "pacing_score": analysis.pacing_score,
        "engagement_score": analysis.engagement_score,
        "coherence_score": analysis.coherence_score,
        "analysis_report": analysis.analysis_report,
        "suggestions": analysis.suggestions,
        "word_count": analysis.word_count,
        "dialogue_ratio": analysis.dialogue_ratio,
        "description_ratio": analysis.description_ratio,
        "created_at": format_optional_export_datetime(analysis.created_at),
    }))
}

fn build_project_default_style_payload(
    style: Option<&writing_style::Model>,
    style_name_mapping: &HashMap<i32, String>,
    style_id: Option<i32>,
) -> Value {
    if let Some(style) = style {
        json!({ "style_name": style.name })
    } else if let Some(style_id) = style_id {
        json!({ "style_name": style_name_mapping.get(&style_id).cloned() })
    } else {
        Value::Null
    }
}

fn parse_json_text(raw: Option<&str>) -> Option<Value> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    serde_json::from_str(raw).ok()
}

pub fn build_project_export_txt_content(
    project: &crate::models::project::Model,
    chapters: &[chapter::Model],
) -> String {
    let mut text = String::new();
    text.push_str(&format!("项目：{}\n", project.title));
    if let Some(ref desc) = project.description {
        if !desc.is_empty() {
            text.push_str(&format!("简介：{}\n", desc));
        }
    }
    if let Some(ref theme) = project.theme {
        if !theme.is_empty() {
            text.push_str(&format!("主题：{}\n", theme));
        }
    }
    if let Some(ref genre) = project.genre {
        if !genre.is_empty() {
            text.push_str(&format!("类型：{}\n", genre));
        }
    }
    text.push_str("\n\n");

    for ch in chapters {
        text.push_str(&format!("第 {} 章：{}\n\n", ch.chapter_number, ch.title));
        if let Some(ref content) = ch.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    text
}

pub fn build_safe_project_export_json_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("project_{}.json", safe_title.trim().replace(' ', "_"))
}

pub fn build_safe_project_export_txt_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.txt", safe_title)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::json;

    use crate::models::{
        career, chapter, character, character_career, generation_history, organization,
        organization_member, outline, plot_analysis, project, project_default_style, relationship,
        story_memory, writing_style,
    };
    use crate::services::project_export_query_service::{
        ProjectExportContext, ProjectExportOptions,
    };

    use super::{
        build_project_export_data_payload, build_project_export_txt_content,
        build_safe_project_export_json_filename, build_safe_project_export_txt_filename,
    };

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn project_model() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试 项目/Title".to_string(),
            description: Some("项目简介".to_string()),
            theme: Some("主题测试".to_string()),
            genre: Some("奇幻".to_string()),
            target_words: 100000,
            current_words: 1234,
            status: "draft".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: "traditional".to_string(),
            world_time_period: Some("架空近古".to_string()),
            world_location: Some("北境王城".to_string()),
            world_atmosphere: Some("冷峻".to_string()),
            world_rules: Some("月相影响法术".to_string()),
            chapter_count: Some(1),
            narrative_perspective: Some("third_person".to_string()),
            character_count: 1,
            default_creative_mode: Some("balanced".to_string()),
            default_story_focus: Some("advance_plot".to_string()),
            default_plot_stage: Some("development".to_string()),
            default_story_creation_brief: Some("保持主线推进".to_string()),
            default_quality_preset: Some("balanced".to_string()),
            default_quality_notes: Some("偏重节奏".to_string()),
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: Some("这里是正文".to_string()),
            summary: Some("章节摘要".to_string()),
            word_count: 5,
            status: "draft".to_string(),
            outline_id: Some("outline-1".to_string()),
            sub_index: 0,
            expansion_plan: Some("{\"beats\":[\"a\"]}".to_string()),
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn context() -> ProjectExportContext {
        ProjectExportContext {
            project: project_model(),
            chapters: vec![chapter_model()],
            characters: vec![character::Model {
                id: "character-1".to_string(),
                project_id: "project-1".to_string(),
                name: "林青".to_string(),
                age: Some("19".to_string()),
                gender: Some("女".to_string()),
                is_organization: false,
                role_type: Some("supporting".to_string()),
                personality: Some("冷静".to_string()),
                background: Some("山门弃徒".to_string()),
                appearance: Some("青衣长剑".to_string()),
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: None,
                state_updated_chapter: None,
                main_career_id: Some("career-1".to_string()),
                main_career_stage: Some(2),
                sub_careers: None,
                avatar_url: None,
                traits: Some("[\"敏锐\",\"谨慎\"]".to_string()),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            outlines: vec![outline::Model {
                id: "outline-1".to_string(),
                project_id: "project-1".to_string(),
                title: "第一卷总纲".to_string(),
                content: Some("大纲内容".to_string()),
                structure: Some("三幕式".to_string()),
                order_index: Some(1),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            relationships: vec![relationship::Model {
                id: "rel-1".to_string(),
                project_id: "project-1".to_string(),
                character_from_id: "character-1".to_string(),
                character_to_id: "character-1".to_string(),
                relationship_type_id: None,
                relationship_name: Some("镜像".to_string()),
                intimacy_level: 50,
                status: "active".to_string(),
                description: Some("自我映照".to_string()),
                started_at: Some("chapter-1".to_string()),
                ended_at: None,
                source: "imported".to_string(),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            organizations: Vec::new(),
            organization_members: Vec::new(),
            writing_styles: vec![writing_style::Model {
                id: 7,
                user_id: Some("user-1".to_string()),
                name: "冷峻风格".to_string(),
                style_type: "custom".to_string(),
                preset_id: None,
                description: Some("风格描述".to_string()),
                prompt_content: "提示词".to_string(),
                order_index: 0,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }],
            generation_history: vec![generation_history::Model {
                id: "history-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: Some("chapter-1".to_string()),
                prompt: Some("提示".to_string()),
                generated_content: Some("生成内容".to_string()),
                model: Some("gpt-test".to_string()),
                tokens_used: Some(128),
                generation_time: Some(1.5),
                created_at: Some(test_datetime()),
            }],
            careers: vec![career::Model {
                id: "career-1".to_string(),
                project_id: "project-1".to_string(),
                name: "剑修".to_string(),
                career_type: "main".to_string(),
                description: Some("职业描述".to_string()),
                category: Some("战斗".to_string()),
                stages: "[\"入门\",\"大成\"]".to_string(),
                max_stage: 10,
                requirements: None,
                special_abilities: None,
                worldview_rules: Some("以剑证道".to_string()),
                attribute_bonuses: None,
                source: "ai".to_string(),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            character_careers: vec![character_career::Model {
                id: "cc-1".to_string(),
                character_id: "character-1".to_string(),
                career_id: "career-1".to_string(),
                career_type: "main".to_string(),
                current_stage: 2,
                stage_progress: Some(30),
                started_at: None,
                reached_current_stage_at: None,
                notes: Some("主修本命剑".to_string()),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            story_memories: vec![story_memory::Model {
                id: "memory-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: Some("chapter-1".to_string()),
                memory_type: "foreshadow".to_string(),
                title: Some("初遇".to_string()),
                content: "雨夜初见".to_string(),
                full_context: None,
                related_characters: Some(json!(["character-1"])),
                related_locations: Some(json!(["后山"])),
                tags: Some(json!(["伏笔"])),
                importance_score: Some(0.9),
                story_timeline: 1,
                chapter_position: 20,
                text_length: 4,
                is_foreshadow: 1,
                foreshadow_resolved_at: None,
                foreshadow_strength: Some(0.8),
                vector_id: None,
                embedding_model: None,
                created_at: Some(test_datetime()),
                updated_at: Some(test_datetime()),
            }],
            plot_analysis: vec![plot_analysis::Model {
                id: "analysis-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: "chapter-1".to_string(),
                plot_stage: Some("opening".to_string()),
                conflict_level: Some(3),
                conflict_types: Some(json!(["external"])),
                emotional_tone: Some("tense".to_string()),
                emotional_intensity: Some(0.7),
                emotional_curve: Some(json!({"start": 0.2, "end": 0.7})),
                hooks: Some(json!([{"text": "悬念"}])),
                hooks_count: 1,
                hooks_avg_strength: Some(0.8),
                foreshadows: Some(json!([{"text": "暗线"}])),
                foreshadows_planted: 1,
                foreshadows_resolved: 0,
                plot_points: Some(json!([{"text": "转折"}])),
                plot_points_count: 1,
                character_states: Some(json!([{"name": "林青"}])),
                scenes: Some(json!([{"name": "雨夜"}])),
                pacing: Some("fast".to_string()),
                overall_quality_score: Some(88.0),
                pacing_score: Some(86.0),
                engagement_score: Some(87.0),
                coherence_score: Some(89.0),
                analysis_report: Some("分析报告".to_string()),
                suggestions: Some(json!(["加强冲突"])),
                word_count: Some(1200),
                dialogue_ratio: Some(0.3),
                description_ratio: Some(0.7),
                created_at: Some(test_datetime()),
            }],
            project_default_style: Some(project_default_style::Model {
                id: 1,
                project_id: "project-1".to_string(),
                style_id: 7,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }),
            project_default_style_style: Some(writing_style::Model {
                id: 7,
                user_id: Some("user-1".to_string()),
                name: "冷峻风格".to_string(),
                style_type: "custom".to_string(),
                preset_id: None,
                description: None,
                prompt_content: "提示词".to_string(),
                order_index: 0,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }),
        }
    }

    #[test]
    fn build_project_export_data_payload_matches_python_style_shape() {
        let payload = build_project_export_data_payload(
            &context(),
            &ProjectExportOptions {
                include_generation_history: true,
                include_writing_styles: true,
                include_careers: true,
                include_memories: true,
                include_plot_analysis: true,
            },
        );

        assert_eq!(payload["version"], "1.1.0");
        assert_eq!(payload["project"]["title"], "测试 项目/Title");
        assert_eq!(payload["project"]["default_story_focus"], "advance_plot");
        assert_eq!(payload["chapters"][0]["outline_title"], "第一卷总纲");
        assert_eq!(payload["chapters"][0]["expansion_plan"]["beats"][0], "a");
        assert_eq!(payload["characters"][0]["name"], "林青");
        assert_eq!(payload["characters"][0]["traits"][0], "敏锐");
        assert_eq!(payload["relationships"][0]["relationship_name"], "镜像");
        assert_eq!(payload["writing_styles"][0]["name"], "冷峻风格");
        assert_eq!(payload["generation_history"][0]["chapter_title"], "第一章");
        assert_eq!(payload["careers"][0]["name"], "剑修");
        assert_eq!(payload["character_careers"][0]["career_name"], "剑修");
        assert_eq!(
            payload["story_memories"][0]["related_characters"][0],
            "林青"
        );
        assert_eq!(payload["plot_analysis"][0]["chapter_title"], "第一章");
        assert_eq!(payload["project_default_style"]["style_name"], "冷峻风格");
    }

    #[test]
    fn build_project_export_data_payload_respects_optional_flags() {
        let payload = build_project_export_data_payload(
            &context(),
            &ProjectExportOptions {
                include_generation_history: false,
                include_writing_styles: false,
                include_careers: false,
                include_memories: false,
                include_plot_analysis: false,
            },
        );

        assert_eq!(payload["generation_history"], json!([]));
        assert_eq!(payload["writing_styles"], json!([]));
        assert_eq!(payload["careers"], json!([]));
        assert_eq!(payload["character_careers"], json!([]));
        assert_eq!(payload["story_memories"], json!([]));
        assert_eq!(payload["plot_analysis"], json!([]));
        assert_eq!(payload["project_default_style"]["style_name"], "冷峻风格");
    }

    #[test]
    fn build_project_export_txt_content_keeps_existing_text_format() {
        let project = project_model();
        let chapters = vec![chapter_model()];

        let text = build_project_export_txt_content(&project, &chapters);

        assert!(text.contains("项目：测试 项目/Title"));
        assert!(text.contains("简介：项目简介"));
        assert!(text.contains("主题：主题测试"));
        assert!(text.contains("类型：奇幻"));
        assert!(text.contains("第 1 章：第一章"));
        assert!(text.contains("这里是正文"));
        assert!(text.contains("\n\n---\n\n"));
    }

    #[test]
    fn build_safe_project_export_filenames_keep_existing_normalization() {
        assert_eq!(
            build_safe_project_export_json_filename("测试 项目/Title"),
            "project_______Title.json"
        );
        assert_eq!(
            build_safe_project_export_txt_filename("测试 项目/Title"),
            "______Title.txt"
        );
    }
}
