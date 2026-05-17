import { Suspense, lazy, memo } from 'react';
import { useChapterGenerationUiStore } from '../store/chapterGenerationUi';

const LazySSELoadingOverlay = lazy(async () => {
  const module = await import('./SSELoadingOverlay');
  return { default: module.SSELoadingOverlay };
});

function SingleChapterGenerationOverlayEntry() {
  const { loading, progress, message } = useChapterGenerationUiStore((state) => state.singleOverlay);

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
