import { getBatchManualReviewInfo } from '../services/modularApi';
import { getTaskTypeLabel, type TrackedBackgroundTask } from '../store/backgroundTasks';

const statusMeta: Record<TrackedBackgroundTask['status'], { color: string; label: string }> = {
  pending: { color: 'default', label: '排队中' },
  running: { color: 'processing', label: '执行中' },
  completed: { color: 'success', label: '已完成' },
  failed: { color: 'error', label: '失败' },
  cancelled: { color: 'warning', label: '已取消' },
};

export const getTaskStatusMeta = (task: TrackedBackgroundTask): { color: string; label: string } => {
  if (task.status === 'failed' && task.reviewRequired) {
    return {
      color: 'warning',
      label: task.terminalLabel || '需人工复核',
    };
  }
  return statusMeta[task.status];
};

export const terminalStatuses = new Set<TrackedBackgroundTask['status']>(['completed', 'failed', 'cancelled']);

export const isTaskManualReviewTerminal = (task: TrackedBackgroundTask) =>
  task.reviewRequired || String(task.terminalReason ?? '').trim().toLowerCase() === 'manual_review';

export const isTaskResumable = (task: TrackedBackgroundTask) => {
  if (task.taskType !== 'chapters_batch_generate' && task.taskType !== 'chapter_single_generate') {
    return false;
  }

  return typeof task.canResume === 'boolean'
    ? task.canResume
    : task.status === 'cancelled' || (task.status === 'failed' && !isTaskManualReviewTerminal(task));
};


export type FailureReasonTag = {
  label: string;
  color: string;
};

type TaskCheckpointCompactionDetail = {
  before?: number | null;
  after?: number | null;
};

type TaskCheckpoint = {
  current_chapter_number?: number | null;
  candidate_index?: number | null;
  candidate_count?: number | null;
  word_count?: number | null;
  generation_path?: string | null;
  attempt_kind?: string | null;
  rerank_used?: boolean | null;
  word_budget_repair_used?: boolean | null;
  winner_candidate_index?: number | null;
  pre_compaction_total_length?: number | null;
  context_budget_limit?: number | null;
  compaction_applied?: boolean | null;
  compaction_details?: Record<string, TaskCheckpointCompactionDetail> | null;
};

const isRecord = (value: unknown): value is Record<string, unknown> => (
  Boolean(value) && typeof value === 'object' && !Array.isArray(value)
);

const toFiniteNumber = (value: unknown): number | null => {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
};

const toNullableBoolean = (value: unknown): boolean | null => (
  typeof value === 'boolean' ? value : null
);

const normalizeCompactionDetails = (
  value: unknown,
): Record<string, TaskCheckpointCompactionDetail> | null => {
  if (!isRecord(value)) return null;

  const entries = Object.entries(value).reduce<Record<string, TaskCheckpointCompactionDetail>>((acc, [key, detail]) => {
    if (!isRecord(detail)) return acc;
    acc[key] = {
      before: toFiniteNumber(detail.before),
      after: toFiniteNumber(detail.after),
    };
    return acc;
  }, {});

  return entries;
};

const contextCompactionFieldLabels: Record<string, string> = {
  recent_chapters_context: '最近章节规划',
  chapter_careers: '职业体系',
  foreshadow_reminders: '伏笔提醒',
  relevant_memories: '相关记忆',
  chapter_characters: '角色信息',
  character_arc_snapshot: '角色弧光',
  continuation_point: '衔接锚点',
  previous_chapter_summary: '上章摘要',
};

const getCompactionFieldNames = (checkpoint?: TaskCheckpoint | null): string[] => {
  if (!checkpoint?.compaction_details) return [];
  return Object.keys(checkpoint.compaction_details)
    .map((fieldName) => contextCompactionFieldLabels[fieldName] ?? fieldName)
    .filter(Boolean);
};

const getCompactionAfterLength = (checkpoint?: TaskCheckpoint | null): number | null => {
  const before = checkpoint?.pre_compaction_total_length;
  if (typeof before !== 'number') return null;
  if (!checkpoint?.compaction_applied) return before;
  if (!checkpoint.compaction_details) return null;

  let saved = 0;
  Object.values(checkpoint.compaction_details).forEach((detail) => {
    if (typeof detail.before === 'number' && typeof detail.after === 'number') {
      saved += Math.max(detail.before - detail.after, 0);
    }
  });
  return Math.max(before - saved, 0);
};

const getCompactionSummary = (checkpoint?: TaskCheckpoint | null): string | null => {
  if (!checkpoint?.compaction_applied) return null;

  const before = checkpoint.pre_compaction_total_length;
  const after = getCompactionAfterLength(checkpoint);
  const limit = checkpoint.context_budget_limit;
  const fieldNames = getCompactionFieldNames(checkpoint).slice(0, 3);
  const fieldLabel = fieldNames.length > 0 ? `?${fieldNames.join('?')}?` : '';

  if (typeof before === 'number' && typeof after === 'number' && typeof limit === 'number') {
    return `上下文压缩 ${before}→${after} / ${limit}${fieldLabel}`;
  }
  if (typeof before === 'number' && typeof after === 'number') {
    return `上下文压缩 ${before}→${after}${fieldLabel}`;
  }
  return fieldLabel ? `已压缩${fieldLabel}` : '已压缩';
};

const getGenerationPathLabel = (value?: string | null): string => {
  switch (value) {
    case 'single_pass':
      return '单轮直出';
    case 'rerank_retry':
      return '重排复选';
    case 'word_budget_repair':
      return '字数修复';
    default:
      return value ? value : '';
  }
};

const getAttemptKindLabel = (value?: string | null): string => {
  switch (value) {
    case 'initial_candidate':
      return '初始候选';
    case 'rerank_candidate':
      return '重排候选';
    case 'word_budget_repair':
      return '字数修复';
    default:
      return value ? value : '';
  }
};

const getTaskCheckpoint = (task: TrackedBackgroundTask): TaskCheckpoint | null => {
  if (!isRecord(task.checkpoint)) return null;
  return {
    current_chapter_number: toFiniteNumber(task.checkpoint.current_chapter_number),
    candidate_index: toFiniteNumber(task.checkpoint.candidate_index),
    candidate_count: toFiniteNumber(task.checkpoint.candidate_count),
    word_count: toFiniteNumber(task.checkpoint.word_count),
    generation_path: typeof task.checkpoint.generation_path === 'string' ? task.checkpoint.generation_path : null,
    attempt_kind: typeof task.checkpoint.attempt_kind === 'string' ? task.checkpoint.attempt_kind : null,
    rerank_used: toNullableBoolean(task.checkpoint.rerank_used),
    word_budget_repair_used: toNullableBoolean(task.checkpoint.word_budget_repair_used),
    winner_candidate_index: toFiniteNumber(task.checkpoint.winner_candidate_index),
    pre_compaction_total_length: toFiniteNumber(task.checkpoint.pre_compaction_total_length),
    context_budget_limit: toFiniteNumber(task.checkpoint.context_budget_limit),
    compaction_applied: toNullableBoolean(task.checkpoint.compaction_applied),
    compaction_details: normalizeCompactionDetails(task.checkpoint.compaction_details),
  };
};

export const getTaskCheckpointSummary = (task: TrackedBackgroundTask): string | null => {
  const checkpoint = getTaskCheckpoint(task);
  if (!checkpoint) return null;

  const parts: string[] = [];
  if (typeof checkpoint.current_chapter_number === 'number') {
    parts.push(`第 ${checkpoint.current_chapter_number} 章`);
  }
  if (typeof checkpoint.candidate_index === 'number' && typeof checkpoint.candidate_count === 'number') {
    parts.push(`候选 ${checkpoint.candidate_index}/${checkpoint.candidate_count}`);
  }
  if (typeof checkpoint.word_count === 'number' && checkpoint.word_count > 0) {
    parts.push(`${checkpoint.word_count} 字`);
  }
  const compactionSummary = getCompactionSummary(checkpoint);
  if (compactionSummary) {
    parts.push(compactionSummary);
  }
  return parts.length > 0 ? `检查点：${parts.join(' · ')}` : null;
};

export const getTaskCheckpointTags = (task: TrackedBackgroundTask): FailureReasonTag[] => {
  const checkpoint = getTaskCheckpoint(task);
  if (!checkpoint) return [];

  const tags: FailureReasonTag[] = [];
  const pushTag = (label: string, color: string) => {
    if (!label || tags.some((tag) => tag.label === label)) return;
    tags.push({ label, color });
  };

  const generationPathLabel = getGenerationPathLabel(checkpoint.generation_path);
  if (generationPathLabel) pushTag(generationPathLabel, 'blue');
  const attemptKindLabel = getAttemptKindLabel(checkpoint.attempt_kind);
  if (attemptKindLabel && attemptKindLabel !== generationPathLabel) pushTag(attemptKindLabel, 'purple');
  if (checkpoint.rerank_used) pushTag('启用重排', 'geekblue');
  if (checkpoint.word_budget_repair_used) pushTag('字数修复', 'orange');
  if (checkpoint.compaction_applied) pushTag('上下文压缩', 'gold');
  if (typeof checkpoint.winner_candidate_index === 'number') {
    pushTag(`胜出候选 ${checkpoint.winner_candidate_index}`, 'green');
  }
  return tags;
};


export const getTaskDestination = (task: TrackedBackgroundTask): string | null => {
  if (!task.projectId) {
    if (task.taskType.startsWith('wizard_')) return '/wizard';
    if (task.taskType.startsWith('inspiration_')) return `/inspiration?task_id=${encodeURIComponent(task.taskId)}`;
    if (task.taskType.startsWith('book_import_')) return '/projects?view=book-import';
    if (task.taskType.startsWith('polish_')) return '/projects';
    return null;
  }

  switch (task.taskType) {
    case 'careers_generate_system':
    case 'wizard_career_system':
      return `/project/${task.projectId}/careers`;
    case 'character_generate':
    case 'wizard_characters':
      return `/project/${task.projectId}/characters`;
    case 'organization_generate':
      return `/project/${task.projectId}/organizations`;
    case 'world_regenerate':
    case 'wizard_world_building':
      return `/project/${task.projectId}/world-setting`;
    case 'outline_generate':
    case 'outline_expand':
    case 'outline_batch_expand':
    case 'wizard_outline':
      return `/project/${task.projectId}/outline`;
    case 'book_import_apply':
    case 'book_import_retry_failed_steps':
      return `/project/${task.projectId}/chapters`;
    case 'polish_text':
    case 'polish_batch':
      return `/project/${task.projectId}`;
    case 'chapters_batch_generate':
    case 'chapter_single_generate':
    case 'chapter_analysis':
    case 'chapter_regenerate':
    case 'chapter_partial_regenerate':
      return `/project/${task.projectId}/chapters`;
    default:
      return `/project/${task.projectId}`;
  }
};

export const getCompletionNotice = (task: TrackedBackgroundTask): { title: string; description: string } => {
  const taskLabel = getTaskTypeLabel(task.taskType);
  if (task.status === 'completed') {
    return {
      title: `${taskLabel}已完成`,
      description: task.message || '后台任务执行完成',
    };
  }
  if (task.status === 'failed') {
    return {
      title: `${taskLabel}执行失败`,
      description: task.error || task.message || '后台任务执行失败',
    };
  }
  return {
    title: `${taskLabel}已取消`,
    description: task.message || '后台任务已取消',
  };
};

export const getTaskDisplayMessage = (task: TrackedBackgroundTask): string => {
  if (task.taskType !== 'chapter_analysis') {
    return task.message || '任务执行中...';
  }

  if (task.status === 'completed') return '章节分析已完成';
  if (task.status === 'failed') return task.error || '章节分析失败';
  if (task.status === 'cancelled') return '章节分析已取消';
  return `章节分析进行中 (${task.progress}%)`;
};

export const extractFailureReasonTags = (task: TrackedBackgroundTask): FailureReasonTag[] => {
  const source = `${task.error ?? ''} ${task.message ?? ''}`.toLowerCase();
  const tags: FailureReasonTag[] = [];
  const manualReviewInfo = (task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate')
    ? getBatchManualReviewInfo(
      task.failedChapters,
      task.error,
      task.terminalReason,
      task.terminalLabel,
      task.reviewRequired,
    )
    : null;

  const pushTag = (label: string, color: string) => {
    if (!tags.some((tag) => tag.label === label)) {
      tags.push({ label, color });
    }
  };

  if (manualReviewInfo) {
    pushTag(manualReviewInfo.label, 'gold');
    if (manualReviewInfo.failedMetrics.length > 0) {
      pushTag('质量门禁拦截', 'orange');
    }
  }

  if (!source.trim()) {
    return tags.length > 0 ? tags.slice(0, 2) : [{ label: '未知原因', color: 'default' }];
  }

  if (
    source.includes('timeout') ||
    source.includes('time out') ||
    source.includes('timed out') ||
    source.includes('超时') ||
    source.includes('deadline exceeded')
  ) {
    pushTag('超时', 'gold');
  }

  if (
    source.includes('401') ||
    source.includes('403') ||
    source.includes('unauthorized') ||
    source.includes('forbidden') ||
    source.includes('permission') ||
    source.includes('权限') ||
    source.includes('认证') ||
    source.includes('token') ||
    source.includes('apikey') ||
    source.includes('api key')
  ) {
    pushTag('权限错误', 'red');
  }

  if (
    source.includes('429') ||
    source.includes('rate limit') ||
    source.includes('quota') ||
    source.includes('配额') ||
    source.includes('限流') ||
    source.includes('余额不足') ||
    source.includes('too many requests') ||
    source.includes('insufficient_quota')
  ) {
    pushTag('限流/配额', 'volcano');
  }

  if (
    source.includes('network') ||
    source.includes('socket') ||
    source.includes('connection') ||
    source.includes('connect') ||
    source.includes('econn') ||
    source.includes('dns') ||
    source.includes('网络') ||
    source.includes('连接')
  ) {
    pushTag('网络异常', 'cyan');
  }

  if (
    source.includes('model') ||
    source.includes('模型') ||
    source.includes('provider') ||
    source.includes('completion') ||
    source.includes('llm')
  ) {
    pushTag('模型错误', 'purple');
  }

  if (
    source.includes('context length') ||
    source.includes('maximum context') ||
    source.includes('too long') ||
    source.includes('length') ||
    source.includes('上下文') ||
    source.includes('长度超限') ||
    source.includes('token limit')
  ) {
    pushTag('上下文过长', 'magenta');
  }

  if (
    source.includes('invalid') ||
    source.includes('validation') ||
    source.includes('missing') ||
    source.includes('required') ||
    source.includes('参数') ||
    source.includes('格式') ||
    source.includes('校验')
  ) {
    pushTag('参数问题', 'orange');
  }

  if (tags.length === 0) {
    pushTag('未知原因', 'default');
  }

  return tags.slice(0, 2);
};


export const formatRelativeTime = (timestamp: number): string => {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return '刚刚更新';
  if (diff < 3_600_000) return `${Math.max(1, Math.floor(diff / 60_000))} 分钟前更新`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前更新`;
  return `${Math.floor(diff / 86_400_000)} 天前更新`;
};
