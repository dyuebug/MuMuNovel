import { createElement, useCallback, useEffect, useMemo, useRef } from 'react';
import { Button, notification } from 'antd';
import { useShallow } from 'zustand/react/shallow';
import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';
import { useBackgroundTaskStore } from '../../../store/backgroundTasks';
import {
  selectVisibleBackgroundTaskStatusSignatures,
  selectVisibleBackgroundTasks,
} from '../model/selectors';
import {
  getCompletionNotice,
  getTaskDestination,
  terminalStatuses,
} from '../../../components/backgroundTaskPresentation';

export const useTaskNotifications = (params: {
  knownProjectIds: Set<string>;
  onNavigate: (to: string) => void;
}) => {
  const { knownProjectIds, onNavigate } = params;
  const statusSnapshotRef = useRef<Record<string, TrackedBackgroundTask['status']>>({});
  const statusSnapshotReadyRef = useRef(false);
  const statusPriority = useMemo(
    () => ({
      running: 0,
      pending: 1,
      failed: 2,
      cancelled: 3,
      completed: 4,
    } as const),
    [],
  );
  const visibleTaskStatusSignatures = useBackgroundTaskStore(
    useShallow(
      useCallback(
        (state) => selectVisibleBackgroundTaskStatusSignatures(state.tasks, knownProjectIds),
        [knownProjectIds, statusPriority],
      ),
    ),
  );

  useEffect(() => {
    const tasks = selectVisibleBackgroundTasks(
      useBackgroundTaskStore.getState().tasks,
      knownProjectIds,
      statusPriority,
    );
    const currentSnapshot = Object.fromEntries(tasks.map((task) => [task.taskId, task.status]));

    if (!statusSnapshotReadyRef.current) {
      statusSnapshotRef.current = currentSnapshot;
      statusSnapshotReadyRef.current = true;
      return;
    }

    for (const task of tasks) {
      const previousStatus = statusSnapshotRef.current[task.taskId];
      if (!previousStatus || previousStatus === task.status || !terminalStatuses.has(task.status)) {
        continue;
      }

      const notice = getCompletionNotice(task);
      const targetRoute = getTaskDestination(task);
      const notificationKey = `task-result-${task.taskId}-${task.status}`;

      notification.open({
        key: notificationKey,
        message: notice.title,
        description: notice.description,
        duration: 6,
        btn: targetRoute
          ? createElement(
            Button,
            {
              type: 'link',
              size: 'small',
              onClick: () => {
                notification.destroy(notificationKey);
                onNavigate(targetRoute);
              },
            },
            '查看详情',
          )
          : undefined,
      });
    }

    statusSnapshotRef.current = currentSnapshot;
  }, [knownProjectIds, onNavigate, statusPriority, visibleTaskStatusSignatures]);
};
