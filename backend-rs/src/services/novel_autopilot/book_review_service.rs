use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    models::{chapter, plot_analysis},
    services::{
        book_completion_consistency_service::{
            load_book_completion_consistency, BookCompletionConsistencyError,
            BookCompletionConsistencyReport,
        },
        chapter_content_digest_service::chapter_content_digest,
    },
};

const BOOK_REVIEW_SCHEMA_VERSION: u32 = 1;
const BOOK_REVIEW_QUALITY_TARGET: f64 = 8.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BookReviewServiceError {
    Consistency(BookCompletionConsistencyError),
    Database,
    Serialization,
}

impl BookReviewServiceError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Consistency(error) => error.code(),
            Self::Database => "database_error",
            Self::Serialization => "book_review_serialization_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BookReviewRewriteReference {
    pub chapter_id: String,
    pub chapter_number: i32,
    pub analysis_id: String,
    pub source_content_digest: String,
    pub reason_code: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookReviewSummary {
    pub ready: bool,
    pub consistency: BookCompletionConsistencyReport,
    pub expected_analysis_count: u32,
    pub analyzed_chapter_count: u32,
    pub below_target_chapter_count: u32,
    pub suggestion_chapter_count: u32,
    pub pending_rewrites: Vec<BookReviewRewriteReference>,
    pub result_digest: String,
}

#[derive(Debug, Serialize)]
struct BookReviewDigestInput<'a> {
    schema_version: u32,
    consistency_result_digest: &'a str,
    chapters: &'a [BookReviewChapterDigestFact],
}

#[derive(Debug, Serialize)]
struct BookReviewChapterDigestFact {
    chapter_id: String,
    chapter_number: i32,
    sub_index: i32,
    content_digest: String,
    analysis_id: Option<String>,
    analysis_source_content_digest: Option<String>,
    overall_quality_milli: Option<i64>,
    pacing_milli: Option<i64>,
    engagement_milli: Option<i64>,
    coherence_milli: Option<i64>,
    has_suggestions: bool,
}

pub(crate) async fn load_book_review_summary(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    expected_chapter_count: u32,
) -> Result<BookReviewSummary, BookReviewServiceError> {
    let consistency =
        load_book_completion_consistency(db, project_id, user_id, expected_chapter_count)
            .await
            .map_err(BookReviewServiceError::Consistency)?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .order_by_asc(chapter::Column::SubIndex)
        .all(db)
        .await
        .map_err(|_| BookReviewServiceError::Database)?;
    let analyses = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(project_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|_| BookReviewServiceError::Database)?;

    evaluate_book_review(consistency, &chapters, &analyses)
}

fn evaluate_book_review(
    consistency: BookCompletionConsistencyReport,
    chapters: &[chapter::Model],
    analyses: &[plot_analysis::Model],
) -> Result<BookReviewSummary, BookReviewServiceError> {
    let mut chapter_facts = Vec::new();
    let mut pending_rewrites = Vec::new();
    let mut analyzed_chapter_count = 0_u32;
    let mut below_target_chapter_count = 0_u32;
    let mut suggestion_chapter_count = 0_u32;

    for chapter in chapters {
        let Some(content) = chapter.content.as_deref() else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        let content_digest = chapter_content_digest(content);
        let analysis = analyses.iter().find(|analysis| {
            analysis.chapter_id == chapter.id
                && analysis.source_content_digest.as_deref() == Some(content_digest.as_str())
        });
        let has_suggestions = analysis
            .and_then(|analysis| analysis.suggestions.as_ref())
            .is_some_and(has_non_empty_json);
        let below_target = analysis
            .and_then(|analysis| analysis.overall_quality_score)
            .is_none_or(|score| !score.is_finite() || score < BOOK_REVIEW_QUALITY_TARGET);

        if let Some(analysis) = analysis {
            analyzed_chapter_count = analyzed_chapter_count.saturating_add(1);
            if below_target {
                below_target_chapter_count = below_target_chapter_count.saturating_add(1);
            }
            if has_suggestions {
                suggestion_chapter_count = suggestion_chapter_count.saturating_add(1);
            }
            if below_target || has_suggestions {
                pending_rewrites.push(BookReviewRewriteReference {
                    chapter_id: chapter.id.clone(),
                    chapter_number: chapter.chapter_number,
                    analysis_id: analysis.id.clone(),
                    source_content_digest: content_digest.clone(),
                    reason_code: if below_target {
                        "book_review_quality_below_target".to_string()
                    } else {
                        "book_review_suggestions".to_string()
                    },
                    attempt: 1,
                });
            }
        }

        chapter_facts.push(BookReviewChapterDigestFact {
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            sub_index: chapter.sub_index,
            content_digest,
            analysis_id: analysis.map(|analysis| analysis.id.clone()),
            analysis_source_content_digest: analysis
                .and_then(|analysis| analysis.source_content_digest.clone()),
            overall_quality_milli: analysis
                .and_then(|analysis| normalize_score(analysis.overall_quality_score)),
            pacing_milli: analysis.and_then(|analysis| normalize_score(analysis.pacing_score)),
            engagement_milli: analysis
                .and_then(|analysis| normalize_score(analysis.engagement_score)),
            coherence_milli: analysis
                .and_then(|analysis| normalize_score(analysis.coherence_score)),
            has_suggestions,
        });
    }

    let expected_analysis_count = u32::try_from(chapter_facts.len()).unwrap_or(u32::MAX);
    let result_digest = digest_review(&consistency.result_digest, &chapter_facts)?;
    Ok(BookReviewSummary {
        ready: consistency.ready && analyzed_chapter_count == expected_analysis_count,
        consistency,
        expected_analysis_count,
        analyzed_chapter_count,
        below_target_chapter_count,
        suggestion_chapter_count,
        pending_rewrites,
        result_digest,
    })
}

fn normalize_score(score: Option<f64>) -> Option<i64> {
    score
        .filter(|score| score.is_finite())
        .map(|score| (score * 1_000.0).round() as i64)
}

fn has_non_empty_json(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(_) => true,
        serde_json::Value::String(value) => !value.trim().is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
    }
}

fn digest_review(
    consistency_result_digest: &str,
    chapters: &[BookReviewChapterDigestFact],
) -> Result<String, BookReviewServiceError> {
    let bytes = serde_json::to_vec(&BookReviewDigestInput {
        schema_version: BOOK_REVIEW_SCHEMA_VERSION,
        consistency_result_digest,
        chapters,
    })
    .map_err(|_| BookReviewServiceError::Serialization)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
