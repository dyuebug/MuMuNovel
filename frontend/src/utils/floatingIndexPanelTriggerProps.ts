import type { CSSProperties, ReactNode } from 'react';
import { FLOATING_INDEX_PANEL_TRIGGER_TOOLTIP } from './floatingIndexPanelViewHelpers';

export type FloatingIndexPanelTriggerProps = {
  icon: ReactNode;
  onClick: () => void;
  style: CSSProperties;
  tooltip: string;
  type: 'primary';
};

export function buildFloatingIndexPanelTriggerProps({
  icon,
  isMobile,
  onClick,
}: {
  icon: ReactNode;
  isMobile: boolean;
  onClick: () => void;
}): FloatingIndexPanelTriggerProps {
  return {
    icon,
    onClick,
    style: {
      right: isMobile ? 24 : 48,
      bottom: isMobile ? 80 : 48,
    },
    tooltip: FLOATING_INDEX_PANEL_TRIGGER_TOOLTIP,
    type: 'primary',
  };
}