import { Suspense, lazy, memo } from 'react';
import type { ChapterBatchGenerateModalProps } from './ChapterBatchGenerateModal';

const LazyChapterBatchGenerateModal = lazy(() => import('./ChapterBatchGenerateModal'));

type ChapterBatchGenerateModalEntryProps = {
  visible: boolean;
  modalProps: ChapterBatchGenerateModalProps;
};

function ChapterBatchGenerateModalEntry({
  visible,
  modalProps,
}: ChapterBatchGenerateModalEntryProps) {
  if (!visible) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <LazyChapterBatchGenerateModal {...modalProps} />
    </Suspense>
  );
}

export default memo(ChapterBatchGenerateModalEntry);