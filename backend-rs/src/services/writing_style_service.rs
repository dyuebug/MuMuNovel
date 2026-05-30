use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::{json, Value};

use crate::models::{project, project_default_style, writing_style};
use crate::services::writing_style_request_service::{
    CreateWritingStyleRequest, UpdateWritingStyleRequest,
};

struct PresetDefinition {
    preset_id: &'static str,
    name: &'static str,
    description: &'static str,
    prompt_content: &'static str,
    order_index: i32,
}

const BASE_PRESET_DEFINITIONS: &[PresetDefinition] = &[
    PresetDefinition {
        preset_id: "natural",
        name: "自然流畅",
        description: "自然流畅的叙事风格，适合现代都市、现实题材",
        prompt_content: r#"写作风格建议：
1. 叙述像身边人讲故事，口语自然，不端着
2. 长短句交替，关键处用短句提速，情绪段落可适度放长
3. 情绪落在动作、停顿和细节里，少用空泛形容词
4. 偶尔可用贴场景的网络表达，点到即止，避免生硬玩梗"#,
        order_index: 1,
    },
    PresetDefinition {
        preset_id: "classical",
        name: "古典优雅",
        description: "古典文雅的写作风格，适合古装、仙侠题材",
        prompt_content: r#"写作风格建议：
1. 以典雅白话为底，句式有古风韵味但保持易读
2. 长句铺意境，短句落情绪，读感要有起伏
3. 意象与用典适度，宁少勿滥，避免堆砌辞藻
4. 人物对话符合时代身份，不要突然冒出现代网络口头禅"#,
        order_index: 2,
    },
    PresetDefinition {
        preset_id: "modern",
        name: "现代简约",
        description: "现代简约风格，适合轻小说、网文快节奏叙事",
        prompt_content: r#"写作风格建议：
1. 语言干净直接，信息清晰，像当下网文读者熟悉的叙述节奏
2. 多用对话和行动推进剧情，段落利落，少空转
3. 长短句混用，转折处可用短句“收一下”，增强冲击
4. 可少量加入自然口语和轻梗，但必须服务人物与情境"#,
        order_index: 3,
    },
    PresetDefinition {
        preset_id: "literary",
        name: "文艺细腻",
        description: "文艺细腻风格，注重心理描写和氛围营造",
        prompt_content: r#"写作风格建议：
1. 文字细腻但不矫情，像在轻声讲一段真事
2. 长句描摹氛围，短句点破心绪，让情感有呼吸感
3. 心理描写要具体可感，避免大段抽象抒情
4. 比喻和修辞克制使用，读起来顺滑，不要“为了文艺而文艺”"#,
        order_index: 4,
    },
    PresetDefinition {
        preset_id: "suspense",
        name: "紧张悬疑",
        description: "紧张悬疑风格，适合推理、惊悚题材",
        prompt_content: r#"写作风格建议：
1. 信息要清楚，氛围要压迫，读者能看懂也会紧张
2. 长句铺线索，短句制造顿挫和压迫感
3. 悬念与伏笔要可回收，关键信息别故弄玄虚
4. 对话贴近人物当下状态，可有口语感，但不插无关玩梗"#,
        order_index: 5,
    },
    PresetDefinition {
        preset_id: "humorous",
        name: "幽默诙谐",
        description: "幽默诙谐风格，适合轻松搞笑题材",
        prompt_content: r#"写作风格建议：
1. 语气轻松机灵，像朋友互怼互逗，别油腻
2. 包袱尽量来自人物关系和情境反差，不靠硬抖段子
3. 长短句配合节奏，笑点后留一点“回弹空间”
4. 网络热梗可用但要新鲜、克制、贴场景，避免连续刷梗"#,
        order_index: 6,
    },
];

const LOW_AI_PRESET_DEFINITIONS: &[PresetDefinition] = &[
    PresetDefinition {
        preset_id: "low_ai_life",
        name: "低AI生活化",
        description:
            "低AI感的生活化网文叙事，强调日常现场里的眼前麻烦、真人对白、动作反馈与带余波的柔和章尾",
        prompt_content: r#"写作风格建议：
1. 开场可以更贴近日常现场，但前段必须让读者看见眼前麻烦、情绪摩擦、秘密失衡或局面变化，少用背景概述起手
2. 叙述像真人在讲亲历故事，优先写正在发生的动作、人物反应和场面变化，再补必要解释，不要写成说明文
3. 日常戏也要让“动作/试探→反馈→余波或代价”可见，哪怕没有大冲突，也要写出关系变化、情绪波动或下一步压力
4. 对话要有停顿、改口、打断、潜台词和角色声线差异，允许少量口语毛边，但不要写成轮流讲道理
5. 句式长短交替，保留生活噪声与嘴感；别整段一个节拍，也别把每句都打磨成金句、口号或过分工整的排比
6. 每个场景尽量给一个可视化抓手：动作、物件、身体反应、环境细节，少用空泛形容词堆情绪；情绪推进优先靠细节与反应，而不是作者替人物总结
7. 遇到设定、术语或背景信息时，在几句内用角色追问、吐槽或现场反应补一句人话解释，不要整段灌说明，也不要写成讲义
8. 章尾可以更柔和，但至少留下情绪余震、关系余波、秘密悬挂或下一步动作牵引，避免鸡汤总结、预告腔和模板化收束
9. 比喻要克制：同一自然段不要连着堆“像……/仿佛/像……一样”；能直接写动作、表情、声音和结果，就不要先做抽象比喻
10. 少用“下一秒/那一瞬/忽然/不是……而是……”这类固定推进句，允许出现朴素、直接、没那么漂亮的过渡句"#,
        order_index: 7,
    },
    PresetDefinition {
        preset_id: "low_ai_serial",
        name: "低AI连载感",
        description: "低AI感的番茄连载风格，强调快开场、目标阻力选择链、小爽点反馈与顺滑追更钩子",
        prompt_content: r#"写作风格建议：
1. 开篇尽量在150-300字内落到异常、任务压力、关系摩擦、危险逼近或信息缺口，让读者迅速知道“这一章为什么要看”
2. 正文优先写当下正在发生的动作、人物反应和局面变化，再补必要解释，避免大段概述替代现场
3. 单章尽量形成“开场钩子→冲突推进→小爆发→章尾牵引”的节奏，至少让目标、阻力、选择和即时后果可见，别只报结果不写过程
4. 每章最好给一个可感知的小爽点或阶段回报，并写出“铺垫→爆发→反馈/余波”；哪怕不是打脸，也要让读者感到局面真的被推动
5. 句子可以更短、更口语、更有现场颗粒感，但对白仍要分角色声线，保留停顿、反问、改口和话里有话，不要所有人说成同一种腔调
6. 配角不必长篇输出，只要做出会改变局面的主动选择就算有效推进；关键推进尽量带出新麻烦、损失、筹码变化或关系变化
7. 遇到术语、设定或规则时，三句内补一句读者能听懂的人话解释，可借追问、吐槽或身体反馈带出，别写成讲义，也别让设定说明压过剧情
8. 章尾优先停在信息缺口、危险临门、身份反转或选择未决上，宁可留动作停顿，也别用总结腔、鸡汤句收束
9. 比喻要省着用：单段尽量只保留1个强比喻，别把疼痛、危险、异常都写成“像什么”；能直写动作后果就直写
10. 慎用“下一秒/那一瞬/忽然/不是……而是……”等模板句式，别让整章每段都像在卡点或凹质感"#,
        order_index: 8,
    },
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

async fn ensure_base_preset_styles(
    db: &DatabaseConnection,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now().naive_utc();
    let existing_styles = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.is_null())
        .all(db)
        .await?;

    let existing_preset_ids = existing_styles
        .into_iter()
        .filter_map(|style| style.preset_id)
        .collect::<std::collections::HashSet<_>>();

    let mut modified = false;
    for definition in BASE_PRESET_DEFINITIONS {
        if existing_preset_ids.contains(definition.preset_id) {
            continue;
        }

        let model = writing_style::ActiveModel {
            user_id: Set(None),
            name: Set(definition.name.to_string()),
            style_type: Set("preset".to_string()),
            preset_id: Set(Some(definition.preset_id.to_string())),
            description: Set(Some(definition.description.to_string())),
            prompt_content: Set(definition.prompt_content.to_string()),
            order_index: Set(definition.order_index),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        model.insert(db).await?;
        modified = true;
    }

    Ok(modified)
}

async fn sync_low_ai_presets(
    db: &DatabaseConnection,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now().naive_utc();
    let target_preset_ids = LOW_AI_PRESET_DEFINITIONS
        .iter()
        .map(|definition| definition.preset_id)
        .collect::<Vec<_>>();

    let existing_styles = writing_style::Entity::find()
        .filter(writing_style::Column::PresetId.is_in(target_preset_ids))
        .filter(
            writing_style::Column::UserId
                .is_null()
                .or(writing_style::Column::StyleType.eq("preset")),
        )
        .all(db)
        .await?;

    let mut modified = false;
    let mut has_global = LOW_AI_PRESET_DEFINITIONS
        .iter()
        .map(|definition| (definition.preset_id, false))
        .collect::<std::collections::HashMap<_, _>>();

    for style in existing_styles {
        let Some(definition) = LOW_AI_PRESET_DEFINITIONS
            .iter()
            .find(|candidate| Some(candidate.preset_id) == style.preset_id.as_deref())
        else {
            continue;
        };

        if style.user_id.is_none() {
            has_global.insert(definition.preset_id, true);
        }

        let mut active: writing_style::ActiveModel = style.clone().into();
        let mut changed = false;

        if style.name != definition.name {
            active.name = Set(definition.name.to_string());
            changed = true;
        }
        if style.style_type != "preset" {
            active.style_type = Set("preset".to_string());
            changed = true;
        }
        if style.description.as_deref() != Some(definition.description) {
            active.description = Set(Some(definition.description.to_string()));
            changed = true;
        }
        if style.prompt_content != definition.prompt_content {
            active.prompt_content = Set(definition.prompt_content.to_string());
            changed = true;
        }
        if style.user_id.is_none() && style.order_index != definition.order_index {
            active.order_index = Set(definition.order_index);
            changed = true;
        }

        if changed {
            active.updated_at = Set(now);
            active.update(db).await?;
            modified = true;
        }
    }

    let existing_global_styles = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.is_null())
        .order_by_desc(writing_style::Column::OrderIndex)
        .one(db)
        .await?;
    let mut max_order = existing_global_styles
        .map(|style| style.order_index)
        .unwrap_or(0);

    for definition in LOW_AI_PRESET_DEFINITIONS {
        if has_global
            .get(definition.preset_id)
            .copied()
            .unwrap_or(false)
        {
            continue;
        }

        let model = writing_style::ActiveModel {
            user_id: Set(None),
            name: Set(definition.name.to_string()),
            style_type: Set("preset".to_string()),
            preset_id: Set(Some(definition.preset_id.to_string())),
            description: Set(Some(definition.description.to_string())),
            prompt_content: Set(definition.prompt_content.to_string()),
            order_index: Set(std::cmp::max(definition.order_index, max_order + 1)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        model.insert(db).await?;
        max_order += 1;
        modified = true;
    }

    Ok(modified)
}

async fn ensure_preset_styles(
    db: &DatabaseConnection,
) -> Result<Vec<writing_style::Model>, Box<dyn std::error::Error + Send + Sync>> {
    ensure_base_preset_styles(db).await?;
    sync_low_ai_presets(db).await?;

    let styles = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.is_null())
        .order_by_asc(writing_style::Column::OrderIndex)
        .all(db)
        .await?;

    Ok(styles)
}

async fn get_project_default_style_id(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<Option<i32>, Box<dyn std::error::Error + Send + Sync>> {
    let default_style = project_default_style::Entity::find()
        .filter(project_default_style::Column::ProjectId.eq(project_id))
        .one(db)
        .await?;

    Ok(default_style.map(|model| model.style_id))
}

async fn ensure_project_access(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let project_model = project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await?;

    if project_model.is_none() {
        return Err("项目不存在或无权访问".into());
    }

    Ok(())
}

pub struct WritingStyleService;

impl WritingStyleService {
    pub async fn list_presets(
        db: &DatabaseConnection,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let styles = ensure_preset_styles(db).await?;

        Ok(json!(styles
            .iter()
            .map(|style| json!({
                "id": style.id,
                "preset_id": style.preset_id,
                "name": style.name,
                "description": style.description,
                "prompt_content": style.prompt_content,
                "style_type": style.style_type,
                "order_index": style.order_index,
            }))
            .collect::<Vec<_>>()))
    }

    pub async fn list_user_styles(
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let preset_styles = ensure_preset_styles(db).await?;
        let user_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .order_by_asc(writing_style::Column::OrderIndex)
            .all(db)
            .await?;

        let items: Vec<Value> = preset_styles
            .iter()
            .chain(user_styles.iter())
            .map(|style| style_to_value(style, false))
            .collect();

        Ok(json!({
            "styles": items,
            "items": items,
            "total": preset_styles.len() + user_styles.len(),
        }))
    }

    pub async fn list_project_styles(
        db: &DatabaseConnection,
        user_id: &str,
        project_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        ensure_project_access(db, user_id, project_id).await?;

        let preset_styles = ensure_preset_styles(db).await?;
        let user_styles = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .order_by_asc(writing_style::Column::OrderIndex)
            .all(db)
            .await?;
        let default_style_id = get_project_default_style_id(db, project_id).await?;

        let items: Vec<Value> = preset_styles
            .iter()
            .chain(user_styles.iter())
            .map(|style| style_to_value(style, Some(style.id) == default_style_id))
            .collect();

        Ok(json!({
            "styles": items,
            "items": items,
            "total": preset_styles.len() + user_styles.len(),
        }))
    }

    pub async fn get_style(
        db: &DatabaseConnection,
        user_id: &str,
        style_id: i32,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let style = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("写作风格不存在")?;

        if style.user_id.is_some() && style.user_id.as_deref() != Some(user_id) {
            return Err("无权查看其他用户的风格".into());
        }

        let is_default = project_default_style::Entity::find()
            .filter(project_default_style::Column::StyleId.eq(style_id))
            .one(db)
            .await?
            .is_some();

        Ok(style_to_value(&style, is_default))
    }

    pub async fn create_style(
        db: &DatabaseConnection,
        user_id: &str,
        request: &CreateWritingStyleRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        ensure_preset_styles(db).await?;

        let preset_id = request.preset_id().map(str::to_string);
        let mut name = request.name().map(str::to_string);
        let mut description = request.description().map(str::to_string);
        let mut prompt_content = request.prompt_content().map(str::to_string);

        if let Some(ref preset_id) = preset_id {
            let preset_style = writing_style::Entity::find()
                .filter(writing_style::Column::UserId.is_null())
                .filter(writing_style::Column::PresetId.eq(preset_id.clone()))
                .one(db)
                .await?;

            let preset_style =
                preset_style.ok_or_else(|| format!("预设风格 '{}' 不存在", preset_id))?;

            if name.is_none() {
                name = Some(preset_style.name);
            }
            if description.is_none() {
                description = preset_style.description;
            }
            if prompt_content.is_none() {
                prompt_content = Some(preset_style.prompt_content);
            }
        }

        let name = name.filter(|value| !value.trim().is_empty());
        let prompt_content = prompt_content.filter(|value| !value.trim().is_empty());
        if name.is_none() || prompt_content.is_none() {
            return Err("name 和 prompt_content 是必填字段".into());
        }

        let user_style_count = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .count(db)
            .await? as i32;
        let now = Utc::now().naive_utc();
        let model = writing_style::ActiveModel {
            user_id: Set(Some(user_id.to_string())),
            name: Set(name.unwrap_or_default()),
            style_type: Set(request.style_type().map(str::to_string).unwrap_or_else(|| {
                if preset_id.is_some() {
                    "preset".to_string()
                } else {
                    "custom".to_string()
                }
            })),
            preset_id: Set(preset_id),
            description: Set(description),
            prompt_content: Set(prompt_content.unwrap_or_default()),
            order_index: Set(user_style_count + 1),
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
        request: &UpdateWritingStyleRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("写作风格不存在")?;

        if existing.user_id.is_none() {
            return Err("不能修改全局预设风格，只能修改自定义风格".into());
        }
        if existing.user_id.as_deref() != Some(user_id) {
            return Err("无权修改其他用户的风格".into());
        }

        let mut active: writing_style::ActiveModel = existing.into();
        let mut content_changed = false;

        if let Some(value) = request.name() {
            active.name = Set(value.to_string());
            content_changed = true;
        }
        if let Some(value) = request.description() {
            active.description = Set(Some(value.to_string()));
            content_changed = true;
        }
        if let Some(value) = request.prompt_content() {
            active.prompt_content = Set(value.to_string());
            content_changed = true;
        }
        if let Some(value) = request.order_index() {
            active.order_index = Set(value);
        }
        if content_changed {
            active.style_type = Set("custom".to_string());
        }
        active.updated_at = Set(Utc::now().naive_utc());

        let saved = active.update(db).await?;
        let is_default = project_default_style::Entity::find()
            .filter(project_default_style::Column::StyleId.eq(style_id))
            .one(db)
            .await?
            .is_some();

        Ok(style_to_value(&saved, is_default))
    }

    pub async fn delete_style(
        db: &DatabaseConnection,
        user_id: &str,
        style_id: i32,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let existing = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("写作风格不存在")?;

        if existing.user_id.is_none() {
            return Err("不能删除全局预设风格，只能删除自定义风格".into());
        }
        if existing.user_id.as_deref() != Some(user_id) {
            return Err("无权删除其他用户的风格".into());
        }

        let is_default = project_default_style::Entity::find()
            .filter(project_default_style::Column::StyleId.eq(style_id))
            .one(db)
            .await?
            .is_some();
        if is_default {
            return Err("不能删除默认风格，请先设置其他风格为默认".into());
        }

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
        ensure_project_access(db, user_id, project_id).await?;

        let style = writing_style::Entity::find_by_id(style_id)
            .one(db)
            .await?
            .ok_or("写作风格不存在")?;
        if style.user_id.is_some() && style.user_id.as_deref() != Some(user_id) {
            return Err("无权操作其他用户的风格".into());
        }

        let now = Utc::now().naive_utc();
        let existing_default = project_default_style::Entity::find()
            .filter(project_default_style::Column::ProjectId.eq(project_id))
            .one(db)
            .await?;
        if let Some(existing_default) = existing_default {
            project_default_style::Entity::delete_by_id(existing_default.id)
                .exec(db)
                .await?;
        }

        let model = project_default_style::ActiveModel {
            project_id: Set(project_id.to_string()),
            style_id: Set(style_id),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        model.insert(db).await?;

        Ok(json!({
            "message": "默认风格设置成功",
            "project_id": project_id,
            "style_id": style_id,
            "style_name": style.name,
        }))
    }

    pub async fn initialize_defaults(
        db: &DatabaseConnection,
        user_id: &str,
        project_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        ensure_project_access(db, user_id, project_id).await?;
        self::WritingStyleService::list_project_styles(db, user_id, project_id).await
    }
}
