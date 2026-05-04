import { useEffect, useMemo, useState } from 'react';
import { message } from 'antd';
import type { NavigateFunction } from 'react-router-dom';
import { noAuthRedirectConfig } from '../../../services/core/httpClient';
import {
  backgroundTaskApi,
  chapterApi,
  chapterBatchTaskApi,
  chapterSingleTaskApi,
} from '../../../services/modularApi';
import { OPEN_BACKGROUND_TASK_CENTER_EVENT } from '../../../constants/backgroundTaskEvents';
import { useStore } from '../../../store';
import { useBackgroundTaskStore, type TrackedBackgroundTask } from '../../../store/backgroundTasks';
import { isTaskResumable } from '../../../components/backgroundTaskPresentation';
import { selectActiveBackgroundTasks, selectBackgroundTaskSections, selectVisibleBackgroundTasks, type TaskFilter } from '../model/selectors';
import { buildBackgroundTaskCenterSummary } from '../model/summary';
import { useRecoverableTaskSync } from './useRecoverableTaskSync';
import { useTaskNotifications } from './useTaskNotifications';

const statusPriority: Record<TrackedBackgroundTask['status'], number> = {
  running: 0,
  pending: 1,
  failed: 2,
  cancelled: 3,
  completed: 4,
};

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

export type BackgroundTaskCenterController = {
  hiddenByRoute: boolean;
  focusProjectId: string | null;
  open: boolean;
  setOpen: (next: boolean) => void;
  isMobile: boolean;
  tasks: TrackedBackgroundTask[];
  activeTasks: TrackedBackgroundTask[];
  taskFilter: TaskFilter;
  setTaskFilter: (next: TaskFilter) => void;
  filterOptions: Array<{ label: string; value: TaskFilter }>;
  taskSections: ReturnType<typeof selectBackgroundTaskSections>;
  summary: ReturnType<typeof buildBackgroundTaskCenterSummary>;
  clearTerminalTasks: () => void;
  resumeAllRecoverableTasks: () => Promise<void>;
  removeTask: (taskId: string) => void;
  cancelTask: (task: TrackedBackgroundTask) => Promise<void>;
  resumeTask: (task: TrackedBackgroundTask) => Promise<void>;
  canCancelTask: (task: TrackedBackgroundTask) => boolean;
  canResumeTask: (task: TrackedBackgroundTask) => boolean;
  cancellingTaskIds: Record<string, boolean>;
  resumingTaskIds: Record<string, boolean>;
  onNavigate: NavigateFunction;
};

export const useBackgroundTaskCenterController = (params: {
  pathname: string;
  navigate: NavigateFunction;
  isMobile: boolean;
}): BackgroundTaskCenterController => {
  const { pathname, navigate, isMobile } = params;

  const [open, setOpen] = useState(false);
  const [taskFilter, setTaskFilter] = useState<TaskFilter>('overview');
  const [cancellingTaskIds, setCancellingTaskIds] = useState<Record<string, boolean>>({});
  const [resumingTaskIds, setResumingTaskIds] = useState<Record<string, boolean>>({});

  const currentProject = useStore((state) => state.currentProject);
  const projects = useStore((state) => state.projects);
  const hiddenByRoute = pathname === '/login' || pathname.startsWith('/auth/callback');
  const knownProjectIds = useMemo(() => new Set(projects.map((project) => project.id)), [projects]);
  const routeProjectId = useMemo(() => {
    const matched = pathname.match(/^\/project\/([^/]+)/);
    return matched?.[1] ?? null;
  }, [pathname]);
  const rawFocusProjectId = routeProjectId ?? currentProject?.id ?? null;
  const focusProjectId =
    rawFocusProjectId && (knownProjectIds.size === 0 || knownProjectIds.has(rawFocusProjectId))
      ? rawFocusProjectId
      : null;

  const tasksMap = useBackgroundTaskStore((state) => state.tasks);
  const removeTask = useBackgroundTaskStore((state) => state.removeTask);
  const clearTerminalTasks = useBackgroundTaskStore((state) => state.clearTerminalTasks);

  const tasks = useMemo(
    () => selectVisibleBackgroundTasks(tasksMap, knownProjectIds, statusPriority),
    [tasksMap, knownProjectIds],
  );

  const activeTasks = useMemo(() => selectActiveBackgroundTasks(tasks), [tasks]);
  const activeTaskPollKey = useMemo(
    () => activeTasks.map((task) => `${task.taskType}:${task.taskId}`).join('|'),
    [activeTasks],
  );

  const filterOptions = useMemo(
    () => [
      { label: '总览', value: 'overview' as const },
      { label: '进行中', value: 'active' as const },
      { label: '失败', value: 'failed' as const },
      ...(focusProjectId ? [{ label: '当前项目', value: 'current' as const }] : []),
    ],
    [focusProjectId],
  );

  useEffect(() => {
    if (!focusProjectId && taskFilter === 'current') {
      setTaskFilter('overview');
    }
  }, [focusProjectId, taskFilter]);

  const taskSections = useMemo(
    () => selectBackgroundTaskSections(tasks, focusProjectId, taskFilter),
    [tasks, focusProjectId, taskFilter],
  );

  const summary = useMemo(
    () => buildBackgroundTaskCenterSummary({
      tasks,
      activeTasks,
      focusProjectId,
      isTaskResumable,
    }),
    [tasks, activeTasks, focusProjectId],
  );

  useEffect(() => {
    const handleOpenTaskCenter = () => setOpen(true);
    window.addEventListener(OPEN_BACKGROUND_TASK_CENTER_EVENT, handleOpenTaskCenter);

    return () => {
      window.removeEventListener(OPEN_BACKGROUND_TASK_CENTER_EVENT, handleOpenTaskCenter);
    };
  }, []);

  useRecoverableTaskSync({
    hiddenByRoute,
    open,
    activeTasksCount: activeTasks.length,
  });

  useEffect(() => {
    if (hiddenByRoute) return;
    if (activeTasks.length === 0) return;

    let stopped = false;
    const handleMissingTask = (taskId: string, error: unknown) => {
      if (getErrorResponseStatus(error) !== 404) return;
      removeTask(taskId);
    };

    const poll = async () => {
      if (stopped) return;
      await Promise.allSettled(
        activeTasks.map((task) => {
          if (task.taskType === 'chapters_batch_generate') {
            return chapterBatchTaskApi
              .getBatchGenerateStatus(task.taskId, task.projectId)
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          if (task.taskType === 'chapter_single_generate') {
            return chapterSingleTaskApi
              .getSingleGenerateTaskStatus(task.taskId, task.projectId)
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          if (task.taskType === 'chapter_analysis') {
            const chapterId = typeof task.checkpoint?.chapter_id === 'string'
              ? task.checkpoint.chapter_id
              : undefined;
            if (!chapterId) return Promise.resolve(null);
            return chapterApi
              .getChapterAnalysisStatus(chapterId, task.projectId, noAuthRedirectConfig())
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          return backgroundTaskApi
            .getTaskStatus(task.taskId)
            .catch((error: unknown) => handleMissingTask(task.taskId, error));
        }),
      );
    };

    void poll();
    const timer = window.setInterval(() => {
      void poll();
    }, 2000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [activeTaskPollKey, activeTasks, hiddenByRoute, removeTask]);

  useTaskNotifications({
    tasks,
    onNavigate: (to) => navigate(to),
  });

  const canResumeTask = (task: TrackedBackgroundTask) => isTaskResumable(task);

  const canCancelTask = (task: TrackedBackgroundTask) =>
    task.taskType !== 'chapter_analysis';

  const cancelTask = async (task: TrackedBackgroundTask) => {
    const taskId = task.taskId;
    if (cancellingTaskIds[taskId]) return;

    setCancellingTaskIds((prev) => ({ ...prev, [taskId]: true }));
    try {
      if (task.taskType === 'chapters_batch_generate') {
        await chapterBatchTaskApi.cancelBatchGenerateTask(taskId, task.projectId);
      } else if (task.taskType === 'chapter_single_generate') {
        await chapterSingleTaskApi.cancelSingleGenerateTask(taskId, task.projectId);
      } else {
        await backgroundTaskApi.cancelTask(taskId);
      }
      message.info('正在取消后台任务...');
    } catch (error) {
      const err = error as Error;
      message.error(err.message || '取消后台任务失败');
    } finally {
      setCancellingTaskIds((prev) => {
        const next = { ...prev };
        delete next[taskId];
        return next;
      });
    }
  };

  const resumeTask = async (task: TrackedBackgroundTask) => {
    const taskId = task.taskId;
    if (resumingTaskIds[taskId]) return;
    if (!canResumeTask(task)) return;

    setResumingTaskIds((prev) => ({ ...prev, [taskId]: true }));
    try {
      if (task.taskType === 'chapters_batch_generate') {
        await chapterBatchTaskApi.resumeBatchGenerateTask(taskId, task.projectId);
      } else {
        await chapterSingleTaskApi.resumeSingleGenerateTask(taskId, task.projectId);
      }
      message.success('已创建继续任务，正在排队执行');
    } catch (error) {
      const err = error as Error;
      message.error(err.message || '继续任务失败');
    } finally {
      setResumingTaskIds((prev) => {
        const next = { ...prev };
        delete next[taskId];
        return next;
      });
    }
  };

  const resumeAllRecoverableTasks = async () => {
    const recoverableTasks = tasks.filter(canResumeTask).filter((task) => !resumingTaskIds[task.taskId]);
    if (recoverableTasks.length === 0) {
      message.info('暂无可继续的失败任务');
      return;
    }

    setResumingTaskIds((prev) => {
      const next = { ...prev };
      recoverableTasks.forEach((task) => {
        next[task.taskId] = true;
      });
      return next;
    });

    const results = await Promise.allSettled(
      recoverableTasks.map((task) =>
        task.taskType === 'chapters_batch_generate'
          ? chapterBatchTaskApi.resumeBatchGenerateTask(task.taskId, task.projectId)
          : chapterSingleTaskApi.resumeSingleGenerateTask(task.taskId, task.projectId),
      ),
    );

    const successCount = results.filter((result) => result.status === 'fulfilled').length;
    const failedCount = results.length - successCount;

    setResumingTaskIds((prev) => {
      const next = { ...prev };
      recoverableTasks.forEach((task) => {
        delete next[task.taskId];
      });
      return next;
    });

    if (successCount > 0) {
      message.success(`已重新排队 ${successCount} 个任务`);
    }
    if (failedCount > 0) {
      message.warning(`${failedCount} 个任务继续失败，请逐个检查`);
    }
  };

  return {
    hiddenByRoute,
    focusProjectId,
    open,
    setOpen,
    isMobile,
    tasks,
    activeTasks,
    taskFilter,
    setTaskFilter,
    filterOptions,
    taskSections,
    summary,
    clearTerminalTasks,
    resumeAllRecoverableTasks,
    removeTask,
    cancelTask,
    resumeTask,
    canCancelTask,
    canResumeTask,
    cancellingTaskIds,
    resumingTaskIds,
    onNavigate: navigate,
  };
};
