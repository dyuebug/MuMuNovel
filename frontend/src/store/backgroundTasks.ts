import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type BackgroundTaskRuntimeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface TrackedBackgroundTask {
  taskId: string;
  taskType: string;
  projectId?: string;
  status: BackgroundTaskRuntimeStatus;
  progress: number;
  message: string;
  error?: string | null;
  createdAt: number;
  updatedAt: number;
  completedAt?: number;
}

interface UpsertTaskPayload {
  task_id: string;
  task_type?: string;
  project_id?: string;
  status?: BackgroundTaskRuntimeStatus;
  progress?: number;
  message?: string;
  error?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
  completed_at?: string | null;
}

interface BackgroundTaskState {
  tasks: Record<string, TrackedBackgroundTask>;
  upsertTask: (task: UpsertTaskPayload) => void;
  removeTask: (taskId: string) => void;
  clearTerminalTasks: () => void;
}

const TERMINAL_STATUSES: BackgroundTaskRuntimeStatus[] = ['completed', 'failed', 'cancelled'];

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

        const merged: TrackedBackgroundTask = {
          taskId: task.task_id,
          taskType: task.task_type ?? existing?.taskType ?? 'unknown',
          projectId: task.project_id ?? existing?.projectId,
          status: incomingStatus,
          progress: normalizeProgress(task.progress ?? existing?.progress),
          message: task.message ?? existing?.message ?? '',
          error: task.error ?? existing?.error ?? null,
          createdAt,
          updatedAt,
          completedAt,
        };

        const nextTasks = { ...get().tasks, [task.task_id]: merged };

        // 控制本地缓存规模，避免历史任务无限累积。
        const allTasks = Object.values(nextTasks).sort((a, b) => b.updatedAt - a.updatedAt);
        const keep = new Set(allTasks.slice(0, 30).map((item) => item.taskId));
        const compacted = Object.fromEntries(
          Object.entries(nextTasks).filter(([taskId]) => keep.has(taskId))
        );

        set({ tasks: compacted });
      },
      removeTask: (taskId) => {
        const next = { ...get().tasks };
        delete next[taskId];
        set({ tasks: next });
      },
      clearTerminalTasks: () => {
        const active = Object.fromEntries(
          Object.entries(get().tasks).filter(([, task]) => !TERMINAL_STATUSES.includes(task.status))
        );
        set({ tasks: active });
      },
    }),
    {
      name: 'background-task-store',
      partialize: (state) => ({ tasks: state.tasks }),
    }
  )
);

export const isActiveBackgroundTask = (task: TrackedBackgroundTask) =>
  task.status === 'pending' || task.status === 'running';

export const getTaskTypeLabel = (taskType: string): string => {
  const labels: Record<string, string> = {
    chapters_batch_generate: '批量章节生成',
    chapter_single_generate: '单章生成',
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
