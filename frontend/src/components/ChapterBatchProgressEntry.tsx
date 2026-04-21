import { Suspense, lazy, memo } from 'react';

const LazySSEProgressModal = lazy(async () => {
  const module = await import('./SSEProgressModal');
  return { default: module.SSEProgressModal };
});

type ChapterBatchProgressEntryProps = {
  visible: boolean;
  progress: number;
  message: string;
  onCancel: () => void;
};

function ChapterBatchProgressEntry({
  visible,
  progress,
  message,
  onCancel,
}: ChapterBatchProgressEntryProps) {
  if (!visible) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <LazySSEProgressModal
        visible={visible}
        progress={progress}
        message={message}
        title={'Batch generation'}
        onCancel={onCancel}
        cancelButtonText={'Close'}
        blocking={false}
      />
    </Suspense>
  );
}

export default memo(ChapterBatchProgressEntry);