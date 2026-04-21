import { useMemo } from 'react';
import type { ReactNode } from 'react';
import {
  buildFloatingIndexPanelTriggerProps,
  type FloatingIndexPanelTriggerProps,
} from '../utils/floatingIndexPanelTriggerProps';

type UseFloatingIndexTriggerPropsOptions = {
  icon: ReactNode;
  isMobile: boolean;
  onClick: () => void;
};

export const useFloatingIndexTriggerProps = ({
  icon,
  isMobile,
  onClick,
}: UseFloatingIndexTriggerPropsOptions): FloatingIndexPanelTriggerProps => {
  return useMemo(
    () => buildFloatingIndexPanelTriggerProps({
      icon,
      isMobile,
      onClick,
    }),
    [icon, isMobile, onClick],
  );
};