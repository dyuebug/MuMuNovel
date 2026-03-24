const WORD_COUNT_CACHE_KEY = 'chapter_default_word_count';

export const DEFAULT_WORD_COUNT = 3000;

export const getCachedWordCount = (): number => {
  try {
    const cached = localStorage.getItem(WORD_COUNT_CACHE_KEY);

    if (cached) {
      const value = parseInt(cached, 10);
      if (!Number.isNaN(value) && value >= 500 && value <= 10000) {
        return value;
      }
    }
  } catch (error) {
    console.warn('Failed to read cached word count.', error);
  }

  return DEFAULT_WORD_COUNT;
};

export const setCachedWordCount = (value: number): void => {
  try {
    localStorage.setItem(WORD_COUNT_CACHE_KEY, String(value));
  } catch (error) {
    console.warn('Failed to persist cached word count.', error);
  }
};
