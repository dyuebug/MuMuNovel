import type { Chapter } from '../types';

export type ChapterReaderModalState = {
  visible: boolean;
  chapter: Chapter;
};

export function buildChapterReaderModalState({
  readerVisible,
  readingChapter,
}: {
  readerVisible: boolean;
  readingChapter: Chapter | null;
}): ChapterReaderModalState | null {
  if (!readerVisible || !readingChapter) {
    return null;
  }

  return {
    visible: readerVisible,
    chapter: readingChapter,
  };
}