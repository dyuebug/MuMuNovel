import { useMemo } from 'react';
import type { FloatingIndexPanelSourceGroup } from '../utils/floatingIndexPanelContracts';
import {
  buildFloatingIndexPanelState,
  type FloatingIndexPanelState,
} from '../utils/floatingIndexPanelState';

type UseFloatingIndexPanelStateOptions = {
  groupedChapters: FloatingIndexPanelSourceGroup[];
  isIndexPanelVisible: boolean;
};

export const useFloatingIndexPanelState = ({
  groupedChapters,
  isIndexPanelVisible,
}: UseFloatingIndexPanelStateOptions): FloatingIndexPanelState | null => {
  return useMemo(
    () => buildFloatingIndexPanelState({
      groupedChapters,
      isIndexPanelVisible,
    }),
    [groupedChapters, isIndexPanelVisible],
  );
};