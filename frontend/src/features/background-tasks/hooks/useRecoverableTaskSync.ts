import { useEffect, useRef } from 'react';
import { backgroundTaskApi, chapterBatchTaskApi } from '../../../services/modularApi';
import { useBackgroundTaskStore } from '../../../store/backgroundTasks';

const getErrorResponseStatus = (error: unknown): number | null => {
  if (typeof error !== 'object' || error === null || !('response' in error)) {
    return null;
  }

  const response = (error as { response?: unknown }).response;
  if (typeof response !== 'object' || response === null || !('status' in response)) {
    return null;
  }

  const status = (response as { status?: unknown }).status;
  return typeof status === 'number' ? status : null;
};

let backgroundTasksApiSupported = true;
let chapterActiveTasksApiSupported = true;
let recoverableTasksSyncPromise: Promise<void> | null = null;

export const useRecoverableTaskSync = (params: {
  hiddenByRoute: boolean;
  open: boolean;
  activeTasksCount: number;
}) => {
  const { hiddenByRoute, open, activeTasksCount } = params;
  const recoverableTasksInitializedRef = useRef(false);

  useEffect(() => {
    if (hiddenByRoute) return;

    let stopped = false;

    const syncRecoverableTasks = async () => {
      if (stopped) return;
      if (recoverableTasksSyncPromise) {
        await recoverableTasksSyncPromise;
        return;
      }

      const backgroundRequest = backgroundTasksApiSupported
        ? backgroundTaskApi.listTasks({ active_only: true, limit: 100 })
          .then((response) => ({ ok: true, items: response.items || [] }))
          .catch((error: unknown) => {
            if (getErrorResponseStatus(error) === 404) {
              backgroundTasksApiSupported = false;
            }
            return { ok: false, items: [] as Array<{ task_id: string }> };
          })
        : Promise.resolve({ ok: false, items: [] as Array<{ task_id: string }> });

      const chapterRequest = chapterActiveTasksApiSupported
        ? chapterBatchTaskApi.listActiveTasks(100)
          .then((response) => ({ ok: true, items: response.items || [] }))
          .catch((error: unknown) => {
            if (getErrorResponseStatus(error) === 404) {
              chapterActiveTasksApiSupported = false;
            }
            return { ok: false, items: [] as Array<{ batch_id: string }> };
          })
        : Promise.resolve({ ok: false, items: [] as Array<{ batch_id: string }> });

      recoverableTasksSyncPromise = (async () => {
        const [backgroundResult, chapterResult] = await Promise.all([backgroundRequest, chapterRequest]);
        if (stopped) return;

        if (backgroundResult.ok || chapterResult.ok) {
          const activeIds = [
            ...backgroundResult.items.map((item) => item.task_id),
            ...chapterResult.items.map((item) => item.batch_id),
          ];
          useBackgroundTaskStore.getState().pruneMissingActiveTasks(activeIds);
        }
      })();

      try {
        await recoverableTasksSyncPromise;
      } finally {
        recoverableTasksSyncPromise = null;
      }
    };

    let initialSyncTimer: number | null = null;

    if (!recoverableTasksInitializedRef.current || open) {
      recoverableTasksInitializedRef.current = true;

      if (!open && activeTasksCount === 0) {
        initialSyncTimer = window.setTimeout(() => {
          if (!stopped) {
            void syncRecoverableTasks();
          }
        }, 2500);
      } else {
        void syncRecoverableTasks();
      }
    }

    if (!open) {
      return () => {
        stopped = true;
        if (initialSyncTimer !== null) {
          window.clearTimeout(initialSyncTimer);
        }
      };
    }

    const timer = window.setInterval(() => {
      void syncRecoverableTasks();
    }, 8000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [activeTasksCount, hiddenByRoute, open]);
};
