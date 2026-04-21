import { Suspense, lazy, memo } from 'react';

const LazyChapterAnalysis = lazy(() => import('./ChapterAnalysis'));

type ChapterAnalysisEntryProps = {
  chapterId: string | null;
  visible: boolean;
  onClose: () => void;
};

function ChapterAnalysisEntry({
  chapterId,
  visible,
  onClose,
}: ChapterAnalysisEntryProps) {
  if (!chapterId) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <LazyChapterAnalysis
        chapterId={chapterId}
        visible={visible}
        onClose={onClose}
      />
    </Suspense>
  );
}

export default memo(ChapterAnalysisEntry);