import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import { useStore } from '../../store';
import type { BackgroundTaskListResponse, BackgroundTaskStatus } from './backgroundTaskTypes';

const getKnownProjectIds = () =>
  new Set(useStore.getState().projects.map((project) => project.id));

export const buildMissingBackgroundTaskStatus = (taskId: string): BackgroundTaskStatus => {
  const now = new Date().toISOString();
  return {
    task_id: taskId,
    task_type: 'unknown',
    project_id: '',
    status: 'cancelled',
    progress: 100,
    message: '任务不存在',
    result: null,
    error: null,
    created_at: now,
    updated_at: now,
    completed_at: now,
  };
};

export const syncBackgroundTaskToStore = (task: BackgroundTaskStatus) => {
  useBackgroundTaskStore.getState().upsertTask(task);
  return task;
};

export const removeBackgroundTaskFromStore = (taskId: string) => {
  useBackgroundTaskStore.getState().removeTask(taskId);
};

export const syncBackgroundTaskListToStore = (
  data: BackgroundTaskListResponse,
): BackgroundTaskListResponse => {
  const projectIds = getKnownProjectIds();
  const shouldFilterByProject = projectIds.size > 0;
  const items = shouldFilterByProject
    ? (data.items || []).filter((item) => !item.project_id || projectIds.has(item.project_id))
    : (data.items || []);

  if (shouldFilterByProject) {
    useBackgroundTaskStore.getState().pruneTasksByProjectIds([...projectIds]);
  }

  for (const item of items) {
    useBackgroundTaskStore.getState().upsertTask(item);
  }

  return { ...data, items };
};
