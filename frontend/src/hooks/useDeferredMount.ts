import { useEffect, useState } from 'react';

export const useDeferredMount = (enabled = true, timeoutMs = 120) => {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    if (!enabled || mounted) {
      return;
    }

    const timerId = setTimeout(() => {
      setMounted(true);
    }, timeoutMs);

    return () => {
      clearTimeout(timerId);
    };
  }, [enabled, mounted, timeoutMs]);

  return mounted;
};
