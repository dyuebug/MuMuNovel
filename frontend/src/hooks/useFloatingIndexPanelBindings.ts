import type { ReactNode } from 'react';
import type { FloatingIndexPanelSourceGroup } from '../utils/floatingIndexPanelContracts';
import { useFloatingIndexPanelLifecycle } from './useFloatingIndexPanelLifecycle';
import { useFloatingIndexPanelState } from './useFloatingIndexPanelState';
import { useFloatingIndexTriggerProps } from './useFloatingIndexTriggerProps';

type UseFloatingIndexPanelBindingsOptions = {
  groupedChapters: FloatingIndexPanelSourceGroup[];
  icon: ReactNode;
  isMobile: boolean;
};

export const useFloatingIndexPanelBindings = ({
  groupedChapters,
  icon,
  isMobile,
}: UseFloatingIndexPanelBindingsOptions) => {
  const {
    handleCloseIndexPanel,
    handleOpenIndexPanel,
    isIndexPanelVisible,
  } = useFloatingIndexPanelLifecycle();

  const floatingIndexPanelState = useFloatingIndexPanelState({
    groupedChapters: isIndexPanelVisible ? groupedChapters : [],
    isIndexPanelVisible,
  });

  const floatingIndexPanelTriggerProps = useFloatingIndexTriggerProps({
    icon,
    isMobile,
    onClick: handleOpenIndexPanel,
  });

  return {
    floatingIndexPanelState,
    floatingIndexPanelTriggerProps,
    handleCloseIndexPanel,
  };
};
