import { Suspense, lazy, memo } from 'react';
import type { ExpansionPlanData } from '../types';
import type { ChapterPlanEditorModalState } from '../pages/chapterPlanEditorModalHelpers';
import WorkflowEntryFallback from './WorkflowEntryFallback';

const LazyExpansionPlanEditor = lazy(() => import('./ExpansionPlanEditor'));

type ChapterPlanEditorEntryProps = {
  planEditorModalState: ChapterPlanEditorModalState | null;
  onSave: (planData: ExpansionPlanData) => Promise<void>;
  onCancel: () => void;
};

function ChapterPlanEditorEntry({
  planEditorModalState,
  onSave,
  onCancel,
}: ChapterPlanEditorEntryProps) {
  if (!planEditorModalState) {
    return null;
  }

  return (
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          eyebrow="Plan Editor"
          title="正在展开章节扩写规划面板"
          message="系统正在恢复章节扩写计划、摘要上下文与保存入口，原有保存链路和章节数据不会发生变化。"
          tags={[
            { label: '扩写规划', color: 'cyan' },
            { label: '编辑工作区恢复中', color: 'processing' },
            { label: '保存逻辑保持原样', color: 'green' },
          ]}
        />
      )}
    >
      <LazyExpansionPlanEditor
        visible={planEditorModalState.visible}
        planData={planEditorModalState.planData}
        chapterSummary={planEditorModalState.chapterSummary}
        projectId={planEditorModalState.projectId}
        onSave={onSave}
        onCancel={onCancel}
      />
    </Suspense>
  );
}

export default memo(ChapterPlanEditorEntry);
