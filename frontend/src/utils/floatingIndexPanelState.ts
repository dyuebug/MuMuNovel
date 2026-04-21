import type {
  FloatingIndexPanelGroup,
  FloatingIndexPanelSourceGroup,
} from './floatingIndexPanelContracts';
import {
  formatFloatingIndexOutlineLabel,
  resolveFloatingIndexOutlineTagColor,
} from './floatingIndexPanelViewHelpers';

export type FloatingIndexPanelState = {
  visible: boolean;
  groupedChapters: FloatingIndexPanelGroup[];
};

export function buildFloatingIndexPanelState({
  groupedChapters,
  isIndexPanelVisible,
}: {
  groupedChapters: FloatingIndexPanelSourceGroup[];
  isIndexPanelVisible: boolean;
}): FloatingIndexPanelState | null {
  if (!isIndexPanelVisible) {
    return null;
  }

  return {
    visible: isIndexPanelVisible,
    groupedChapters: groupedChapters.map((group) => ({
      chapters: group.chapters,
      key: group.key,
      outlineLabel: formatFloatingIndexOutlineLabel(group.outlineTitle),
      outlineTagColor: resolveFloatingIndexOutlineTagColor(group.outlineId),
    })),
  };
}