import { Suspense, lazy, memo } from 'react';
import type { ExpansionPlanData } from '../types';
import type { ChapterPlanEditorModalState } from '../pages/chapterPlanEditorModalHelpers';

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
    <Suspense fallback={null}>
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