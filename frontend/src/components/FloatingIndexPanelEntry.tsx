import { Suspense, lazy, memo } from 'react';
import { FloatButton } from 'antd';
import type { FloatingIndexPanelState } from '../utils/floatingIndexPanelState';
import type { FloatingIndexPanelTriggerProps } from '../utils/floatingIndexPanelTriggerProps';

const LazyFloatingIndexPanel = lazy(() => import('./FloatingIndexPanel'));

type FloatingIndexPanelEntryProps = {
  floatingIndexPanelState: FloatingIndexPanelState | null;
  floatingIndexPanelTriggerProps: FloatingIndexPanelTriggerProps;
  onClose: () => void;
  onChapterSelect: (chapterId: string) => void;
};

function FloatingIndexPanelEntry({
  floatingIndexPanelState,
  floatingIndexPanelTriggerProps,
  onClose,
  onChapterSelect,
}: FloatingIndexPanelEntryProps) {
  return (
    <>
      <FloatButton {...floatingIndexPanelTriggerProps} />

      {floatingIndexPanelState ? (
        <Suspense fallback={null}>
          <LazyFloatingIndexPanel
            visible={floatingIndexPanelState.visible}
            onClose={onClose}
            groupedChapters={floatingIndexPanelState.groupedChapters}
            onChapterSelect={onChapterSelect}
          />
        </Suspense>
      ) : null}
    </>
  );
}

export default memo(FloatingIndexPanelEntry);