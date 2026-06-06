use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::ai::clients::openai::OpenAIClient;
use crate::models::story_memory;
use crate::services::settings_service::SettingsService;

const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-large";
const DEFAULT_EMBEDDING_DIMENSION: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVectorMemoryRecord {
    id: String,
    project_id: String,
    chapter_id: String,
    memory_type: String,
    content: String,
    metadata: Value,
    embedding: Vec<f32>,
    embedding_model: String,
    created_at: String,
}

fn vector_index_root() -> PathBuf {
    PathBuf::from("../backend/data/vector_memory")
}

fn vector_index_path(project_id: &str) -> PathBuf {
    vector_index_root().join(format!("{project_id}.json"))
}

async fn load_project_records(project_id: &str) -> Result<Vec<StoredVectorMemoryRecord>, String> {
    let path = vector_index_path(project_id);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|error| format!("read vector index failed: {}", error))?;
    serde_json::from_str::<Vec<StoredVectorMemoryRecord>>(&content)
        .map_err(|error| format!("decode vector index failed: {}", error))
}

async fn save_project_records(
    project_id: &str,
    records: &[StoredVectorMemoryRecord],
) -> Result<(), String> {
    let path = vector_index_path(project_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("create vector index dir failed: {}", error))?;
    }

    let content = serde_json::to_string_pretty(records)
        .map_err(|error| format!("encode vector index failed: {}", error))?;
    tokio::fs::write(path, content)
        .await
        .map_err(|error| format!("write vector index failed: {}", error))
}

fn fallback_embedding(content: &str) -> Vec<f32> {
    let mut values = vec![0.0_f32; DEFAULT_EMBEDDING_DIMENSION];
    for (index, byte) in content.as_bytes().iter().enumerate() {
        values[index % DEFAULT_EMBEDDING_DIMENSION] += (*byte as f32) / 255.0;
    }
    values
}

async fn request_openai_embedding(
    api_key: &str,
    base_url: &str,
    model: &str,
    content: &str,
) -> Result<Vec<f32>, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/embeddings", base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "model": model,
            "input": content,
        }))
        .send()
        .await
        .map_err(|error| format!("embedding request failed: {}", error))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("embedding response read failed: {}", error))?;
    if !status.is_success() {
        return Err(format!(
            "embedding request failed with status {}: {}",
            status, body
        ));
    }

    let payload: Value = serde_json::from_str(&body)
        .map_err(|error| format!("decode embedding failed: {}", error))?;
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("embedding"))
        .and_then(Value::as_array)
        .ok_or_else(|| "embedding payload missing data[0].embedding".to_string())?;

    let embedding = data
        .iter()
        .filter_map(Value::as_f64)
        .map(|value| value as f32)
        .collect::<Vec<_>>();
    if embedding.is_empty() {
        return Err("embedding payload was empty".to_string());
    }
    Ok(embedding)
}

async fn build_embedding_for_memory(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    content: &str,
) -> Result<(Vec<f32>, String), String> {
    let ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
        .await
        .map_err(|error| error.to_string())?;
    let embedding_model = DEFAULT_EMBEDDING_MODEL.to_string();

    if OpenAIClient::is_official_openai_base_url(&ai_config.base_url)
        || ai_config.base_url.contains("/v1")
    {
        match request_openai_embedding(
            &ai_config.api_key,
            &ai_config.base_url,
            &embedding_model,
            content,
        )
        .await
        {
            Ok(embedding) => return Ok((embedding, embedding_model)),
            Err(_) => {}
        }
    }

    Ok((
        fallback_embedding(content),
        "fallback-hash-embedding".to_string(),
    ))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f64 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }

    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        let left_value = *left_value as f64;
        let right_value = *right_value as f64;
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

fn record_matches_types(
    record: &StoredVectorMemoryRecord,
    chapter_id: Option<&str>,
    memory_types: &[String],
) -> bool {
    let chapter_matches = record.chapter_id == chapter_id.unwrap_or_default();
    let type_matches = memory_types.iter().any(|item| item == &record.memory_type);
    chapter_matches && type_matches
}

static PROJECT_INDEX_LOCK: Mutex<()> = Mutex::const_new(());

pub(crate) async fn upsert_story_memory_vector_record(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    memory: &story_memory::Model,
    content_for_embedding: &str,
    metadata: Value,
) -> Result<(), String> {
    let _guard = PROJECT_INDEX_LOCK.lock().await;
    let mut records = load_project_records(&memory.project_id).await?;
    records.retain(|item| item.id != memory.id);
    let (embedding, embedding_model) =
        build_embedding_for_memory(db, user_id, content_for_embedding).await?;
    records.push(StoredVectorMemoryRecord {
        id: memory.id.clone(),
        project_id: memory.project_id.clone(),
        chapter_id: memory.chapter_id.clone().unwrap_or_default(),
        memory_type: memory.memory_type.clone(),
        content: content_for_embedding.to_string(),
        metadata,
        embedding,
        embedding_model,
        created_at: chrono::Utc::now().to_rfc3339(),
    });
    save_project_records(&memory.project_id, &records).await
}

pub(crate) async fn delete_story_memory_vector_records_by_types(
    project_id: &str,
    chapter_id: Option<&str>,
    memory_types: &[String],
) -> Result<(), String> {
    let _guard = PROJECT_INDEX_LOCK.lock().await;
    let records = load_project_records(project_id).await?;
    let filtered = records
        .into_iter()
        .filter(|item| !record_matches_types(item, chapter_id, memory_types))
        .collect::<Vec<_>>();
    save_project_records(project_id, &filtered).await
}

pub(crate) async fn delete_story_memory_vector_records_by_chapter(
    project_id: &str,
    chapter_id: &str,
) -> Result<(), String> {
    let _guard = PROJECT_INDEX_LOCK.lock().await;
    let records = load_project_records(project_id).await?;
    let filtered = records
        .into_iter()
        .filter(|item| item.chapter_id != chapter_id)
        .collect::<Vec<_>>();
    save_project_records(project_id, &filtered).await
}

#[derive(Debug, Clone)]
pub(crate) struct VectorMemorySearchHit {
    pub(crate) memory_id: String,
    pub(crate) similarity: f64,
}

pub(crate) async fn search_story_memory_vector_records(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    project_id: &str,
    query: &str,
    memory_types: &[String],
    min_importance: f64,
    limit: usize,
) -> Result<Vec<VectorMemorySearchHit>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let _guard = PROJECT_INDEX_LOCK.lock().await;
    let records = load_project_records(project_id).await?;
    let (query_embedding, _embedding_model) =
        build_embedding_for_memory(db, user_id, query).await?;

    let mut hits = records
        .into_iter()
        .filter(|record| {
            memory_types.is_empty() || memory_types.iter().any(|item| item == &record.memory_type)
        })
        .filter(|record| {
            record
                .metadata
                .get("importance_score")
                .and_then(Value::as_f64)
                .unwrap_or(0.5)
                >= min_importance
        })
        .map(|record| VectorMemorySearchHit {
            memory_id: record.id,
            similarity: cosine_similarity(&query_embedding, &record.embedding),
        })
        .filter(|item| item.similarity > 0.0)
        .collect::<Vec<_>>();

    hits.sort_by(|left, right| {
        right
            .similarity
            .partial_cmp(&left.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::{
        cosine_similarity, fallback_embedding, record_matches_types, StoredVectorMemoryRecord,
    };
    use serde_json::json;

    #[test]
    fn should_build_deterministic_fallback_embedding() {
        let left = fallback_embedding("memory content");
        let right = fallback_embedding("memory content");
        assert_eq!(left, right);
        assert_eq!(left.len(), 256);
    }

    #[test]
    fn should_match_vector_record_by_chapter_and_type() {
        let record = StoredVectorMemoryRecord {
            id: "m-1".to_string(),
            project_id: "p-1".to_string(),
            chapter_id: "c-1".to_string(),
            memory_type: "research_reference".to_string(),
            content: "content".to_string(),
            metadata: json!({"title": "x"}),
            embedding: vec![0.1, 0.2],
            embedding_model: "fallback".to_string(),
            created_at: "2026-05-29T16:00:00+08:00".to_string(),
        };

        assert!(record_matches_types(
            &record,
            Some("c-1"),
            &["research_reference".to_string()]
        ));
        assert!(!record_matches_types(
            &record,
            Some("c-2"),
            &["research_reference".to_string()]
        ));
        assert!(!record_matches_types(
            &record,
            Some("c-1"),
            &["foreshadow".to_string()]
        ));
    }

    #[test]
    fn should_compute_cosine_similarity_for_same_and_different_vectors() {
        let same = cosine_similarity(&[1.0, 2.0], &[1.0, 2.0]);
        let different = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);

        assert!(same > 0.99);
        assert!(different < 0.01);
    }
}
