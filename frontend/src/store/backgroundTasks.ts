import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ActiveStoryRepairPayload } from '../types';

export type BackgroundTaskRuntimeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface TrackedBackgroundTask {
  taskId: string;
  taskType: string;
  projectId?: string;
  status: BackgroundTaskRuntimeStatus;
  progress: number;
  message: string;
  result?: Record<string, unknown> | null;
  error?: string | null;
  stageCode?: string;
  executionMode?: 'interactive' | 'auto';
  workflowScope?: string;
  checkpoint?: Record<string, unknown> | null;
  failedChapters?: Array<Record<string, unknown>>;
  activeStoryRepairPayload?: ActiveStoryRepairPayload | null;
  terminalReason?: string | null;
  terminalLabel?: string | null;
  reviewRequired?: boolean;
  canResume?: boolean;
  createdAt: number;
  updatedAt: number;
  completedAt?: number;
}


interface UpsertTaskPayload {
  task_id: string;
  task_type?: string | null;
  project_id?: string | null;
  status?: BackgroundTaskRuntimeStatus;
  progress?: number;
  message?: string;
  result?: Record<string, unknown> | null;
  error?: string | null;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  workflow_scope?: string | null;
  checkpoint?: Record<string, unknown> | null;
  failed_chapters?: Array<Record<string, unknown>>;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
  terminal_reason?: string | null;
  terminal_label?: string | null;
  review_required?: boolean | null;
  can_resume?: boolean | null;
  created_at?: string | null;
  updated_at?: string | null;
  completed_at?: string | null;
}


type ActiveTaskScope = 'background' | 'chapter_batch';

interface BackgroundTaskState {
  tasks: Record<string, TrackedBackgroundTask>;
  upsertTask: (task: UpsertTaskPayload) => void;
  removeTask: (taskId: string) => void;
  removeTasksByProjectId: (projectId: string) => void;
  pruneTasksByProjectIds: (projectIds: string[]) => void;
  pruneMissingActiveTasks: (activeTaskIds: string[], scope?: ActiveTaskScope) => void;
  clearTerminalTasks: () => void;
  pruneExpiredTerminalTasks: () => void;
}

const TERMINAL_STATUSES: BackgroundTaskRuntimeStatus[] = ['completed', 'failed', 'cancelled'];
const TERMINAL_TASK_RETENTION_MS = 1000 * 60 * 60 * 12;
const MAX_PERSISTED_TASKS = 30;
const MAX_TERMINAL_TASKS = 12;
const ACTIVE_TASK_GRACE_MS = 1000 * 60;

const isChapterManagedTask = (taskType: string) =>
  taskType === 'chapters_batch_generate' || taskType === 'chapter_single_generate' || taskType === 'chapter_analysis';

const matchesActiveTaskScope = (task: TrackedBackgroundTask, scope?: ActiveTaskScope) => {
  if (!scope) {
    return true;
  }

  if (scope === 'chapter_batch') {
    return task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate';
  }

  if (scope === 'background') {
    return !isChapterManagedTask(task.taskType);
  }

  return true;
};


const toTimestamp = (value?: string | null): number | undefined => {
  if (!value) return undefined;
  const next = new Date(value).getTime();
  return Number.isNaN(next) ? undefined : next;
};

const normalizeProgress = (progress?: number): number => {
  if (typeof progress !== 'number' || Number.isNaN(progress)) return 0;
  if (progress < 0) return 0;
  if (progress > 100) return 100;
  return Math.round(progress);
};

const compactTasks = (tasks: Record<string, TrackedBackgroundTask>): Record<string, TrackedBackgroundTask> => {
  const now = Date.now();
  const allTasks = Object.values(tasks).sort((a, b) => b.updatedAt - a.updatedAt);

  const activeTasks = allTasks.filter((task) => !TERMINAL_STATUSES.includes(task.status));
  const recentTerminalTasks = allTasks
    .filter((task) => TERMINAL_STATUSES.includes(task.status))
    .filter((task) => now - (task.completedAt ?? task.updatedAt) <= TERMINAL_TASK_RETENTION_MS)
    .slice(0, MAX_TERMINAL_TASKS);

  const keep = new Set(
    [...activeTasks, ...recentTerminalTasks]
      .slice(0, MAX_PERSISTED_TASKS)
      .map((item) => item.taskId)
  );

  return Object.fromEntries(
    Object.entries(tasks).filter(([taskId]) => keep.has(taskId))
  );
};

export const useBackgroundTaskStore = create<BackgroundTaskState>()(
  persist(
    (set, get) => ({
      tasks: {},
      upsertTask: (task) => {
        if (!task.task_id) return;

        const now = Date.now();
        const existing = get().tasks[task.task_id];
        const incomingStatus = task.status ?? existing?.status ?? 'pending';
        const terminal = TERMINAL_STATUSES.includes(incomingStatus);

        const createdAt =
          toTimestamp(task.created_at) ??
          existing?.createdAt ??
          now;

        const updatedAt =
          toTimestamp(task.updated_at) ??
          now;

        const completedAt = terminal
          ? (toTimestamp(task.completed_at) ?? existing?.completedAt ?? now)
          : undefined;

        const isActiveStatus = incomingStatus === 'pending' || incomingStatus === 'running';
        const terminalReason = task.terminal_reason !== undefined
          ? task.terminal_reason
          : terminal ? (existing?.terminalReason ?? null) : null;
        const terminalLabel = task.terminal_label !== undefined
          ? task.terminal_label
          : terminal ? (existing?.terminalLabel ?? null) : null;
        const reviewRequired = typeof task.review_required === 'boolean'
          ? task.review_required
          : incomingStatus === 'failed'
            ? (existing?.reviewRequired ?? false)
            : false;
        const canResume = typeof task.can_resume === 'boolean'
          ? task.can_resume
          : incomingStatus === 'failed' || incomingStatus === 'cancelled';
        const result = task.result !== undefined
          ? task.result
          : isActiveStatus ? null : (existing?.result ?? null);
        const error = task.error !== undefined
          ? task.error
          : isActiveStatus ? null : (existing?.error ?? null);
        const failedChapters = task.failed_chapters !== undefined
          ? task.failed_chapters
          : incomingStatus === 'failed' ? (existing?.failedChapters ?? []) : [];

        const merged: TrackedBackgroundTask = {
          taskId: task.task_id,
          taskType: task.task_type ?? existing?.taskType ?? 'unknown',
          projectId: task.project_id ?? existing?.projectId,
          status: incomingStatus,
          progress: normalizeProgress(task.progress ?? existing?.progress),
          message: task.message ?? existing?.message ?? '',
          result,
          error,
          stageCode: task.stage_code ?? existing?.stageCode,
          executionMode: task.execution_mode ?? existing?.executionMode ?? 'interactive',
          workflowScope: task.workflow_scope ?? existing?.workflowScope,
          checkpoint: task.checkpoint ?? existing?.checkpoint ?? null,
          failedChapters,
          activeStoryRepairPayload: task.active_story_repair_payload ?? existing?.activeStoryRepairPayload ?? null,
          terminalReason: isActiveStatus ? null : terminalReason,
          terminalLabel: isActiveStatus ? null : terminalLabel,
          reviewRequired: isActiveStatus ? false : reviewRequired,
          canResume: isActiveStatus ? false : canResume,
          createdAt,
          updatedAt,
          completedAt: isActiveStatus ? undefined : completedAt,
        };

        const nextTasks = { ...get().tasks, [task.task_id]: merged };
        set({ tasks: compactTasks(nextTasks) });
      },
      removeTask: (taskId) => {
        const next = { ...get().tasks };
        delete next[taskId];
        set({ tasks: next });
      },
      removeTasksByProjectId: (projectId) => {
        if (!projectId) return;
        const next = Object.fromEntries(
          Object.entries(get().tasks).filter(([, task]) => task.projectId !== projectId)
        );
        set({ tasks: next });
      },
      pruneTasksByProjectIds: (projectIds) => {
        const allowed = new Set(projectIds);
        const next = Object.fromEntries(
          Object.entries(get().tasks).filter(([, task]) =>
            !task.projectId || allowed.has(task.projectId)
          )
        );
        set({ tasks: next });
      },
      pruneMissingActiveTasks: (activeTaskIds, scope) => {
        const now = Date.now();
        const activeSet = new Set(activeTaskIds);
        const next = Object.fromEntries(
          Object.entries(get().tasks).filter(([, task]) => {
            if (TERMINAL_STATUSES.includes(task.status)) {
              return true;
            }
            if (!matchesActiveTaskScope(task, scope)) {
              return true;
            }
            if (activeSet.has(task.taskId)) {
              return true;
            }
            if (now - task.updatedAt <= ACTIVE_TASK_GRACE_MS) {
              return true;
            }
            return false;
          })
        );
        set({ tasks: next });
      },
      clearTerminalTasks: () => {
        const active = Object.fromEntries(
          Object.entries(get().tasks).filter(([, task]) => !TERMINAL_STATUSES.includes(task.status))
        );
        set({ tasks: active });
      },
      pruneExpiredTerminalTasks: () => {
        set({ tasks: compactTasks(get().tasks) });
      },
    }),
    {
      name: 'background-task-store',
      partialize: (state) => ({ tasks: state.tasks }),
      onRehydrateStorage: () => (state) => {
        state?.pruneExpiredTerminalTasks();
      },
    }
  )
);

export const isActiveBackgroundTask = (task: TrackedBackgroundTask) =>
  task.status === 'pending' || task.status === 'running';

export const getTaskTypeLabel = (taskType: string): string => {
  const labels: Record<string, string> = {
    chapters_batch_generate: '批量章节生成',
    chapter_single_generate: '单章生成',
    chapter_analysis: '章节分析',
    careers_generate_system: '职业生成',
    character_generate: '角色生成',
    organization_generate: '组织生成',
    world_regenerate: '世界观重建',
    outline_generate: '大纲生成',
    outline_expand: '大纲展开',
    outline_batch_expand: '批量展开',
    wizard_world_building: '向导-世界观',
    wizard_career_system: '向导-职业体系',
    wizard_characters: '向导-角色',
    wizard_outline: '向导-大纲',
  };
  return labels[taskType] ?? taskType;
};
