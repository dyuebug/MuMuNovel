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

fn is_sentence_boundary(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n')
}

pub fn trim_text_to_sentence_boundary(text: &str, hard_limit: usize) -> String {
    trim_text_to_sentence_boundary_with_lookback(text, hard_limit, 220)
}

pub fn trim_text_to_sentence_boundary_with_lookback(
    text: &str,
    hard_limit: usize,
    lookback_chars: usize,
) -> String {
    let normalized_text = text.to_string();
    let char_count = normalized_text.chars().count();
    if hard_limit == 0 || char_count <= hard_limit {
        return normalized_text.trim().to_string();
    }

    let search_start = hard_limit.saturating_sub(lookback_chars.max(80));
    let mut best_boundary_index = None;
    for (char_index, ch) in normalized_text.chars().enumerate() {
        if char_index < search_start {
            continue;
        }
        if char_index > hard_limit {
            break;
        }
        if is_sentence_boundary(ch) {
            best_boundary_index = Some(char_index);
        }
    }

    if let Some(boundary_index) = best_boundary_index.filter(|index| *index >= search_start) {
        return normalized_text
            .chars()
            .take(boundary_index + 1)
            .collect::<String>()
            .trim()
            .to_string();
    }

    let mut trimmed_text = normalized_text
        .chars()
        .take(hard_limit)
        .collect::<String>()
        .trim_end_matches(['，', ',', '、', ' '])
        .to_string();
    if let Some(last_char) = trimmed_text.chars().last() {
        if !is_sentence_boundary(last_char) {
            trimmed_text.push('。');
        }
    }
    trimmed_text.trim().to_string()
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
        trim_text_to_sentence_boundary, trim_text_to_sentence_boundary_with_lookback,
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

    #[test]
    fn should_trim_generated_text_to_recent_sentence_boundary_like_python_owner() {
        let input = "第一句还在铺垫。第二句推进冲突！第三句继续延展到更长内容";

        let trimmed = trim_text_to_sentence_boundary_with_lookback(input, 17, 80);

        assert_eq!(trimmed, "第一句还在铺垫。第二句推进冲突！");
    }

    #[test]
    fn should_trim_generated_text_with_sentence_fallback_when_no_boundary_is_nearby() {
        let input = "没有句界的连续正文内容";

        let trimmed = trim_text_to_sentence_boundary(input, 5);

        assert_eq!(trimmed, "没有句界的。");
    }

    #[test]
    fn should_count_unicode_characters_when_trimming_to_sentence_boundary() {
        let input = "甲乙丙丁，戊己庚辛。壬癸";

        let trimmed = trim_text_to_sentence_boundary(input, 9);

        assert_eq!(trimmed, "甲乙丙丁，戊己庚辛。");
    }
}
