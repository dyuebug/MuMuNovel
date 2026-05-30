pub(crate) const DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT: i32 = 3000;
pub(crate) const MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT: i32 = 1;

pub(crate) fn normalize_chapter_generation_target_word_count(
    target_word_count: Option<i32>,
) -> i32 {
    target_word_count
        .unwrap_or(DEFAULT_CHAPTER_GENERATION_TARGET_WORD_COUNT)
        .max(MIN_CHAPTER_GENERATION_TARGET_WORD_COUNT)
}

#[cfg(test)]
mod tests {
    use super::normalize_chapter_generation_target_word_count;

    #[test]
    fn should_normalize_chapter_generation_target_word_count() {
        assert_eq!(normalize_chapter_generation_target_word_count(None), 3000);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(normalize_chapter_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }
}
