import { Suspense, lazy, memo } from 'react';

const LazySSELoadingOverlay = lazy(async () => {
  const module = await import('./SSELoadingOverlay');
  return { default: module.SSELoadingOverlay };
});

type SingleChapterGenerationOverlayEntryProps = {
  loading: boolean;
  progress: number;
  message: string;
};

function SingleChapterGenerationOverlayEntry({
  loading,
  progress,
  message,
}: SingleChapterGenerationOverlayEntryProps) {
  if (!loading) {
    return null;
  }

  return (
    <Suspense fallback={null}>
      <LazySSELoadingOverlay
        loading={loading}
        progress={progress}
        message={message}
        blocking={false}
      />
    </Suspense>
  );
}

export default memo(SingleChapterGenerationOverlayEntry);