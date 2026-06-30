import type { BackgroundTaskRuntimeStatus, BackgroundTaskStatus } from '../services/modules/backgroundTaskTypes';
import type { ActiveStoryRepairPayload } from '../types';

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

export type UpsertTaskPayload = Partial<BackgroundTaskStatus> & {
  task_id: string;
  failed_chapters?: Array<Record<string, unknown>>;
};

export type ActiveTaskScope = 'background' | 'chapter_batch';

export const TERMINAL_STATUSES: BackgroundTaskRuntimeStatus[] = ['completed', 'failed', 'cancelled'];
export const ACTIVE_TASK_GRACE_MS = 1000 * 60;

const TERMINAL_TASK_RETENTION_MS = 1000 * 60 * 60 * 12;
const MAX_PERSISTED_TASKS = 30;
const MAX_TERMINAL_TASKS = 12;

const TASK_TYPE_LABELS: Record<string, string> = {
  chapters_batch_generate: '批量章节生成',
  chapter_single_generate: '单章生成',
  chapter_analysis: '章节分析',
  chapter_regenerate: '章节重生成',
  chapter_partial_regenerate: '章节局部重写',
  book_import_apply: '拆书导入执行',
  book_import_retry_failed_steps: '拆书失败步骤重试',
  polish_text: 'AI 去味',
  polish_batch: '批量 AI 去味',
  inspiration_generate_options: '灵感选项生成',
  inspiration_refine_options: '灵感选项优化',
  inspiration_quick_generate: '灵感快速补全',
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

const isChapterManagedTask = (taskType: string) =>
  taskType === 'chapters_batch_generate'
  || taskType === 'chapter_single_generate'
  || taskType === 'chapter_analysis'
  || taskType === 'chapter_regenerate'
  || taskType === 'chapter_partial_regenerate';

export const matchesActiveTaskScope = (task: TrackedBackgroundTask, scope?: ActiveTaskScope) => {
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

export const compactTasks = (tasks: Record<string, TrackedBackgroundTask>): Record<string, TrackedBackgroundTask> => {
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

export const mergeTrackedBackgroundTask = (
  task: UpsertTaskPayload,
  existing?: TrackedBackgroundTask,
  now = Date.now(),
): TrackedBackgroundTask => {
  const incomingStatus = task.status ?? existing?.status ?? 'pending';
  const terminal = TERMINAL_STATUSES.includes(incomingStatus);

  const createdAt = toTimestamp(task.created_at) ?? existing?.createdAt ?? now;
  const updatedAt = toTimestamp(task.updated_at) ?? now;
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
  const isManualReviewTerminal =
    reviewRequired || String(terminalReason ?? '').trim().toLowerCase() === 'manual_review';
  const canResume = typeof task.can_resume === 'boolean'
    ? task.can_resume
    : incomingStatus === 'cancelled' || (incomingStatus === 'failed' && !isManualReviewTerminal);
  const result = task.result !== undefined
    ? task.result
    : isActiveStatus ? null : (existing?.result ?? null);
  const error = task.error !== undefined
    ? task.error
    : isActiveStatus ? null : (existing?.error ?? null);
  const failedChapters = task.failed_chapters !== undefined
    ? task.failed_chapters
    : incomingStatus === 'failed' ? (existing?.failedChapters ?? []) : [];

  return {
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
};

export const isActiveBackgroundTask = (task: TrackedBackgroundTask) =>
  task.status === 'pending' || task.status === 'running';

export const getTaskTypeLabel = (taskType: string): string =>
  TASK_TYPE_LABELS[taskType] ?? taskType;
