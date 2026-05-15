import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import type { ActiveStoryRepairPayload, AnalysisTask } from '../../types';

export type BatchTaskRuntimeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
export type ChapterGenerationTaskType = 'chapters_batch_generate' | 'chapter_single_generate';
export type ChapterBatchFailedChapter = Record<string, unknown>;

export type ChapterBatchManualReviewInfo = {
  label: string;
  message: string;
  failedMetrics: string[];
};

const isNonEmptyString = (value: unknown): value is string => (
  typeof value === 'string' && value.trim().length > 0
);

export const getBatchManualReviewInfo = (
  failedChapters?: Array<ChapterBatchFailedChapter> | null,
  fallbackErrorMessage?: string | null,
  terminalReason?: string | null,
  terminalLabel?: string | null,
  reviewRequired?: boolean | null,
  mode: 'batch' | 'single' = 'single',
): ChapterBatchManualReviewInfo | null => {
  const matched = (failedChapters || []).find((item) => {
    if (!item || typeof item !== 'object' || Array.isArray(item)) return false;
    const decision = typeof item.quality_gate_decision === 'string' ? item.quality_gate_decision.trim().toLowerCase() : '';
    return decision === 'manual_review';
  });

  const fallback = String(fallbackErrorMessage || '').trim();
  const isManualReviewTerminal = reviewRequired === true || String(terminalReason || '').trim().toLowerCase() === 'manual_review';
  const defaultLabel = mode === 'batch' ? '已生成，建议优化' : '需人工复核';
  const defaultMessage = mode === 'batch'
    ? '已生成，建议优化：当前内容已生成并保留，可根据质量提示决定是否进一步优化。'
    : '需人工复核：当前候选稿需要人工复核后再决定是否保存。';

  if (!matched && !isManualReviewTerminal) {
    if (!fallback) return null;
    if (!fallback.startsWith('需复核:') && !fallback.toLowerCase().includes('manual review')) return null;
    return {
      label: defaultLabel,
      message: fallback,
      failedMetrics: [],
    };
  }

  const label = isNonEmptyString(terminalLabel)
    ? terminalLabel.trim()
    : matched && isNonEmptyString(matched.quality_gate_label)
      ? matched.quality_gate_label.trim()
      : defaultLabel;
  const message = matched && isNonEmptyString(matched.error)
    ? matched.error.trim()
    : fallback || defaultMessage;
  const failedMetrics = matched && Array.isArray(matched.quality_gate_failed_metrics)
    ? matched.quality_gate_failed_metrics.filter((item): item is string => isNonEmptyString(item))
    : [];

  return {
    label,
    message,
    failedMetrics,
  };
};

export const normalizeChapterTaskStatus = (status: string): BatchTaskRuntimeStatus => {
  if (status === 'running' || status === 'completed' || status === 'failed' || status === 'cancelled') {
    return status;
  }
  return 'pending';
};

const buildChapterGenerateTaskMessage = (
  taskType: ChapterGenerationTaskType,
  status: BatchTaskRuntimeStatus,
  total: number,
  completed: number,
  currentChapterNumber?: number | null,
  errorMessage?: string | null,
  failedChapters?: Array<ChapterBatchFailedChapter> | null,
  terminalReason?: string | null,
  terminalLabel?: string | null,
  reviewRequired?: boolean | null,
) => {
  const taskName = taskType === 'chapter_single_generate'
    ? '单章生成'
    : '批量生成';
  if (status === 'failed') {
    const manualReviewInfo = (taskType === 'chapters_batch_generate' || taskType === 'chapter_single_generate')
      ? getBatchManualReviewInfo(
        failedChapters,
        errorMessage,
        terminalReason,
        terminalLabel,
        reviewRequired,
        taskType === 'chapters_batch_generate' ? 'batch' : 'single',
      )
      : null;
    if (manualReviewInfo) return taskType === 'chapters_batch_generate' ? taskName + '已生成，建议优化' : taskName + '待人工复核';
    return errorMessage || taskName + '失败';
  }
  if (status === 'cancelled') return `${taskName}已取消`;
  if (status === 'completed') return `${taskName}完成 (${completed}/${total})`;
  if (currentChapterNumber) return `${taskName}中：第 ${currentChapterNumber} 章 (${completed}/${total})`;
  if (status === 'running') return `${taskName}中 (${completed}/${total})`;
  return `${taskName}排队中 (${completed}/${total})`;
};

export const upsertChapterTaskToStore = (data: {
  taskType: ChapterGenerationTaskType;
  taskId: string;
  status: string;
  total: number;
  completed: number;
  projectId?: string;
  currentChapterNumber?: number | null;
  errorMessage?: string | null;
  stageCode?: string | null;
  executionMode?: 'interactive' | 'auto' | null;
  checkpoint?: Record<string, unknown> | null;
  failedChapters?: Array<ChapterBatchFailedChapter> | null;
  activeStoryRepairPayload?: ActiveStoryRepairPayload | null;
  terminalReason?: string | null;
  terminalLabel?: string | null;
  reviewRequired?: boolean | null;
  canResume?: boolean | null;
  createdAt?: string | null;
  completedAt?: string | null;
}) => {
  const normalizedStatus = normalizeChapterTaskStatus(data.status);
  const checkpointProgressRaw = data.checkpoint && typeof data.checkpoint.progress === 'number'
    ? data.checkpoint.progress
    : null;
  const checkpointProgress = checkpointProgressRaw !== null
    ? Math.max(0, Math.min(100, Math.round(checkpointProgressRaw)))
    : null;
  const derivedProgress = data.total > 0 ? Math.round((data.completed / data.total) * 100) : 0;
  const progress = checkpointProgress ?? derivedProgress;
  const now = new Date().toISOString();
  useBackgroundTaskStore.getState().upsertTask({
    task_id: data.taskId,
    task_type: data.taskType,
    project_id: data.projectId,
    status: normalizedStatus,
    progress,
    message: buildChapterGenerateTaskMessage(
      data.taskType,
      normalizedStatus,
      data.total,
      data.completed,
      data.currentChapterNumber,
      data.errorMessage,
      data.failedChapters ?? null,
      data.terminalReason,
      data.terminalLabel,
      data.reviewRequired,
    ),
    error: data.errorMessage ?? null,
    stage_code: data.stageCode ?? undefined,
    execution_mode: data.executionMode ?? undefined,
    checkpoint: data.checkpoint ?? undefined,
    failed_chapters: data.failedChapters ?? undefined,
    active_story_repair_payload: data.activeStoryRepairPayload ?? undefined,
    terminal_reason: data.terminalReason,
    terminal_label: data.terminalLabel,
    review_required: data.reviewRequired,
    can_resume: data.canResume,
    created_at: data.createdAt ?? now,
    updated_at: now,
    completed_at: data.completedAt ?? null,
  });
};

export const upsertChapterAnalysisTaskToStore = (
  task: AnalysisTask,
  projectId?: string,
  messageOverride?: string
) => {
  if (!task?.has_task || !task.task_id || task.status === 'none') return;
  const status = task.status === 'running' || task.status === 'completed' || task.status === 'failed'
    ? task.status
    : 'pending';
  const messageText =
    messageOverride ??
    (status === 'completed'
      ? '章节分析已完成'
      : status === 'failed'
        ? (task.error_message || '章节分析失败')
        : `章节分析进行中 (${task.progress ?? 0}%)`);

  useBackgroundTaskStore.getState().upsertTask({
    task_id: task.task_id,
    task_type: 'chapter_analysis',
    project_id: projectId,
    status,
    progress: task.progress ?? 0,
    message: messageText,
    error: task.error_message ?? null,
    stage_code: 'analysis',
    execution_mode: 'interactive',
    checkpoint: { chapter_id: task.chapter_id },
    created_at: task.created_at ?? undefined,
    updated_at: new Date().toISOString(),
    completed_at: task.completed_at ?? undefined,
  });
};
