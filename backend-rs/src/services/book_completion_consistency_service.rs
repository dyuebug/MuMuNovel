use std::collections::{BTreeMap, BTreeSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::{chapter, outline, project};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BookCompletionConsistencyError {
    NotFoundOrAccessDenied,
    Database,
    InvalidExpectedChapterCount,
    Serialization,
}

impl BookCompletionConsistencyError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::NotFoundOrAccessDenied => "not_found_or_access_denied",
            Self::Database => "database_error",
            Self::InvalidExpectedChapterCount => "invalid_expected_chapter_count",
            Self::Serialization => "book_completion_report_serialization_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookCompletionChapterFact {
    pub chapter_id: String,
    pub chapter_number: i32,
    pub sub_index: i32,
    pub has_content: bool,
    pub outline_id: Option<String>,
}

impl From<chapter::Model> for BookCompletionChapterFact {
    fn from(chapter: chapter::Model) -> Self {
        Self {
            chapter_id: chapter.id,
            chapter_number: chapter.chapter_number,
            sub_index: chapter.sub_index,
            has_content: chapter
                .content
                .as_deref()
                .is_some_and(|content| !content.trim().is_empty()),
            outline_id: chapter.outline_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BookCompletionChapterPosition {
    pub chapter_number: i32,
    pub sub_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BookCompletionConsistencyReport {
    pub ready: bool,
    pub expected_chapter_count: u32,
    pub distinct_chapter_number_count: u32,
    pub chapter_record_count: u32,
    pub completed_chapter_record_count: u32,
    pub missing_chapter_numbers: Vec<u32>,
    pub unexpected_chapter_numbers: Vec<i32>,
    pub invalid_chapter_positions: Vec<BookCompletionChapterPosition>,
    pub duplicate_chapter_positions: Vec<BookCompletionChapterPosition>,
    pub blank_content_chapter_ids: Vec<String>,
    pub orphan_outline_chapter_ids: Vec<String>,
    pub result_digest: String,
}

#[derive(Serialize)]
struct BookCompletionConsistencyDigestInput<'a> {
    schema_version: u32,
    expected_chapter_count: u32,
    distinct_chapter_number_count: u32,
    chapter_record_count: u32,
    completed_chapter_record_count: u32,
    missing_chapter_numbers: &'a [u32],
    unexpected_chapter_numbers: &'a [i32],
    invalid_chapter_positions: &'a [BookCompletionChapterPosition],
    duplicate_chapter_positions: &'a [BookCompletionChapterPosition],
    blank_content_chapter_ids: &'a [String],
    orphan_outline_chapter_ids: &'a [String],
}

pub(crate) async fn load_book_completion_consistency(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    expected_chapter_count: u32,
) -> Result<BookCompletionConsistencyReport, BookCompletionConsistencyError> {
    if expected_chapter_count == 0 || expected_chapter_count > i32::MAX as u32 {
        return Err(BookCompletionConsistencyError::InvalidExpectedChapterCount);
    }

    project::Entity::find_by_id(project_id)
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|_| BookCompletionConsistencyError::Database)?
        .ok_or(BookCompletionConsistencyError::NotFoundOrAccessDenied)?;

    let outline_ids = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|_| BookCompletionConsistencyError::Database)?
        .into_iter()
        .map(|outline| outline.id)
        .collect::<BTreeSet<_>>();
    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .order_by_asc(chapter::Column::SubIndex)
        .all(db)
        .await
        .map_err(|_| BookCompletionConsistencyError::Database)?
        .into_iter()
        .map(BookCompletionChapterFact::from)
        .collect::<Vec<_>>();

    evaluate_book_completion_consistency(expected_chapter_count, &chapters, &outline_ids)
}

pub(crate) fn evaluate_book_completion_consistency(
    expected_chapter_count: u32,
    chapters: &[BookCompletionChapterFact],
    outline_ids: &BTreeSet<String>,
) -> Result<BookCompletionConsistencyReport, BookCompletionConsistencyError> {
    if expected_chapter_count == 0 || expected_chapter_count > i32::MAX as u32 {
        return Err(BookCompletionConsistencyError::InvalidExpectedChapterCount);
    }

    let expected_max = expected_chapter_count as i32;
    let mut distinct_chapter_numbers = BTreeSet::new();
    let mut position_counts = BTreeMap::<(i32, i32), u32>::new();
    let mut unexpected_chapter_numbers = BTreeSet::new();
    let mut invalid_chapter_positions = BTreeSet::new();
    let mut blank_content_chapter_ids = Vec::new();
    let mut orphan_outline_chapter_ids = Vec::new();
    let mut completed_chapter_record_count = 0u32;

    for chapter in chapters {
        if chapter.chapter_number <= 0 || chapter.sub_index < 0 {
            invalid_chapter_positions.insert((chapter.chapter_number, chapter.sub_index));
        } else {
            distinct_chapter_numbers.insert(chapter.chapter_number);
            if chapter.chapter_number > expected_max {
                unexpected_chapter_numbers.insert(chapter.chapter_number);
            }
        }
        *position_counts
            .entry((chapter.chapter_number, chapter.sub_index))
            .or_default() += 1;

        if chapter.has_content {
            completed_chapter_record_count = completed_chapter_record_count.saturating_add(1);
        } else {
            blank_content_chapter_ids.push(chapter.chapter_id.clone());
        }
        if chapter
            .outline_id
            .as_ref()
            .is_some_and(|outline_id| !outline_ids.contains(outline_id))
        {
            orphan_outline_chapter_ids.push(chapter.chapter_id.clone());
        }
    }

    let missing_chapter_numbers = (1..=expected_chapter_count)
        .filter(|chapter_number| !distinct_chapter_numbers.contains(&(*chapter_number as i32)))
        .collect::<Vec<_>>();
    let duplicate_chapter_positions = position_counts
        .into_iter()
        .filter_map(|((chapter_number, sub_index), count)| {
            (count > 1).then_some(BookCompletionChapterPosition {
                chapter_number,
                sub_index,
            })
        })
        .collect::<Vec<_>>();
    let invalid_chapter_positions = invalid_chapter_positions
        .into_iter()
        .map(
            |(chapter_number, sub_index)| BookCompletionChapterPosition {
                chapter_number,
                sub_index,
            },
        )
        .collect::<Vec<_>>();
    let unexpected_chapter_numbers = unexpected_chapter_numbers.into_iter().collect::<Vec<_>>();
    blank_content_chapter_ids.sort();
    orphan_outline_chapter_ids.sort();

    let distinct_chapter_number_count =
        u32::try_from(distinct_chapter_numbers.len()).unwrap_or(u32::MAX);
    let chapter_record_count = u32::try_from(chapters.len()).unwrap_or(u32::MAX);
    let ready = missing_chapter_numbers.is_empty()
        && unexpected_chapter_numbers.is_empty()
        && invalid_chapter_positions.is_empty()
        && duplicate_chapter_positions.is_empty()
        && blank_content_chapter_ids.is_empty()
        && orphan_outline_chapter_ids.is_empty();
    let serialized = serde_json::to_vec(&BookCompletionConsistencyDigestInput {
        schema_version: 1,
        expected_chapter_count,
        distinct_chapter_number_count,
        chapter_record_count,
        completed_chapter_record_count,
        missing_chapter_numbers: &missing_chapter_numbers,
        unexpected_chapter_numbers: &unexpected_chapter_numbers,
        invalid_chapter_positions: &invalid_chapter_positions,
        duplicate_chapter_positions: &duplicate_chapter_positions,
        blank_content_chapter_ids: &blank_content_chapter_ids,
        orphan_outline_chapter_ids: &orphan_outline_chapter_ids,
    })
    .map_err(|_| BookCompletionConsistencyError::Serialization)?;

    Ok(BookCompletionConsistencyReport {
        ready,
        expected_chapter_count,
        distinct_chapter_number_count,
        chapter_record_count,
        completed_chapter_record_count,
        missing_chapter_numbers,
        unexpected_chapter_numbers,
        invalid_chapter_positions,
        duplicate_chapter_positions,
        blank_content_chapter_ids,
        orphan_outline_chapter_ids,
        result_digest: format!("sha256:{}", hex::encode(Sha256::digest(serialized))),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        evaluate_book_completion_consistency, BookCompletionChapterFact,
        BookCompletionConsistencyError,
    };

    fn chapter(
        id: &str,
        chapter_number: i32,
        sub_index: i32,
        has_content: bool,
        outline_id: Option<&str>,
    ) -> BookCompletionChapterFact {
        BookCompletionChapterFact {
            chapter_id: id.to_string(),
            chapter_number,
            sub_index,
            has_content,
            outline_id: outline_id.map(str::to_string),
        }
    }

    #[test]
    fn complete_book_allows_distinct_subchapters() {
        let outlines = BTreeSet::from(["outline-1".to_string()]);
        let report = evaluate_book_completion_consistency(
            2,
            &[
                chapter("chapter-1", 1, 0, true, Some("outline-1")),
                chapter("chapter-1-1", 1, 1, true, Some("outline-1")),
                chapter("chapter-2", 2, 0, true, None),
            ],
            &outlines,
        )
        .expect("evaluate complete book");

        assert!(report.ready);
        assert_eq!(report.distinct_chapter_number_count, 2);
        assert_eq!(report.chapter_record_count, 3);
        assert_eq!(report.completed_chapter_record_count, 3);
        assert!(report.result_digest.starts_with("sha256:"));
    }

    #[test]
    fn incomplete_book_reports_structural_and_content_issues() {
        let report = evaluate_book_completion_consistency(
            3,
            &[
                chapter("chapter-1", 1, 0, true, Some("missing-outline")),
                chapter("chapter-1-duplicate", 1, 0, true, None),
                chapter("chapter-3", 3, 0, false, None),
                chapter("chapter-4", 4, 0, true, None),
                chapter("chapter-invalid", 0, -1, true, None),
            ],
            &BTreeSet::new(),
        )
        .expect("evaluate incomplete book");

        assert!(!report.ready);
        assert_eq!(report.missing_chapter_numbers, vec![2]);
        assert_eq!(report.unexpected_chapter_numbers, vec![4]);
        assert_eq!(report.duplicate_chapter_positions.len(), 1);
        assert_eq!(report.invalid_chapter_positions.len(), 1);
        assert_eq!(report.blank_content_chapter_ids, vec!["chapter-3"]);
        assert_eq!(report.orphan_outline_chapter_ids, vec!["chapter-1"]);
    }

    #[test]
    fn report_digest_is_stable_for_equivalent_input_order() {
        let first = evaluate_book_completion_consistency(
            2,
            &[
                chapter("chapter-2", 2, 0, true, None),
                chapter("chapter-1", 1, 0, false, None),
            ],
            &BTreeSet::new(),
        )
        .expect("first report");
        let second = evaluate_book_completion_consistency(
            2,
            &[
                chapter("chapter-1", 1, 0, false, None),
                chapter("chapter-2", 2, 0, true, None),
            ],
            &BTreeSet::new(),
        )
        .expect("second report");

        assert_eq!(first, second);
    }

    #[test]
    fn rejects_zero_expected_chapter_count() {
        assert_eq!(
            evaluate_book_completion_consistency(0, &[], &BTreeSet::new()).unwrap_err(),
            BookCompletionConsistencyError::InvalidExpectedChapterCount
        );
    }
}
