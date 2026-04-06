import { useCallback, useState } from 'react';

export const useBusyNavigationGuard = () => {
  const [isBusy, setBusy] = useState(false);

  const releaseBusy = useCallback(() => {
    setBusy(false);
  }, []);

  const shouldDisableNavigation = useCallback((guardActive: boolean) => {
    return guardActive && isBusy;
  }, [isBusy]);

  return {
    isBusy,
    setBusy,
    releaseBusy,
    shouldDisableNavigation,
  };
};
