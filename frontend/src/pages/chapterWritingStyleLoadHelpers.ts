import { message } from 'antd';
import { writingStyleApi } from '../services/modularApi';
import type { WritingStyle } from '../types';

export type ChapterWritingStylesCacheEntry = {
  styles: WritingStyle[];
  defaultStyleId?: number;
};

export async function loadChapterWritingStyles({
  projectId,
  writingStylesLoadPromises,
  writingStylesCache,
  setWritingStyles,
  setSelectedStyleId,
  normalizeWritingStyleOptions,
  areWritingStylesEqual,
}: {
  projectId: string;
  writingStylesLoadPromises: Map<string, Promise<void>>;
  writingStylesCache: Map<string, ChapterWritingStylesCacheEntry>;
  setWritingStyles: (value: WritingStyle[] | ((previousStyles: WritingStyle[]) => WritingStyle[])) => void;
  setSelectedStyleId: (value: number | undefined | ((previousStyleId: number | undefined) => number | undefined)) => void;
  normalizeWritingStyleOptions: (styles: WritingStyle[]) => WritingStyle[];
  areWritingStylesEqual: (leftStyles: WritingStyle[], rightStyles: WritingStyle[]) => boolean;
}): Promise<void> {
  const cachedStyles = writingStylesCache.get(projectId);
  if (cachedStyles) {
    setWritingStyles(cachedStyles.styles);
    setSelectedStyleId(cachedStyles.defaultStyleId);
    return;
  }

  const existingPromise = writingStylesLoadPromises.get(projectId);
  if (existingPromise) {
    await existingPromise;
    return;
  }

  const loadPromise = (async () => {
    try {
      const response = await writingStyleApi.getProjectStyles(projectId);
      const normalizedStyles = normalizeWritingStyleOptions(response.styles);

      setWritingStyles((previousStyles: WritingStyle[]) => (
        areWritingStylesEqual(previousStyles, normalizedStyles) ? previousStyles : normalizedStyles
      ));

      const defaultStyle = normalizedStyles.find((style) => style.is_default);
      setSelectedStyleId((previousStyleId: number | undefined) => (
        previousStyleId === defaultStyle?.id ? previousStyleId : defaultStyle?.id
      ));

      writingStylesCache.set(projectId, {
        styles: normalizedStyles,
        defaultStyleId: defaultStyle?.id,
      });
    } catch (error) {
      console.error('Failed to load writing styles.', error);
      message.error('Failed to load writing styles.');
    }
  })();

  writingStylesLoadPromises.set(projectId, loadPromise);
  try {
    await loadPromise;
  } finally {
    writingStylesLoadPromises.delete(projectId);
  }
}
