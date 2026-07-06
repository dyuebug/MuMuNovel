import { Suspense, lazy, memo } from 'react';
import type { ChapterBatchGenerateModalProps } from './ChapterBatchGenerateModal';
import WorkflowEntryFallback from './WorkflowEntryFallback';

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
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          eyebrow="Batch Generation"
          title="正在整理批量生成工作台"
          message="系统正在恢复批量章节生成面板，现有模型配置、质量预设和任务状态逻辑不会发生变化。"
          tags={[
            { label: '批量生成工作流', color: 'purple' },
            { label: '质量设置保持原样', color: 'green' },
          ]}
        />
      )}
    >
      <LazyChapterBatchGenerateModal {...modalProps} />
    </Suspense>
  );
}

export default memo(ChapterBatchGenerateModalEntry);
