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
  const groupCount = viewModel.resultsModel.filteredGroups.length;
  const chapterCount = viewModel.resultsModel.filteredGroups.reduce(
    (sum, group) => sum + group.chapters.length,
    0,
  );
  const hasSearch = viewModel.searchModel.searchTerm.trim().length > 0;

  return (
    <FloatingIndexPanelDrawer
      visible={visible}
      onClose={onClose}
      groupCount={groupCount}
      chapterCount={chapterCount}
      hasSearch={hasSearch}
    >
      <FloatingIndexPanelContent viewModel={viewModel} />
    </FloatingIndexPanelDrawer>
  );
}

export default memo(FloatingIndexPanel);
