use crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext;
use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextCompactionProfile {
    total: usize,
    characters_info: usize,
    chapter_careers: usize,
    foreshadow_reminders: usize,
    relevant_memories: usize,
    continuation_point: usize,
    previous_chapter_summary: usize,
    reference_items: usize,
    recent_chapters_context: usize,
    recent_chapters_count: usize,
}

fn preview_text(text: &str, max_length: usize) -> Option<String> {
    let value = text.trim();
    if value.is_empty() {
        return None;
    }
    let shortened = if value.chars().count() <= max_length {
        value.to_string()
    } else {
        let prefix: String = value.chars().take(max_length).collect();
        format!("{prefix}...")
    };
    Some(shortened)
}

fn split_context_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(current);
                current = Vec::new();
            }
            continue;
        }
        current.push(line.to_string());
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn compact_tail_text(text: &str, max_length: usize) -> Option<String> {
    let value = text.trim();
    if value.is_empty() {
        return None;
    }
    let count = value.chars().count();
    if count <= max_length {
        return Some(value.to_string());
    }
    Some(value.chars().skip(count - max_length).collect())
}

fn compact_recent_chapters_context(
    text: &str,
    max_chapters: usize,
    max_length: usize,
    line_preview_length: usize,
) -> Option<String> {
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    if lines.is_empty() {
        return None;
    }

    let (header, chapter_lines) = if lines[0].starts_with('【') {
        (lines[0].clone(), lines[1..].to_vec())
    } else {
        ("【最近章节规划】".to_string(), lines)
    };

    let start = chapter_lines.len().saturating_sub(max_chapters);
    let selected = &chapter_lines[start..];

    let mut compact_lines = vec![header];
    for line in selected {
        if let Some(preview) = preview_text(line, line_preview_length) {
            compact_lines.push(preview);
        }
    }
    preview_text(&compact_lines.join("\n"), max_length)
}

fn compact_bulleted_reference_block(
    text: &str,
    max_items: usize,
    max_length: usize,
    detail_lines_per_item: usize,
    line_preview_length: usize,
) -> Option<String> {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut header_lines = Vec::new();
    let mut items: Vec<Vec<String>> = Vec::new();
    let mut current_item: Vec<String> = Vec::new();

    for raw_line in lines {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('【') {
            if !current_item.is_empty() {
                items.push(current_item);
                current_item = Vec::new();
            }
            if !header_lines.iter().any(|header| header == line) {
                header_lines.push(line.to_string());
            }
            continue;
        }
        if line.starts_with("- ") {
            if !current_item.is_empty() {
                items.push(current_item);
            }
            current_item = vec![line.to_string()];
            continue;
        }
        if !current_item.is_empty() {
            current_item.push(line.to_string());
        } else if !header_lines.is_empty() {
            current_item = vec![line.to_string()];
        } else {
            header_lines.push(line.to_string());
        }
    }
    if !current_item.is_empty() {
        items.push(current_item);
    }

    let mut selected_lines = header_lines;
    for item in items.into_iter().take(max_items) {
        if let Some(preview) = preview_text(&item[0], line_preview_length) {
            selected_lines.push(preview);
        }
        for detail in item.iter().skip(1).take(detail_lines_per_item) {
            if let Some(preview) = preview_text(detail, line_preview_length) {
                selected_lines.push(preview);
            }
        }
    }

    preview_text(&selected_lines.join("\n"), max_length)
}

fn compact_entity_context(
    text: &str,
    max_blocks: usize,
    max_length: usize,
    max_lines_per_block: usize,
    line_preview_length: usize,
    preferred_prefixes: &[&str],
) -> Option<String> {
    let blocks = split_context_blocks(text);
    if blocks.is_empty() {
        return None;
    }

    let mut compact_blocks = Vec::new();
    for block in blocks.into_iter().take(max_blocks) {
        let header = preview_text(&block[0], line_preview_length)?;
        let detail_lines: Vec<String> = block
            .iter()
            .skip(1)
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        let mut selected_details = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for prefix in preferred_prefixes {
            for line in &detail_lines {
                if seen.contains(line) || !line.starts_with(prefix) {
                    continue;
                }
                if let Some(preview) = preview_text(line, line_preview_length) {
                    selected_details.push(preview);
                }
                seen.insert(line.clone());
                if selected_details.len() >= max_lines_per_block.saturating_sub(1) {
                    break;
                }
            }
            if selected_details.len() >= max_lines_per_block.saturating_sub(1) {
                break;
            }
        }

        if selected_details.len() < max_lines_per_block.saturating_sub(1) {
            for line in &detail_lines {
                if seen.contains(line) {
                    continue;
                }
                if let Some(preview) = preview_text(line, line_preview_length) {
                    selected_details.push(preview);
                }
                seen.insert(line.clone());
                if selected_details.len() >= max_lines_per_block.saturating_sub(1) {
                    break;
                }
            }
        }

        let mut compact_block_lines = vec![header];
        compact_block_lines.extend(selected_details);
        compact_blocks.push(compact_block_lines.join("\n"));
    }

    preview_text(&compact_blocks.join("\n\n"), max_length)
}

fn resolve_context_compaction_profile(
    target_word_count: i32,
    mode: &str,
) -> ContextCompactionProfile {
    let resolved_target = target_word_count.max(1200) as usize;
    if mode == "one-to-one" {
        ContextCompactionProfile {
            total: (resolved_target.saturating_mul(155) / 100).clamp(3600, 5200),
            characters_info: 1280,
            chapter_careers: 560,
            foreshadow_reminders: 420,
            relevant_memories: 760,
            continuation_point: 420,
            previous_chapter_summary: 220,
            reference_items: 5,
            recent_chapters_context: 0,
            recent_chapters_count: 0,
        }
    } else {
        ContextCompactionProfile {
            total: (resolved_target.saturating_mul(185) / 100).clamp(4800, 6800),
            characters_info: 1500,
            chapter_careers: 720,
            foreshadow_reminders: 520,
            relevant_memories: 860,
            continuation_point: 420,
            previous_chapter_summary: 220,
            reference_items: 6,
            recent_chapters_context: 960,
            recent_chapters_count: 4,
        }
    }
}

fn total_context_length(
    payload: &PromptContextProviderPayload,
    previous_context: &PreviousChapterPromptContext,
) -> usize {
    [
        payload.characters_info.as_str(),
        payload.chapter_careers.as_str(),
        payload.recent_chapters_context.as_str(),
        payload.previous_chapter_summary.as_str(),
        payload.foreshadow_reminders.as_str(),
        payload.relevant_memories.as_str(),
        previous_context.continuation_point.as_str(),
        previous_context.previous_chapter_content.as_str(),
    ]
    .into_iter()
    .map(|value| value.chars().count())
    .sum()
}

fn maybe_replace(current_value: &mut String, candidate: Option<String>) {
    let current_trimmed = current_value.trim();
    let Some(candidate) = candidate.map(|value| value.trim().to_string()) else {
        return;
    };
    if current_trimmed.is_empty()
        || candidate.is_empty()
        || candidate.chars().count() >= current_trimmed.chars().count()
    {
        return;
    }
    *current_value = candidate;
}

pub(crate) fn compact_generation_context(
    outline_mode: &str,
    target_word_count: i32,
    mut provider_payload: PromptContextProviderPayload,
    mut previous_context: PreviousChapterPromptContext,
) -> (PromptContextProviderPayload, PreviousChapterPromptContext) {
    let profile = resolve_context_compaction_profile(target_word_count, outline_mode);

    if profile.recent_chapters_context > 0 {
        let compacted_recent_context = compact_recent_chapters_context(
            &provider_payload.recent_chapters_context,
            profile.recent_chapters_count,
            profile.recent_chapters_context,
            180,
        );
        maybe_replace(
            &mut provider_payload.recent_chapters_context,
            compacted_recent_context,
        );
    }
    let compacted_chapter_careers = compact_entity_context(
        &provider_payload.chapter_careers,
        3,
        profile.chapter_careers,
        6,
        92,
        &["描述:", "分类:", "阶段体系:", "特殊能力:"],
    );
    maybe_replace(
        &mut provider_payload.chapter_careers,
        compacted_chapter_careers,
    );
    let compacted_foreshadow_reminders = compact_bulleted_reference_block(
        &provider_payload.foreshadow_reminders,
        profile.reference_items,
        profile.foreshadow_reminders,
        2,
        92,
    );
    maybe_replace(
        &mut provider_payload.foreshadow_reminders,
        compacted_foreshadow_reminders,
    );
    let compacted_relevant_memories = compact_bulleted_reference_block(
        &provider_payload.relevant_memories,
        profile.reference_items,
        profile.relevant_memories,
        1,
        92,
    );
    maybe_replace(
        &mut provider_payload.relevant_memories,
        compacted_relevant_memories,
    );
    let compacted_continuation_point = compact_tail_text(
        &previous_context.continuation_point,
        profile.continuation_point,
    );
    maybe_replace(
        &mut previous_context.continuation_point,
        compacted_continuation_point,
    );
    let compacted_previous_chapter_content = compact_tail_text(
        &previous_context.previous_chapter_content,
        profile.continuation_point,
    );
    maybe_replace(
        &mut previous_context.previous_chapter_content,
        compacted_previous_chapter_content,
    );
    let compacted_previous_summary = preview_text(
        &provider_payload.previous_chapter_summary,
        profile.previous_chapter_summary,
    );
    maybe_replace(
        &mut provider_payload.previous_chapter_summary,
        compacted_previous_summary,
    );

    if total_context_length(&provider_payload, &previous_context) > profile.total {
        let compacted_characters_info = compact_entity_context(
            &provider_payload.characters_info,
            6,
            profile.characters_info,
            6,
            92,
            &[
                "当前状态",
                "生存状态",
                "主职业",
                "副职业",
                "关系网络",
                "组织归属",
                "组织类型",
                "组织目的",
            ],
        );
        maybe_replace(
            &mut provider_payload.characters_info,
            compacted_characters_info,
        );
    }

    (provider_payload, previous_context)
}

pub(crate) fn build_generation_context_compaction_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service",
        "scope": "shared_generation_context_compaction",
        "python_source_map": [
            "backend/app/services/chapter_context_service.py",
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/chapter_regeneration_context_service.py"
        ],
        "rust_target_map": [
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/context_compaction_owner.rs",
            "backend-rs/src/services/chapter_regeneration_prepare_service.rs",
            "backend-rs/src/services/chapter_generation_prompt_service.rs"
        ],
        "rust_owner_functions": [
            "compact_generation_context",
            "resolve_context_compaction_profile",
            "compact_recent_chapters_context",
            "compact_entity_context",
            "compact_bulleted_reference_block",
            "compact_tail_text",
            "maybe_replace"
        ],
        "behavior_contract": {
            "modes": ["one-to-one", "one-to-many"],
            "one_to_one_skips_recent_chapters_context": true,
            "one_to_many_recent_chapters_count": 4,
            "min_target_word_count": 1200,
            "compacted_provider_fields": [
                "characters_info",
                "chapter_careers",
                "recent_chapters_context",
                "previous_chapter_summary",
                "foreshadow_reminders",
                "relevant_memories"
            ],
            "compacted_previous_context_fields": [
                "continuation_point",
                "previous_chapter_content"
            ],
            "replacement_policy": "replace_only_when_candidate_is_non_empty_and_shorter",
            "prompt_visibility_preserved": [
                "research_query",
                "research_assets",
                "external_assets",
                "reference_assets",
                "mcp_references"
            ]
        },
        "active_consumers": [
            "chapter_generation_runtime_service",
            "chapter_regeneration_prepare_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo test chapter_regeneration_prepare_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_chapter_context_compaction_as_source_map_until_explicit_freeze_delete_round",
            "runtime_knob": "ChapterCandidateRouteGatewayConfig",
            "compatibility_note": "Context compaction must preserve prompt-visible provider payload fields and only shorten oversized context"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{build_generation_context_compaction_owner_contract, compact_generation_context};
    use crate::services::chapter_generation_prompt_service::PreviousChapterPromptContext;
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;

    fn build_payload() -> PromptContextProviderPayload {
        PromptContextProviderPayload {
            characters_info: "【角色】\n角色甲\n当前状态: ".to_string() + &"甲".repeat(1500),
            chapter_careers: "【职业】\n职业甲\n描述: ".to_string() + &"乙".repeat(800),
            recent_chapters_context: format!(
                "【最近章节规划】\n{}\n{}\n{}\n{}\n{}",
                "第一章：".to_string() + &"丙".repeat(240),
                "第二章：".to_string() + &"丁".repeat(240),
                "第三章：".to_string() + &"戊".repeat(240),
                "第四章：".to_string() + &"己".repeat(240),
                "第五章：".to_string() + &"庚".repeat(240),
            ),
            previous_chapter_summary: "辛".repeat(400),
            foreshadow_reminders: format!(
                "【伏笔提醒】\n- 伏笔一{}\n  细节{}\n- 伏笔二{}\n  细节{}",
                "壬".repeat(180),
                "癸".repeat(180),
                "子".repeat(180),
                "丑".repeat(180)
            ),
            relevant_memories: format!(
                "【相关记忆】\n- 记忆一{}\n- 记忆二{}\n- 记忆三{}",
                "寅".repeat(180),
                "卯".repeat(180),
                "辰".repeat(180)
            ),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        }
    }

    #[test]
    fn should_compact_one_to_many_generation_context_fields() {
        let payload = build_payload();
        let previous = PreviousChapterPromptContext {
            continuation_point: "午".repeat(600),
            previous_chapter_content: "未".repeat(600),
        };

        let (compacted_payload, compacted_previous) =
            compact_generation_context("one-to-many", 3000, payload, previous);

        assert!(compacted_payload.recent_chapters_context.chars().count() < 960 + 4);
        assert!(compacted_payload.chapter_careers.chars().count() < 720 + 4);
        assert!(compacted_payload.foreshadow_reminders.chars().count() < 520 + 4);
        assert!(compacted_payload.relevant_memories.chars().count() < 860 + 4);
        assert!(compacted_payload.previous_chapter_summary.chars().count() < 220 + 4);
        assert_eq!(compacted_previous.continuation_point.chars().count(), 420);
        assert_eq!(
            compacted_previous.previous_chapter_content.chars().count(),
            420
        );
    }

    #[test]
    fn should_skip_recent_context_compaction_for_one_to_one_mode() {
        let payload = build_payload();
        let previous = PreviousChapterPromptContext {
            continuation_point: "午".repeat(300),
            previous_chapter_content: "未".repeat(300),
        };

        let original_recent = payload.recent_chapters_context.clone();
        let (compacted_payload, _) =
            compact_generation_context("one-to-one", 2600, payload, previous);

        assert_eq!(compacted_payload.recent_chapters_context, original_recent);
        assert!(compacted_payload.previous_chapter_summary.chars().count() < 220 + 4);
    }

    #[test]
    fn should_publish_generation_context_compaction_owner_contract() {
        let contract = build_generation_context_compaction_owner_contract();

        assert_eq!(contract["owner"], "chapter_generation_runtime_service");
        assert_eq!(contract["scope"], "shared_generation_context_compaction");
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_context_service.py"
        );
        assert_eq!(
            contract["rust_target_map"][0],
            "backend-rs/src/services/chapter_generation_runtime_service.rs"
        );
        assert_eq!(
            contract["rust_owner_functions"][0],
            "compact_generation_context"
        );
        assert_eq!(
            contract["behavior_contract"]["one_to_one_skips_recent_chapters_context"],
            true
        );
        assert_eq!(
            contract["behavior_contract"]["compacted_provider_fields"][5],
            "relevant_memories"
        );
        assert_eq!(
            contract["behavior_contract"]["prompt_visibility_preserved"][4],
            "mcp_references"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter_regeneration_prepare_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "keep_python_chapter_context_compaction_as_source_map_until_explicit_freeze_delete_round"
        );
    }
}
