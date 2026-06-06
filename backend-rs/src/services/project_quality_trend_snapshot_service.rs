use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::services::chapter_story_repair_quality_context_service::{
    advance_quality_metrics_summary_state, build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state_from_history,
};

const PROJECT_QUALITY_TREND_CACHE_MAX_SIZE: usize = 128;
const PROJECT_QUALITY_TREND_SNAPSHOT_DIR: &str = "../backend/data/project_quality_trend_snapshots";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProjectQualityTrendSnapshot {
    item_keys: Vec<(String, String)>,
    items: Vec<Value>,
    metrics_history: Vec<Value>,
    #[serde(rename = "_summary_state")]
    summary_state: Option<Value>,
    summary: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedProjectQualityTrendSnapshot {
    pub items: Vec<Value>,
    pub summary: Option<Value>,
}

#[derive(Debug, Default)]
struct ProjectQualityTrendSnapshotCache {
    entries: HashMap<String, ProjectQualityTrendSnapshot>,
    insertion_order: VecDeque<String>,
}

impl ProjectQualityTrendSnapshotCache {
    fn get(&self, cache_key: &str) -> Option<ProjectQualityTrendSnapshot> {
        self.entries.get(cache_key).cloned()
    }

    fn insert(
        &mut self,
        cache_key: String,
        snapshot: ProjectQualityTrendSnapshot,
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

struct ProjectQualityTrendSnapshotRequest<'a> {
    project_id: &'a str,
    limit: usize,
    items: &'a [Value],
    metrics_history: &'a [Value],
    total_chapters: i64,
    analyzed_chapters: i64,
    last_generated_at: Option<&'a str>,
}

fn project_quality_trend_snapshot_cache() -> &'static Mutex<ProjectQualityTrendSnapshotCache> {
    static CACHE: OnceLock<Mutex<ProjectQualityTrendSnapshotCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ProjectQualityTrendSnapshotCache::default()))
}

fn project_quality_trend_snapshot_root() -> PathBuf {
    PathBuf::from(PROJECT_QUALITY_TREND_SNAPSHOT_DIR)
}

fn build_project_quality_trend_cache_key(project_id: &str, limit: usize) -> String {
    format!("{project_id}:{limit}")
}

fn normalize_snapshot_file_stem(project_id: &str, limit: usize) -> String {
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
    format!("{normalized_project_id}__{limit}")
}

fn project_quality_trend_snapshot_path(
    snapshot_root: &Path,
    project_id: &str,
    limit: usize,
) -> PathBuf {
    snapshot_root.join(format!(
        "{}.json",
        normalize_snapshot_file_stem(project_id, limit)
    ))
}

fn build_project_quality_trend_item_keys(items: &[Value]) -> Vec<(String, String)> {
    items
        .iter()
        .filter_map(|item| {
            let chapter_id = item.get("chapter_id").and_then(Value::as_str)?;
            Some((
                chapter_id.to_string(),
                item.get("history_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect()
}

fn decorate_project_quality_metrics_summary(
    summary: Option<Value>,
    total_chapters: i64,
    analyzed_chapters: i64,
    last_generated_at: Option<&str>,
) -> Option<Value> {
    let mut summary = summary?;
    let summary_object = summary.as_object_mut()?;
    summary_object.insert("total_chapters".to_string(), json!(total_chapters));
    summary_object.insert("analyzed_chapters".to_string(), json!(analyzed_chapters));
    summary_object.insert(
        "last_generated_at".to_string(),
        last_generated_at.map(Value::from).unwrap_or(Value::Null),
    );
    Some(summary)
}

fn build_project_quality_trend_snapshot(
    request: &ProjectQualityTrendSnapshotRequest<'_>,
    summary_state: Option<Value>,
) -> ProjectQualityTrendSnapshot {
    let resolved_summary_state = summary_state.or_else(|| {
        build_quality_metrics_summary_state_from_history(request.metrics_history, "batch")
    });
    let summary = build_quality_metrics_summary_from_state(
        resolved_summary_state.as_ref(),
        request.metrics_history,
        "batch",
    );

    ProjectQualityTrendSnapshot {
        item_keys: build_project_quality_trend_item_keys(request.items),
        items: request.items.to_vec(),
        metrics_history: request.metrics_history.to_vec(),
        summary_state: resolved_summary_state,
        summary: decorate_project_quality_metrics_summary(
            summary,
            request.total_chapters,
            request.analyzed_chapters,
            request.last_generated_at,
        ),
    }
}

fn try_advance_project_quality_trend_snapshot(
    cached_snapshot: Option<&ProjectQualityTrendSnapshot>,
    request: &ProjectQualityTrendSnapshotRequest<'_>,
) -> Option<ProjectQualityTrendSnapshot> {
    let cached_snapshot = cached_snapshot?;
    let current_item_keys = build_project_quality_trend_item_keys(request.items);
    if current_item_keys == cached_snapshot.item_keys {
        return Some(build_project_quality_trend_snapshot(
            request,
            cached_snapshot.summary_state.clone(),
        ));
    }

    if current_item_keys.is_empty()
        || cached_snapshot.item_keys.is_empty()
        || cached_snapshot
            .summary_state
            .as_ref()
            .and_then(Value::as_object)
            .is_none()
        || cached_snapshot.item_keys.len() != cached_snapshot.metrics_history.len()
    {
        return None;
    }

    let max_overlap = cached_snapshot.item_keys.len().min(current_item_keys.len());
    let mut overlap = 0;
    for size in (1..=max_overlap).rev() {
        if cached_snapshot.item_keys[cached_snapshot.item_keys.len() - size..]
            == current_item_keys[..size]
        {
            overlap = size;
            break;
        }
    }
    if overlap == 0 {
        return None;
    }

    let dropped_count = cached_snapshot.item_keys.len() - overlap;
    let appended_count = current_item_keys.len() - overlap;
    if appended_count == 0 || dropped_count > appended_count {
        return None;
    }

    let mut working_history = cached_snapshot.metrics_history.clone();
    let mut working_state = cached_snapshot.summary_state.clone();
    let mut append_index = overlap;

    for _ in 0..dropped_count {
        if append_index >= request.metrics_history.len() || working_history.is_empty() {
            return None;
        }

        let dropped_event = working_history.first().cloned()?;
        let appended_event = request.metrics_history[append_index].clone();
        let mut next_history = working_history[1..].to_vec();
        next_history.push(appended_event.clone());

        let next_state = advance_quality_metrics_summary_state(
            working_state.as_ref(),
            &appended_event,
            &next_history,
            Some(&dropped_event),
            "batch",
        )?;
        working_state = Some(next_state);
        working_history = next_history;
        append_index += 1;
    }

    if append_index != request.metrics_history.len() {
        return None;
    }

    Some(build_project_quality_trend_snapshot(request, working_state))
}

async fn load_project_quality_trend_snapshot(
    snapshot_root: &Path,
    project_id: &str,
    limit: usize,
) -> Result<Option<ProjectQualityTrendSnapshot>, String> {
    let snapshot_path = project_quality_trend_snapshot_path(snapshot_root, project_id, limit);
    let content = match tokio::fs::read_to_string(&snapshot_path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read project quality trend snapshot failed: {error}"
            ))
        }
    };

    match serde_json::from_str::<ProjectQualityTrendSnapshot>(&content) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(_) => Ok(None),
    }
}

async fn persist_project_quality_trend_snapshot(
    snapshot_root: &Path,
    project_id: &str,
    limit: usize,
    snapshot: &ProjectQualityTrendSnapshot,
) -> Result<(), String> {
    let snapshot_path = project_quality_trend_snapshot_path(snapshot_root, project_id, limit);
    if let Some(parent) = snapshot_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!("create project quality trend snapshot dir failed: {error}")
        })?;
    }

    let serialized = serde_json::to_string(snapshot)
        .map_err(|error| format!("encode project quality trend snapshot failed: {error}"))?;
    tokio::fs::write(&snapshot_path, serialized)
        .await
        .map_err(|error| format!("write project quality trend snapshot failed: {error}"))
}

async fn resolve_project_quality_trend_snapshot_with_dependencies(
    cache: &Mutex<ProjectQualityTrendSnapshotCache>,
    snapshot_root: &Path,
    request: &ProjectQualityTrendSnapshotRequest<'_>,
    max_cache_size: usize,
) -> Result<ResolvedProjectQualityTrendSnapshot, String> {
    let cache_key = build_project_quality_trend_cache_key(request.project_id, request.limit);
    let mut cached_snapshot = {
        let guard = cache.lock().await;
        guard.get(&cache_key)
    };
    if cached_snapshot.is_none() {
        if let Some(persisted_snapshot) =
            load_project_quality_trend_snapshot(snapshot_root, request.project_id, request.limit)
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

    let snapshot = try_advance_project_quality_trend_snapshot(cached_snapshot.as_ref(), request)
        .unwrap_or_else(|| build_project_quality_trend_snapshot(request, None));
    {
        let mut guard = cache.lock().await;
        guard.insert(cache_key, snapshot.clone(), max_cache_size);
    }
    let resolved_snapshot = ResolvedProjectQualityTrendSnapshot {
        items: snapshot.items.clone(),
        summary: snapshot.summary.clone(),
    };

    persist_project_quality_trend_snapshot(
        snapshot_root,
        request.project_id,
        request.limit,
        &snapshot,
    )
    .await?;

    Ok(resolved_snapshot)
}

pub(crate) async fn resolve_project_quality_trend_snapshot(
    project_id: &str,
    limit: usize,
    items: &[Value],
    metrics_history: &[Value],
    total_chapters: i64,
    analyzed_chapters: i64,
    last_generated_at: Option<&str>,
) -> Result<ResolvedProjectQualityTrendSnapshot, String> {
    let request = ProjectQualityTrendSnapshotRequest {
        project_id,
        limit,
        items,
        metrics_history,
        total_chapters,
        analyzed_chapters,
        last_generated_at,
    };

    resolve_project_quality_trend_snapshot_with_dependencies(
        project_quality_trend_snapshot_cache(),
        &project_quality_trend_snapshot_root(),
        &request,
        PROJECT_QUALITY_TREND_CACHE_MAX_SIZE,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        build_project_quality_trend_cache_key, build_project_quality_trend_snapshot,
        project_quality_trend_snapshot_path,
        resolve_project_quality_trend_snapshot_with_dependencies,
        try_advance_project_quality_trend_snapshot, ProjectQualityTrendSnapshot,
        ProjectQualityTrendSnapshotCache, ProjectQualityTrendSnapshotRequest,
    };
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    fn temp_snapshot_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "mumu_project_quality_trend_snapshot_test_{}",
            Uuid::new_v4()
        ))
    }

    fn quality_item(chapter_id: &str, history_id: &str, chapter_number: i32, score: f64) -> Value {
        json!({
            "chapter_id": chapter_id,
            "chapter_number": chapter_number,
            "title": format!("第{}章", chapter_number),
            "status": "completed",
            "history_id": history_id,
            "generated_at": format!("2026-06-02T12:0{}:00", chapter_number),
            "latest_quality_metrics": quality_metrics(score),
        })
    }

    fn quality_metrics(score: f64) -> Value {
        json!({
            "overall_score": score,
            "conflict_chain_hit_rate": score - 10.0,
            "rule_grounding_hit_rate": score + 2.0,
            "outline_alignment_rate": score - 8.0,
            "dialogue_naturalness_rate": score - 1.0,
            "opening_hook_rate": score - 6.0,
            "payoff_chain_rate": score - 14.0,
            "cliffhanger_rate": score + 4.0,
            "pacing_score": (score / 10.0),
            "quality_runtime_context": {
                "plot_stage": "development",
                "chapter_count": 3,
                "current_chapter_number": 2,
            }
        })
    }

    fn request<'a>(
        project_id: &'a str,
        limit: usize,
        items: &'a [Value],
        metrics_history: &'a [Value],
    ) -> ProjectQualityTrendSnapshotRequest<'a> {
        ProjectQualityTrendSnapshotRequest {
            project_id,
            limit,
            items,
            metrics_history,
            total_chapters: 3,
            analyzed_chapters: metrics_history.len() as i64,
            last_generated_at: Some("2026-06-02T12:03:00"),
        }
    }

    #[test]
    fn should_reuse_cached_summary_state_for_identical_project_quality_trend_window() {
        let items = vec![
            quality_item("chapter-1", "history-1", 1, 78.0),
            quality_item("chapter-2", "history-2", 2, 82.0),
        ];
        let metrics_history = vec![quality_metrics(78.0), quality_metrics(82.0)];
        let request = request("project-1", 2, &items, &metrics_history);
        let cached_snapshot = ProjectQualityTrendSnapshot {
            summary_state: Some(json!({
                "scope": "batch",
                "chapter_count": 2,
                "first_overall_score": 78.0,
                "last_overall_score": 82.0,
                "recent_history": metrics_history,
                "pacing_score_total": 16.0,
                "pacing_score_count": 2,
                "overall_score_total": 160.0,
                "conflict_chain_hit_rate_total": 140.0,
                "rule_grounding_hit_rate_total": 164.0,
                "outline_alignment_rate_total": 144.0,
                "dialogue_naturalness_rate_total": 158.0,
                "opening_hook_rate_total": 148.0,
                "payoff_chain_rate_total": 132.0,
                "cliffhanger_rate_total": 168.0,
                "cached_marker": "reuse"
            })),
            ..build_project_quality_trend_snapshot(&request, None)
        };

        let snapshot = try_advance_project_quality_trend_snapshot(Some(&cached_snapshot), &request)
            .expect("identical window should reuse cached summary state");

        assert_eq!(snapshot.item_keys, cached_snapshot.item_keys);
        assert_eq!(
            snapshot
                .summary_state
                .as_ref()
                .and_then(|value| value.get("cached_marker")),
            Some(&json!("reuse"))
        );
        assert_eq!(
            snapshot
                .summary
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            snapshot
                .summary
                .as_ref()
                .and_then(|value| value.get("total_chapters")),
            Some(&json!(3))
        );
    }

    #[test]
    fn should_incrementally_slide_project_quality_trend_snapshot_when_window_moves() {
        let cached_items = vec![
            quality_item("chapter-1", "history-1", 1, 76.0),
            quality_item("chapter-2", "history-2", 2, 80.0),
        ];
        let cached_history = vec![quality_metrics(76.0), quality_metrics(80.0)];
        let cached_request = request("project-1", 2, &cached_items, &cached_history);
        let cached_snapshot = ProjectQualityTrendSnapshot {
            summary_state: Some(json!({
                "scope": "batch",
                "chapter_count": 2,
                "first_overall_score": 76.0,
                "last_overall_score": 80.0,
                "recent_history": cached_history,
                "pacing_score_total": 15.6,
                "pacing_score_count": 2,
                "overall_score_total": 156.0,
                "conflict_chain_hit_rate_total": 136.0,
                "rule_grounding_hit_rate_total": 160.0,
                "outline_alignment_rate_total": 140.0,
                "dialogue_naturalness_rate_total": 154.0,
                "opening_hook_rate_total": 144.0,
                "payoff_chain_rate_total": 128.0,
                "cliffhanger_rate_total": 164.0,
                "cached_marker": "advanced"
            })),
            ..build_project_quality_trend_snapshot(&cached_request, None)
        };

        let next_items = vec![
            quality_item("chapter-2", "history-2", 2, 80.0),
            quality_item("chapter-3", "history-3", 3, 84.0),
        ];
        let next_history = vec![quality_metrics(80.0), quality_metrics(84.0)];
        let next_request = request("project-1", 2, &next_items, &next_history);

        let snapshot =
            try_advance_project_quality_trend_snapshot(Some(&cached_snapshot), &next_request)
                .expect("sliding window should advance cached summary state");

        assert_eq!(
            snapshot.item_keys[0],
            ("chapter-2".to_string(), "history-2".to_string())
        );
        assert_eq!(
            snapshot.item_keys[1],
            ("chapter-3".to_string(), "history-3".to_string())
        );
        assert_eq!(
            snapshot
                .summary_state
                .as_ref()
                .and_then(|value| value.get("cached_marker")),
            Some(&json!("advanced"))
        );
        assert_eq!(
            snapshot
                .summary_state
                .as_ref()
                .and_then(|value| value.get("first_overall_score")),
            Some(&json!(80.0))
        );
        assert_eq!(
            snapshot
                .summary_state
                .as_ref()
                .and_then(|value| value.get("last_overall_score")),
            Some(&json!(84.0))
        );
        assert_eq!(
            snapshot
                .summary
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(2))
        );
    }

    #[tokio::test]
    async fn should_restore_project_quality_trend_snapshot_from_persisted_store_after_cache_clear()
    {
        let snapshot_root = temp_snapshot_root();
        let cache = Mutex::new(ProjectQualityTrendSnapshotCache::default());
        let items = vec![
            quality_item("chapter-1", "history-1", 1, 78.0),
            quality_item("chapter-2", "history-2", 2, 84.0),
        ];
        let metrics_history = vec![quality_metrics(78.0), quality_metrics(84.0)];
        let request = request("project-restore", 2, &items, &metrics_history);

        let first_resolved = resolve_project_quality_trend_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &request,
            128,
        )
        .await
        .expect("first resolve should persist snapshot");
        assert_eq!(
            first_resolved
                .summary
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(2))
        );

        let snapshot_path =
            project_quality_trend_snapshot_path(&snapshot_root, "project-restore", 2);
        let mut persisted_snapshot: ProjectQualityTrendSnapshot = serde_json::from_str(
            &tokio::fs::read_to_string(&snapshot_path)
                .await
                .expect("persisted snapshot should exist"),
        )
        .expect("persisted snapshot should decode");
        persisted_snapshot.summary_state = persisted_snapshot.summary_state.map(|mut value| {
            if let Some(object) = value.as_object_mut() {
                object.insert("restored_marker".to_string(), json!("disk"));
            }
            value
        });
        tokio::fs::write(
            &snapshot_path,
            serde_json::to_string(&persisted_snapshot).expect("snapshot should encode"),
        )
        .await
        .expect("persisted snapshot rewrite should succeed");

        let cache_key = build_project_quality_trend_cache_key("project-restore", 2);
        {
            let mut guard = cache.lock().await;
            guard.entries.remove(&cache_key);
            guard.insertion_order.clear();
        }

        let second_resolved = resolve_project_quality_trend_snapshot_with_dependencies(
            &cache,
            &snapshot_root,
            &request,
            128,
        )
        .await
        .expect("second resolve should restore persisted snapshot");

        assert_eq!(
            second_resolved
                .summary
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(2))
        );
        let guard = cache.lock().await;
        let restored_snapshot = guard
            .entries
            .get(&cache_key)
            .expect("restored snapshot should be cached");
        assert_eq!(
            restored_snapshot
                .summary_state
                .as_ref()
                .and_then(|value| value.get("restored_marker")),
            Some(&json!("disk"))
        );
    }
}
