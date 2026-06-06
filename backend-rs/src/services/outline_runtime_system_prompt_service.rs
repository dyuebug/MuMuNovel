use crate::models::project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineRuntimeStage {
    Opening,
    Continuation,
}

impl OutlineRuntimeStage {
    fn phase_text(self) -> &'static str {
        match self {
            Self::Opening => "开局阶段",
            Self::Continuation => "续写阶段",
        }
    }
}

pub(crate) fn build_outline_runtime_system_prompt(
    project: &project::Model,
    chapter_count: usize,
    stage: OutlineRuntimeStage,
) -> String {
    format!(
        "【大纲生成阶段】\n\
- 当前阶段：{}\n\
- 本轮目标章节数：{}\n\n\
【世界观锚点】\n\
- 时间背景：{}\n\
- 地理位置：{}\n\
- 氛围基调：{}\n\
- 世界规则：{}\n\n\
【剧情质量硬约束】\n\
- 每章至少包含一次“目标受阻→角色选择→代价/新麻烦”的推进链\n\
- 每章至少给一个可直接写成对白场景的冲突对话钩子（双方立场有差异）\n\
- 每章至少让一位核心配角出现反预期行为，并补一句动机说明\n\
- 世界规则必须作用于事件结果，不能只做名词陈列\n\
- 若同段出现2个及以上术语，需在三句内补一条通俗解释思路\n\
- 摘要优先写“发生了什么”，避免空泛总结和模板化衔接词\n\n\
【输出补充建议】\n\
- 可在章节对象中补充字段：conflict_line / decision / cost / rule_impact / dialogue_hook / character_turns\n\
- 新增字段应与 summary 保持一致，不得互相矛盾\n",
        stage.phase_text(),
        chapter_count,
        project.world_time_period.as_deref().unwrap_or("未设定"),
        project.world_location.as_deref().unwrap_or("未设定"),
        project.world_atmosphere.as_deref().unwrap_or("未设定"),
        project.world_rules.as_deref().unwrap_or("未设定"),
    )
}

#[cfg(test)]
mod tests {
    use super::{build_outline_runtime_system_prompt, OutlineRuntimeStage};
    use crate::models::project;
    use chrono::NaiveDateTime;

    fn project_model() -> project::Model {
        project::Model {
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
        }
    }

    #[test]
    fn should_build_opening_outline_runtime_system_prompt() {
        let prompt =
            build_outline_runtime_system_prompt(&project_model(), 3, OutlineRuntimeStage::Opening);

        assert!(prompt.contains("当前阶段：开局阶段"));
        assert!(prompt.contains("本轮目标章节数：3"));
        assert!(prompt.contains("时间背景：乱世末年"));
        assert!(prompt.contains("每章至少让一位核心配角出现反预期行为"));
        assert!(prompt.contains(
            "conflict_line / decision / cost / rule_impact / dialogue_hook / character_turns"
        ));
    }

    #[test]
    fn should_build_continuation_outline_runtime_system_prompt_with_defaults() {
        let mut project = project_model();
        project.world_time_period = None;
        project.world_location = None;
        project.world_atmosphere = None;
        project.world_rules = None;

        let prompt =
            build_outline_runtime_system_prompt(&project, 5, OutlineRuntimeStage::Continuation);

        assert!(prompt.contains("当前阶段：续写阶段"));
        assert!(prompt.contains("本轮目标章节数：5"));
        assert!(prompt.contains("时间背景：未设定"));
        assert!(prompt.contains("世界规则：未设定"));
        assert!(prompt.contains("摘要优先写“发生了什么”"));
    }
}
