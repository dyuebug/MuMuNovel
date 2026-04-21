import { Suspense, lazy, memo } from 'react';
import type { ChapterReaderModalState } from '../pages/chapterReaderModalHelpers';

const LazyChapterReader = lazy(() => import('./ChapterReader'));

type ChapterReaderEntryProps = {
  chapterReaderModalState: ChapterReaderModalState | null;
  onClose: () => void;
  onChapterChange: (chapterId: string) => void | Promise<void>;
};

function ChapterReaderEntry({
  chapterReaderModalState,
  onClose,
  onChapterChange,
}: ChapterReaderEntryProps) {
  if (!chapterReaderModalState) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <LazyChapterReader
        visible={chapterReaderModalState.visible}
        chapter={chapterReaderModalState.chapter}
        onClose={onClose}
        onChapterChange={onChapterChange}
      />
    </Suspense>
  );
}

export default memo(ChapterReaderEntry);