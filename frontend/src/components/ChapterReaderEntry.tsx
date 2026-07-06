import { Suspense, lazy, memo } from 'react';
import type { ChapterReaderModalState } from '../pages/chapterReaderModalHelpers';
import WorkflowEntryFallback from './WorkflowEntryFallback';

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
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          variant="fullscreen"
          eyebrow="Reader Workspace"
          title="正在进入章节阅读工作区"
          message="阅读器正在恢复正文、导航和阅读设置，原有章节切换与本地持久化逻辑保持不变。"
          tags={[
            { label: '沉浸阅读', color: 'blue' },
            { label: '导航链路恢复中', color: 'processing' },
            { label: '设置逻辑保持原样', color: 'green' },
          ]}
        />
      )}
    >
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
