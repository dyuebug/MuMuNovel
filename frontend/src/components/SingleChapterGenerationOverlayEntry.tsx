import { Suspense, lazy, memo } from 'react';
import { useChapterGenerationUiStore } from '../store/chapterGenerationUi';
import WorkflowEntryFallback from './WorkflowEntryFallback';

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
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          variant="floating"
          eyebrow="Generation Overlay"
          title="正在接管单章生成覆盖层"
          message="系统正在恢复单章生成中的进度遮罩与提示文案，原有生成状态、阻塞策略与退出时序保持不变。"
          tags={[
            { label: '单章生成', color: 'gold' },
            { label: '覆盖层恢复中', color: 'processing' },
            { label: '状态逻辑保持原样', color: 'green' },
          ]}
        />
      )}
    >
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
