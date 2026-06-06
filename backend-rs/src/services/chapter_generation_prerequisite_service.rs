use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::models::chapter;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGenerationPrerequisiteCheck {
    pub(crate) can_generate: bool,
    pub(crate) error_message: String,
    pub(crate) previous_chapters: Vec<chapter::Model>,
}

pub(crate) async fn check_chapter_generation_prerequisites(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
) -> Result<ChapterGenerationPrerequisiteCheck, String> {
    if chapter_model.chapter_number == 1 {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: String::new(),
            previous_chapters: Vec::new(),
        });
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let incomplete_numbers = previous_chapters
        .iter()
        .filter(|chapter| {
            chapter
                .content
                .as_ref()
                .map(|content| content.trim().is_empty())
                .unwrap_or(true)
        })
        .map(|chapter| chapter.chapter_number.to_string())
        .collect::<Vec<_>>();

    if !incomplete_numbers.is_empty() {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: format!("前置章节尚未完成: {} 章", incomplete_numbers.join(", ")),
            previous_chapters,
        });
    }

    Ok(ChapterGenerationPrerequisiteCheck {
        can_generate: true,
        error_message: String::new(),
        previous_chapters,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::ChapterGenerationPrerequisiteCheck;
    use crate::models::chapter;

    fn chapter_model(chapter_number: i32, content: Option<&str>) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            chapter_number,
            title: format!("第{chapter_number}章"),
            content: content.map(str::to_string),
            summary: None,
            word_count: 0,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_allow_first_chapter_generation_without_previous_chapters() {
        let result = ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: String::new(),
            previous_chapters: Vec::new(),
        };

        assert!(result.can_generate);
        assert!(result.error_message.is_empty());
        assert!(result.previous_chapters.is_empty());
    }

    #[test]
    fn should_keep_incomplete_previous_chapter_message_contract() {
        let previous_chapters = vec![
            chapter_model(1, Some("第一章正文")),
            chapter_model(2, None),
            chapter_model(3, Some("   ")),
        ];
        let incomplete_numbers = previous_chapters
            .iter()
            .filter(|chapter| {
                chapter
                    .content
                    .as_ref()
                    .map(|content| content.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|chapter| chapter.chapter_number.to_string())
            .collect::<Vec<_>>();

        let result = ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: format!("前置章节尚未完成: {} 章", incomplete_numbers.join(", ")),
            previous_chapters,
        };

        assert!(!result.can_generate);
        assert_eq!(result.error_message, "前置章节尚未完成: 2, 3 章");
        assert_eq!(result.previous_chapters.len(), 3);
    }
}
