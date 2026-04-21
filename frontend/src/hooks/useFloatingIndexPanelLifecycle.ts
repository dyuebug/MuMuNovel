import { useCallback, useState } from 'react';
import {
  closeFloatingIndexPanel,
  openFloatingIndexPanel,
} from '../utils/floatingIndexPanelLifecycle';

export const useFloatingIndexPanelLifecycle = () => {
  const [isIndexPanelVisible, setIsIndexPanelVisible] = useState(false);

  const handleOpenIndexPanel = useCallback(() => {
    openFloatingIndexPanel({ setIsIndexPanelVisible });
  }, []);

  const handleCloseIndexPanel = useCallback(() => {
    closeFloatingIndexPanel({ setIsIndexPanelVisible });
  }, []);

  return {
    handleCloseIndexPanel,
    handleOpenIndexPanel,
    isIndexPanelVisible,
  };
};