use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::models::{chapter, generation_history};
use crate::services::chapter_quality_metrics_query_service::load_latest_quality_metric_records_for_chapter_ids;
use crate::services::chapter_story_repair_quality_context_service::{
    build_quality_metrics_summary_from_state, build_quality_metrics_summary_state_from_history,
    extract_repair_guidance_object, normalize_guidance_items,
};

const OUTLINE_QUALITY_SUMMARY_CACHE_MAX_SIZE: usize = 128;
const OUTLINE_QUALITY_SUMMARY_SNAPSHOT_DIR: &str =
    "../backend/data/outline_quality_summary_snapshots";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct OutlineQualitySummarySnapshot {
    chapter_keys: Vec<(String, i32)>,
    history_count: i64,
    history_latest_created_at: Option<String>,
    summary: Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct OutlineQualityGuidanceBundle {
    pub quality_repair_guidance: String,
    pub quality_trend_guidance: String,
}

#[derive(Debug, Default)]
struct OutlineQualitySummarySnapshotCache {
    entries: HashMap<String, OutlineQualitySummarySnapshot>,
    insertion_order: VecDeque<String>,
}

impl OutlineQualitySummarySnapshotCache {
    fn get(&self, cache_key: &str) -> Option<OutlineQualitySummarySnapshot> {
        self.entries.get(cache_key).cloned()
    }

    fn insert(
        &mut self,
        cache_key: String,
        snapshot: OutlineQualitySummarySnapshot,
        max_cache_size: usize,
    ) {
        if !self.entries.contains_key(&cache_key) {
            self.insertion_order.push_back(cache_key.clone());
        }
        self.entries.insert(cache_key, snapshot);

        while self.entries.len() > max_cache_size {
            let Some(oldest_key) = self.insertion_order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest_key);
        }
    }
}

struct OutlineQualitySummarySnapshotRequest<'a> {
    project_id: &'a str,
    chapter_limit: usize,
    chapter_keys: &'a [(String, i32)],
    history_count: i64,
    history_latest_created_at: Option<&'a str>,
    summary: &'a Value,
}

fn outline_quality_summary_snapshot_cache() -> &'static Mutex<OutlineQualitySummarySnapshotCache> {
    static CACHE: OnceLock<Mutex<OutlineQualitySummarySnapshotCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(OutlineQualitySummarySnapshotCache::default()))
}

fn outline_quality_summary_snapshot_root() -> PathBuf {
    PathBuf::from(OUTLINE_QUALITY_SUMMARY_SNAPSHOT_DIR)
}

fn build_outline_quality_summary_cache_key(project_id: &str, chapter_limit: usize) -> String {
    format!("{project_id}:{chapter_limit}")
}

fn normalize_snapshot_file_stem(project_id: &str, chapter_limit: usize) -> String {
    let normalized_project_id = project_id
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized_project_id = if normalized_project_id.is_empty() {
        "project".to_string()
    } else {
        normalized_project_id
    };
    format!("{normalized_project_id}__{chapter_limit}")
}

fn outline_quality_summary_snapshot_path(
    snapshot_root: &Path,
    project_id: &str,
    chapter_limit: usize,
) -> PathBuf {
    snapshot_root.join(format!(
        "{}.json",
        normalize_snapshot_file_stem(project_id, chapter_limit)
    ))
}

fn serialize_outline_quality_summary_timestamp(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_outline_quality_summary_snapshot_fresh(
    cached_snapshot: Option<&OutlineQualitySummarySnapshot>,
    request: &OutlineQualitySummarySnapshotRequest<'_>,
) -> bool {
    let Some(cached_snapshot) = cached_snapshot else {
        return false;
    };
    cached_snapshot.chapter_keys == request.chapter_keys
        && cached_snapshot.history_count == request.history_count
        && cached_snapshot.history_latest_created_at
            == serialize_outline_quality_summary_timestamp(request.history_latest_created_at)
        && cached_snapshot.summary.is_object()
}

fn build_outline_quality_summary_snapshot(
    request: &OutlineQualitySummarySnapshotRequest<'_>,
) -> OutlineQualitySummarySnapshot {
    OutlineQualitySummarySnapshot {
        chapter_keys: request.chapter_keys.to_vec(),
        history_count: request.history_count.max(0),
        history_latest_created_at: serialize_outline_quality_summary_timestamp(
            request.history_latest_created_at,
        ),
        summary: request.summary.clone(),
    }
}

async fn load_outline_quality_summary_snapshot(
    snapshot_root: &Path,
    project_id: &str,
    chapter_limit: usize,
) -> Result<Option<OutlineQualitySummarySnapshot>, String> {
    let snapshot_path =
        outline_quality_summary_snapshot_path(snapshot_root, project_id, chapter_limit);
    let content = match tokio::fs::read_to_string(&snapshot_path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read outline quality summary snapshot failed: {error}"
            ))
        }
    };

    match serde_json::from_str::<OutlineQualitySummarySnapshot>(&content) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(_) => Ok(None),
    }
}

async fn persist_outline_quality_summary_snapshot(
    snapshot_root: &Path,
    project_id: &str,
    chapter_limit: usize,
    snapshot: &OutlineQualitySummarySnapshot,
) -> Result<(), String> {
    let snapshot_path =
        outline_quality_summary_snapshot_path(snapshot_root, project_id, chapter_limit);
    if let Some(parent) = snapshot_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!("create outline quality summary snapshot dir failed: {error}")
        })?;
    }

    let serialized = serde_json::to_string(snapshot)
        .map_err(|error| format!("encode outline quality summary snapshot failed: {error}"))?;
    tokio::fs::write(&snapshot_path, serialized)
        .await
        .map_err(|error| format!("write outline quality summary snapshot failed: {error}"))
}

async fn resolve_outline_quality_summary_snapshot_with_dependencies(
    cache: &Mutex<OutlineQualitySummarySnapshotCache>,
    snapshot_root: &Path,
    request: &OutlineQualitySummarySnapshotRequest<'_>,
    max_cache_size: usize,
) -> Result<Value, String> {
    let cache_key =
        build_outline_quality_summary_cache_key(request.project_id, request.chapter_limit);
    let mut cached_snapshot = {
        let guard = cache.lock().await;
        guard.get(&cache_key)
    };
    if cached_snapshot.is_none() {
        if let Some(persisted_snapshot) = load_outline_quality_summary_snapshot(
            snapshot_root,
            request.project_id,
            request.chapter_limit,
        )
        .await?
        {
            let mut guard = cache.lock().await;
            guard.insert(
                cache_key.clone(),
                persisted_snapshot.clone(),
                max_cache_size,
            );
            cached_snapshot = Some(persisted_snapshot);
        }
    }

    if is_outline_quality_summary_snapshot_fresh(cached_snapshot.as_ref(), request) {
        return Ok(cached_snapshot
            .and_then(|snapshot| snapshot.summary.as_object().cloned().map(Value::Object))
            .unwrap_or_else(|| json!({})));
    }

    let snapshot = build_outline_quality_summary_snapshot(request);
    {
        let mut guard = cache.lock().await;
        guard.insert(cache_key, snapshot.clone(), max_cache_size);
    }
    persist_outline_quality_summary_snapshot(
        snapshot_root,
        request.project_id,
        request.chapter_limit,
        &snapshot,
    )
    .await?;

    Ok(snapshot.summary)
}

fn build_outline_quality_summary_chapter_keys(chapters: &[chapter::Model]) -> Vec<(String, i32)> {
    chapters
        .iter()
        .map(|item| (item.id.trim().to_string(), item.chapter_number))
        .filter(|(chapter_id, _)| !chapter_id.is_empty())
        .collect()
}

fn format_metric_value(value: Option<&Value>) -> Option<String> {
    let numeric = value.and_then(Value::as_f64)?;
    if (numeric.fract() - 0.0).abs() < f64::EPSILON {
        Some((numeric as i64).to_string())
    } else {
        Some(format!("{numeric:.1}"))
    }
}

fn build_outline_quality_repair_guidance_from_summary(summary: &Value) -> String {
    let Some(summary_object) = summary.as_object() else {
        return String::new();
    };
    if summary_object.is_empty() {
        return String::new();
    }

    let Some(mut repair_guidance) = extract_repair_guidance_object(Some(summary)) else {
        return String::new();
    };
    let chapter_count = summary
        .get("chapter_count")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    repair_guidance
        .entry("source".to_string())
        .or_insert_with(|| json!("recent_chapter_quality_summary"));
    repair_guidance
        .entry("source_label".to_string())
        .or_insert_with(|| {
            if chapter_count > 0 {
                json!(format!("最近{chapter_count}章质量汇总"))
            } else {
                json!("最近章节质量汇总")
            }
        });

    let summary_text = repair_guidance
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let source_label = repair_guidance
        .get("source_label")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let weakest_metric_label = repair_guidance
        .get("weakest_metric_label")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let weakest_metric_value = format_metric_value(repair_guidance.get("weakest_metric_value"));
    let focus_areas = repair_guidance
        .get("focus_areas")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let failed_metrics = repair_guidance
        .get("quality_gate_failed_metrics")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let quality_gate_label = repair_guidance
        .get("quality_gate_label")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let quality_gate_summary = repair_guidance
        .get("quality_gate_summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();

    if source_label.is_empty()
        && weakest_metric_label.is_empty()
        && summary_text.is_empty()
        && focus_areas.is_empty()
        && failed_metrics.is_empty()
        && quality_gate_label.is_empty()
        && quality_gate_summary.is_empty()
    {
        return String::new();
    }

    let mut lines = vec!["【诊断优先级卡】".to_string()];
    if !source_label.is_empty() {
        lines.push(format!("- 诊断来源：{source_label}"));
    }
    if !quality_gate_label.is_empty() {
        if quality_gate_summary.is_empty() || quality_gate_summary == summary_text {
            lines.push(format!("- 质量门禁：{quality_gate_label}"));
        } else {
            lines.push(format!(
                "- 质量门禁：{}（{}）",
                quality_gate_label, quality_gate_summary
            ));
        }
    } else if !quality_gate_summary.is_empty() {
        lines.push(format!("- 质量门禁：{quality_gate_summary}"));
    }
    if !failed_metrics.is_empty() {
        lines.push(format!("- 门禁失败维度：{}", failed_metrics.join(" / ")));
    }
    if !weakest_metric_label.is_empty() {
        let metric_line = weakest_metric_value
            .map(|value| format!("{}（当前值：{}）", weakest_metric_label, value))
            .unwrap_or_else(|| weakest_metric_label.to_string());
        lines.push(format!("- 当前最弱项：{metric_line}"));
    }
    if !focus_areas.is_empty() {
        lines.push(format!("- 优先修复维度：{}", focus_areas.join(" / ")));
    }
    if !summary_text.is_empty() {
        lines.push(format!("- 诊断结论：{summary_text}"));
    }
    lines.push("- 先把最弱项拆成每章的目标、阻力、回报与章尾牵引，再统一分配节拍。".to_string());

    lines.join("\n")
}

fn trend_note_from_summary(summary: &Value) -> Option<String> {
    let trend = summary
        .get("overall_score_trend")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let delta = summary.get("overall_score_delta").and_then(Value::as_f64);
    let note = match trend {
        "rising" => "整体质量趋势在回升，本轮可以稳中求进。",
        "stable" => "整体质量趋势相对稳定，本轮要优先补短板。",
        "falling" => "整体质量趋势在下滑，本轮必须主动修复关键短板。",
        _ => return None,
    };

    Some(match delta {
        Some(delta) => format!("- 趋势判断：{}（最近综合分变化 {:+.1}）。", note, delta),
        None => format!("- 趋势判断：{}。", note),
    })
}

fn build_outline_story_quality_trend_guidance_from_summary(summary: &Value) -> String {
    let Some(summary_object) = summary.as_object() else {
        return String::new();
    };
    if summary_object.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【大纲近期质量趋势】".to_string()];
    let chapter_count = summary
        .get("chapter_count")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if chapter_count > 0 {
        lines.push(format!("- 参考范围：最近 {chapter_count} 章的生成反馈。"));
    }
    if let Some(trend_note) = trend_note_from_summary(summary) {
        lines.push(trend_note);
    }
    if let Some(value) = summary.get("avg_payoff_chain_rate").and_then(Value::as_f64) {
        lines.push(format!(
            "- 最近回报兑现均值：{value:.1}%，后续章节要优先回收旧承诺与已埋伏笔。"
        ));
    }
    if let Some(value) = summary.get("avg_cliffhanger_rate").and_then(Value::as_f64) {
        lines.push(format!(
            "- 最近章尾牵引均值：{value:.1}%，后续章节要在尾段留下明确的未决问题、危险或代价。"
        ));
    }
    if let Some(value) = summary.get("avg_pacing_score").and_then(Value::as_f64) {
        lines.push(format!(
            "- 最近节奏稳定度均值：{value:.1}/10，后续章节要保持冲突推进和信息释放的连续压强。"
        ));
    }
    let focus_areas = summary
        .get("recent_focus_areas")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 3))
        .unwrap_or_default();
    if !focus_areas.is_empty() {
        lines.push(format!("- 最近高频修复焦点：{}。", focus_areas.join(" / ")));
    }
    lines.push("- 后续章节要优先确保承诺回收、规则落地与章尾牵引三条线同时成立。".to_string());

    lines.join("\n")
}

async fn load_outline_quality_summary(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_limit: usize,
) -> Result<Value, String> {
    let recent_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_desc(chapter::Column::ChapterNumber)
        .limit(chapter_limit as u64)
        .all(db)
        .await
        .map_err(|error| format!("load outline quality chapters failed: {error}"))?;
    let chapter_keys = build_outline_quality_summary_chapter_keys(&recent_chapters);
    let chapter_ids = recent_chapters
        .iter()
        .map(|item| item.id.trim().to_string())
        .filter(|item: &String| !item.is_empty())
        .collect::<Vec<_>>();
    if chapter_ids.is_empty() {
        return Ok(json!({}));
    }

    let history_count = generation_history::Entity::find()
        .filter(generation_history::Column::ProjectId.eq(project_id))
        .filter(generation_history::Column::ChapterId.is_in(chapter_ids.clone()))
        .count(db)
        .await
        .map_err(|error| format!("count outline quality history failed: {error}"))?
        as i64;
    let latest_history_created_at = generation_history::Entity::find()
        .filter(generation_history::Column::ProjectId.eq(project_id))
        .filter(generation_history::Column::ChapterId.is_in(chapter_ids.clone()))
        .order_by_desc(generation_history::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| format!("load latest outline quality history failed: {error}"))?
        .and_then(|history| history.created_at)
        .map(|value| value.format("%Y-%m-%dT%H:%M:%S").to_string());

    let computed_summary = if history_count <= 0 {
        json!({})
    } else {
        let metrics_by_chapter =
            load_latest_quality_metric_records_for_chapter_ids(db, &chapter_ids)
                .await
                .map_err(|error| format!("load outline quality metric records failed: {error}"))?;
        let metrics_history = chapter_ids
            .iter()
            .filter_map(|chapter_id| {
                metrics_by_chapter
                    .get(chapter_id)
                    .map(|record| record.latest_quality_metrics.clone())
            })
            .collect::<Vec<_>>();
        let summary_state =
            build_quality_metrics_summary_state_from_history(&metrics_history, "outline");
        build_quality_metrics_summary_from_state(
            summary_state.as_ref(),
            &metrics_history,
            "outline",
        )
        .unwrap_or_else(|| json!({}))
    };

    let request = OutlineQualitySummarySnapshotRequest {
        project_id,
        chapter_limit,
        chapter_keys: &chapter_keys,
        history_count,
        history_latest_created_at: latest_history_created_at.as_deref(),
        summary: &computed_summary,
    };

    resolve_outline_quality_summary_snapshot_with_dependencies(
        outline_quality_summary_snapshot_cache(),
        &outline_quality_summary_snapshot_root(),
        &request,
        OUTLINE_QUALITY_SUMMARY_CACHE_MAX_SIZE,
    )
    .await
}

pub(crate) async fn build_outline_quality_guidance_bundle(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_limit: usize,
) -> Result<OutlineQualityGuidanceBundle, String> {
    let summary = load_outline_quality_summary(db, project_id, chapter_limit).await?;
    Ok(OutlineQualityGuidanceBundle {
        quality_repair_guidance: build_outline_quality_repair_guidance_from_summary(&summary),
        quality_trend_guidance: build_outline_story_quality_trend_guidance_from_summary(&summary),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_outline_quality_repair_guidance_from_summary,
        build_outline_quality_summary_cache_key,
        build_outline_story_quality_trend_guidance_from_summary,
        outline_quality_summary_snapshot_path,
        resolve_outline_quality_summary_snapshot_with_dependencies,
        OutlineQualitySummarySnapshotCache, OutlineQualitySummarySnapshotRequest,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    fn temp_snapshot_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "mumu_outline_quality_summary_snapshot_test_{}",
            Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn should_reuse_fresh_outline_quality_summary_snapshot_from_cache() {
        let cache = Mutex::new(OutlineQualitySummarySnapshotCache::default());
        let snapshot_root = temp_snapshot_root();
        let chapter_keys = vec![("chapter-3".to_string(), 3), ("chapter-2".to_string(), 2)];
        let first_summary = json!({
            "chapter_count": 2,
            "repair_guidance": {"summary": "cached guidance"},
        });
        let second_summary = json!({
            "chapter_count": 2,
            "repair_guidance": {"summary": "new guidance"},
        });

        let first_request = OutlineQualitySummarySnapshotRequest {
            project_id: "project-1",
            chapter_limit: 2,
            chapter_keys: &chapter_keys,
            history_count: 4,
            history_latest_created_at: Some("2026-06-02T18:00:00"),
            summary: &first_summary,
        };
        let second_request = OutlineQualitySummarySnapshotRequest {
            summary: &second_summary,
            ..first_request
        };

        let first_resolved = resolve_outline_quality_summary_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &first_request,
            128,
        )
        .await
        .expect("first resolve should persist snapshot");
        assert_eq!(
            first_resolved["repair_guidance"]["summary"],
            json!("cached guidance")
        );

        let second_resolved = resolve_outline_quality_summary_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &second_request,
            128,
        )
        .await
        .expect("fresh cache should be reused");
        assert_eq!(
            second_resolved["repair_guidance"]["summary"],
            json!("cached guidance")
        );
    }

    #[tokio::test]
    async fn should_restore_outline_quality_summary_snapshot_from_disk_after_cache_clear() {
        let cache = Mutex::new(OutlineQualitySummarySnapshotCache::default());
        let snapshot_root = temp_snapshot_root();
        let chapter_keys = vec![("chapter-3".to_string(), 3), ("chapter-2".to_string(), 2)];
        let persisted_summary = json!({
            "chapter_count": 2,
            "repair_guidance": {"summary": "disk guidance"},
        });
        let incoming_summary = json!({
            "chapter_count": 2,
            "repair_guidance": {"summary": "incoming guidance"},
        });

        let first_request = OutlineQualitySummarySnapshotRequest {
            project_id: "project-restore",
            chapter_limit: 2,
            chapter_keys: &chapter_keys,
            history_count: 5,
            history_latest_created_at: Some("2026-06-02T18:10:00"),
            summary: &persisted_summary,
        };
        resolve_outline_quality_summary_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &first_request,
            128,
        )
        .await
        .expect("first resolve should persist snapshot");

        let cache_key = build_outline_quality_summary_cache_key("project-restore", 2);
        {
            let mut guard = cache.lock().await;
            guard.entries.remove(&cache_key);
            guard.insertion_order.clear();
        }

        let second_request = OutlineQualitySummarySnapshotRequest {
            summary: &incoming_summary,
            ..first_request
        };
        let second_resolved = resolve_outline_quality_summary_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &second_request,
            128,
        )
        .await
        .expect("second resolve should restore snapshot from disk");
        assert_eq!(
            second_resolved["repair_guidance"]["summary"],
            json!("disk guidance")
        );

        let snapshot_path =
            outline_quality_summary_snapshot_path(&snapshot_root, "project-restore", 2);
        assert!(snapshot_path.exists());
    }

    #[test]
    fn should_build_outline_quality_guidance_blocks_from_summary() {
        let summary = json!({
            "chapter_count": 3,
            "overall_score_trend": "falling",
            "avg_payoff_chain_rate": 61.0,
            "avg_cliffhanger_rate": 58.0,
            "avg_pacing_score": 6.8,
            "recent_focus_areas": ["回收旧承诺", "章尾牵引"],
            "repair_guidance": {
                "summary": "最近几章的尾钩力度不足",
                "focus_areas": ["章尾牵引", "回报兑现"],
                "weakest_metric_label": "章尾牵引",
                "weakest_metric_value": 61.0
            }
        });

        let repair_guidance = build_outline_quality_repair_guidance_from_summary(&summary);
        let trend_guidance = build_outline_story_quality_trend_guidance_from_summary(&summary);

        assert!(repair_guidance.contains("【诊断优先级卡】"));
        assert!(repair_guidance.contains("当前最弱项：章尾牵引"));
        assert!(trend_guidance.contains("【大纲近期质量趋势】"));
        assert!(trend_guidance.contains("最近 3 章"));
        assert!(trend_guidance.contains("后续章节"));
        assert!(!trend_guidance.contains("本章"));
    }
}
