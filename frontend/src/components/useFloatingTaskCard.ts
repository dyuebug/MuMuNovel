import { useCallback, useEffect, useRef, useState } from 'react';

interface UseFloatingTaskCardOptions {
  active: boolean;
  blocking?: boolean;
  autoCollapseMs?: number;
}

export const useFloatingTaskCard = ({
  active,
  blocking = true,
  autoCollapseMs = 2500,
}: UseFloatingTaskCardOptions) => {
  const [collapsed, setCollapsed] = useState(false);
  const [floatButtonOffset, setFloatButtonOffset] = useState(0);
  const interactedRef = useRef(false);

  useEffect(() => {
    if (blocking || !active || typeof window === 'undefined') {
      interactedRef.current = false;
      setCollapsed(false);
      return;
    }

    interactedRef.current = false;
    setCollapsed(false);

    const timer = window.setTimeout(() => {
      if (!interactedRef.current) {
        setCollapsed(true);
      }
    }, autoCollapseMs);

    return () => window.clearTimeout(timer);
  }, [active, autoCollapseMs, blocking]);

  useEffect(() => {
    if (blocking || !active || typeof window === 'undefined') {
      setFloatButtonOffset(0);
      return;
    }

    let rafId: number | null = null;

    const recalculateOffset = () => {
      const nodes = Array.from(document.querySelectorAll<HTMLElement>('.ant-float-btn, .ant-float-btn-group'));
      let nextOffset = 0;

      for (const node of nodes) {
        const style = window.getComputedStyle(node);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
          continue;
        }

        const rect = node.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) {
          continue;
        }

        const rightDistance = window.innerWidth - rect.right;
        if (rightDistance > 180) {
          continue;
        }

        const clearance = Math.max(0, window.innerHeight - rect.top + 12);
        if (clearance > nextOffset) {
          nextOffset = clearance;
        }
      }

      setFloatButtonOffset((prev) => (Math.abs(prev - nextOffset) < 1 ? prev : nextOffset));
    };

    const scheduleRecalculate = () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
      rafId = window.requestAnimationFrame(recalculateOffset);
    };

    scheduleRecalculate();

    window.addEventListener('resize', scheduleRecalculate);
    window.addEventListener('scroll', scheduleRecalculate, true);

    const observer = new MutationObserver(scheduleRecalculate);
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style'],
    });

    return () => {
      window.removeEventListener('resize', scheduleRecalculate);
      window.removeEventListener('scroll', scheduleRecalculate, true);
      observer.disconnect();
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [active, blocking]);

  const toggleCollapsed = useCallback(() => {
    interactedRef.current = true;
    setCollapsed((prev) => !prev);
  }, []);

  const floatingBottom = floatButtonOffset > 0
    ? `calc(max(16px, env(safe-area-inset-bottom)) + ${Math.round(floatButtonOffset)}px)`
    : 'max(16px, env(safe-area-inset-bottom))';

  return {
    collapsed,
    floatingBottom,
    toggleCollapsed,
  };
};

