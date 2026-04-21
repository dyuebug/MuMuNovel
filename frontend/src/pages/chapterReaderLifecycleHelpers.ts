import { message } from 'antd';
import type { Chapter } from '../types';

export function openChapterReader({
  chapter,
  setReadingChapter,
  setReaderVisible,
}: {
  chapter: Chapter;
  setReadingChapter: (chapter: Chapter | null) => void;
  setReaderVisible: (visible: boolean) => void;
}): void {
  setReadingChapter(chapter);
  setReaderVisible(true);
}

export function closeChapterReader({
  setReadingChapter,
  setReaderVisible,
}: {
  setReadingChapter: (chapter: Chapter | null) => void;
  setReaderVisible: (visible: boolean) => void;
}): void {
  setReaderVisible(false);
  setReadingChapter(null);
}

export async function loadReaderChapter({
  chapterId,
  setReadingChapter,
}: {
  chapterId: string;
  setReadingChapter: (chapter: Chapter | null) => void;
}): Promise<void> {
  try {
    const response = await fetch(`/api/chapters/${chapterId}`);

    if (!response.ok) {
      throw new Error('Failed to load chapter.');
    }

    const newChapter = await response.json() as Chapter;
    setReadingChapter(newChapter);
  } catch {
    message.error('Failed to load chapter.');
  }
}
