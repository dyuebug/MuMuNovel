import { memo } from 'react';
import FloatingIndexPanelContent from './FloatingIndexPanelContent';
import FloatingIndexPanelDrawer from './FloatingIndexPanelDrawer';
import { useFloatingIndexPanelViewModel } from '../hooks/useFloatingIndexPanelViewModel';
import type { FloatingIndexPanelGroup } from '../utils/floatingIndexPanelContracts';

type FloatingIndexPanelProps = {
  visible: boolean;
  onClose: () => void;
  groupedChapters: FloatingIndexPanelGroup[];
  onChapterSelect: (chapterId: string) => void;
};

function FloatingIndexPanel({
  visible,
  onClose,
  groupedChapters,
  onChapterSelect,
}: FloatingIndexPanelProps) {
  const viewModel = useFloatingIndexPanelViewModel({
    groupedChapters,
    onChapterSelect,
    onClose,
  });

  return (
    <FloatingIndexPanelDrawer visible={visible} onClose={onClose}>
      <FloatingIndexPanelContent viewModel={viewModel} />
    </FloatingIndexPanelDrawer>
  );
}

export default memo(FloatingIndexPanel);