import { createElement, useEffect, useRef } from 'react';
import { Button, notification } from 'antd';
import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';
import {
  getCompletionNotice,
  getTaskDestination,
  terminalStatuses,
} from '../../../components/backgroundTaskPresentation';

export const useTaskNotifications = (params: {
  tasks: TrackedBackgroundTask[];
  onNavigate: (to: string) => void;
}) => {
  const { tasks, onNavigate } = params;
  const statusSnapshotRef = useRef<Record<string, TrackedBackgroundTask['status']>>({});
  const statusSnapshotReadyRef = useRef(false);

  useEffect(() => {
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
  }, [tasks, onNavigate]);
};
