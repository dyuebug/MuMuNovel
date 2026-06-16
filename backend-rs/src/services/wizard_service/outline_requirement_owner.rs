fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

const OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT: usize = 3600;
const OUTLINE_RUNTIME_BASE_REQUIREMENTS_LIMIT: usize = 520;
const OUTLINE_RUNTIME_STORY_CREATION_BRIEF_LIMIT: usize = 220;
const OUTLINE_RUNTIME_QUALITY_REPAIR_GUIDANCE_LIMIT: usize = 320;
const OUTLINE_RUNTIME_MEMORY_GUIDANCE_LIMIT: usize = 820;
const OUTLINE_RUNTIME_STORY_LONG_TERM_GOAL_LIMIT: usize = 220;
const OUTLINE_RUNTIME_STORY_CHARACTER_FOCUS_ANCHOR_LIMIT: usize = 180;
const OUTLINE_RUNTIME_STORY_FORESHADOW_PAYOFF_PLAN_LIMIT: usize = 240;
const OUTLINE_RUNTIME_STORY_RELATIONSHIP_STATE_LEDGER_LIMIT: usize = 220;
const OUTLINE_RUNTIME_STORY_CHARACTER_STATE_LEDGER_LIMIT: usize = 220;
const OUTLINE_RUNTIME_QUALITY_TREND_GUIDANCE_LIMIT: usize = 240;
const OUTLINE_RUNTIME_STORY_ORGANIZATION_STATE_LEDGER_LIMIT: usize = 200;
const OUTLINE_RUNTIME_STORY_CAREER_STATE_LEDGER_LIMIT: usize = 200;
const OUTLINE_RUNTIME_STORY_FORESHADOW_STATE_LEDGER_LIMIT: usize = 200;
const OUTLINE_RUNTIME_STORY_PACING_BUDGET_LIMIT: usize = 180;
const OUTLINE_RUNTIME_STORY_VOLUME_PACING_LIMIT: usize = 160;
const OUTLINE_COMPACT_GUIDANCE_BLOCK_LIMIT: usize = 110;

fn ellipsize_story_runtime_text(text: &str, limit: usize) -> String {
    let normalized = text.trim();
    if limit == 0 {
        return String::new();
    }
    let normalized_chars: Vec<char> = normalized.chars().collect();
    if normalized_chars.len() <= limit {
        return normalized.to_string();
    }
    if limit <= 3 {
        return normalized_chars.into_iter().take(limit).collect();
    }

    let prefix = normalized_chars
        .into_iter()
        .take(limit - 3)
        .collect::<String>();
    format!("{}...", prefix.trim_end())
}

fn truncate_story_runtime_block(block: &str, limit: usize) -> String {
    let normalized = block.trim();
    if normalized.is_empty() || limit == 0 {
        return String::new();
    }

    let normalized_chars = normalized.chars().count();
    if normalized_chars <= limit {
        return normalized.to_string();
    }

    let lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }
    if lines.len() == 1 {
        return ellipsize_story_runtime_text(&lines[0], limit);
    }

    let head = lines[0].clone();
    if head.chars().count() >= limit {
        return ellipsize_story_runtime_text(&head, limit);
    }

    let mut kept_lines = vec![head];
    let mut current_length = kept_lines[0].chars().count();
    for line in lines.into_iter().skip(1) {
        let separator_length = 1usize;
        let line_length = line.chars().count();
        let projected_length = current_length + separator_length + line_length;
        if projected_length <= limit {
            kept_lines.push(line);
            current_length = projected_length;
            continue;
        }

        let remaining = limit.saturating_sub(current_length + separator_length);
        if remaining > 6 {
            kept_lines.push(ellipsize_story_runtime_text(&line, remaining));
        } else if let Some(last_line) = kept_lines.last_mut() {
            *last_line = format!("{}...", last_line.trim_end_matches('.').trim_end());
        }
        break;
    }

    kept_lines.join("\n")
}

fn join_story_runtime_blocks_with_budget(blocks: &[String], total_limit: Option<usize>) -> String {
    let normalized_blocks = blocks
        .iter()
        .map(|block| block.trim())
        .filter(|block| !block.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if normalized_blocks.is_empty() {
        return String::new();
    }

    let Some(total_limit) = total_limit.filter(|value| *value > 0) else {
        return normalized_blocks.join("\n\n");
    };

    let mut merged_blocks = Vec::new();
    let mut current_length = 0usize;
    for block in normalized_blocks {
        let separator_length = if merged_blocks.is_empty() { 0 } else { 2 };
        let block_length = block.chars().count();
        let projected_length = current_length + separator_length + block_length;
        if projected_length <= total_limit {
            merged_blocks.push(block);
            current_length = projected_length;
            continue;
        }

        let remaining = total_limit.saturating_sub(current_length + separator_length);
        if remaining < 80 {
            break;
        }

        let truncated = truncate_story_runtime_block(&block, remaining);
        if !truncated.is_empty() {
            merged_blocks.push(truncated);
        }
        break;
    }

    merged_blocks.join("\n\n")
}

fn build_story_creation_brief_block(story_creation_brief: Option<&str>) -> Option<String> {
    let story_creation_brief = trimmed_non_empty(story_creation_brief)?;
    Some(
        [
            "【本轮创作总控】".to_string(),
            format!("- 执行摘要：{}", story_creation_brief),
            "- 先按总控摘要定目标、推进与收束，再参考后续卡片补细节，不要彼此打架。".to_string(),
        ]
        .join("\n"),
    )
}

pub(crate) fn build_project_long_term_goal(
    theme: Option<&str>,
    description: Option<&str>,
    story_creation_brief: Option<&str>,
    chapter_count: Option<usize>,
    target_word_count: Option<usize>,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(theme) = trimmed_non_empty(theme) {
        parts.push(format!(
            "主线主题：{}，整本书需要围绕它持续升级冲突与选择。",
            theme
        ));
    }
    if let Some(description) = trimmed_non_empty(description) {
        let compact = description.chars().take(90).collect::<String>();
        parts.push(format!("项目简介：{}", compact));
    }
    if let Some(story_creation_brief) = trimmed_non_empty(story_creation_brief) {
        let compact = story_creation_brief.chars().take(90).collect::<String>();
        parts.push(format!("创作总控：{}", compact));
    }
    if let Some(chapter_count) = chapter_count.filter(|value| *value > 0) {
        parts.push(format!(
            "整体篇幅预计约 {} 章，推进时要兼顾起势、升级与回报收束。",
            chapter_count
        ));
    } else if let Some(target_word_count) = target_word_count.filter(|value| *value > 0) {
        parts.push(format!(
            "整体体量预计约 {} 字，推进时避免前松后挤。",
            target_word_count
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn normalize_plot_stage(value: Option<&str>) -> Option<&str> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("development") | Some("发展") | Some("发展阶段") => Some("development"),
        Some("climax") | Some("高潮") | Some("高潮阶段") => Some("climax"),
        Some("ending") | Some("结局") | Some("结局阶段") => Some("ending"),
        _ => None,
    }
}

fn normalize_prompt_list(values: Option<&[String]>, limit: usize) -> Vec<String> {
    let mut items = Vec::new();
    for value in values.unwrap_or_default() {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        let normalized = normalized.to_string();
        if items.contains(&normalized) {
            continue;
        }
        items.push(normalized);
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn normalize_creative_mode(value: Option<&str>) -> Option<&str> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("balanced") | Some("均衡") | Some("均衡推进") => Some("balanced"),
        Some("hook") | Some("钩子") | Some("钩子优先") => Some("hook"),
        Some("emotion") | Some("情绪") | Some("情绪沉浸") => Some("emotion"),
        Some("suspense") | Some("悬念") | Some("悬念拉满") => Some("suspense"),
        Some("relationship") | Some("关系") | Some("关系张力") => Some("relationship"),
        Some("payoff") | Some("爽点") | Some("爽点推进") => Some("payoff"),
        Some(other) => Some(other),
        None => None,
    }
}

fn normalize_story_focus(value: Option<&str>) -> Option<&str> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("advance_plot") | Some("主线") | Some("主线推进") | Some("推进剧情") => {
            Some("advance_plot")
        }
        Some("deepen_character") | Some("人物") | Some("人物塑形") | Some("塑造人物") => {
            Some("deepen_character")
        }
        Some("escalate_conflict") | Some("冲突") | Some("冲突升级") | Some("升级冲突") => {
            Some("escalate_conflict")
        }
        Some("reveal_mystery") | Some("谜团") | Some("谜团揭示") | Some("揭示真相") => {
            Some("reveal_mystery")
        }
        Some("relationship_shift") | Some("关系") | Some("关系转折") | Some("关系变化") => {
            Some("relationship_shift")
        }
        Some("foreshadow_payoff") | Some("伏笔") | Some("伏笔回收") | Some("回收伏笔") => {
            Some("foreshadow_payoff")
        }
        Some("relationship_tension") => Some("relationship_shift"),
        Some("character_growth") => Some("deepen_character"),
        Some("worldbuilding") => Some("reveal_mystery"),
        Some(other) => Some(other),
        None => None,
    }
}

fn normalize_quality_preset(value: Option<&str>) -> Option<&str> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("balanced") | Some("均衡") | Some("均衡质感") => Some("balanced"),
        Some("plot_drive") | Some("强情节") | Some("强情节回报") => Some("plot_drive"),
        Some("immersive") | Some("沉浸") | Some("沉浸场景感") => Some("immersive"),
        Some("emotion_drama") | Some("情绪关系") | Some("情绪关系向") => {
            Some("emotion_drama")
        }
        Some("clean_prose") | Some("克制文风") | Some("克制干净文风") => {
            Some("clean_prose")
        }
        Some("tight_prose") => Some("clean_prose"),
        Some(other) => Some(other),
        None => None,
    }
}

fn creative_mode_label(value: &str) -> &str {
    match normalize_creative_mode(Some(value)).unwrap_or(value) {
        "balanced" => "均衡推进",
        "hook" => "钩子优先",
        "emotion" => "情绪沉浸",
        "suspense" => "悬念拉满",
        "relationship" => "关系张力",
        "payoff" => "爽点推进",
        _ => value,
    }
}

fn story_focus_label(value: &str) -> &str {
    match normalize_story_focus(Some(value)).unwrap_or(value) {
        "advance_plot" => "主线推进",
        "deepen_character" => "人物塑形",
        "escalate_conflict" => "冲突升级",
        "reveal_mystery" => "谜团揭示",
        "relationship_shift" => "关系转折",
        "foreshadow_payoff" => "伏笔回收",
        _ => value,
    }
}

fn plot_stage_label(value: &str) -> &str {
    match value {
        "ending" => "结局阶段",
        "opening" => "开局阶段",
        "development" => "发展阶段",
        "climax" => "高潮阶段",
        "结局" => "结局阶段",
        _ => value,
    }
}

fn quality_preset_label(value: &str) -> &str {
    match normalize_quality_preset(Some(value)).unwrap_or(value) {
        "balanced" => "均衡质感",
        "plot_drive" => "情节推进优先",
        "immersive" => "沉浸感优先",
        "emotion_drama" => "情绪关系向",
        "clean_prose" => "克制干净文风",
        _ => value,
    }
}

fn split_quality_note_items(quality_notes: Option<&str>, limit: usize) -> Vec<String> {
    let Some(notes) = trimmed_non_empty(quality_notes) else {
        return Vec::new();
    };
    let normalized = notes.replace('；', ";");
    let mut items = Vec::new();
    for raw in normalized.split(['\n', ';']) {
        let normalized = raw
            .trim()
            .trim_start_matches(|ch: char| {
                ch.is_whitespace()
                    || ch == '-'
                    || ch == '*'
                    || ch == '•'
                    || ch == '·'
                    || ch == '.'
                    || ch == ')'
                    || ch == '('
                    || ch == '、'
                    || ch.is_ascii_digit()
            })
            .trim();
        if normalized.is_empty() {
            continue;
        }
        let normalized = normalized.to_string();
        if items.contains(&normalized) {
            continue;
        }
        items.push(normalized);
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn build_outline_quality_preference_block(
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
) -> Option<String> {
    let normalized_preset = normalize_quality_preset(quality_preset);
    let note_items = split_quality_note_items(quality_notes, 4);
    if normalized_preset.is_none() && note_items.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    if let Some(value) = normalized_preset {
        lines.push(format!(
            "【质量预设】当前采用“{}”",
            quality_preset_label(value)
        ));
        match value {
            "balanced" => {
                lines.push(
                    "- 兼顾推进、情绪、信息释放与章尾牵引，不让单一维度长期压场。".to_string(),
                );
                lines.push("- 每轮既要有推进结果，也要有读者可感的回报与余味。".to_string());
            }
            "plot_drive" => {
                lines.push("- 优先强化开头抓力、动作桥段、爽点回收和章尾牵引。".to_string());
                lines.push("- 减少空转解释和过度铺垫，让大纲更偏连载可追读节奏。".to_string());
            }
            "immersive" => {
                lines.push("- 优先强化设定落地、场景密度、空间感与视角稳定。".to_string());
                lines.push("- 信息说明尽量压进事件和场景里，减少说明书式铺陈。".to_string());
            }
            "emotion_drama" => {
                lines.push("- 优先强化情绪落点、对白推进、关系余波和误伤后的后效。".to_string());
                lines.push("- 让关系变化反向推动下一轮行动，而不只是情绪点缀。".to_string());
            }
            "clean_prose" => {
                lines.push("- 优先强化信息压缩、重复压缩、总结腔抑制和表达克制。".to_string());
                lines.push("- 减少花哨总结与自我解释，让结构更清楚干净。".to_string());
            }
            _ => {}
        }
    } else {
        lines.push("【质量偏好补充】".to_string());
    }

    if !note_items.is_empty() {
        lines.push(format!("- 补充偏好：{}", note_items.join(" / ")));
    }

    Some(lines.join("\n"))
}

fn build_outline_creative_mode_block(creative_mode: Option<&str>) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode)?;
    let mut lines = vec![format!(
        "【创作模式】当前采用“{}”",
        creative_mode_label(normalized_mode)
    )];
    match normalized_mode {
        "balanced" => {
            lines.push("- 同时照顾钩子、推进、情绪和信息释放，不偏科。".to_string());
            lines.push("- 每章都要既能往下推，又能留下后续空间。".to_string());
        }
        "hook" => {
            lines.push("- 每章优先设计读者会想点下一章的悬挂点和动作牵引。".to_string());
            lines.push("- 关键信息不要一次讲透，尽量把转折放在章尾或场尾。".to_string());
        }
        "emotion" => {
            lines.push("- 每章都明确情绪波峰波谷，让冲突带出人物内在变化。".to_string());
            lines.push("- 安排能让人物情绪外露的场面，不只给事件结果。".to_string());
        }
        "suspense" => {
            lines.push("- 优先铺信息差、误导、遮蔽与逐层揭开，避免过早讲透底牌。".to_string());
            lines.push("- 每章至少留一个会迫使角色继续追查的新疑点。".to_string());
        }
        "relationship" => {
            lines.push("- 每章尽量让人物关系产生位移：靠近、撕裂、试探或互相利用。".to_string());
            lines.push("- 冲突优先落在人与人之间的立场差和利益差上。".to_string());
        }
        "payoff" => {
            lines
                .push("- 优先规划反转、收获、打脸、突破等即时反馈，避免一直憋压不放。".to_string());
            lines.push("- 每章都给读者一个清晰可感的阶段性兑现点。".to_string());
        }
        _ => return None,
    }
    Some(lines.join("\n"))
}

fn build_outline_story_focus_block(story_focus: Option<&str>) -> Option<String> {
    let normalized_focus = normalize_story_focus(story_focus)?;
    let mut lines = vec![format!(
        "【结构侧重点】当前优先“{}”",
        story_focus_label(normalized_focus)
    )];
    match normalized_focus {
        "advance_plot" => {
            lines.push("- 本轮大纲优先让事件产生明确推进，不要原地打转。".to_string());
            lines.push("- 每章都要形成新的行动结果、局势变化或任务升级。".to_string());
        }
        "deepen_character" => {
            lines.push("- 本轮优先安排能暴露人物选择、弱点、执念与成长代价的章节。".to_string());
            lines.push("- 不要只给事件节点，也要给人物变化节点。".to_string());
        }
        "escalate_conflict" => {
            lines.push("- 本轮优先让阻力变强、代价变高、对立面更具体。".to_string());
            lines.push("- 章节之间要形成持续抬升的压力链，不重复同级冲突。".to_string());
        }
        "reveal_mystery" => {
            lines.push("- 本轮优先安排线索出现、误导修正和真相推进的章节。".to_string());
            lines.push("- 揭示要分层，不要一口气把所有底牌讲透。".to_string());
        }
        "relationship_shift" => {
            lines.push("- 本轮优先安排人物关系发生靠近、破裂、试探或重组。".to_string());
            lines.push("- 让关系变化能反向影响后续行动，而不只是情绪点缀。".to_string());
        }
        "foreshadow_payoff" => {
            lines.push("- 本轮优先处理前文埋下的信息、承诺、物件或关系线索。".to_string());
            lines.push("- 回收时既要兑现，也要顺手打开新的后续空间。".to_string());
        }
        _ => return None,
    }
    Some(lines.join("\n"))
}

fn build_outline_combo_text(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    fallback: &str,
) -> String {
    let mut labels = Vec::new();
    if let Some(value) = normalize_creative_mode(creative_mode) {
        labels.push(creative_mode_label(value).to_string());
    }
    if let Some(value) = normalize_story_focus(story_focus) {
        labels.push(story_focus_label(value).to_string());
    }
    if let Some(value) = normalize_plot_stage(plot_stage) {
        labels.push(plot_stage_label(value).to_string());
    }
    if labels.is_empty() {
        fallback.to_string()
    } else {
        labels.join(" / ")
    }
}

fn build_outline_narrative_blueprint_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut beat = "先立主任务，中段持续加压，尾段安排转折并抛出下一轮问题。".to_string();
    let mut avoid = "不要平均摊功能，也不要只做信息罗列。".to_string();

    match normalized_mode {
        Some("hook") => {
            beat = "开篇尽快抛异常或任务，转折尽量落在章尾或场尾。".to_string();
            avoid = "不要把关键信息一次讲透，避免平收。".to_string();
        }
        Some("emotion") => {
            beat = "让推进、情绪外露与关系反馈同步发生，别只给事件结果。".to_string();
        }
        Some("suspense") => {
            beat = "用信息差、误导和逐层揭开来组织节拍。".to_string();
            avoid = "不要过早讲透底牌，也别只靠遮掩不推进。".to_string();
        }
        Some("relationship") => {
            beat = "把关系位移写进主推进，让靠近、撕裂或试探改变后续行动。".to_string();
        }
        Some("payoff") => {
            beat = "优先形成铺垫→兑现→反馈链，让每章都有阶段性回报。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("deepen_character") => {
            beat = "用选择、失误、坚持和代价来组织节拍，让人物在推进里显形。".to_string();
        }
        Some("escalate_conflict") => {
            beat = "每一章都把阻力和代价抬高一级，避免重复同级拉扯。".to_string();
        }
        Some("reveal_mystery") => {
            beat = "每一章都推进一点认知刷新，让线索和误判交替发力。".to_string();
        }
        Some("relationship_shift") => {
            beat = "节拍要围着关系位移展开，让站位和信任结构真正变化。".to_string();
        }
        Some("foreshadow_payoff") => {
            beat = "明确指定要回收的旧承诺/旧线索，并让兑现顺手打开新空间。".to_string();
        }
        _ => {}
    }

    match normalized_stage {
        Some("climax") => {
            beat = "高潮阶段要逼近正面碰撞，转折不能只是外围晃动。".to_string();
            avoid = "不要临近爆发还继续只铺不收。".to_string();
        }
        Some("ending") => {
            beat = "结局阶段优先回收主承诺、主悬念和关键关系线。".to_string();
            avoid = "不要在收束期再新开大主枝线。".to_string();
        }
        _ => {}
    }

    Some(
        [
            format!(
                "【结构蓝图】本轮按“{}”组织大纲节拍",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认结构")
            ),
            format!("- 节拍：{}", beat),
            format!("- 避免：{}", avoid),
        ]
        .join("\n"),
    )
}

fn build_outline_story_objective_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut objective = "让本轮章节承担清晰主任务，不平均摊功能。".to_string();
    let mut closing = "尾段留下下一轮必须回应的问题或新任务。".to_string();

    match normalized_mode {
        Some("hook") => {
            objective = "优先把异常、危险或未决任务挂到每章开头，快速抓住注意力。".to_string();
            closing = "钩子留在迫近危险、未决选择或刚被掀开的异常上。".to_string();
        }
        Some("emotion") => {
            objective = "目标除了推进事件，还要逼出人物情绪波动和关系反馈。".to_string();
        }
        Some("suspense") => {
            objective = "主任务优先围绕追查、误判修正和危险逼近展开。".to_string();
        }
        Some("relationship") => {
            objective = "主任务要直接推动站位变化、信任重排或立场试探。".to_string();
        }
        Some("payoff") => {
            objective = "本轮重点兑现前文铺垫，并带出更大后果。".to_string();
            closing = "回报后立刻抛出新的失衡或更高目标，别停在爽点本身。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            objective = "核心目标是把局势往前推一格，至少形成新的行动结果。".to_string();
        }
        Some("deepen_character") => {
            objective = "核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。".to_string();
        }
        Some("escalate_conflict") => {
            closing = "尾段把人物钉在更高代价区，确保下一轮没法轻退。".to_string();
        }
        Some("relationship_shift") => {
            objective = "核心目标是推动关系位移，让人物之后的说话方式和站位变掉。".to_string();
        }
        Some("foreshadow_payoff") => {
            objective = "核心目标是兑现前文埋设，并顺手打开新的后续空间。".to_string();
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            closing = "发展阶段先压实当前推进结果，再给后续升级留口。".to_string();
        }
        Some("climax") => {
            objective = "高潮阶段的主任务必须逼近核心碰撞，不能只在外围加码。".to_string();
        }
        Some("ending") => {
            objective = "结局阶段优先回收主承诺、主悬念与关键关系线。".to_string();
            closing = "收束期更适合留余味和后效，别用硬卖关子抢走主收束。".to_string();
        }
        _ => {}
    }

    Some(
        [
            format!(
                "【大纲目标卡】本轮主任务优先按“{}”落地",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认目标")
            ),
            format!("- 目标：{}", objective),
            format!("- 收束：{}", closing),
        ]
        .join("\n"),
    )
}

fn build_outline_story_result_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut result = "这一轮结束后，主线应进入更具体、更难回头的新局面。".to_string();
    let mut fallout = "尾段要把下一轮必须回应的压力、问题或任务钉住。".to_string();

    match normalized_mode {
        Some("hook") => {
            result = "本轮结束后，读者要感到故事被明显拽进下一段更危险的局面。".to_string();
            fallout = "余波优先落在未决选择、临门危险或刚被挑开的异常上。".to_string();
        }
        Some("emotion") => {
            result = "结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。".to_string();
        }
        Some("suspense") => {
            result = "至少拿到更接近真相的新证据，同时制造新的误判空间。".to_string();
        }
        Some("relationship") => {
            result = "结果里必须出现明确的关系位移、立场变化或信任重排。".to_string();
        }
        Some("payoff") => {
            result = "结果要让读者看到铺垫兑现、回报落地，并感到不是白等。".to_string();
            fallout = "兑现后要顺势推向更高目标或更大麻烦。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            result = "推进结果必须清晰可见：行动产生了后果，局势换了位置。".to_string();
        }
        Some("deepen_character") => {
            result = "结果要让人物的弱点、执念或价值判断真正显形。".to_string();
        }
        Some("escalate_conflict") => {
            result = "推进结果不是前进一步，而是把人推入更高代价的冲突区。".to_string();
            fallout = "余波要继续抬高冲突，让下一轮没有轻松退路。".to_string();
        }
        Some("reveal_mystery") => {
            result = "揭示结果必须真实推进谜团，而不是只制造更多模糊表述。".to_string();
        }
        Some("relationship_shift") => {
            result = "关系结果必须明确到足以改变两人后续的说话方式、站位或合作条件。".to_string();
        }
        Some("foreshadow_payoff") => {
            result = "结果要让前文埋设获得兑现，同时打开新的后续空间。".to_string();
            fallout = "余波放在兑现后的新承诺、新代价或更大失衡上。".to_string();
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            fallout = "发展阶段的余波要把后续任务钉实，避免下一章重复上一章。".to_string();
        }
        Some("climax") => {
            result = "推进结果要逼近或触发正面碰撞，不能只是外围晃动。".to_string();
        }
        Some("ending") => {
            result = "揭示结果优先服务主承诺、主悬念和关键伏笔的回收。".to_string();
            fallout = "收束期更适合留余味和后效，不能抢走主收束。".to_string();
        }
        _ => {}
    }

    Some(
        [
            format!(
                "【大纲结果卡】写完后至少让读者感知到以下变化（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认结果")
            ),
            format!("- 结果：{}", result),
            format!("- 余波：{}", fallout),
        ]
        .join("\n"),
    )
}

fn build_outline_story_payoff_chain_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut payoff = "这一轮至少承接一个已有铺垫，或埋下一个近章可回收的小钩点。".to_string();
    let mut feedback = "兑现之后要带出局势变化、关系余震、资源得失或新的行动压力。".to_string();

    match normalized_mode {
        Some("hook") => {
            payoff = "钩子型兑现最好来得更快，让读者更早尝到“这章真的有事发生”的回报。".to_string();
        }
        Some("emotion") => {
            feedback = "兑现后的余波优先写关系温差、情绪后坐力和人物自我认知变化。".to_string();
        }
        Some("suspense") => {
            payoff = "悬念型兑现更适合“揭半层真相 + 打开更危险缺口”。".to_string();
        }
        Some("relationship") => {
            payoff = "关系型兑现优先落在站位变化、信任转移、边界突破或彻底决裂。".to_string();
        }
        Some("payoff") => {
            payoff = "优先锁定前文明确埋过的承诺、伏笔或能力点，不要临时找替身回收。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            feedback = "兑现后的反馈必须推动主线进入下一格，别回收完又回到原地。".to_string();
        }
        Some("deepen_character") => {
            payoff = "兑现瞬间最好顺便照出人物的底线、成长、执念或迟来的代价感。".to_string();
        }
        Some("escalate_conflict") => {
            feedback = "回收后不要泄压，最好把人物推进更难的冲突层级。".to_string();
        }
        Some("reveal_mystery") => {
            payoff = "优先给一个有效答案，但同时暴露更关键的缺口或更大的反常。".to_string();
        }
        Some("relationship_shift") => {
            feedback = "兑现后的反馈要让关系真的变得不一样，而不是只在心理旁白里说“其实变了”。"
                .to_string();
        }
        Some("foreshadow_payoff") => {
            payoff = "尽量指定哪条旧伏笔要回收，不要泛泛地说“注意前后呼应”。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("ending") {
        feedback = "结局阶段优先回收主承诺、主关系和主谜面，再保留必要余波。".to_string();
    }

    Some(
        [
            format!(
                "【大纲爽点回收卡】本轮请形成可感知的“铺垫→兑现→反馈”链条（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认回收")
            ),
            format!("- 回收：{}", payoff),
            format!("- 反馈：{}", feedback),
        ]
        .join("\n"),
    )
}

fn build_outline_story_rule_grounding_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut rule = "至少让一个核心规则、行业逻辑或力量边界在情节里真正起作用。".to_string();
    let mut cost = "规则一旦出手，要带出门槛、冷却、风险、限制或现实成本。".to_string();

    match normalized_mode {
        Some("hook") => {
            rule = "设定最好一上来就制造麻烦、压力或危险，让规则本身成为抓手。".to_string();
        }
        Some("emotion") => {
            rule =
                "规则落地最好能压到情绪与关系，让人物因为规则约束、代价或失手而受伤。".to_string();
        }
        Some("suspense") => {
            rule = "规则触发最好带出异常征兆、反常反馈或认知缺口。".to_string();
        }
        Some("relationship") => {
            rule = "设定最好落在身份、契约、门第或组织纪律上，直接影响人物站位。".to_string();
        }
        Some("payoff") => {
            rule = "优先让前文埋过的规则真正兑现，展示它终于生效时的爽点与后效。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            cost = "规则生效后必须推动主线，不要只是展示世界观却不改局势。".to_string();
        }
        Some("deepen_character") => {
            rule = "最好通过人物主动触发、拒绝触发或误用规则，暴露他的价值判断与软肋。".to_string();
        }
        Some("escalate_conflict") => {
            cost = "规则的代价、限制或反噬要把冲突抬高，而不是轻松替角色解围。".to_string();
        }
        Some("reveal_mystery") => {
            rule = "规则落地应顺带暴露机制缺口、异常样本或隐藏条件，让谜团推进。".to_string();
        }
        Some("relationship_shift") => {
            cost = "设定效果最好改写人与人之间的信任、合作权限或站队关系。".to_string();
        }
        Some("foreshadow_payoff") => {
            rule = "优先回收前文提过的规则伏笔，让读者感到之前那句设定现在真有用。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("ending") {
        cost = "结局阶段优先回收最核心的规则承诺与代价，不要再抛全新体系。".to_string();
    }

    Some(
        [
            format!(
                "【大纲设定落地卡】本轮请让规则与设定真正进场（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认设定落地")
            ),
            format!("- 规则：{}", rule),
            format!("- 代价：{}", cost),
        ]
        .join("\n"),
    )
}

fn build_outline_story_opening_hook_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut opening = "开篇尽快亮出主任务、异常或局势缺口，别慢热铺背景。".to_string();
    let mut pull = "前段就要给出读者会继续追下去的抓手：危险、疑点或待做选择。".to_string();

    match normalized_mode {
        Some("hook") => {
            opening = "开篇优先把异常、危险或未决任务放到前台，不要等半章再进入事件。".to_string();
            pull = "抓手优先落在倒计时危险、未决选择或突然翻面的信息上。".to_string();
        }
        Some("emotion") => {
            pull = "抓手可以落在人物被压住的情绪、说不出口的话和关系试探上。".to_string();
        }
        Some("suspense") => {
            opening = "开篇先抛异常线索、危险信号或误判苗头，再补必要背景。".to_string();
            pull = "抓手优先落在信息差、证据变化和答案只揭半层的状态上。".to_string();
        }
        Some("relationship") => {
            opening = "开篇先把关系张力、站位差或试探动作摆上台面。".to_string();
        }
        Some("payoff") => {
            opening = "开篇尽快回扣前文埋设，提醒读者这轮会有兑现。".to_string();
            pull = "抓手留在兑现条件逼近和代价同步抬高上。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            opening = "开篇先亮明本轮要推进的事，别让读者等太久才知道这章要干嘛。".to_string();
        }
        Some("deepen_character") => {
            pull = "抓手最好让人物马上面对一项会暴露性格的选择题。".to_string();
        }
        Some("relationship_shift") => {
            pull = "抓手尽量来自一次会改变关系位置的互动或试探。".to_string();
        }
        Some("foreshadow_payoff") => {
            opening = "开篇尽快把前文埋下的人、物、承诺或代价重新拉回现场。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("climax") {
        opening = "高潮阶段开篇尽快把人物推到主碰撞现场，不再外围试探。".to_string();
    }

    Some(
        [
            format!(
                "【大纲开篇钩子卡】开篇先把读者抓进当前任务（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认开篇抓力")
            ),
            format!("- 开篇：{}", opening),
            format!("- 拉力：{}", pull),
        ]
        .join("\n"),
    )
}

fn build_outline_story_cliffhanger_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut unresolved = "卷尾几章要留一个足够具体的未决点，能自然牵引下一轮主任务。".to_string();
    let mut aftertaste = "尾声保留情绪余波、关系余震、代价阴影或认知反照。".to_string();

    match normalized_mode {
        Some("hook") => {
            unresolved = "未决点优先是迫近选择、倒计时危险或刚被掀开的麻烦。".to_string();
            aftertaste = "下一步逼力要明确到人物不得不马上应对，而不是以后再说。".to_string();
        }
        Some("emotion") => {
            aftertaste =
                "余味最好落在误伤后的沉默、靠近失败后的反弹，或关系未说破的震荡上。".to_string();
        }
        Some("suspense") => {
            unresolved = "未决点最好是线索翻面、认知裂缝、危险升级或答案只揭开半层。".to_string();
        }
        Some("relationship") => {
            unresolved = "未决点最好和立场未定、关系悬空、合作破裂或信任临界绑定。".to_string();
        }
        Some("payoff") => {
            unresolved =
                "兑现之后要留一个新失衡或新代价，说明故事没有在爽点处直接封口。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            aftertaste = "结尾逼力必须能接到主线下一步，不要只留下气氛而没有行动方向。".to_string();
        }
        Some("deepen_character") => {
            aftertaste = "余味最好让读者记住人物此刻的新伤口、新认知或新自我怀疑。".to_string();
        }
        Some("escalate_conflict") => {
            unresolved =
                "未决点应落在冲突升级后的更难位置：谁先出手、谁先失控、谁先付代价。".to_string();
        }
        Some("relationship_shift") => {
            aftertaste = "余味要落在关系新站位上，让读者感到他们回不到原来的相处方式。".to_string();
        }
        Some("foreshadow_payoff") => {
            unresolved = "未决点可以是旧伏笔兑现后的新空缺，说明兑现带来了新的问题。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("ending") {
        aftertaste = "结局阶段更适合保留余波、代价、阴影或尚未完全愈合的裂口。".to_string();
    }

    Some(
        [
            format!(
                "【大纲结尾悬停卡】收尾请留下继续推进的牵引（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认悬停")
            ),
            format!("- 未决点：{}", unresolved),
            format!("- 尾钩：{}", aftertaste),
        ]
        .join("\n"),
    )
}

fn build_outline_story_character_arc_card_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut character = "这一轮至少让核心人物的外在线任务更明确，不只推动事件壳子。".to_string();
    let mut relationship = "让关键关系在信任、站队或依赖上出现可见变化。".to_string();

    match normalized_mode {
        Some("hook") => {
            character =
                "人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。".to_string();
        }
        Some("emotion") => {
            relationship = "关系线最好呈现安慰失败、靠近受阻或误伤后的余震。".to_string();
        }
        Some("suspense") => {
            character = "通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。".to_string();
        }
        Some("relationship") => {
            relationship =
                "关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。".to_string();
        }
        Some("payoff") => {
            character = "人物应因为兑现获得成长回报，或承担兑现带来的新责任。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            character = "人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。".to_string();
        }
        Some("deepen_character") => {
            character = "内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。".to_string();
        }
        Some("escalate_conflict") => {
            relationship = "更强冲突最好同步改写人物之间的站位与依赖结构。".to_string();
        }
        Some("reveal_mystery") => {
            character =
                "外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。".to_string();
        }
        Some("relationship_shift") => {
            relationship =
                "关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。".to_string();
        }
        Some("foreshadow_payoff") => {
            character = "人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("ending") {
        relationship = "结局阶段要让关键关系线出现收束、定局或带余温的最终位移。".to_string();
    }

    Some(
        [
            format!(
                "【大纲角色弧光卡】本轮至少让人物弧光出现以下推进（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认弧光")
            ),
            format!("- 人物：{}", character),
            format!("- 关系：{}", relationship),
        ]
        .join("\n"),
    )
}

fn build_outline_story_execution_checklist_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return None;
    }

    let mut emphasis =
        "开场立主任务 → 中段持续加压 → 后段安排关键转折 → 尾段抛实下一轮问题。".to_string();

    match normalized_mode {
        Some("hook") => {
            emphasis =
                "开场尽快抛异常/任务 → 中段压紧信息缺口 → 尾段把危险或选择钉牢。".to_string();
        }
        Some("emotion") => {
            emphasis = "开场带出情绪缺口 → 中段用互动和误伤持续加压 → 收尾保留余震。".to_string();
        }
        Some("suspense") => {
            emphasis =
                "开场抛疑点 → 中段扩大信息差与误判代价 → 尾段留下更尖锐的新疑点。".to_string();
        }
        Some("relationship") => {
            emphasis =
                "开场摆出关系张力 → 中段通过对话/行动挤压站位 → 收尾悬置新关系姿态。".to_string();
        }
        Some("payoff") => {
            emphasis = "开场回扣旧铺垫 → 中段推近兑现条件 → 转折落兑现并带新后果。".to_string();
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            emphasis =
                "开场亮明要推进的事 → 中段每次推进都带新结果 → 尾段把主线下一步钉实。".to_string();
        }
        Some("deepen_character") => {
            emphasis = "中段把压力尽量变成选择题，让人物性格在决策里显形。".to_string();
        }
        Some("escalate_conflict") => {
            emphasis = "中段每一轮加压都要比上一轮更狠，转折要把冲突推向正面碰撞。".to_string();
        }
        Some("reveal_mystery") => {
            emphasis =
                "开场尽快抛线索/异常 → 中段用调查和误导修正推进认知 → 转折修正一次关键判断。"
                    .to_string();
        }
        Some("relationship_shift") => {
            emphasis = "中段每次互动都推动信任或站队位移，转折要让关系位置真正改变。".to_string();
        }
        Some("foreshadow_payoff") => {
            emphasis = "开场拉回旧埋设 → 转折优先落实兑现 → 收尾保留回收后的新缺口。".to_string();
        }
        _ => {}
    }

    if normalized_stage == Some("climax") {
        emphasis = "高潮阶段开场尽快进入主碰撞，中段持续抬高代价和时限，收尾把更大余波推向下章。"
            .to_string();
    }

    Some(
        [
            format!(
                "【大纲执行清单】本轮优先按以下节奏执行（{}）",
                build_outline_combo_text(creative_mode, story_focus, plot_stage, "默认执行节奏")
            ),
            format!("- 执行：{}", emphasis),
            "- 验收：每章都要有任务、阻力、结果与尾钩，不写平推空转章。".to_string(),
        ]
        .join("\n"),
    )
}

fn build_compact_outline_guidance_blocks(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
) -> Vec<String> {
    [
        build_outline_quality_preference_block(quality_preset, quality_notes),
        build_outline_creative_mode_block(creative_mode),
        build_outline_story_focus_block(story_focus),
        build_outline_narrative_blueprint_block(creative_mode, story_focus, plot_stage),
        build_outline_story_objective_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_result_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_payoff_chain_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_rule_grounding_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_opening_hook_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_cliffhanger_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_character_arc_card_block(creative_mode, story_focus, plot_stage),
        build_outline_story_execution_checklist_block(creative_mode, story_focus, plot_stage),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn build_story_long_term_goal_block(long_term_goal: Option<&str>) -> Option<String> {
    let long_term_goal = trimmed_non_empty(long_term_goal)?;
    Some(
        [
            "【长线目标锚点】".to_string(),
            format!("- 本书长线目标：{}", long_term_goal),
            "- 本轮输出必须服务这条长线，不要只完成局部热闹。".to_string(),
            "- 高潮、反转和情绪爆点都要能回扣主线目标、长期代价或最终回报。".to_string(),
        ]
        .join("\n"),
    )
}

fn build_story_character_focus_anchor_block(
    focus_names: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let focus_items = normalize_prompt_list(focus_names, 4);
    if focus_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let joined_focus = focus_items.join(" / ");
    Some(
        [
            format!("【{}角色焦点锚点】", scene_label),
            format!("- 本轮优先照亮角色：{}", joined_focus),
            "- 让这些角色分别承担决定、反应或关系位移，不要只挂名出场。".to_string(),
            "- 重要情绪变化尽量落在这些角色的选择与后果上，避免镜头平均摊薄。".to_string(),
        ]
        .join("\n"),
    )
}

fn build_story_foreshadow_payoff_plan_block(
    foreshadow_payoff_plan: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let payoff_items = normalize_prompt_list(foreshadow_payoff_plan, 3);
    if payoff_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}伏笔兑现计划】", scene_label),
        "- 本轮优先处理以下伏笔/回报链：".to_string(),
    ];
    lines.extend(payoff_items.into_iter().map(|item| format!("  - {}", item)));
    lines.push("- 兑现时要带出新信息、新代价或新失衡，避免只做口头回收。".to_string());
    Some(lines.join("\n"))
}

fn build_story_character_state_ledger_block(
    character_state_ledger: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let state_items = normalize_prompt_list(character_state_ledger, 4);
    if state_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}人物状态账本】", scene_label),
        "- 以下状态是本轮必须延续的人物处境、压力或阶段变化：".to_string(),
    ];
    lines.extend(state_items.into_iter().map(|item| format!("  - {}", item)));
    lines.push("- 用动作、选择、代价和情绪反应把这些状态写实，不要只在说明句里复述。".to_string());
    Some(lines.join("\n"))
}

fn build_story_relationship_state_ledger_block(
    relationship_state_ledger: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let relationship_items = normalize_prompt_list(relationship_state_ledger, 4);
    if relationship_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}关系状态账本】", scene_label),
        "- 以下关系线必须在互动、站队或对白里继续推进：".to_string(),
    ];
    lines.extend(
        relationship_items
            .into_iter()
            .map(|item| format!("  - {}", item)),
    );
    lines.push("- 至少让其中一条关系出现可见位移，不要只重复旧情绪。".to_string());
    Some(lines.join("\n"))
}

fn build_story_foreshadow_state_ledger_block(
    foreshadow_state_ledger: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let foreshadow_items = normalize_prompt_list(foreshadow_state_ledger, 4);
    if foreshadow_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}伏笔状态账本】", scene_label),
        "- 以下伏笔或承诺需要推进、兑现或制造新的回响：".to_string(),
    ];
    lines.extend(
        foreshadow_items
            .into_iter()
            .map(|item| format!("  - {}", item)),
    );
    lines.push("- 把伏笔状态落在事件结果、信息揭示或代价变化上，不要只口头提醒。".to_string());
    Some(lines.join("\n"))
}

fn build_story_organization_state_ledger_block(
    organization_state_ledger: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let organization_items = normalize_prompt_list(organization_state_ledger, 4);
    if organization_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}组织状态账本】", scene_label),
        "- 以下组织或势力状态需要继续影响资源、命令、站队或地盘：".to_string(),
    ];
    lines.extend(
        organization_items
            .into_iter()
            .map(|item| format!("  - {}", item)),
    );
    lines.push("- 组织变化要落实到人物决策与局势后果，不要只写背景说明。".to_string());
    Some(lines.join("\n"))
}

fn build_story_career_state_ledger_block(
    career_state_ledger: Option<&[String]>,
    scene: &str,
) -> Option<String> {
    let career_items = normalize_prompt_list(career_state_ledger, 4);
    if career_items.is_empty() {
        return None;
    }

    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };
    let mut lines = vec![
        format!("【{}职业状态账本】", scene_label),
        "- 以下职业或能力成长状态要继续体现在技能使用、瓶颈或代价上：".to_string(),
    ];
    lines.extend(career_items.into_iter().map(|item| format!("  - {}", item)));
    lines.push("- 职业推进要落到任务结果、能力应用和成长成本，不要只报阶段名。".to_string());
    Some(lines.join("\n"))
}

fn allocate_volume_segments(chapter_count: usize) -> Vec<(&'static str, usize)> {
    let total = chapter_count;
    if total == 0 {
        return Vec::new();
    }
    if total == 1 {
        return vec![("development", 1)];
    }
    if total == 2 {
        return vec![("development", 1), ("ending", 1)];
    }
    if total == 3 {
        return vec![("development", 1), ("climax", 1), ("ending", 1)];
    }

    let mut development_count = ((total as f64) * 0.45).round() as usize;
    let mut climax_count = ((total as f64) * 0.35).round() as usize;
    development_count = development_count.max(1);
    climax_count = climax_count.max(1);
    let mut ending_count = total.saturating_sub(development_count + climax_count);

    if ending_count < 1 {
        ending_count = 1;
        if development_count >= climax_count && development_count > 1 {
            development_count -= 1;
        } else if climax_count > 1 {
            climax_count -= 1;
        }
    }

    let mut segments = Vec::new();
    if development_count > 0 {
        segments.push(("development", development_count));
    }
    if climax_count > 0 {
        segments.push(("climax", climax_count));
    }
    if ending_count > 0 {
        segments.push(("ending", ending_count));
    }
    segments
}

fn plot_stage_mission(value: &str) -> &str {
    match value {
        "development" => "立局、铺变量、建立目标与第一轮压力。",
        "climax" => "持续抬压、逼近正面碰撞、推动关键反转。",
        "ending" => "回收承诺、兑现伏笔、收束关系并留下余味。",
        _ => "",
    }
}

fn build_story_pacing_budget_block(
    chapter_count: Option<usize>,
    current_chapter_number: Option<usize>,
    target_word_count: Option<usize>,
    plot_stage: Option<&str>,
    scene: &str,
) -> Option<String> {
    let total = chapter_count.filter(|value| *value > 0);
    let current = current_chapter_number.filter(|value| *value > 0);
    let target = target_word_count.filter(|value| *value > 0);
    let normalized_stage = normalize_plot_stage(plot_stage);
    let scene_label = if scene == "outline" {
        "大纲"
    } else {
        "章节"
    };

    let mut lines = vec![format!("【{}节奏预算】", scene_label)];
    if let (Some(total), Some(current)) = (total, current) {
        lines.push(format!("- 当前进度：第{}/{}章。", current, total));
        let mut cursor = 1usize;
        for (stage, count) in allocate_volume_segments(total) {
            let start_chapter = cursor;
            let end_chapter = cursor + count - 1;
            cursor = end_chapter + 1;
            if start_chapter <= current && current <= end_chapter {
                lines.push(format!(
                    "- 结构位置：当前位于第{}-{}章的{}段，本轮要完成这一段该有的推进。",
                    start_chapter,
                    end_chapter,
                    plot_stage_label(stage)
                ));
                break;
            }
        }
    } else if let Some(total) = total {
        lines.push(format!(
            "- 计划体量：约{}章，推进时先按整卷节奏分配资源，不要只顾单点刺激。",
            total
        ));
    }

    if let Some(target) = target {
        if scene == "chapter" {
            lines.push(format!(
                "- 本章目标字数：约{}字，可在保证节奏完整的前提下浮动 ±20%。",
                target
            ));
        } else {
            lines.push(format!(
                "- 单章体量可参考约{}字，避免开局章节过短或信息堆积失衡。",
                target
            ));
        }
    }

    if let Some(stage) = normalized_stage {
        lines.push(format!(
            "- 阶段重点：{}，优先完成该阶段最关键的任务，不要提前透支后续高潮。",
            plot_stage_label(stage)
        ));
    }

    if lines.len() == 1 {
        return None;
    }

    if scene == "chapter" {
        lines.push(
            "- 节奏上要做到：开场尽快立题，中段持续加压，尾段留下动作牵引或情绪余震。".to_string(),
        );
    } else {
        lines.push(
            "- 规划时要兼顾起势、升级、回报与续航，不要把所有强刺激都堆在前几章。".to_string(),
        );
    }

    Some(lines.join("\n"))
}

fn build_volume_pacing_block(
    chapter_count: Option<usize>,
    plot_stage: Option<&str>,
) -> Option<String> {
    let total = chapter_count.filter(|value| *value > 0)?;
    let normalized_stage = normalize_plot_stage(plot_stage);
    let segments = allocate_volume_segments(total);
    if segments.is_empty() {
        return None;
    }

    let mut lines = vec![format!(
        "【卷级节奏】若本轮规划 {} 章，建议整体按以下节奏分段",
        total
    )];
    let mut cursor = 1usize;
    for (stage, count) in segments {
        let start_chapter = cursor;
        let end_chapter = cursor + count - 1;
        cursor = end_chapter + 1;
        lines.push(format!(
            "- 第{}-{}章：{}，重点任务是{}",
            start_chapter,
            end_chapter,
            plot_stage_label(stage),
            plot_stage_mission(stage)
        ));
    }

    if let Some(stage) = normalized_stage {
        lines.push(format!(
            "- 当前用户指定重点阶段：{}，本轮应优先把资源集中到这一段的核心任务。",
            plot_stage_label(stage)
        ));
    }

    Some(lines.join("\n"))
}

fn build_outline_runtime_preference_block(
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
) -> Option<String> {
    let creative_mode = trimmed_non_empty(creative_mode);
    let story_focus = trimmed_non_empty(story_focus);
    let plot_stage = trimmed_non_empty(plot_stage);
    let quality_preset = trimmed_non_empty(quality_preset);
    let quality_notes = trimmed_non_empty(quality_notes);

    if creative_mode.is_none()
        && story_focus.is_none()
        && plot_stage.is_none()
        && quality_preset.is_none()
        && quality_notes.is_none()
    {
        return None;
    }

    let mut lines = vec!["【运行时创作偏好】".to_string()];
    if let Some(value) = creative_mode.as_deref() {
        lines.push(format!("- 创意模式：{}", creative_mode_label(value)));
    }
    if let Some(value) = story_focus.as_deref() {
        lines.push(format!("- 叙事焦点：{}", story_focus_label(value)));
    }
    if let Some(value) = plot_stage.as_deref() {
        lines.push(format!("- 情节阶段：{}", plot_stage_label(value)));
    }
    if let Some(value) = quality_preset.as_deref() {
        lines.push(format!("- 质量预设：{}", quality_preset_label(value)));
    }
    if let Some(value) = quality_notes.as_deref() {
        lines.push(format!("- 质量备注：{}", value));
    }

    Some(lines.join("\n"))
}

fn build_opening_outline_constraints_block(outline_count: usize) -> String {
    format!(
        "【开局大纲约束】这是小说的开局部分，请生成{}个大纲节点，重点关注：\n\
1. 引入主要角色和世界观设定\n\
2. 建立主线冲突和故事钩子\n\
3. 展开初期情节，为后续发展埋下伏笔\n\
4. 若包含第1-3章，尽量体现黄金三章节奏（钩子→升级→小高潮）\n\
5. 每章至少一个小爽点与一个章尾钩子，避免平推\n\
6. 不要试图完结故事，这只是开始部分\n\
7. 不要在JSON字符串值中使用中文引号（\"\"''），请使用【】或《》标记",
        outline_count
    )
}

fn build_continue_outline_constraints_block(chapter_count: usize) -> String {
    format!(
        "【续写大纲约束】请基于已有大纲续写接下来的{}章，重点关注：\n\
1. 与已有章节自然衔接，避免重复复述旧事件\n\
2. 每章都要有新的阻力升级、角色选择和即时后果\n\
3. 优先回收最近章节已经埋下的冲突、承诺或风险线索\n\
4. 每章至少一个小爽点与一个章尾钩子，避免平推\n\
5. 世界规则必须落地到事件结果，而不是只做背景名词\n\
6. 保持角色关系和成长轨迹连续，不要无根据跳变\n\
7. 不要在JSON字符串值中使用中文引号（\"\"''），请使用【】或《》标记",
        chapter_count
    )
}

pub(crate) fn build_wizard_outline_requirements(
    base_requirements: Option<&str>,
    outline_count: usize,
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    story_creation_brief: Option<&str>,
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
    project_long_term_goal: Option<&str>,
    target_word_count: Option<usize>,
    quality_repair_guidance: Option<&str>,
    quality_trend_guidance: Option<&str>,
    compact_mode: bool,
) -> String {
    let mut block_specs = Vec::new();

    block_specs.push((
        OUTLINE_RUNTIME_BASE_REQUIREMENTS_LIMIT,
        trimmed_non_empty(base_requirements),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_CREATION_BRIEF_LIMIT,
        build_story_creation_brief_block(story_creation_brief),
    ));
    if !compact_mode {
        block_specs.push((
            usize::MAX,
            build_outline_runtime_preference_block(
                creative_mode,
                story_focus,
                plot_stage,
                quality_preset,
                quality_notes,
            ),
        ));
    }
    block_specs.push((
        OUTLINE_RUNTIME_QUALITY_REPAIR_GUIDANCE_LIMIT,
        trimmed_non_empty(quality_repair_guidance),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_QUALITY_TREND_GUIDANCE_LIMIT,
        trimmed_non_empty(quality_trend_guidance),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_LONG_TERM_GOAL_LIMIT,
        build_story_long_term_goal_block(project_long_term_goal),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_PACING_BUDGET_LIMIT,
        build_story_pacing_budget_block(
            Some(outline_count),
            None,
            target_word_count,
            plot_stage,
            "outline",
        ),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_VOLUME_PACING_LIMIT,
        build_volume_pacing_block(Some(outline_count), plot_stage),
    ));
    block_specs.push((
        usize::MAX,
        Some(build_opening_outline_constraints_block(outline_count)),
    ));

    let mut blocks = block_specs
        .into_iter()
        .filter_map(|(limit, block)| {
            let block = block?;
            let normalized = block.trim();
            if normalized.is_empty() {
                return None;
            }
            Some(if compact_mode {
                truncate_story_runtime_block(normalized, limit)
            } else {
                normalized.to_string()
            })
        })
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>();

    if compact_mode {
        let compact_guidance_blocks = build_compact_outline_guidance_blocks(
            creative_mode,
            story_focus,
            plot_stage,
            quality_preset,
            quality_notes,
        )
        .into_iter()
        .map(|block| {
            truncate_story_runtime_block(block.trim(), OUTLINE_COMPACT_GUIDANCE_BLOCK_LIMIT)
        })
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>();
        if !compact_guidance_blocks.is_empty() {
            let constraint_block = blocks.pop();
            blocks.extend(compact_guidance_blocks);
            if let Some(block) = constraint_block {
                blocks.push(block);
            }
        }
    }

    if compact_mode {
        join_story_runtime_blocks_with_budget(
            &blocks,
            Some(OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT),
        )
    } else {
        blocks.join("\n\n")
    }
}

pub(crate) fn build_continue_outline_requirements(
    base_requirements: Option<&str>,
    chapter_count: usize,
    creative_mode: Option<&str>,
    story_focus: Option<&str>,
    plot_stage: Option<&str>,
    story_creation_brief: Option<&str>,
    quality_preset: Option<&str>,
    quality_notes: Option<&str>,
    project_long_term_goal: Option<&str>,
    focus_names: Option<&[String]>,
    foreshadow_payoff_plan: Option<&[String]>,
    foreshadow_state_ledger: Option<&[String]>,
    character_state_ledger: Option<&[String]>,
    relationship_state_ledger: Option<&[String]>,
    organization_state_ledger: Option<&[String]>,
    career_state_ledger: Option<&[String]>,
    memory_guidance: Option<&str>,
    quality_repair_guidance: Option<&str>,
    quality_trend_guidance: Option<&str>,
    compact_mode: bool,
) -> String {
    let mut block_specs = Vec::new();

    block_specs.push((
        OUTLINE_RUNTIME_BASE_REQUIREMENTS_LIMIT,
        trimmed_non_empty(base_requirements),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_CREATION_BRIEF_LIMIT,
        build_story_creation_brief_block(story_creation_brief),
    ));
    if !compact_mode {
        block_specs.push((
            usize::MAX,
            build_outline_runtime_preference_block(
                creative_mode,
                story_focus,
                plot_stage,
                quality_preset,
                quality_notes,
            ),
        ));
    }
    block_specs.push((
        OUTLINE_RUNTIME_MEMORY_GUIDANCE_LIMIT,
        trimmed_non_empty(memory_guidance),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_QUALITY_REPAIR_GUIDANCE_LIMIT,
        trimmed_non_empty(quality_repair_guidance),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_QUALITY_TREND_GUIDANCE_LIMIT,
        trimmed_non_empty(quality_trend_guidance),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_LONG_TERM_GOAL_LIMIT,
        build_story_long_term_goal_block(project_long_term_goal),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_CHARACTER_FOCUS_ANCHOR_LIMIT,
        build_story_character_focus_anchor_block(focus_names, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_FORESHADOW_PAYOFF_PLAN_LIMIT,
        build_story_foreshadow_payoff_plan_block(foreshadow_payoff_plan, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_FORESHADOW_STATE_LEDGER_LIMIT,
        build_story_foreshadow_state_ledger_block(foreshadow_state_ledger, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_CHARACTER_STATE_LEDGER_LIMIT,
        build_story_character_state_ledger_block(character_state_ledger, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_RELATIONSHIP_STATE_LEDGER_LIMIT,
        build_story_relationship_state_ledger_block(relationship_state_ledger, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_ORGANIZATION_STATE_LEDGER_LIMIT,
        build_story_organization_state_ledger_block(organization_state_ledger, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_CAREER_STATE_LEDGER_LIMIT,
        build_story_career_state_ledger_block(career_state_ledger, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_PACING_BUDGET_LIMIT,
        build_story_pacing_budget_block(Some(chapter_count), None, None, plot_stage, "outline"),
    ));
    block_specs.push((
        OUTLINE_RUNTIME_STORY_VOLUME_PACING_LIMIT,
        build_volume_pacing_block(Some(chapter_count), plot_stage),
    ));
    block_specs.push((
        usize::MAX,
        Some(build_continue_outline_constraints_block(chapter_count)),
    ));

    let mut blocks = block_specs
        .into_iter()
        .filter_map(|(limit, block)| {
            let block = block?;
            let normalized = block.trim();
            if normalized.is_empty() {
                return None;
            }
            Some(if compact_mode {
                truncate_story_runtime_block(normalized, limit)
            } else {
                normalized.to_string()
            })
        })
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>();

    if compact_mode {
        let compact_guidance_blocks = build_compact_outline_guidance_blocks(
            creative_mode,
            story_focus,
            plot_stage,
            quality_preset,
            quality_notes,
        )
        .into_iter()
        .map(|block| {
            truncate_story_runtime_block(block.trim(), OUTLINE_COMPACT_GUIDANCE_BLOCK_LIMIT)
        })
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>();
        if !compact_guidance_blocks.is_empty() {
            let constraint_block = blocks.pop();
            blocks.extend(compact_guidance_blocks);
            if let Some(block) = constraint_block {
                blocks.push(block);
            }
        }
    }

    if compact_mode {
        join_story_runtime_blocks_with_budget(
            &blocks,
            Some(OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT),
        )
    } else {
        blocks.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{build_continue_outline_requirements, build_wizard_outline_requirements};

    #[test]
    fn should_merge_wizard_outline_requirements_with_runtime_preferences_and_quality_guidance() {
        let merged = build_wizard_outline_requirements(
            Some("保留双线并进"),
            3,
            Some("hook"),
            Some("advance_plot"),
            Some("development"),
            Some("突出代价和抉择"),
            Some("plot_drive"),
            Some("减少说明句"),
            Some("主线主题：守住城门后的真相。"),
            Some(2600),
            Some("【诊断优先级卡】\n- 当前最弱项：章尾牵引（当前值：61）"),
            Some("【大纲近期质量趋势】\n- 后续章节要优先回收旧承诺"),
            false,
        );

        assert!(merged.contains("保留双线并进"));
        assert!(merged.contains("钩子优先"));
        assert!(merged.contains("主线推进"));
        assert!(merged.contains("发展阶段"));
        assert!(merged.contains("突出代价和抉择"));
        assert!(merged.contains("情节推进优先"));
        assert!(merged.contains("减少说明句"));
        assert!(merged.contains("【诊断优先级卡】"));
        assert!(merged.contains("【大纲近期质量趋势】"));
        assert!(merged.contains("【长线目标锚点】"));
        assert!(merged.contains("【大纲节奏预算】"));
        assert!(merged.contains("【卷级节奏】"));
        assert!(merged.contains("这是小说的开局部分"));
    }

    #[test]
    fn should_merge_continue_outline_requirements_with_runtime_preferences_and_quality_guidance() {
        let merged = build_continue_outline_requirements(
            Some("保持追杀线持续升温"),
            4,
            Some("payoff"),
            Some("relationship_shift"),
            Some("climax"),
            Some("优先兑现上一章暴露的背叛线索"),
            Some("immersive"),
            Some("减少背景解释"),
            Some("主线主题：逼出夜巡司内部叛徒。"),
            Some(&["沈砚".to_string(), "苏槿".to_string()]),
            Some(&["第1章《夜巡异响》：怀表异响尚未回收".to_string()]),
            Some(&["怀表异响: 怀表异响尚未回收; status=planted".to_string()]),
            Some(&["沈砚: 刚压下旧伤，必须带队追查叛徒。".to_string()]),
            Some(&["沈砚/苏槿: 盟友; 彼此试探".to_string()]),
            Some(&["夜巡司: power=82; location=北城门".to_string()]),
            Some(&["沈砚/夜巡人: stage 2; 晋升受阻".to_string()]),
            Some("【连载记忆与伏笔约束】\n【未完结伏笔】\n1. 怀表异响尚未回收"),
            Some("【诊断优先级卡】\n- 当前最弱项：冲突升级"),
            Some("【大纲近期质量趋势】\n- 最近三章的规则代价描写在下降"),
            false,
        );

        assert!(merged.contains("保持追杀线持续升温"));
        assert!(merged.contains("爽点推进"));
        assert!(merged.contains("关系转折"));
        assert!(merged.contains("高潮阶段"));
        assert!(merged.contains("优先兑现上一章暴露的背叛线索"));
        assert!(merged.contains("沉浸感优先"));
        assert!(merged.contains("减少背景解释"));
        assert!(merged.contains("【连载记忆与伏笔约束】"));
        assert!(merged.contains("【诊断优先级卡】"));
        assert!(merged.contains("【大纲近期质量趋势】"));
        assert!(merged.contains("【长线目标锚点】"));
        assert!(merged.contains("【大纲角色焦点锚点】"));
        assert!(merged.contains("【大纲伏笔兑现计划】"));
        assert!(merged.contains("【大纲伏笔状态账本】"));
        assert!(merged.contains("【大纲人物状态账本】"));
        assert!(merged.contains("【大纲关系状态账本】"));
        assert!(merged.contains("【大纲组织状态账本】"));
        assert!(merged.contains("【大纲职业状态账本】"));
        assert!(merged.contains("怀表异响尚未回收"));
        assert!(merged.contains("怀表异响: 怀表异响尚未回收; status=planted"));
        assert!(merged.contains("沈砚: 刚压下旧伤，必须带队追查叛徒。"));
        assert!(merged.contains("沈砚/苏槿: 盟友; 彼此试探"));
        assert!(merged.contains("夜巡司: power=82; location=北城门"));
        assert!(merged.contains("沈砚/夜巡人: stage 2; 晋升受阻"));
        assert!(merged.contains("沈砚 / 苏槿"));
        assert!(merged.contains("【大纲节奏预算】"));
        assert!(merged.contains("【卷级节奏】"));
        assert!(merged.contains("【续写大纲约束】"));
        assert!(merged.contains("基于已有大纲续写接下来的4章"));
        assert!(
            merged
                .find("【连载记忆与伏笔约束】")
                .expect("memory guidance index")
                < merged
                    .find("【诊断优先级卡】")
                    .expect("repair guidance index")
        );
    }

    #[test]
    fn should_apply_compact_mode_budget_for_outline_requirements() {
        let merged = build_continue_outline_requirements(
            Some("基础要求".repeat(120).as_str()),
            6,
            Some("payoff"),
            Some("relationship_shift"),
            Some("climax"),
            Some("本轮需要先压缩旧信息，再把背叛线和组织冲突一起推高，所有章节都要带出新的选择后果与回收。"),
            Some("emotion_drama"),
            Some("减少说明句，增加动作反馈"),
            Some("主线主题：逼出夜巡司内部叛徒，并让代价真实落到角色身上。"),
            Some(&["沈砚".to_string(), "苏槿".to_string(), "顾寒舟".to_string()]),
            Some(&[
                "第1章《夜巡异响》：怀表异响尚未回收，需要在续写里形成新的代价反馈。".to_string(),
                "第2章《暗门试探》：城门内应线索必须进入可验证阶段。".to_string(),
            ]),
            Some(&["怀表异响: 怀表异响尚未回收; status=planted; payoff_window=chapters 6-8".to_string()]),
            Some(&["沈砚: 旧伤反复发作，但必须维持队伍控制力并压住内部分裂。".to_string()]),
            Some(&["沈砚/苏槿: 盟友; 彼此试探并开始交换真实筹码。".to_string()]),
            Some(&["夜巡司: power=82; location=北城门; pressure=high; trust=fragile".to_string()]),
            Some(&["沈砚/夜巡人: stage 2; 晋升受阻，必须靠结果换取话语权。".to_string()]),
            Some(
                "【连载记忆与伏笔约束】\n【未完结伏笔】\n1. 怀表异响尚未回收\n2. 北城门押送线还未与内应名单合流\n3. 苏槿尚未交出她真正隐藏的交换条件\n4. 组织内部清洗压力持续升高",
            ),
            Some("【诊断优先级卡】\n- 当前最弱项：冲突升级\n- 需要减少解释性复述，增强结果推进。"),
            Some("【大纲近期质量趋势】\n- 最近三章的规则代价描写在下降，续写时必须把代价直接落在行动结果上。"),
            true,
        );

        assert!(merged.contains("【本轮创作总控】"));
        assert!(merged.contains("【质量预设】"));
        assert!(merged.contains("【结构蓝图】"));
        assert!(merged.contains("【大纲目标卡】"));
        assert!(merged.contains("【大纲结果卡】"));
        assert!(merged.contains("【大纲爽点回收卡】"));
        assert!(merged.contains("【大纲设定落地卡】"));
        assert!(merged.contains("【大纲开篇钩子卡】"));
        assert!(merged.contains("【大纲结尾悬停卡】"));
        assert!(merged.contains("【大纲角色弧光卡】"));
        assert!(merged.contains("【大纲执行清单】"));
        assert!(merged.contains("【连载记忆与伏笔约束】"));
        assert!(merged.contains("..."));
        assert!(merged.chars().count() <= 3600);
    }
}
