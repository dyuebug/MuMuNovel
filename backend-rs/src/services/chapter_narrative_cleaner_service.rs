pub fn is_likely_chapter_meta_line(line: &str) -> bool {
    let stripped = line.trim();
    if stripped.is_empty() {
        return false;
    }
    if stripped.starts_with("```") {
        return true;
    }

    let lowered = stripped.to_lowercase();
    let meta_prefixes = ["以下是章节正文：", "以下是正文：", "章节正文：", "正文："];
    if meta_prefixes.iter().any(|prefix| stripped == *prefix) {
        return true;
    }

    let prefix_checks = ["步骤", "step", "执行"];
    if prefix_checks
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
    {
        return true;
    }

    let contains_checks = [
        "调用 agent",
        "流程说明",
        "步骤说明",
        "流程日志",
        "步骤日志",
        "流程总结",
        "步骤总结",
        "流程复盘",
        "步骤复盘",
        "流程评审",
        "步骤评审",
        "方案对比",
        "方案评审",
        "复盘结论",
        "执行计划",
    ];
    if contains_checks
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        return true;
    }

    (lowered.starts_with("作为ai")
        || lowered.starts_with("作为 ai")
        || lowered.starts_with("身为ai")
        || lowered.starts_with("身为 ai")
        || lowered.starts_with("作为助手")
        || lowered.starts_with("身为助手")
        || lowered.starts_with("作为模型")
        || lowered.starts_with("身为模型"))
        && [':', '：', '?', '？', ',', '，']
            .iter()
            .any(|c| stripped.contains(*c))
}

pub fn lightly_polish_template_phrases(text: &str) -> String {
    let mut result = String::new();
    let mut next_second_seen = 0;
    let mut that_moment_seen = 0;
    for line in text.lines() {
        let mut current = line.to_string();
        if current.contains("下一秒") {
            next_second_seen += current.matches("下一秒").count();
            if next_second_seen > 1 {
                current = current.replacen("下一秒，", "", 1);
                current = current.replacen("下一秒、", "", 1);
                current = current.replacen("下一秒", "", 1);
            }
        }
        if current.contains("那一瞬") {
            that_moment_seen += current.matches("那一瞬").count();
            if that_moment_seen > 1 {
                current = current.replacen("那一瞬，", "", 1);
                current = current.replacen("那一瞬、", "", 1);
                current = current.replacen("那一瞬", "", 1);
            }
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(current.trim_end());
    }

    result = result.replace("像是有什么", "像有");
    result = result.replace("像有什么", "像有");
    result
}

pub fn sanitize_generated_narrative_text(text: &str) -> (String, usize) {
    let original = text.replace("\r\n", "\n").trim().to_string();
    if original.is_empty() {
        return (String::new(), 0);
    }

    let mut removed_line_count = 0usize;
    let mut kept_lines = Vec::new();
    for raw_line in original.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() {
            kept_lines.push(String::new());
            continue;
        }
        if is_likely_chapter_meta_line(stripped) {
            removed_line_count += 1;
            continue;
        }
        kept_lines.push(raw_line.to_string());
    }

    let mut cleaned = kept_lines.join("\n");
    while cleaned.contains("\n\n\n") {
        cleaned = cleaned.replace("\n\n\n", "\n\n");
    }
    cleaned = lightly_polish_template_phrases(cleaned.trim());
    while cleaned.contains("\n\n\n") {
        cleaned = cleaned.replace("\n\n\n", "\n\n");
    }
    (cleaned.trim().to_string(), removed_line_count)
}

pub fn contains_chapter_workflow_meta_text(text: &str) -> bool {
    text.lines().any(is_likely_chapter_meta_line)
}

#[cfg(test)]
mod tests {
    use super::{
        contains_chapter_workflow_meta_text, is_likely_chapter_meta_line,
        lightly_polish_template_phrases, sanitize_generated_narrative_text,
    };

    #[test]
    fn should_detect_meta_lines_and_strip_them_from_generated_text() {
        let input = "以下是章节正文：\n步骤说明\n正常正文第一段。\n\n正常正文第二段。";

        assert!(is_likely_chapter_meta_line("以下是章节正文："));
        assert!(is_likely_chapter_meta_line("步骤说明"));
        assert!(contains_chapter_workflow_meta_text(input));

        let (cleaned, removed_count) = sanitize_generated_narrative_text(input);

        assert_eq!(removed_count, 2);
        assert_eq!(cleaned, "正常正文第一段。\n\n正常正文第二段。");
    }

    #[test]
    fn should_lightly_polish_template_phrases_and_collapse_extra_blank_lines() {
        let input = "下一秒，他冲了出去。\n下一秒，他看见门后的人。\n\n\n那一瞬，空气都凝固了。\n那一瞬，他意识到像是有什么不对。";

        let polished = lightly_polish_template_phrases(input);
        assert!(polished.contains("像有不对"));
        assert!(!polished.contains("下一秒，他看见门后的人。"));
        assert!(!polished.contains("那一瞬，他意识到"));

        let (cleaned, removed_count) = sanitize_generated_narrative_text(input);
        assert_eq!(removed_count, 0);
        assert!(!cleaned.contains("\n\n\n"));
        assert!(cleaned.contains("像有不对"));
    }

    #[test]
    fn should_return_empty_for_blank_or_meta_only_generated_text() {
        let (blank_cleaned, blank_removed) = sanitize_generated_narrative_text("   \r\n   ");
        assert_eq!(blank_cleaned, "");
        assert_eq!(blank_removed, 0);

        let meta_only = "```markdown\n作为AI：我将开始执行\n流程说明";
        assert!(contains_chapter_workflow_meta_text(meta_only));

        let (cleaned, removed_count) = sanitize_generated_narrative_text(meta_only);
        assert_eq!(cleaned, "");
        assert_eq!(removed_count, 3);
    }
}
