import { Suspense, lazy, useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useLocation } from 'react-router-dom';

import { Button, Modal, Form, message, Space, Tag, Card, Typography, Row, Col, Divider, theme } from 'antd';

import { DownloadOutlined, RocketOutlined, BookOutlined, PlusOutlined } from '@ant-design/icons';
import { useShallow } from 'zustand/react/shallow';

import { useStore } from '../store';
import { useBackgroundTaskStore } from '../store/backgroundTasks';
import { useChapterAnalysisUiStore } from '../store/chapterAnalysisUi';
import { useChapterGenerationUiStore } from '../store/chapterGenerationUi';
import { useChapterSync } from '../store/hooks';
import type { Chapter, ChapterUpdate, WritingStyle, AnalysisTask, ExpansionPlanData, ChapterLatestQualityMetrics, ChapterQualityMetrics, ChapterQualityMetricsSummary, ChapterQualityProfileSummary, ActiveStoryRepairPayload, CreativeMode, PlotStage, QualityPreset, StoryFocus } from '../types';
import type { ChapterBatchGenerateModalProps } from '../components/ChapterBatchGenerateModal';
import ChapterAnalysisEntry from '../components/ChapterAnalysisEntry';
import ChapterBasicModalEntry from '../components/ChapterBasicModalEntry';
import ChapterBatchGenerateModalEntry from '../components/ChapterBatchGenerateModalEntry';
import ChapterBatchProgressEntry from '../components/ChapterBatchProgressEntry';
import ChapterListSection from '../components/ChapterListSection';
import ChapterPlanEditorEntry from '../components/ChapterPlanEditorEntry';
import ChapterReaderEntry from '../components/ChapterReaderEntry';
import FloatingIndexPanelEntry from '../components/FloatingIndexPanelEntry';
import SingleChapterGenerationOverlayEntry from '../components/SingleChapterGenerationOverlayEntry';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';

import {
  type CreationPresetId,
  type StoryAcceptanceCard,
  type StoryCharacterArcCard,
  type StoryCreationControlCard,
  type StoryExecutionChecklist,
  type StoryObjectiveCard,
  type StoryRepairPromptPayload,
  type StoryRepairTargetCard,
  type StoryRepetitionRiskCard,
  type StoryResultCard,
} from '../utils/creationPresetsCore';
import {
  getCachedWordCount,
} from '../utils/storyCreationWordCount';
import {
  buildStorySceneOutlineSuggestion,
  STORY_CREATION_PROMPT_WARN_THRESHOLD,
} from '../utils/storyCreationPrompt';
import {
  CREATIVE_MODE_OPTIONS,
  STORY_FOCUS_OPTIONS,
} from '../utils/generationPreferenceOptions';
import {
  EMPTY_STORY_BEAT_PLANNER_DRAFT,
  EMPTY_STORY_SCENE_OUTLINE_DRAFT,
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshot,
  type StoryCreationSnapshotReason,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import {
  startSingleChapterGenerationWorkflow,
} from './chapterSingleGenerationHelpers';
import type { BatchGenerateFormValues } from './chapterBatchGenerationRequestHelpers';
import {
  cancelBatchGenerationWorkflow,
  openBatchGenerationWorkflow,
  startBatchGenerationWorkflow,
} from './chapterBatchGenerationWorkflowHelpers';
import type { BatchTaskMeta } from './chapterBatchGenerationPollingHelpers';
import {
  restoreBatchGenerationWorkflow,
  startBatchPollingWorkflow,
} from './chapterBatchGenerationCoordinationHelpers';
import {
  getPersistedChapterBatchTaskMeta,
  persistChapterBatchTaskMeta,
  removePersistedChapterBatchTaskMeta,
} from './chapterBatchTaskMetaStorageHelpers';
import {
  closeAnalysisWorkflow,
  loadAnalysisTasksWorkflow,
  refreshAnalysisTaskWorkflow,
  startAnalysisPollingTaskWorkflow,
} from './chapterAnalysisTaskCoordinationHelpers';
import {
  applyChapterAnalysisPollingState,
  ensureChapterAnalysisPolling,
  pollChapterAnalysisTasksBatch,
  stopChapterAnalysisPolling,
  syncChapterAnalysisTasksFromBatch,
} from './chapterAnalysisTaskPollingHelpers';
import {
  deleteChapterWithRefreshWorkflow,
  initializeChapterProjectWorkflow,
  reloadChapterProjectWorkflow,
} from './chapterProjectCoordinationHelpers';
import { queueDeferredBatchAnalysis } from './chapterDeferredBatchAnalysisHelpers';
import {
  confirmChapterExportWorkflow,
  openExpansionPlanPreviewWorkflow,
  openManualCreateChapterWorkflow,
  openSingleChapterGenerateWorkflow,
} from './chapterActionDialogCoordinationHelpers';
import {
  loadChapterWritingStyles,
  type ChapterWritingStylesCacheEntry,
} from './chapterWritingStyleLoadHelpers';
import {
  loadChapterAvailableModels,
  type ModelOption,
} from './chapterModelLoadHelpers';
import { syncStoryCreationAutoDrafts } from './chapterStoryCreationAutoSyncHelpers';
import {
  persistBatchStoryCreationDraftWorkflow,
  persistSingleStoryCreationDraftWorkflow,
  restoreBatchStoryCreationPersistenceWorkflow,
  restoreSingleStoryCreationPersistenceWorkflow,
} from './chapterStoryCreationPersistenceCoordinationHelpers';
import {
  copyStoryCreationPrompt,
  resolveStoryCreationPromptState,
} from './chapterStoryCreationPromptHelpers';
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Paragraph, Text } = Typography;
import { buildStoryCreationDerivedState } from './chapterStoryCreationDerivedStateHelpers';
import {
  applyBatchStoryCreationSnapshotWorkflow,
  applySingleStoryCreationSnapshotWorkflow,
  deleteBatchStoryCreationSnapshotWorkflow,
  deleteSingleStoryCreationSnapshotWorkflow,
  saveBatchStoryCreationSnapshotWorkflow,
  saveSingleStoryCreationSnapshotWorkflow,
} from './chapterStoryCreationSnapshotWorkflowHelpers';
import { openChapterEditorWorkflow } from './chapterEditorOpenHelpers';
import { openChapterModalWorkflow } from './chapterModalOpenHelpers';
import { submitChapterModalWorkflow } from './chapterModalSubmitHelpers';
import {
  closeChapterEditor,
  submitChapterEditorWorkflow,
} from './chapterEditorLifecycleHelpers';
import {
  closeChapterReader,
  loadReaderChapter,
  openChapterReader,
} from './chapterReaderLifecycleHelpers';
import { buildChapterReaderModalState } from './chapterReaderModalHelpers';
import {
  closeChapterPlanEditor,
  openChapterPlanEditor,
  saveChapterPlan,
} from './chapterPlanEditorLifecycleHelpers';
import { buildChapterPlanEditorData } from './chapterPlanEditorDataHelpers';
import { useFloatingIndexPanelBindings } from '../hooks/useFloatingIndexPanelBindings';
import { selectChapterListItem } from './chapterSelectionHelpers';
import { buildChapterPlanEditorModalState } from './chapterPlanEditorModalHelpers';



type SingleStoryPresetState = {
  singleStoryAcceptanceCard?: StoryAcceptanceCard;
  singleStoryCharacterArcCard?: StoryCharacterArcCard;
  singleStoryCreationControlCard?: StoryCreationControlCard;
  singleStoryExecutionChecklist?: StoryExecutionChecklist;
  singleStoryObjectiveCard?: StoryObjectiveCard;
  singleStoryRepairPayload?: StoryRepairPromptPayload;
  singleStoryRepairTargetCard?: StoryRepairTargetCard;
  singleStoryRepetitionRiskCard?: StoryRepetitionRiskCard;
  singleStoryResultCard?: StoryResultCard;
};

const EMPTY_SINGLE_STORY_PRESET_STATE: SingleStoryPresetState = {};

type GroupedChapterViewModel = {
  key: string;
  outlineId: string | null;
  outlineTitle: string;
  outlineOrder: number;
  chapters: Chapter[];
  totalWordCount: number;
};

const areChapterReferenceArraysEqual = (left: Chapter[], right: Chapter[]) => (
  left.length === right.length
  && left.every((chapter, index) => chapter === right[index])
);

const mergeChapterArrayPreservingReference = (
  previousChapters: Chapter[],
  nextChapters: Chapter[],
): Chapter[] => (
  areChapterReferenceArraysEqual(previousChapters, nextChapters)
    ? previousChapters
    : nextChapters
);

const mergeGroupedChaptersPreservingReferences = (
  previousGroups: GroupedChapterViewModel[],
  nextGroups: GroupedChapterViewModel[],
): GroupedChapterViewModel[] => {
  if (nextGroups.length === 0) {
    return previousGroups.length === 0 ? previousGroups : nextGroups;
  }

  const previousGroupMap = new Map(previousGroups.map((group) => [group.key, group]));
  const mergedGroups = nextGroups.map((group) => {
    const previousGroup = previousGroupMap.get(group.key);
    if (!previousGroup) {
      return group;
    }

    if (
      previousGroup.outlineId === group.outlineId
      && previousGroup.outlineTitle === group.outlineTitle
      && previousGroup.outlineOrder === group.outlineOrder
      && previousGroup.totalWordCount === group.totalWordCount
      && areChapterReferenceArraysEqual(previousGroup.chapters, group.chapters)
    ) {
      return previousGroup;
    }

    return group;
  });

  if (
    previousGroups.length === mergedGroups.length
    && previousGroups.every((group, index) => group === mergedGroups[index])
  ) {
    return previousGroups;
  }

  return mergedGroups;
};

const LazyChapterEditorModalContent = lazy(() => import('../components/ChapterEditorModalContent'));

const loadStoryCreationPersistence = () => import('../utils/storyCreationPersistence');
const isAnalysisTaskInProgress = (task?: AnalysisTask | null): boolean => (
  task?.status === 'pending' || task?.status === 'running'
);

type BatchGenerationCheckpointCompactionDetail = {
  before?: number | null;
  after?: number | null;
};

type BatchGenerationCheckpoint = {
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
  compaction_details?: Record<string, BatchGenerationCheckpointCompactionDetail> | null;
};

const isObjectRecord = (value: unknown): value is Record<string, unknown> => (
  Boolean(value) && typeof value === 'object' && !Array.isArray(value)
);

const toOptionalNumber = (value: unknown): number | null => {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
};

const toOptionalBoolean = (value: unknown): boolean | null => (
  typeof value === 'boolean' ? value : null
);

const normalizeBatchGenerationCompactionDetails = (
  value: unknown,
): Record<string, BatchGenerationCheckpointCompactionDetail> | null => {
  if (!isObjectRecord(value)) return null;

  const entries = Object.entries(value).reduce<Record<string, BatchGenerationCheckpointCompactionDetail>>((acc, [key, detail]) => {
    if (!isObjectRecord(detail)) return acc;
    acc[key] = {
      before: toOptionalNumber(detail.before),
      after: toOptionalNumber(detail.after),
    };
    return acc;
  }, {});

  return entries;
};

const batchContextCompactionFieldLabels: Record<string, string> = {
  recent_chapters_context: 'Recent chapters context',
  chapter_careers: 'Chapter careers',
  foreshadow_reminders: 'Foreshadow reminders',
  relevant_memories: 'Relevant memories',
  chapter_characters: 'Chapter characters',
  character_arc_snapshot: 'Character arc snapshot',
  continuation_point: 'Continuation point',
  previous_chapter_summary: 'Previous chapter summary',
};

const getBatchCompactionFieldNames = (checkpoint?: BatchGenerationCheckpoint | null): string[] => {
  if (!checkpoint?.compaction_details) return [];
  return Object.keys(checkpoint.compaction_details)
    .map((fieldName) => batchContextCompactionFieldLabels[fieldName] ?? fieldName)
    .filter(Boolean);
};

const getBatchCompactionAfterLength = (checkpoint?: BatchGenerationCheckpoint | null): number | null => {
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

const buildBatchCompactionHint = (checkpoint?: BatchGenerationCheckpoint | null): string => {
  if (!checkpoint?.compaction_applied) return '';

  const before = checkpoint.pre_compaction_total_length;
  const after = getBatchCompactionAfterLength(checkpoint);
  const limit = checkpoint.context_budget_limit;
  const fieldNames = getBatchCompactionFieldNames(checkpoint).slice(0, 3);
  const fieldLabel = fieldNames.length > 0 ? `[${fieldNames.join(' ')}]` : '';

  if (typeof before === 'number' && typeof after === 'number' && typeof limit === 'number') {
    return `Context compacted: ${before} -> ${after}/${limit}${fieldLabel}`;
  }
  if (typeof before === 'number' && typeof after === 'number') {
    return `Context compacted: ${before} -> ${after}${fieldLabel}`;
  }
  return fieldLabel ? `Context compacted fields ${fieldLabel}` : 'Context compacted fields';
};

const normalizeBatchGenerationCheckpoint = (value: unknown): BatchGenerationCheckpoint | null => {
  if (!isObjectRecord(value)) return null;
  return {
    current_chapter_number: toOptionalNumber(value.current_chapter_number),
    candidate_index: toOptionalNumber(value.candidate_index),
    candidate_count: toOptionalNumber(value.candidate_count),
    word_count: toOptionalNumber(value.word_count),
    generation_path: typeof value.generation_path === 'string' ? value.generation_path : null,
    attempt_kind: typeof value.attempt_kind === 'string' ? value.attempt_kind : null,
    rerank_used: toOptionalBoolean(value.rerank_used),
    word_budget_repair_used: toOptionalBoolean(value.word_budget_repair_used),
    winner_candidate_index: toOptionalNumber(value.winner_candidate_index),
    pre_compaction_total_length: toOptionalNumber(value.pre_compaction_total_length),
    context_budget_limit: toOptionalNumber(value.context_budget_limit),
    compaction_applied: toOptionalBoolean(value.compaction_applied),
    compaction_details: normalizeBatchGenerationCompactionDetails(value.compaction_details),
  };
};

const getBatchGenerationPathLabel = (value?: string | null): string => {
  switch (value) {
    case 'single_pass':
      return '单轮生成';
    case 'rerank_retry':
      return '重排重试';
    case 'word_budget_repair':
      return '字数修复';
    default:
      return value ? value : '';
  }
};

const buildBatchGenerationCheckpointHint = (checkpoint?: BatchGenerationCheckpoint | null): string => {
  if (!checkpoint) return '';
  const parts: string[] = [];
  if (typeof checkpoint.candidate_index === 'number' && typeof checkpoint.candidate_count === 'number') {
    parts.push(`候选 ${checkpoint.candidate_index}/${checkpoint.candidate_count}`);
  }
  if (typeof checkpoint.word_count === 'number' && checkpoint.word_count > 0) {
    parts.push(`${checkpoint.word_count} 字`);
  }
  const generationPathLabel = getBatchGenerationPathLabel(checkpoint.generation_path);
  if (generationPathLabel) {
    parts.push(`路径：${generationPathLabel}`);
  }
  if (typeof checkpoint.winner_candidate_index === 'number') {
    parts.push(`胜出候选：${checkpoint.winner_candidate_index}`);
  }
  const compactionHint = buildBatchCompactionHint(checkpoint);
  if (compactionHint) {
    parts.push(compactionHint);
  }
  return parts.join(' | ');
};

const collectActiveAnalysisChapterIds = (tasksMap: Record<string, AnalysisTask>): string[] => (
  Object.entries(tasksMap)
    .filter(([, task]) => isAnalysisTaskInProgress(task))
    .map(([chapterId]) => chapterId)
);

const areAnalysisTaskSnapshotsEqual = (leftTask?: AnalysisTask, rightTask?: AnalysisTask): boolean => {
  if (!leftTask || !rightTask) {
    return leftTask === rightTask;
  }

  return (
    leftTask.has_task === rightTask.has_task
    && leftTask.task_id === rightTask.task_id
    && leftTask.chapter_id === rightTask.chapter_id
    && leftTask.status === rightTask.status
    && leftTask.progress === rightTask.progress
    && leftTask.error_message === rightTask.error_message
    && leftTask.error_code === rightTask.error_code
    && leftTask.auto_recovered === rightTask.auto_recovered
    && leftTask.created_at === rightTask.created_at
    && leftTask.started_at === rightTask.started_at
    && leftTask.completed_at === rightTask.completed_at
  );
};

const writingStylesLoadPromises = new Map<string, Promise<void>>();
const writingStylesCache = new Map<string, ChapterWritingStylesCacheEntry>();
const chapterAnalysisTasksCache = new Map<string, Record<string, AnalysisTask>>();

const normalizeWritingStyleOptions = (styles: WritingStyle[]): WritingStyle[] => {
  const seenStyleIds = new Set<number>();
  const normalizedStyles: WritingStyle[] = [];

  styles.forEach((style) => {
    if (!Number.isFinite(style.id) || seenStyleIds.has(style.id)) {
      return;
    }

    seenStyleIds.add(style.id);
    normalizedStyles.push(style);
  });

  return normalizedStyles;
};

const areWritingStylesEqual = (leftStyles: WritingStyle[], rightStyles: WritingStyle[]): boolean => (
  leftStyles.length === rightStyles.length
  && leftStyles.every((style, index) => {
    const rightStyle = rightStyles[index];
    return Boolean(rightStyle)
      && style.id === rightStyle.id
      && style.name === rightStyle.name
      && style.is_default === rightStyle.is_default
      && style.updated_at === rightStyle.updated_at;
  })
);

const MANUAL_STORY_CREATION_BRIEF_SENTINEL = '__manual_story_creation_brief__';

const buildSingleStoryCreationDraftStorageKey = (projectId: string, chapterId: string): string => (
  `${projectId}::single::${chapterId}`
);

const buildBatchStoryCreationDraftStorageKey = (projectId: string): string => (
  `${projectId}::batch`
);

const CHAPTER_BATCH_TASK_REFRESH_KEY_PREFIX = 'background-task-refresh:chapters-batch:';
const CHAPTER_SINGLE_TASK_REFRESH_KEY_PREFIX = 'background-task-refresh:chapters-single:';
const COMPLETED_TASK_REFRESH_RETRY_DELAY_MS = 2000;
const NON_URGENT_REFRESH_DELAY_MS = 96;

type IdleCallbackWindow = Window & typeof globalThis & {
  requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;
  cancelIdleCallback?: (handle: number) => void;
};

const hasChapterBatchTaskRefreshBeenHandled = (taskId: string): boolean => {
  try {
    return sessionStorage.getItem(`${CHAPTER_BATCH_TASK_REFRESH_KEY_PREFIX}${taskId}`) === '1';
  } catch {
    return false;
  }
};

const markChapterBatchTaskRefreshHandled = (taskId: string) => {
  try {
    sessionStorage.setItem(`${CHAPTER_BATCH_TASK_REFRESH_KEY_PREFIX}${taskId}`, '1');
  } catch {
    // ignore sessionStorage failures
  }
};

const hasChapterSingleTaskRefreshBeenHandled = (taskId: string): boolean => {
  try {
    return sessionStorage.getItem(`${CHAPTER_SINGLE_TASK_REFRESH_KEY_PREFIX}${taskId}`) === '1';
  } catch {
    return false;
  }
};

const markChapterSingleTaskRefreshHandled = (taskId: string) => {
  try {
    sessionStorage.setItem(`${CHAPTER_SINGLE_TASK_REFRESH_KEY_PREFIX}${taskId}`, '1');
  } catch {
    // ignore sessionStorage failures
  }
};

const createRefreshTaskLock = () => {
  const inFlightTaskIds = new Set<string>();

  return {
    acquire(taskId: string) {
      if (!taskId || inFlightTaskIds.has(taskId)) {
        return false;
      }
      inFlightTaskIds.add(taskId);
      return true;
    },
    release(taskId: string) {
      if (!taskId) return;
      inFlightTaskIds.delete(taskId);
    },
  };
};

const scheduleNonUrgentRefreshTask = (
  callback: () => void,
): (() => void) => {
  const windowWithIdleCallback = window as IdleCallbackWindow;

  if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {
    const idleHandle = windowWithIdleCallback.requestIdleCallback(() => {
      callback();
    }, { timeout: 400 });

    return () => {
      if (typeof windowWithIdleCallback.cancelIdleCallback === 'function') {
        windowWithIdleCallback.cancelIdleCallback(idleHandle);
      }
    };
  }

  const timerId = window.setTimeout(callback, NON_URGENT_REFRESH_DELAY_MS);
  return () => {
    window.clearTimeout(timerId);
  };
};

const selectLatestCompletedBatchRefreshTaskSignature = (
  tasks: Record<string, ReturnType<typeof useBackgroundTaskStore.getState>['tasks'][string]>,
  projectId?: string | null,
): string => {
  if (!projectId) {
    return '';
  }

  let latestTask: ReturnType<typeof useBackgroundTaskStore.getState>['tasks'][string] | undefined;
  let latestTimestamp = -1;

  Object.values(tasks).forEach((task) => {
    if (
      task.projectId !== projectId
      || task.taskType !== 'chapters_batch_generate'
      || task.status !== 'completed'
      || hasChapterBatchTaskRefreshBeenHandled(task.taskId)
    ) {
      return;
    }

    const timestamp = task.completedAt ?? task.updatedAt;
    if (timestamp > latestTimestamp) {
      latestTask = task;
      latestTimestamp = timestamp;
    }
  });

  if (!latestTask) {
    return '';
  }

  return `${latestTask.taskId}:${latestTask.completedAt ?? latestTask.updatedAt}`;
};

const selectLatestCompletedSingleRefreshTaskSignature = (
  tasks: Record<string, ReturnType<typeof useBackgroundTaskStore.getState>['tasks'][string]>,
  projectId?: string | null,
): string => {
  if (!projectId) {
    return '';
  }

  let latestTask: ReturnType<typeof useBackgroundTaskStore.getState>['tasks'][string] | undefined;
  let latestTimestamp = -1;

  Object.values(tasks).forEach((task) => {
    if (
      task.projectId !== projectId
      || task.taskType !== 'chapter_single_generate'
      || task.status !== 'completed'
      || hasChapterSingleTaskRefreshBeenHandled(task.taskId)
    ) {
      return;
    }

    const timestamp = task.completedAt ?? task.updatedAt;
    if (timestamp > latestTimestamp) {
      latestTask = task;
      latestTimestamp = timestamp;
    }
  });

  if (!latestTask) {
    return '';
  }

  const chapterId = typeof latestTask.checkpoint?.chapter_id === 'string'
    ? latestTask.checkpoint.chapter_id
    : '';

  return `${latestTask.taskId}:${chapterId}:${latestTask.completedAt ?? latestTask.updatedAt}`;
};

export default function Chapters() {
  const location = useLocation();
  const { token } = theme.useToken();

  const {
    currentProjectId,
    currentProjectTitle,
    currentProjectOutlineMode,
    currentProjectNarrativePerspective,
    projectDefaultCreativeMode,
    projectDefaultStoryFocus,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBriefRaw,
    projectDefaultQualityPreset,
    projectDefaultQualityNotesRaw,
  } = useStore(useShallow((state) => ({
    currentProjectId: state.currentProject?.id ?? null,
    currentProjectTitle: state.currentProject?.title ?? '',
    currentProjectOutlineMode: state.currentProject?.outline_mode ?? null,
    currentProjectNarrativePerspective: state.currentProject?.narrative_perspective ?? '',
    projectDefaultCreativeMode: state.currentProject?.default_creative_mode,
    projectDefaultStoryFocus: state.currentProject?.default_story_focus,
    projectDefaultPlotStage: state.currentProject?.default_plot_stage,
    projectDefaultStoryCreationBriefRaw: state.currentProject?.default_story_creation_brief ?? '',
    projectDefaultQualityPreset: state.currentProject?.default_quality_preset,
    projectDefaultQualityNotesRaw: state.currentProject?.default_quality_notes ?? '',
  })));
  const completedBatchRefreshLockRef = useRef(createRefreshTaskLock());
  const completedSingleRefreshLockRef = useRef(createRefreshTaskLock());
  const sortedChaptersCacheRef = useRef<Chapter[]>([]);
  const groupedChaptersCacheRef = useRef<GroupedChapterViewModel[]>([]);
  const latestCompletedBatchRefreshTaskSignature = useBackgroundTaskStore(
    useCallback(
      (state) => selectLatestCompletedBatchRefreshTaskSignature(state.tasks, currentProjectId),
      [currentProjectId],
    ),
  );
  const latestCompletedSingleRefreshTaskSignature = useBackgroundTaskStore(
    useCallback(
      (state) => selectLatestCompletedSingleRefreshTaskSignature(state.tasks, currentProjectId),
      [currentProjectId],
    ),
  );
  const projectDefaultStoryCreationBrief = projectDefaultStoryCreationBriefRaw.trim();
  const projectDefaultQualityNotes = projectDefaultQualityNotesRaw.trim();

  const chapters = useStore((state) => state.chapters);

  const outlines = useStore((state) => state.outlines);

  const setCurrentChapter = useStore((state) => state.setCurrentChapter);

  const setCurrentProject = useStore((state) => state.setCurrentProject);

  const [modal, contextHolder] = Modal.useModal();

  const [isModalOpen, setIsModalOpen] = useState(false);

  const [isEditorOpen, setIsEditorOpen] = useState(false);

  const [isContinuing, setIsContinuing] = useState(false);

  const [editingId, setEditingId] = useState<string | null>(null);

  const editingChapterIdRef = useRef<string | null>(null);

  const isEditorOpenRef = useRef(false);
  const isPageActiveRef = useRef(true);

  const [runningSingleChapterTasks, setRunningSingleChapterTasks] = useState<Record<string, string>>({});
  const batchRefreshCancelRef = useRef<(() => void) | null>(null);
  const singleRefreshCancelRef = useRef<(() => void) | null>(null);
  const batchFollowUpRefreshCancelRef = useRef<(() => void) | null>(null);
  const singleFollowUpRefreshCancelRef = useRef<(() => void) | null>(null);

  const [form] = Form.useForm();

  const [editorForm] = Form.useForm();

  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);

  const [writingStyles, setWritingStyles] = useState<WritingStyle[]>([]);

  const [selectedStyleId, setSelectedStyleId] = useState<number | undefined>();

  const [targetWordCount, setTargetWordCount] = useState<number>(getCachedWordCount);

  const [availableModels, setAvailableModels] = useState<ModelOption[]>([]);
  const [selectedModel, setSelectedModel] = useState<string | undefined>();
  const [batchSelectedModel, setBatchSelectedModel] = useState<string | undefined>(); // batch generation model
  const [temporaryNarrativePerspective, setTemporaryNarrativePerspective] = useState<string | undefined>(); // temporary narrative perspective
  const [selectedCreativeMode, setSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [batchSelectedCreativeMode, setBatchSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [selectedStoryFocus, setSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [batchSelectedStoryFocus, setBatchSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [selectedPlotStage, setSelectedPlotStage] = useState<PlotStage | undefined>();
  const [batchSelectedPlotStage, setBatchSelectedPlotStage] = useState<PlotStage | undefined>();
  const [selectedQualityPreset, setSelectedQualityPreset] = useState<QualityPreset | undefined>();
  const [batchSelectedQualityPreset, setBatchSelectedQualityPreset] = useState<QualityPreset | undefined>();
  const [selectedQualityNotes, setSelectedQualityNotes] = useState('');
  const [batchSelectedQualityNotes, setBatchSelectedQualityNotes] = useState('');
  const [singleStoryCreationBriefDraft, setSingleStoryCreationBriefDraft] = useState('');
  const [batchStoryCreationBriefDraft, setBatchStoryCreationBriefDraft] = useState('');
  const [singleStoryBeatPlannerDraft, setSingleStoryBeatPlannerDraft] = useState<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const [batchStoryBeatPlannerDraft, setBatchStoryBeatPlannerDraft] = useState<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const [singleStorySceneOutlineDraft, setSingleStorySceneOutlineDraft] = useState<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const [batchStorySceneOutlineDraft, setBatchStorySceneOutlineDraft] = useState<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const [singleStoryCreationSnapshots, setSingleStoryCreationSnapshots] = useState<StoryCreationSnapshot[]>([]);
  const [batchStoryCreationSnapshots, setBatchStoryCreationSnapshots] = useState<StoryCreationSnapshot[]>([]);
  const [batchSystemStoryCreationBrief, setBatchSystemStoryCreationBrief] = useState('');
  const [batchSystemStoryBeatPlanner, setBatchSystemStoryBeatPlanner] = useState<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const [batchSuggestedStorySceneOutline, setBatchSuggestedStorySceneOutline] = useState<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const [analysisVisible, setAnalysisVisible] = useState(false);
  const singleStoryCreationAutoBriefRef = useRef('');
  const batchStoryCreationAutoBriefRef = useRef('');
  const singleStoryBeatPlannerAutoRef = useRef<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const batchStoryBeatPlannerAutoRef = useRef<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const singleStorySceneOutlineAutoRef = useRef<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const batchStorySceneOutlineAutoRef = useRef<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const chaptersByIdRef = useRef<Record<string, Chapter>>({});

  const [singleStoryPresetState, setSingleStoryPresetState] = useState<SingleStoryPresetState>(EMPTY_SINGLE_STORY_PRESET_STATE);
  const {
    singleStoryAcceptanceCard,
    singleStoryCharacterArcCard,
    singleStoryCreationControlCard,
    singleStoryExecutionChecklist,
    singleStoryObjectiveCard,
    singleStoryRepairTargetCard,
    singleStoryRepetitionRiskCard,
    singleStoryResultCard,
  } = singleStoryPresetState;

  const resolveCreationPresetById = useCallback(async (presetId?: CreationPresetId | null) => {
    const { getCreationPresetById } = await import('../utils/creationPresetsCore');
    return getCreationPresetById(presetId);
  }, []);

  const resolveCreationPresetByModes = useCallback(async (
    creativeMode?: CreativeMode,
    storyFocus?: StoryFocus,
  ) => {
    const { getCreationPresetByModes } = await import('../utils/creationPresetsCore');
    return getCreationPresetByModes(creativeMode, storyFocus);
  }, []);

  const inferPlotStage = useCallback(async (options: {
    chapterNumber?: number | null;
    totalChapters?: number | null;
    presetId?: CreationPresetId | null;
    storyFocus?: StoryFocus;
    metrics?: ChapterQualityMetrics | null;
  }) => {
    const { inferCreationPlotStage } = await import('../utils/creationPresetsCore');
    return inferCreationPlotStage(options);
  }, []);

  const applySingleCreationPreset = useCallback(async (presetId: CreationPresetId) => {
    const preset = await resolveCreationPresetById(presetId);
    if (!preset) return;
    setSelectedCreativeMode(preset.creativeMode);
    setSelectedStoryFocus(preset.storyFocus);
  }, [resolveCreationPresetById]);

  const applyBatchCreationPreset = useCallback(async (presetId: CreationPresetId) => {
    const preset = await resolveCreationPresetById(presetId);
    if (!preset) return;
    setBatchSelectedCreativeMode(preset.creativeMode);
    setBatchSelectedStoryFocus(preset.storyFocus);
  }, [resolveCreationPresetById]);
  const [analysisChapterId, setAnalysisChapterId] = useState<string | null>(null);


  const analysisTasksMapRef = useRef<Record<string, AnalysisTask>>({});
  const currentProjectIdRef = useRef<string | null>(null);
  const pollingIntervalsRef = useRef<Set<string>>(new Set());
  const analysisPollingIntervalRef = useRef<number | null>(null);

  const areAnalysisTasksEqual = (
    left: Record<string, AnalysisTask>,
    right: Record<string, AnalysisTask>
  ) => {
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);

    if (leftKeys.length !== rightKeys.length) {
      return false;
    }

    return leftKeys.every((key) => areAnalysisTaskSnapshotsEqual(left[key], right[key]));
  };

  const updateAnalysisTasksMap = useCallback((
    updater: Record<string, AnalysisTask> | ((prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)
  ) => {
    const setTasksMap = useChapterAnalysisUiStore.getState().setTasksMap;
    setTasksMap((prev: Record<string, AnalysisTask>) => {
      const next = typeof updater === 'function'
        ? (updater as (prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)(prev)
        : updater;

      if (areAnalysisTasksEqual(prev, next)) {
        return prev;
      }

      analysisTasksMapRef.current = next;

      const projectId = currentProjectIdRef.current;
      if (projectId) {
        chapterAnalysisTasksCache.set(projectId, next);
      }

      return next;
    });
  }, []);




  const [readerVisible, setReaderVisible] = useState(false);

  const [readingChapter, setReadingChapter] = useState<Chapter | null>(null);




  const [planEditorVisible, setPlanEditorVisible] = useState(false);

  const [editingPlanChapter, setEditingPlanChapter] = useState<Chapter | null>(null);




  const [chapterQualityMetrics, setChapterQualityMetrics] = useState<ChapterQualityMetrics | null>(null);
  const [chapterQualityRefreshToken, setChapterQualityRefreshToken] = useState(0);

  const [batchGenerateVisible, setBatchGenerateVisible] = useState(false);
  const [batchGenerating, setBatchGenerating] = useState(false);
  const [batchTaskId, setBatchTaskId] = useState<string | null>(null);
  const batchTaskIdRef = useRef<string | null>(null);
  const [batchForm] = Form.useForm();
  const [manualCreateForm] = Form.useForm();
  const batchStartChapterNumber = Form.useWatch('startChapterNumber', batchForm) as number | undefined;
  const batchEnableAnalysis = Form.useWatch('enableAnalysis', batchForm) as boolean | undefined;
  const singleGenerationOverlayLoading = useChapterGenerationUiStore((state) => state.singleOverlay.loading);
  const shouldTrackBatchQualityMetricsSummary = batchGenerateVisible && !batchGenerating;
  const batchQualityMetricsSummary = useChapterGenerationUiStore(useCallback(
    (state) => (
      shouldTrackBatchQualityMetricsSummary
        ? state.batchProgress?.quality_metrics_summary ?? null
        : null
    ),
    [shouldTrackBatchQualityMetricsSummary],
  ));
  const setBatchProgress = useCallback(
    (progress: {
      status: string;
      total: number;
      completed: number;
      current_chapter_number: number | null;
      progress_percent?: number;
      checkpoint?: BatchGenerationCheckpoint | null;
      estimated_time_minutes?: number;
      latest_quality_metrics?: ChapterLatestQualityMetrics | null;
      quality_metrics_summary?: ChapterQualityMetricsSummary | null;
      quality_profile_summary?: ChapterQualityProfileSummary | null;
      failed_chapters?: Array<Record<string, unknown>>;
      active_story_repair_payload?: ActiveStoryRepairPayload | null;
    } | null) => {
      useChapterGenerationUiStore.getState().setBatchProgress(progress);
    },
    [],
  );

  const maxKnownChapterNumber = useMemo(
    () => chapters.reduce((maxValue, chapter) => Math.max(maxValue, chapter.chapter_number || 0), 0),
    [chapters],
  );

  const knownStructureChapterCount = useMemo(
    () => Math.max(maxKnownChapterNumber, outlines.length),
    [maxKnownChapterNumber, outlines.length],
  );

  const currentEditingChapter = useMemo(
    () => chapters.find((chapter) => chapter.id === editingId),
    [chapters, editingId],
  );


  const singleStoryCreationDraftStorageKey = useMemo(
    () => (currentProjectId && currentEditingChapter?.id
      ? buildSingleStoryCreationDraftStorageKey(currentProjectId, currentEditingChapter.id)
      : null),
    [currentProjectId, currentEditingChapter?.id],
  );

  const batchStoryCreationDraftStorageKey = useMemo(
    () => (currentProjectId ? buildBatchStoryCreationDraftStorageKey(currentProjectId) : null),
    [currentProjectId],
  );

  const resetSingleStoryCreationCockpit = useCallback((chapterNumber?: number | null) => {
    singleStoryCreationAutoBriefRef.current = '';
    singleStoryBeatPlannerAutoRef.current = { ...EMPTY_STORY_BEAT_PLANNER_DRAFT };
    singleStorySceneOutlineAutoRef.current = { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT };
    setTemporaryNarrativePerspective(undefined);
    setSelectedCreativeMode(projectDefaultCreativeMode);
    setSelectedStoryFocus(projectDefaultStoryFocus);
    setSelectedPlotStage(projectDefaultPlotStage);
    setSelectedQualityPreset(projectDefaultQualityPreset);
    setSelectedQualityNotes(projectDefaultQualityNotes);

    if (!projectDefaultPlotStage) {
      void inferPlotStage({
        chapterNumber: chapterNumber ?? undefined,
        totalChapters: knownStructureChapterCount,
      }).then((stage) => {
        setSelectedPlotStage(stage);
      });
    }

    setSingleStoryCreationBriefDraft(projectDefaultStoryCreationBrief);
    setSingleStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setSingleStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [
    inferPlotStage,
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
  ]);

  const resetBatchStoryCreationCockpit = useCallback(() => {
    batchStoryCreationAutoBriefRef.current = '';
    batchStoryBeatPlannerAutoRef.current = { ...EMPTY_STORY_BEAT_PLANNER_DRAFT };
    batchStorySceneOutlineAutoRef.current = { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT };
    setBatchSelectedCreativeMode(projectDefaultCreativeMode);
    setBatchSelectedStoryFocus(projectDefaultStoryFocus);
    setBatchSelectedPlotStage(projectDefaultPlotStage);
    setBatchSelectedQualityPreset(projectDefaultQualityPreset);
    setBatchSelectedQualityNotes(projectDefaultQualityNotes);
    setBatchStoryCreationBriefDraft(projectDefaultStoryCreationBrief);
    setBatchStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setBatchStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
  ]);

  const applyInferredSinglePlotStage = useCallback(async () => {
    const activeSingleCreationPreset = await resolveCreationPresetByModes(selectedCreativeMode, selectedStoryFocus);
    const inferredStage = await inferPlotStage({
      chapterNumber: currentEditingChapter?.chapter_number,
      totalChapters: knownStructureChapterCount,
      presetId: activeSingleCreationPreset?.id,
      storyFocus: selectedStoryFocus,
      metrics: chapterQualityMetrics,
    });
    setSelectedPlotStage(inferredStage);
  }, [chapterQualityMetrics, currentEditingChapter?.chapter_number, inferPlotStage, knownStructureChapterCount, resolveCreationPresetByModes, selectedCreativeMode, selectedStoryFocus]);

  const applyInferredBatchPlotStage = useCallback(async () => {
    const activeBatchCreationPreset = await resolveCreationPresetByModes(batchSelectedCreativeMode, batchSelectedStoryFocus);
    const inferredStage = await inferPlotStage({
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      presetId: activeBatchCreationPreset?.id,
      storyFocus: batchSelectedStoryFocus,
      metrics: chapterQualityMetrics,
    });
    setBatchSelectedPlotStage(inferredStage);
  }, [batchSelectedCreativeMode, batchSelectedStoryFocus, batchStartChapterNumber, chapterQualityMetrics, inferPlotStage, knownStructureChapterCount, resolveCreationPresetByModes]);


  const loadSingleStoryPresetState = useCallback(async () => {
    const [{ buildSingleStoryPresetState }, activeSingleCreationPreset] = await Promise.all([
      import('../utils/singleStoryDerived'),
      resolveCreationPresetByModes(selectedCreativeMode, selectedStoryFocus),
    ]);

    return buildSingleStoryPresetState({
      activePresetId: activeSingleCreationPreset?.id,
      chapterNumber: currentEditingChapter?.chapter_number,
      chapterQualityMetrics,
      knownStructureChapterCount,
      selectedCreativeMode,
      selectedPlotStage,
      selectedStoryFocus,
    });
  }, [
    chapterQualityMetrics,
    currentEditingChapter?.chapter_number,
    knownStructureChapterCount,
    resolveCreationPresetByModes,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
  ]);

  useEffect(() => {
    singleStoryPresetRequestIdRef.current += 1;
    const requestId = singleStoryPresetRequestIdRef.current;

    if (!isEditorOpen) {
      return () => {
        singleStoryPresetRequestIdRef.current += 1;
      };
    }

    void loadSingleStoryPresetState()
      .then((nextState) => {
        if (!isPageActiveRef.current || singleStoryPresetRequestIdRef.current !== requestId || !isEditorOpenRef.current) {
          return;
        }

        setSingleStoryPresetState(nextState);
      })
      .catch((error) => {
        if (isPageActiveRef.current && singleStoryPresetRequestIdRef.current === requestId) {
          console.error('Failed to load single-story preset state.', error);
        }
      });

    return () => {
      singleStoryPresetRequestIdRef.current += 1;
    };
  }, [isEditorOpen, loadSingleStoryPresetState]);

useEffect(() => {
  batchStoryPresetRequestIdRef.current += 1;
  const requestId = batchStoryPresetRequestIdRef.current;

  void Promise.all([
    import('../utils/creationPresetsBatch'),
    resolveCreationPresetByModes(batchSelectedCreativeMode, batchSelectedStoryFocus),
  ]).then(([{
    buildBatchSuggestedStorySceneOutline,
    buildBatchSystemStoryBeatPlanner,
    buildBatchSystemStoryCreationBriefFromSummary,
  }, activeBatchCreationPreset]) => {
    if (!isPageActiveRef.current || batchStoryPresetRequestIdRef.current !== requestId) {
      return;
    }

    const nextBatchSystemStoryCreationBrief = buildBatchSystemStoryCreationBriefFromSummary(
      batchQualityMetricsSummary,
      batchSelectedCreativeMode,
      batchSelectedStoryFocus,
      {
        plotStage: batchSelectedPlotStage,
        chapterNumber: batchStartChapterNumber,
        totalChapters: knownStructureChapterCount,
        activePresetId: activeBatchCreationPreset?.id,
      },
    );
    const nextBatchSystemStoryBeatPlanner = buildBatchSystemStoryBeatPlanner(
      batchSelectedCreativeMode,
      batchSelectedStoryFocus,
      { plotStage: batchSelectedPlotStage },
    );
    const nextBatchSuggestedStorySceneOutline = buildBatchSuggestedStorySceneOutline(
      batchStoryBeatPlannerDraft,
      batchSelectedCreativeMode,
      batchSelectedStoryFocus,
      { plotStage: batchSelectedPlotStage },
    );

    setBatchSystemStoryCreationBrief((previousBrief) => (
      previousBrief === nextBatchSystemStoryCreationBrief ? previousBrief : nextBatchSystemStoryCreationBrief
    ));
    setBatchSystemStoryBeatPlanner((previousPlanner) => (
      areStoryBeatPlannerDraftsEqual(previousPlanner, nextBatchSystemStoryBeatPlanner)
        ? previousPlanner
        : nextBatchSystemStoryBeatPlanner
    ));
    setBatchSuggestedStorySceneOutline((previousOutline) => (
      areStorySceneOutlineDraftsEqual(previousOutline, nextBatchSuggestedStorySceneOutline)
        ? previousOutline
        : nextBatchSuggestedStorySceneOutline
    ));
  }).catch((error) => {
    if (isPageActiveRef.current && batchStoryPresetRequestIdRef.current === requestId) {
      console.error('Failed to load batch-story preset state.', error);
    }
  });

  return () => {
    batchStoryPresetRequestIdRef.current += 1;
  };
}, [
  batchQualityMetricsSummary,
  batchSelectedCreativeMode,
  batchSelectedPlotStage,
  batchSelectedStoryFocus,
  batchStartChapterNumber,
  batchStoryBeatPlannerDraft,
  knownStructureChapterCount,
  resolveCreationPresetByModes,
]);

const singleSystemStoryBeatPlanner = useMemo<StoryBeatPlannerDraft>(() => ({
  openingHook: singleStoryObjectiveCard?.hook || singleStoryExecutionChecklist?.opening || '',
  chapterGoal: singleStoryObjectiveCard?.objective || singleStoryResultCard?.progress || '',
  conflictPressure: singleStoryObjectiveCard?.obstacle || singleStoryExecutionChecklist?.pressure || '',
  turningPoint: singleStoryObjectiveCard?.turn || singleStoryExecutionChecklist?.pivot || '',
  endingHook: singleStoryExecutionChecklist?.closing || singleStoryResultCard?.fallout || '',
}), [singleStoryExecutionChecklist, singleStoryObjectiveCard, singleStoryResultCard]);

const singleSuggestedStorySceneOutline = useMemo<StorySceneOutlineDraft>(() => buildStorySceneOutlineSuggestion({
  beatPlanner: singleStoryBeatPlannerDraft,
  objective: singleStoryObjectiveCard,
  result: singleStoryResultCard,
  acceptance: singleStoryAcceptanceCard,
}), [singleStoryAcceptanceCard, singleStoryBeatPlannerDraft, singleStoryObjectiveCard, singleStoryResultCard]);

const singleSystemStoryCreationBrief = singleStoryCreationControlCard?.promptBrief ?? '';

const singleStoryCreationDerivedState = useMemo(
  () => (
    isEditorOpen
      ? buildStoryCreationDerivedState({
        scope: 'single',
        creativeMode: selectedCreativeMode,
        storyFocus: selectedStoryFocus,
        plotStage: selectedPlotStage,
        narrativePerspective: temporaryNarrativePerspective,
        storyCreationBriefDraft: singleStoryCreationBriefDraft,
        systemStoryCreationBrief: singleSystemStoryCreationBrief,
        projectDefaultStoryCreationBrief,
        beatPlannerDraft: singleStoryBeatPlannerDraft,
        systemBeatPlannerDraft: singleSystemStoryBeatPlanner,
        sceneOutlineDraft: singleStorySceneOutlineDraft,
        suggestedSceneOutlineDraft: singleSuggestedStorySceneOutline,
        storageKey: singleStoryCreationDraftStorageKey,
        hasChapterContext: Boolean(currentEditingChapter),
        resolveStoryCreationPromptState,
      })
      : buildStoryCreationDerivedState({
        scope: 'single',
        creativeMode: undefined,
        storyFocus: undefined,
        plotStage: undefined,
        narrativePerspective: undefined,
        storyCreationBriefDraft: '',
        systemStoryCreationBrief: '',
        projectDefaultStoryCreationBrief: '',
        beatPlannerDraft: EMPTY_STORY_BEAT_PLANNER_DRAFT,
        systemBeatPlannerDraft: EMPTY_STORY_BEAT_PLANNER_DRAFT,
        sceneOutlineDraft: EMPTY_STORY_SCENE_OUTLINE_DRAFT,
        suggestedSceneOutlineDraft: EMPTY_STORY_SCENE_OUTLINE_DRAFT,
        storageKey: null,
        hasChapterContext: false,
        resolveStoryCreationPromptState,
      })
  ),
  [
    currentEditingChapter,
    isEditorOpen,
    projectDefaultStoryCreationBrief,
    resolveStoryCreationPromptState,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
    singleStoryBeatPlannerDraft,
    singleStoryCreationBriefDraft,
    singleStoryCreationDraftStorageKey,
    singleStorySceneOutlineDraft,
    singleSuggestedStorySceneOutline,
    singleSystemStoryBeatPlanner,
    singleSystemStoryCreationBrief,
    temporaryNarrativePerspective,
  ],
);

const {
  defaultBrief: singleDefaultStoryCreationBrief,
  resolvedBrief: resolvedSingleStoryCreationBrief,
  promptLayerLabels: singleStoryCreationPromptLayerLabels,
  promptCharCount: singleStoryCreationPromptCharCount,
  isPromptVerbose: isSingleStoryCreationPromptVerbose,
  isBriefCustomized: isSingleStoryCreationBriefCustomized,
  isBeatPlannerCustomized: isSingleStoryBeatPlannerCustomized,
  isSceneOutlineCustomized: isSingleStorySceneOutlineCustomized,
  isControlCustomized: isSingleStoryCreationControlCustomized,
  currentDraft: singleStoryCreationCurrentDraft,
  canSaveSnapshot: canSaveSingleStoryCreationSnapshot,
} = singleStoryCreationDerivedState;

const batchStoryCreationDerivedState = useMemo(
  () => (
    batchGenerateVisible || batchGenerating
      ? buildStoryCreationDerivedState({
        scope: 'batch',
        creativeMode: batchSelectedCreativeMode,
        storyFocus: batchSelectedStoryFocus,
        plotStage: batchSelectedPlotStage,
        storyCreationBriefDraft: batchStoryCreationBriefDraft,
        systemStoryCreationBrief: batchSystemStoryCreationBrief,
        projectDefaultStoryCreationBrief,
        beatPlannerDraft: batchStoryBeatPlannerDraft,
        systemBeatPlannerDraft: batchSystemStoryBeatPlanner,
        sceneOutlineDraft: batchStorySceneOutlineDraft,
        suggestedSceneOutlineDraft: batchSuggestedStorySceneOutline,
        storageKey: batchStoryCreationDraftStorageKey,
        resolveStoryCreationPromptState,
      })
      : buildStoryCreationDerivedState({
        scope: 'batch',
        creativeMode: undefined,
        storyFocus: undefined,
        plotStage: undefined,
        storyCreationBriefDraft: '',
        systemStoryCreationBrief: '',
        projectDefaultStoryCreationBrief: '',
        beatPlannerDraft: EMPTY_STORY_BEAT_PLANNER_DRAFT,
        systemBeatPlannerDraft: EMPTY_STORY_BEAT_PLANNER_DRAFT,
        sceneOutlineDraft: EMPTY_STORY_SCENE_OUTLINE_DRAFT,
        suggestedSceneOutlineDraft: EMPTY_STORY_SCENE_OUTLINE_DRAFT,
        storageKey: null,
        resolveStoryCreationPromptState,
      })
  ),
  [
    batchGenerateVisible,
    batchGenerating,
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationDraftStorageKey,
    batchStorySceneOutlineDraft,
    batchSuggestedStorySceneOutline,
    batchSystemStoryBeatPlanner,
    batchSystemStoryCreationBrief,
    projectDefaultStoryCreationBrief,
    resolveStoryCreationPromptState,
  ],
);

const {
  defaultBrief: batchDefaultStoryCreationBrief,
  resolvedBrief: resolvedBatchStoryCreationBrief,
  promptLayerLabels: batchStoryCreationPromptLayerLabels,
  promptCharCount: batchStoryCreationPromptCharCount,
  isPromptVerbose: isBatchStoryCreationPromptVerbose,
  isBriefCustomized: isBatchStoryCreationBriefCustomized,
  isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
  isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
  isControlCustomized: isBatchStoryCreationControlCustomized,
  currentDraft: batchStoryCreationCurrentDraft,
  canSaveSnapshot: canSaveBatchStoryCreationSnapshot,
} = batchStoryCreationDerivedState;

useEffect(() => {
    singleStoryRestoreRequestIdRef.current += 1;
    const requestId = singleStoryRestoreRequestIdRef.current;

    if (!isEditorOpen) {
      setSingleStoryCreationSnapshots([]);
      return () => {
        singleStoryRestoreRequestIdRef.current += 1;
      };
    }

    void restoreSingleStoryCreationPersistenceWorkflow({
      currentChapterId: currentEditingChapter?.id,
      currentChapterNumber: currentEditingChapter?.chapter_number,
      storageKey: singleStoryCreationDraftStorageKey,
      loadStoryCreationPersistence,
      resetCockpit: resetSingleStoryCreationCockpit,
      manualBriefSentinel: MANUAL_STORY_CREATION_BRIEF_SENTINEL,
      singleDefaultBrief: singleDefaultStoryCreationBrief,
      projectDefaultBrief: projectDefaultStoryCreationBrief,
      projectDefaultCreativeMode,
      projectDefaultStoryFocus,
      projectDefaultPlotStage,
      projectDefaultQualityPreset,
      projectDefaultQualityNotes,
      totalChapters: knownStructureChapterCount,
      inferPlotStage,
      isCancelled: () => (
        !isPageActiveRef.current
        || singleStoryRestoreRequestIdRef.current !== requestId
        || editingChapterIdRef.current !== (currentEditingChapter?.id ?? null)
      ),
      setAutoBriefRef: (value) => { singleStoryCreationAutoBriefRef.current = value; },
      setBeatPlannerAutoRef: (value) => { singleStoryBeatPlannerAutoRef.current = value; },
      setSceneOutlineAutoRef: (value) => { singleStorySceneOutlineAutoRef.current = value; },
      setTemporaryNarrativePerspective,
      setSelectedCreativeMode,
      setSelectedStoryFocus,
      setSelectedPlotStage,
      setSelectedQualityPreset,
      setSelectedQualityNotes,
      setStoryCreationBriefDraft: setSingleStoryCreationBriefDraft,
      setBeatPlannerDraft: setSingleStoryBeatPlannerDraft,
      setSceneOutlineDraft: setSingleStorySceneOutlineDraft,
      setSnapshots: setSingleStoryCreationSnapshots,
    });

    return () => {
      singleStoryRestoreRequestIdRef.current += 1;
    };
  }, [
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    inferPlotStage,
    isEditorOpen,
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetSingleStoryCreationCockpit,
    singleDefaultStoryCreationBrief,
    singleStoryCreationDraftStorageKey,
  ]);

  useEffect(() => {
    batchStoryRestoreRequestIdRef.current += 1;
    const requestId = batchStoryRestoreRequestIdRef.current;

    if (!batchGenerateVisible && !batchGenerating) {
      setBatchStoryCreationSnapshots([]);
      return () => {
        batchStoryRestoreRequestIdRef.current += 1;
      };
    }

    void restoreBatchStoryCreationPersistenceWorkflow({
      storageKey: batchStoryCreationDraftStorageKey,
      loadStoryCreationPersistence,
      resetCockpit: resetBatchStoryCreationCockpit,
      manualBriefSentinel: MANUAL_STORY_CREATION_BRIEF_SENTINEL,
      batchDefaultBrief: batchDefaultStoryCreationBrief,
      projectDefaultBrief: projectDefaultStoryCreationBrief,
      projectDefaultCreativeMode,
      projectDefaultStoryFocus,
      projectDefaultPlotStage,
      projectDefaultQualityPreset,
      projectDefaultQualityNotes,
      isCancelled: () => (
        !isPageActiveRef.current
        || batchStoryRestoreRequestIdRef.current !== requestId
      ),
      setAutoBriefRef: (value) => { batchStoryCreationAutoBriefRef.current = value; },
      setBeatPlannerAutoRef: (value) => { batchStoryBeatPlannerAutoRef.current = value; },
      setSceneOutlineAutoRef: (value) => { batchStorySceneOutlineAutoRef.current = value; },
      setSelectedCreativeMode: setBatchSelectedCreativeMode,
      setSelectedStoryFocus: setBatchSelectedStoryFocus,
      setSelectedPlotStage: setBatchSelectedPlotStage,
      setSelectedQualityPreset: setBatchSelectedQualityPreset,
      setSelectedQualityNotes: setBatchSelectedQualityNotes,
      setStoryCreationBriefDraft: setBatchStoryCreationBriefDraft,
      setBeatPlannerDraft: setBatchStoryBeatPlannerDraft,
      setSceneOutlineDraft: setBatchStorySceneOutlineDraft,
      setSnapshots: setBatchStoryCreationSnapshots,
    });

    return () => {
      batchStoryRestoreRequestIdRef.current += 1;
    };
  }, [
    batchDefaultStoryCreationBrief,
    batchStoryCreationDraftStorageKey,
    batchGenerateVisible,
    batchGenerating,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetBatchStoryCreationCockpit,
  ]);

  useEffect(() => {
    if (!isEditorOpen) {
      return;
    }

    persistSingleStoryCreationDraftWorkflow({
      currentChapterId: currentEditingChapter?.id,
      storageKey: singleStoryCreationDraftStorageKey,
      loadStoryCreationPersistence,
      creativeMode: selectedCreativeMode,
      storyFocus: selectedStoryFocus,
      plotStage: selectedPlotStage,
      narrativePerspective: temporaryNarrativePerspective,
      storyCreationBriefDraft: singleStoryCreationBriefDraft,
      beatPlannerDraft: singleStoryBeatPlannerDraft,
      sceneOutlineDraft: singleStorySceneOutlineDraft,
      isBriefCustomized: isSingleStoryCreationBriefCustomized,
      isBeatPlannerCustomized: isSingleStoryBeatPlannerCustomized,
      isSceneOutlineCustomized: isSingleStorySceneOutlineCustomized,
    });
  }, [
    currentEditingChapter?.id,
    isEditorOpen,
    isSingleStoryBeatPlannerCustomized,
    isSingleStoryCreationBriefCustomized,
    isSingleStorySceneOutlineCustomized,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
    singleStoryBeatPlannerDraft,
    singleStoryCreationBriefDraft,
    singleStoryCreationDraftStorageKey,
    singleStorySceneOutlineDraft,
    temporaryNarrativePerspective,
  ]);

  useEffect(() => {
    if (!batchGenerateVisible && !batchGenerating) {
      return;
    }

    persistBatchStoryCreationDraftWorkflow({
      storageKey: batchStoryCreationDraftStorageKey,
      loadStoryCreationPersistence,
      creativeMode: batchSelectedCreativeMode,
      storyFocus: batchSelectedStoryFocus,
      plotStage: batchSelectedPlotStage,
      storyCreationBriefDraft: batchStoryCreationBriefDraft,
      beatPlannerDraft: batchStoryBeatPlannerDraft,
      sceneOutlineDraft: batchStorySceneOutlineDraft,
      isBriefCustomized: isBatchStoryCreationBriefCustomized,
      isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
      isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
    });
  }, [
    batchGenerateVisible,
    batchGenerating,
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationDraftStorageKey,
    batchStorySceneOutlineDraft,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStorySceneOutlineCustomized,
  ]);

  const saveSingleStoryCreationSnapshot = useCallback(async (
  reason: StoryCreationSnapshotReason = 'manual',
  options?: { silent?: boolean; label?: string },
): Promise<StoryCreationSnapshot | null> => saveSingleStoryCreationSnapshotWorkflow({
  reason,
  options,
  storageKey: singleStoryCreationDraftStorageKey,
  currentDraft: singleStoryCreationCurrentDraft,
  currentSnapshots: singleStoryCreationSnapshots,
  briefDraft: singleStoryCreationBriefDraft,
  defaultBrief: singleDefaultStoryCreationBrief,
  beatPlannerDraft: singleStoryBeatPlannerDraft,
  sceneOutlineDraft: singleStorySceneOutlineDraft,
  resolveStoryCreationPromptState,
  loadStoryCreationPersistence,
  setSnapshots: setSingleStoryCreationSnapshots,
  chapterNumber: currentEditingChapter?.chapter_number,
}), [
  currentEditingChapter?.chapter_number,
  singleStoryCreationBriefDraft,
  singleStoryCreationCurrentDraft,
  singleDefaultStoryCreationBrief,
  singleStoryBeatPlannerDraft,
  singleStoryCreationDraftStorageKey,
  singleStoryCreationSnapshots,
  singleStorySceneOutlineDraft,
]);

const saveBatchStoryCreationSnapshot = useCallback(async (
  reason: StoryCreationSnapshotReason = 'manual',
  options?: { silent?: boolean; label?: string },
): Promise<StoryCreationSnapshot | null> => saveBatchStoryCreationSnapshotWorkflow({
  reason,
  options,
  storageKey: batchStoryCreationDraftStorageKey,
  currentDraft: batchStoryCreationCurrentDraft,
  currentSnapshots: batchStoryCreationSnapshots,
  briefDraft: batchStoryCreationBriefDraft,
  defaultBrief: batchDefaultStoryCreationBrief,
  beatPlannerDraft: batchStoryBeatPlannerDraft,
  sceneOutlineDraft: batchStorySceneOutlineDraft,
  resolveStoryCreationPromptState,
  loadStoryCreationPersistence,
  setSnapshots: setBatchStoryCreationSnapshots,
}), [
  batchDefaultStoryCreationBrief,
  batchStoryBeatPlannerDraft,
  batchStoryCreationBriefDraft,
  batchStoryCreationCurrentDraft,
  batchStoryCreationDraftStorageKey,
  batchStoryCreationSnapshots,
  batchStorySceneOutlineDraft,
]);

const applySingleStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
  applySingleStoryCreationSnapshotWorkflow({
    snapshot,
    manualBriefSentinel: MANUAL_STORY_CREATION_BRIEF_SENTINEL,
    setAutoBriefRef: (value) => { singleStoryCreationAutoBriefRef.current = value; },
    setBeatPlannerAutoRef: (value) => { singleStoryBeatPlannerAutoRef.current = value; },
    setSceneOutlineAutoRef: (value) => { singleStorySceneOutlineAutoRef.current = value; },
    setTemporaryNarrativePerspective,
    setSelectedCreativeMode,
    setSelectedStoryFocus,
    setSelectedPlotStage,
    setStoryCreationBriefDraft: setSingleStoryCreationBriefDraft,
    setBeatPlannerDraft: setSingleStoryBeatPlannerDraft,
    setSceneOutlineDraft: setSingleStorySceneOutlineDraft,
    inferPlotStage,
    chapterNumber: currentEditingChapter?.chapter_number,
    totalChapters: knownStructureChapterCount,
  });
}, [currentEditingChapter?.chapter_number, inferPlotStage, knownStructureChapterCount]);

const applyBatchStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
  applyBatchStoryCreationSnapshotWorkflow({
    snapshot,
    manualBriefSentinel: MANUAL_STORY_CREATION_BRIEF_SENTINEL,
    setAutoBriefRef: (value) => { batchStoryCreationAutoBriefRef.current = value; },
    setBeatPlannerAutoRef: (value) => { batchStoryBeatPlannerAutoRef.current = value; },
    setSceneOutlineAutoRef: (value) => { batchStorySceneOutlineAutoRef.current = value; },
    setSelectedCreativeMode: setBatchSelectedCreativeMode,
    setSelectedStoryFocus: setBatchSelectedStoryFocus,
    setSelectedPlotStage: setBatchSelectedPlotStage,
    setStoryCreationBriefDraft: setBatchStoryCreationBriefDraft,
    setBeatPlannerDraft: setBatchStoryBeatPlannerDraft,
    setSceneOutlineDraft: setBatchStorySceneOutlineDraft,
  });
}, []);

const deleteSingleStoryCreationSnapshot = useCallback(async (snapshotId: string) => (
  deleteSingleStoryCreationSnapshotWorkflow({
    storageKey: singleStoryCreationDraftStorageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots: setSingleStoryCreationSnapshots,
  })
), [singleStoryCreationDraftStorageKey]);

const deleteBatchStoryCreationSnapshot = useCallback(async (snapshotId: string) => (
  deleteBatchStoryCreationSnapshotWorkflow({
    storageKey: batchStoryCreationDraftStorageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots: setBatchStoryCreationSnapshots,
  })
), [batchStoryCreationDraftStorageKey]);
  useEffect(() => {
    syncStoryCreationAutoDrafts({
      defaultBrief: singleDefaultStoryCreationBrief,
      previousAutoBrief: singleStoryCreationAutoBriefRef.current,
      setAutoBriefRef: (value) => { singleStoryCreationAutoBriefRef.current = value; },
      systemPlanner: singleSystemStoryBeatPlanner,
      previousAutoPlanner: singleStoryBeatPlannerAutoRef.current,
      setAutoPlannerRef: (value) => { singleStoryBeatPlannerAutoRef.current = value; },
      suggestedOutline: singleSuggestedStorySceneOutline,
      previousSuggestedOutline: singleStorySceneOutlineAutoRef.current,
      setAutoSceneOutlineRef: (value) => { singleStorySceneOutlineAutoRef.current = value; },
      setBriefDraft: setSingleStoryCreationBriefDraft,
      setPlannerDraft: setSingleStoryBeatPlannerDraft,
      setSceneOutlineDraft: setSingleStorySceneOutlineDraft,
    });
  }, [singleDefaultStoryCreationBrief, singleSuggestedStorySceneOutline, singleSystemStoryBeatPlanner]);

  useEffect(() => {
    syncStoryCreationAutoDrafts({
      defaultBrief: batchDefaultStoryCreationBrief,
      previousAutoBrief: batchStoryCreationAutoBriefRef.current,
      setAutoBriefRef: (value) => { batchStoryCreationAutoBriefRef.current = value; },
      systemPlanner: batchSystemStoryBeatPlanner,
      previousAutoPlanner: batchStoryBeatPlannerAutoRef.current,
      setAutoPlannerRef: (value) => { batchStoryBeatPlannerAutoRef.current = value; },
      suggestedOutline: batchSuggestedStorySceneOutline,
      previousSuggestedOutline: batchStorySceneOutlineAutoRef.current,
      setAutoSceneOutlineRef: (value) => { batchStorySceneOutlineAutoRef.current = value; },
      setBriefDraft: setBatchStoryCreationBriefDraft,
      setPlannerDraft: setBatchStoryBeatPlannerDraft,
      setSceneOutlineDraft: setBatchStorySceneOutlineDraft,
    });
  }, [batchDefaultStoryCreationBrief, batchSuggestedStorySceneOutline, batchSystemStoryBeatPlanner]);

  const batchPollingIntervalRef = useRef<number | null>(null);
  const batchCloseTimeoutRef = useRef<number | null>(null);

  const batchTaskMetaRef = useRef<Record<string, BatchTaskMeta>>({});
  const singleStoryPresetRequestIdRef = useRef(0);
  const batchStoryPresetRequestIdRef = useRef(0);
  const singleStoryRestoreRequestIdRef = useRef(0);
  const batchStoryRestoreRequestIdRef = useRef(0);
  const completedBatchRefreshRetryTimerRef = useRef<number | null>(null);
  const completedSingleRefreshRetryTimerRef = useRef<number | null>(null);
  const [completedBatchRefreshRetryTick, setCompletedBatchRefreshRetryTick] = useState(0);
  const [completedSingleRefreshRetryTick, setCompletedSingleRefreshRetryTick] = useState(0);



  useEffect(() => {

    const handleResize = () => {

      setIsMobile(window.innerWidth <= 768);

    };



    window.addEventListener('resize', handleResize);

    return () => window.removeEventListener('resize', handleResize);

  }, []);



  useEffect(() => {

    editingChapterIdRef.current = editingId;

  }, [editingId]);



  useEffect(() => {

    isEditorOpenRef.current = isEditorOpen;

  }, [isEditorOpen]);

  useEffect(() => {
    batchTaskIdRef.current = batchTaskId;
  }, [batchTaskId]);

  const scheduleCompletedBatchRefreshRetry = useCallback(() => {
    if (completedBatchRefreshRetryTimerRef.current) {
      clearTimeout(completedBatchRefreshRetryTimerRef.current);
    }

    completedBatchRefreshRetryTimerRef.current = window.setTimeout(() => {
      completedBatchRefreshRetryTimerRef.current = null;
      setCompletedBatchRefreshRetryTick((value) => value + 1);
    }, COMPLETED_TASK_REFRESH_RETRY_DELAY_MS);
  }, []);

  const scheduleCompletedSingleRefreshRetry = useCallback(() => {
    if (completedSingleRefreshRetryTimerRef.current) {
      clearTimeout(completedSingleRefreshRetryTimerRef.current);
    }

    completedSingleRefreshRetryTimerRef.current = window.setTimeout(() => {
      completedSingleRefreshRetryTimerRef.current = null;
      setCompletedSingleRefreshRetryTick((value) => value + 1);
    }, COMPLETED_TASK_REFRESH_RETRY_DELAY_MS);
  }, []);



  const {

    refreshChapters,

    updateChapter,

    deleteChapter,

    generateChapterContentStream

  } = useChapterSync();



  const stopAnalysisPolling = useCallback((clearTrackedChapterIds = true) => {
    stopChapterAnalysisPolling({
      analysisPollingIntervalRef,
      pollingIntervalsRef,
      clearTrackedChapterIds,
    });
  }, []);
  const syncAnalysisTasksFromBatch = useCallback((
    items: Record<string, AnalysisTask>,
    options?: {
      reset?: boolean;
      notifyOnTerminalTransitions?: boolean;
    }
  ) => {
    return syncChapterAnalysisTasksFromBatch({
      items,
      analysisTasksMapRef,
      updateAnalysisTasksMap,
      areAnalysisTaskSnapshotsEqual: (leftTask, rightTask) => areAnalysisTaskSnapshotsEqual(leftTask ?? undefined, rightTask ?? undefined),
      notifyOnTerminalTransitions: options?.notifyOnTerminalTransitions,
      reset: options?.reset,
    });
  }, [updateAnalysisTasksMap]);
  const pollAnalysisTasksBatch = useCallback(async (projectId: string) => {
    await pollChapterAnalysisTasksBatch({
      projectId,
      currentProjectIdRef,
      pollingIntervalsRef,
      analysisTasksMapRef,
      stopAnalysisPolling,
      updateAnalysisTasksMap,
      areAnalysisTaskSnapshotsEqual: (leftTask, rightTask) => areAnalysisTaskSnapshotsEqual(leftTask ?? undefined, rightTask ?? undefined),
      isAnalysisTaskInProgress,
    });
  }, [stopAnalysisPolling, updateAnalysisTasksMap]);
  const ensureAnalysisPolling = useCallback((projectId: string) => {
    ensureChapterAnalysisPolling({
      projectId,
      analysisPollingIntervalRef,
      pollingIntervalsRef,
      stopAnalysisPolling,
      pollAnalysisTasksBatch,
    });
  }, [pollAnalysisTasksBatch, stopAnalysisPolling]);
  const applyAnalysisPollingState = useCallback((projectId: string, tasksMap: Record<string, AnalysisTask>) => {
    applyChapterAnalysisPollingState({
      projectId,
      tasksMap,
      pollingIntervalsRef,
      ensureAnalysisPolling,
      stopAnalysisPolling,
      collectActiveAnalysisChapterIds,
    });
  }, [ensureAnalysisPolling, stopAnalysisPolling]);
  useEffect(() => {
    isPageActiveRef.current = true;

    return () => {
      isPageActiveRef.current = false;
      batchRefreshCancelRef.current?.();
      batchRefreshCancelRef.current = null;
      singleRefreshCancelRef.current?.();
      singleRefreshCancelRef.current = null;
      batchFollowUpRefreshCancelRef.current?.();
      batchFollowUpRefreshCancelRef.current = null;
      singleFollowUpRefreshCancelRef.current?.();
      singleFollowUpRefreshCancelRef.current = null;
      if (batchCloseTimeoutRef.current) {
        clearTimeout(batchCloseTimeoutRef.current);
        batchCloseTimeoutRef.current = null;
      }
      if (completedBatchRefreshRetryTimerRef.current) {
        clearTimeout(completedBatchRefreshRetryTimerRef.current);
        completedBatchRefreshRetryTimerRef.current = null;
      }
      if (completedSingleRefreshRetryTimerRef.current) {
        clearTimeout(completedSingleRefreshRetryTimerRef.current);
        completedSingleRefreshRetryTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    initializeChapterProjectWorkflow({
      projectId: currentProjectId,
      currentProjectIdRef,
      stopAnalysisPolling,
      updateAnalysisTasksMap,
      chapterAnalysisTasksCache,
      chapterCount: chapters.length,
      refreshChapters,
      loadWritingStyles,
      loadAnalysisTasks,
      checkAndRestoreBatchTask,
    });

    return () => {
      stopAnalysisPolling();
    };

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProjectId, setCurrentProject]);

  useEffect(() => {
    const currentBatchPollingIntervalId = batchPollingIntervalRef.current;
    const currentBatchCloseTimeoutId = batchCloseTimeoutRef.current;

    return () => {
      stopAnalysisPolling();

      if (currentBatchPollingIntervalId) {
        clearInterval(currentBatchPollingIntervalId);
      }
      if (currentBatchCloseTimeoutId) {
        clearTimeout(currentBatchCloseTimeoutId);
      }
    };
  }, [stopAnalysisPolling]);

  const loadAnalysisTasks = async (chaptersToLoad?: typeof chapters) => {
    await loadAnalysisTasksWorkflow({
      projectId: currentProjectId ?? undefined,
      chapters,
      chaptersToLoad,
      isPageActiveRef,
      currentProjectIdRef,
      analysisTasksMapRef,
      chapterAnalysisTasksCache,
      updateAnalysisTasksMap,
      applyAnalysisPollingState,
      stopAnalysisPolling,
      areAnalysisTaskSnapshotsEqual: (leftTask, rightTask) => areAnalysisTaskSnapshotsEqual(leftTask ?? undefined, rightTask ?? undefined),
    });
  };

  const startPollingTask = useCallback((chapterId: string) => {
    startAnalysisPollingTaskWorkflow({
      chapterId,
      pollingIntervalsRef,
      currentProjectIdRef,
      currentProjectId: currentProjectId ?? undefined,
      ensureAnalysisPolling,
    });
  }, [currentProjectId, ensureAnalysisPolling]);

  const refreshChapterAnalysisTask = useCallback(async (chapterId: string) => {
    await refreshAnalysisTaskWorkflow({
      chapterId,
      isPageActiveRef,
      currentProjectIdRef,
      currentProjectId: currentProjectId ?? undefined,
      syncAnalysisTasksFromBatch,
      startPollingTask,
      pollingIntervalsRef,
      stopAnalysisPolling,
      isAnalysisTaskInProgress,
    });
  }, [currentProjectId, startPollingTask, stopAnalysisPolling, syncAnalysisTasksFromBatch]);
  const reloadCurrentProject = useCallback(async () => {
    await reloadChapterProjectWorkflow({
      projectId: currentProjectId ?? undefined,
      isPageActiveRef,
      currentProjectIdRef,
      setCurrentProject,
    });
  }, [currentProjectId, setCurrentProject]);
  const handleCloseAnalysis = useCallback(() => {
    closeAnalysisWorkflow({
      analysisChapterId,
      projectId: currentProjectId ?? undefined,
      isPageActiveRef,
      currentProjectIdRef,
      setAnalysisVisible,
      refreshChapterAnalysisTask,
      setAnalysisChapterId,
    });
  }, [
    analysisChapterId,
    currentProjectId,
    refreshChapterAnalysisTask,
  ]);

  useEffect(() => {
    if (!analysisVisible && !analysisChapterId) {
      return;
    }

    setAnalysisVisible(false);
    setAnalysisChapterId(null);
  }, [location.pathname]);
  const triggerDeferredBatchAnalysis = async (

    startChapterNumber: number,

    count: number,

    latestChapters: Chapter[]

  ) => {

    if (!currentProjectId || count <= 0) return;

    await queueDeferredBatchAnalysis({
      projectId: currentProjectId,
      startChapterNumber,
      count,
      latestChapters,
      analysisTasksMap: analysisTasksMapRef.current,
      startPollingTask,
      loadAnalysisTasks,
    });

  };



  const loadWritingStyles = async () => {

    if (!currentProjectId) return;

    await loadChapterWritingStyles({
      projectId: currentProjectId,
      writingStylesLoadPromises,
      writingStylesCache,
      setWritingStyles,
      setSelectedStyleId,
      normalizeWritingStyleOptions,
      areWritingStylesEqual,
    });

  };



  const loadAvailableModels = useCallback(async () => (
    loadChapterAvailableModels({
      setAvailableModels,
      setSelectedModel,
    })
  ), []);



  // Check and restore batch task state.



  const checkAndRestoreBatchTask = async () => {
    await restoreBatchGenerationWorkflow({
      projectId: currentProjectId ?? undefined,
      batchTaskMetaRef,
      getPersistedTaskMeta: getPersistedChapterBatchTaskMeta,
      setBatchTaskId,
      setBatchProgress,
      setBatchGenerating,
      setBatchGenerateVisible,
      startBatchPolling,
      normalizeBatchGenerationCheckpoint,
    });
  };

  useEffect(() => {
    if (!currentProjectId || batchGenerating || batchTaskId) {
      return;
    }

    if (!latestCompletedBatchRefreshTaskSignature) {
      return;
    }

    const [taskId] = latestCompletedBatchRefreshTaskSignature.split(':');
    if (!taskId) {
      return;
    }
    if (!completedBatchRefreshLockRef.current.acquire(taskId)) {
      return;
    }

    let started = false;
    batchRefreshCancelRef.current?.();
    batchFollowUpRefreshCancelRef.current?.();
    batchFollowUpRefreshCancelRef.current = null;
    batchRefreshCancelRef.current = scheduleNonUrgentRefreshTask(() => {
      started = true;
      batchRefreshCancelRef.current = null;

      void refreshChapters(currentProjectId)
        .then((latestChapters) => {
          if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
            return;
          }
          batchFollowUpRefreshCancelRef.current?.();
          batchFollowUpRefreshCancelRef.current = scheduleNonUrgentRefreshTask(() => {
            batchFollowUpRefreshCancelRef.current = null;

            if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
              return;
            }

            const followUpTasks: Array<Promise<unknown>> = [
              loadAnalysisTasks(latestChapters).catch((error) => {
                if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                  return;
                }
                console.error('批量生成完成后刷新分析任务失败，已降级后台重试:', error);
              }),
              reloadCurrentProject().catch((error) => {
                if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                  return;
                }
                console.error('批量生成完成后刷新项目信息失败，已降级后台重试:', error);
              }),
            ];

            void Promise.allSettled(followUpTasks).finally(() => {
              if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                return;
              }
              markChapterBatchTaskRefreshHandled(taskId);
            });
          });
        })
        .catch((error) => {
          if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
            return;
          }
          console.error('刷新批量生成后的章节数据失败:', error);
          scheduleCompletedBatchRefreshRetry();
        })
        .finally(() => {
          completedBatchRefreshLockRef.current.release(taskId);
        });
    });

    return () => {
      if (!started) {
        batchRefreshCancelRef.current?.();
        batchRefreshCancelRef.current = null;
        batchFollowUpRefreshCancelRef.current?.();
        batchFollowUpRefreshCancelRef.current = null;
        completedBatchRefreshLockRef.current.release(taskId);
      }
    };
  }, [
    batchGenerating,
    batchTaskId,
    completedBatchRefreshRetryTick,
    currentProjectId,
    latestCompletedBatchRefreshTaskSignature,
    loadAnalysisTasks,
    refreshChapters,
    reloadCurrentProject,
    scheduleCompletedBatchRefreshRetry,
  ]);

  useEffect(() => {
    if (!currentProjectId || singleGenerationOverlayLoading) {
      return;
    }

    if (!latestCompletedSingleRefreshTaskSignature) {
      return;
    }

    const [taskId, chapterId = ''] = latestCompletedSingleRefreshTaskSignature.split(':');
    if (!taskId) {
      return;
    }
    if (!completedSingleRefreshLockRef.current.acquire(taskId)) {
      return;
    }

    let started = false;
    singleRefreshCancelRef.current?.();
    singleFollowUpRefreshCancelRef.current?.();
    singleFollowUpRefreshCancelRef.current = null;
    singleRefreshCancelRef.current = scheduleNonUrgentRefreshTask(() => {
      started = true;
      singleRefreshCancelRef.current = null;

      void refreshChapters(currentProjectId)
        .then((latestChapters) => {
          if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
            return;
          }
          singleFollowUpRefreshCancelRef.current?.();
          singleFollowUpRefreshCancelRef.current = scheduleNonUrgentRefreshTask(() => {
            singleFollowUpRefreshCancelRef.current = null;

            if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
              return;
            }

            const followUpTasks: Array<Promise<unknown>> = [];
            if (chapterId) {
              followUpTasks.push(refreshChapterAnalysisTask(chapterId).catch((error) => {
                if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                  return;
                }
                console.error('单章生成完成后刷新章节分析状态失败，已降级后台重试:', error);
              }));
            } else {
              followUpTasks.push(loadAnalysisTasks(latestChapters).catch((error) => {
                if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                  return;
                }
                console.error('单章生成完成后刷新分析任务失败，已降级后台重试:', error);
              }));
            }
            followUpTasks.push(reloadCurrentProject().catch((error) => {
              if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                return;
              }
              console.error('单章生成完成后刷新项目信息失败，已降级后台重试:', error);
            }));

            void Promise.allSettled(followUpTasks).finally(() => {
              if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
                return;
              }
              markChapterSingleTaskRefreshHandled(taskId);
            });
          });
        })
        .catch((error) => {
          if (!isPageActiveRef.current || currentProjectIdRef.current !== currentProjectId) {
            return;
          }
          console.error('刷新单章生成后的章节数据失败:', error);
          scheduleCompletedSingleRefreshRetry();
        })
        .finally(() => {
          completedSingleRefreshLockRef.current.release(taskId);
        });
    });

    return () => {
      if (!started) {
        singleRefreshCancelRef.current?.();
        singleRefreshCancelRef.current = null;
        singleFollowUpRefreshCancelRef.current?.();
        singleFollowUpRefreshCancelRef.current = null;
        completedSingleRefreshLockRef.current.release(taskId);
      }
    };
  }, [
    completedSingleRefreshRetryTick,
    currentProjectId,
    singleGenerationOverlayLoading,
    latestCompletedSingleRefreshTaskSignature,
    loadAnalysisTasks,
    refreshChapterAnalysisTask,
    refreshChapters,
    reloadCurrentProject,
    scheduleCompletedSingleRefreshRetry,
  ]);



  const showBrowserNotification = (title: string, body: string, type: 'success' | 'error' | 'info' = 'info') => {

    // Notifications are optional; fall back to console when unsupported.

    if (!('Notification' in window)) {

      return;

    }



    // Show a notification if permission is granted.

    if (Notification.permission === 'granted') {

      // Use a small icon; success/error share the app icon.

      const icon = type === 'success' ? '/logo.svg' : type === 'error' ? '/favicon.ico' : '/logo.svg';



      const notification = new Notification(title, {

        body,

        icon,

        badge: '/favicon.ico',

        tag: 'batch-generation', // de-dupe notifications

        requireInteraction: false, // allow auto-dismiss

        silent: false, // keep notification sound enabled
      });




      notification.onclick = () => {

        window.focus();

        notification.close();

      };




      setTimeout(() => {

        notification.close();

      }, 5000);

    } else if (Notification.permission !== 'denied') {


      Notification.requestPermission().then(permission => {

        if (permission === 'granted') {

          showBrowserNotification(title, body, type);

        }

      });

    }

  };

  // Precompute chapter ordering, grouping and generation availability before early return.

  const sortedChapters = useMemo(() => {
    const nextSortedChapters = [...chapters].sort((a, b) => a.chapter_number - b.chapter_number);
    const mergedSortedChapters = mergeChapterArrayPreservingReference(
      sortedChaptersCacheRef.current,
      nextSortedChapters,
    );
    sortedChaptersCacheRef.current = mergedSortedChapters;
    return mergedSortedChapters;
  }, [chapters]);

  const groupedChapters = useMemo(() => {
    const groups: Record<string, GroupedChapterViewModel> = {};

    sortedChapters.forEach((chapter) => {
      const key = chapter.outline_id || 'uncategorized';

      if (!groups[key]) {
        groups[key] = {
          key,
          outlineId: chapter.outline_id || null,
          outlineTitle: chapter.outline_title || '未命名大纲',
          outlineOrder: chapter.outline_order ?? 999,
          chapters: [],
          totalWordCount: 0,
        };
      }

      groups[key].chapters.push(chapter);
      groups[key].totalWordCount += chapter.word_count || 0;
    });

    const nextGroups = Object.values(groups).sort((a, b) => a.outlineOrder - b.outlineOrder);
    const mergedGroups = mergeGroupedChaptersPreservingReferences(
      groupedChaptersCacheRef.current,
      nextGroups,
    );
    groupedChaptersCacheRef.current = mergedGroups;
    return mergedGroups;
  }, [sortedChapters]);

  const expandedChapterGroupKeys = useMemo(
    () => groupedChapters.map((group) => group.key),
    [groupedChapters],
  );

  const {
    chapterGenerationStateById,
    batchStartChapterOptions,
    firstIncompleteChapter,
  } = useMemo(() => {
    const generationStateById: Record<string, { canGenerate: boolean; disabledReason: string }> = {};
    const batchStartOptions: Chapter[] = [];

    let incompletePreviousChapterLabel = '';
    let currentChapterNumber: number | null = null;
    let currentChapterGroup: Array<{ chapter: Chapter; hasContent: boolean }> = [];
    let firstIncompleteChapterValue: Chapter | undefined;

    const appendIncompleteChapterNumber = (chapterNumber: number) => {
      incompletePreviousChapterLabel = incompletePreviousChapterLabel
        ? `${incompletePreviousChapterLabel}, ${chapterNumber}`
        : `${chapterNumber}`;
    };

    const flushChapterGroup = () => {
      currentChapterGroup.forEach(({ chapter: groupChapter, hasContent }) => {
        if (!hasContent) {
          appendIncompleteChapterNumber(groupChapter.chapter_number);
        }
      });

      currentChapterGroup = [];
    };

    sortedChapters.forEach((chapter) => {
      if (currentChapterNumber !== null && chapter.chapter_number !== currentChapterNumber) {
        flushChapterGroup();
      }

      currentChapterNumber = chapter.chapter_number;
      const hasContent = Boolean(chapter.content?.trim());

      if (!firstIncompleteChapterValue && !hasContent) {
        firstIncompleteChapterValue = chapter;
      }

      const disabledReason = incompletePreviousChapterLabel
        ? `请先完成前序章节：${incompletePreviousChapterLabel}`
        : '';

      generationStateById[chapter.id] = {
        canGenerate: disabledReason === '',
        disabledReason,
      };

      if (!hasContent && disabledReason === '') {
        batchStartOptions.push(chapter);
      }

      currentChapterGroup.push({ chapter, hasContent });
    });

    return {
      chapterGenerationStateById: generationStateById,
      batchStartChapterOptions: batchStartOptions,
      firstIncompleteChapter: firstIncompleteChapterValue,
    };
  }, [sortedChapters]);



  const sortedOutlines = useMemo(
    () => [...outlines].sort((a, b) => a.order_index - b.order_index),
    [outlines]
  );

  useEffect(() => {
    chaptersByIdRef.current = Object.fromEntries(
      chapters.map((chapter) => [chapter.id, chapter]),
    );
  }, [chapters]);



  const canGenerateChapter = (chapter: Chapter): boolean => {

    return chapterGenerationStateById[chapter.id]?.canGenerate ?? false;

  };



  const getGenerateDisabledReason = (chapter: Chapter): string => {

    return chapterGenerationStateById[chapter.id]?.disabledReason || '';

  };
  const currentEditingCanGenerate = currentEditingChapter ? canGenerateChapter(currentEditingChapter) : false;
  const currentEditingGenerateDisabledReason = currentEditingChapter ? getGenerateDisabledReason(currentEditingChapter) : "";
  const canAnalyzeCurrentChapter = Boolean(currentEditingChapter?.id && currentEditingChapter.content?.trim());

  const editingPlanEditorData = useMemo(
    () => buildChapterPlanEditorData(editingPlanChapter),
    [editingPlanChapter],
  );

  const chapterReaderModalState = useMemo(
    () => buildChapterReaderModalState({
      readerVisible,
      readingChapter,
    }),
    [readerVisible, readingChapter],
  );


  const planEditorModalState = useMemo(
    () => buildChapterPlanEditorModalState({
      planEditorVisible,
      editingPlanChapter,
      editingPlanEditorData,
      currentProjectId: currentProjectId ?? undefined,
    }),
    [currentProjectId, editingPlanChapter, editingPlanEditorData, planEditorVisible],
  );


  const handleOpenModal = useCallback((id: string) => {
    const chapter = chaptersByIdRef.current[id];
    if (!chapter) {
      return;
    }

    openChapterModalWorkflow({
      chapterId: chapter.id,
      chapters: [chapter],
      form,
      setEditingId,
      setIsModalOpen,
    });

  }, [form]);



  const handleSubmit = async (values: ChapterUpdate) => {

    await submitChapterModalWorkflow({
      editingId,
      values,
      updateChapter,
      refreshChapters,
      setIsModalOpen,
      form,
    });

  };



  const handleOpenEditor = useCallback((id: string) => {
    const chapter = chaptersByIdRef.current[id];
    if (!chapter) {
      return;
    }

    openChapterEditorWorkflow({
      chapterId: chapter.id,
      chapters: [chapter],
      editorForm,
      setCurrentChapter,
      resetSingleStoryCreationCockpit,
      setEditingId,
      setIsEditorOpen,
      setChapterQualityMetrics,
      loadAvailableModels,
    });

  }, [editorForm, loadAvailableModels, resetSingleStoryCreationCockpit, setCurrentChapter]);



  const handleEditorSubmit = async (values: ChapterUpdate) => {

    await submitChapterEditorWorkflow({
      editingId,
      currentProjectId: currentProjectId ?? undefined,
      values,
      updateChapter,
      setCurrentProject,
      setChapterQualityMetrics,
      setIsEditorOpen,
      setEditingId,
      setCurrentChapter,
    });

  };



  const handleGenerate = async () => {
    await startSingleChapterGenerationWorkflow({
      editingId,
      runningSingleChapterTasks,
      saveSingleStoryCreationSnapshot,
      setIsContinuing,
      loadSingleStoryPresetState,
      resolveStoryCreationPromptState,
      singleStoryCreationBriefDraft,
      projectDefaultStoryCreationBrief,
      singleStoryBeatPlannerDraft,
      singleStorySceneOutlineDraft,
      selectedStyleId,
      targetWordCount,
      selectedModel,
      temporaryNarrativePerspective,
      selectedCreativeMode,
      selectedStoryFocus,
      selectedPlotStage,
      selectedQualityPreset,
      selectedQualityNotes,
      generateChapterContentStream,
      currentProjectId: currentProjectId ?? undefined,
      isPageActiveRef,
      editorForm,
      isEditorOpenRef,
      editingChapterIdRef,
      updateAnalysisTasksMap,
      startPollingTask,
      setRunningSingleChapterTasks,
      setChapterQualityRefreshToken,
    });
  };
  const showGenerateModal = async (chapter: Chapter) => {
    await openSingleChapterGenerateWorkflow({
      modal,
      chapter,
      sortedChapters,
      writingStyles,
      selectedStyleId,
      selectedCreativeMode,
      selectedStoryFocus,
      selectedPlotStage,
      targetWordCount,
      handleGenerate,
      message,
    });
  };
  const handleBatchGenerate = async (values: BatchGenerateFormValues) => {
    if (!currentProjectId) return;

    await startBatchGenerationWorkflow({
      values,
      projectId: currentProjectId,
      selectedStyleId,
      targetWordCount,
      model: batchSelectedModel,
      creativeMode: batchSelectedCreativeMode,
      storyFocus: batchSelectedStoryFocus,
      plotStage: batchSelectedPlotStage,
      qualityPreset: batchSelectedQualityPreset,
      qualityNotes: batchSelectedQualityNotes,
      qualityMetricsSummary: batchQualityMetricsSummary,
      batchStoryCreationBriefDraft,
      batchDefaultStoryCreationBrief,
      batchStoryBeatPlannerDraft,
      batchStorySceneOutlineDraft,
      knownStructureChapterCount,
      resolveStoryCreationPromptState,
      resolveCreationPresetByModes,
      saveStoryCreationSnapshot: saveBatchStoryCreationSnapshot,
      setBatchGenerating,
      setBatchGenerateVisible,
      setBatchTaskId,
      rememberTaskMeta: (taskId, taskMeta) => {
        batchTaskMetaRef.current[taskId] = taskMeta;
      },
      persistTaskMeta: persistChapterBatchTaskMeta,
      setBatchProgress,
      startBatchPolling,
      showBrowserNotification,
    });
  };






  const startBatchPolling = (taskId: string) => {
    startBatchPollingWorkflow({
      taskId,
      projectId: currentProjectId ?? undefined,
      projectTitle: currentProjectTitle || undefined,
      batchPollingIntervalRef,
      batchCloseTimeoutRef,
      batchTaskMetaRef,
      normalizeBatchGenerationCheckpoint,
      refreshChapters,
      loadAnalysisTasks,
      reloadCurrentProject,
      setBatchProgress,
      setBatchGenerating,
      getPersistedTaskMeta: getPersistedChapterBatchTaskMeta,
      removePersistedTaskMeta: removePersistedChapterBatchTaskMeta,
      triggerDeferredBatchAnalysis,
      showBrowserNotification,
      setBatchGenerateVisible,
      setBatchTaskId,
      isPollingSessionActive: () => (
        isPageActiveRef.current
        && currentProjectIdRef.current === currentProjectId
        && batchTaskIdRef.current === taskId
      ),
    });
  };
  const handleCancelBatchGenerate = async () => {
    await cancelBatchGenerationWorkflow({
      batchTaskId,
      projectId: currentProjectId ?? undefined,
      isPageActiveRef,
      currentProjectIdRef,
      removeTaskMeta: (taskId) => {
        delete batchTaskMetaRef.current[taskId];
        removePersistedChapterBatchTaskMeta(taskId);
      },
      refreshChapters,
      loadAnalysisTasks,
      reloadCurrentProject,
    });
  };



  const handleOpenBatchGenerate = async () => {
    await openBatchGenerationWorkflow({
      batchGenerating,
      firstIncompleteChapter,
      canGenerateChapter,
      getGenerateDisabledReason,
      loadAvailableModels,
      resetBatchStoryCreationCockpit,
      setBatchSelectedModel,
      setBatchSelectedPlotStage,
      projectDefaultPlotStage,
      inferPlotStage,
      knownStructureChapterCount,
      batchForm,
      selectedStyleId,
      cachedWordCount: getCachedWordCount(),
      setBatchGenerateVisible,
    });
  };




  const getStatusText = (status: string) => {
    const texts: Record<string, string> = {
      draft: "Draft",
      writing: "Writing",
      completed: "Completed",
    };

    return texts[status] || status;
  };

  const handleExport = () => {
    confirmChapterExportWorkflow({
      currentProject: useStore.getState().currentProject,
      chapterCount: chapters.length,
      modal,
      message,
    });
  };

  const handleShowAnalysis = useCallback((chapterId: string) => {
    setAnalysisChapterId(chapterId);
    setAnalysisVisible(true);
  }, []);

  const showManualCreateChapterModal = async () => {
    await openManualCreateChapterWorkflow({
      modal,
      chapters,
      manualCreateForm,
      sortedOutlines,
      currentProject: useStore.getState().currentProject,
      refreshChapters,
      setCurrentProject,
      message,
      handleDeleteChapter,
      getStatusText,
    });
  };

  const handleDeleteChapter = useCallback(async (chapterId: string) => {
    await deleteChapterWithRefreshWorkflow({
      chapterId,
      deleteChapter,
      refreshChapters,
      reloadCurrentProject,
      onSuccess: () => {
        message.success("Chapter deleted.");
      },
      onError: (error) => {
        message.error("Delete chapter failed: " + (error.message || "Unknown error"));
      },
    });
  }, [deleteChapter, refreshChapters, reloadCurrentProject]);

  const showExpansionPlanModal = useCallback(async (chapter: Chapter) => {
    await openExpansionPlanPreviewWorkflow({
      modal,
      chapter,
      isMobile,
      message,
    });
  }, [isMobile, modal]);

  const handleOpenPlanEditor = useCallback((chapter: Chapter) => {
    openChapterPlanEditor({
      chapter,
      setEditingPlanChapter,
      setPlanEditorVisible,
    });
  }, []);

  const handleClosePlanEditor = useCallback(() => {
    closeChapterPlanEditor({
      setEditingPlanChapter,
      setPlanEditorVisible,
    });
  }, []);

  const handleSavePlan = async (planData: ExpansionPlanData) => {
    if (!editingPlanChapter) {
      return;
    }

    await saveChapterPlan({
      chapterId: editingPlanChapter.id,
      planData,
      refreshChapters,
      closePlanEditor: handleClosePlanEditor,
    });
  };



  const handleChapterSelect = useCallback((chapterId: string) => {
    selectChapterListItem({ chapterId });
  }, []);



  // 浮动索引面板相关绑定
  const {
    floatingIndexPanelState,
    floatingIndexPanelTriggerProps,
    handleCloseIndexPanel,
  } = useFloatingIndexPanelBindings({
    groupedChapters,
    icon: <BookOutlined />,
    isMobile,
  });


  const handleOpenReader = useCallback((chapter: Chapter) => {

    openChapterReader({
      chapter,
      setReadingChapter,
      setReaderVisible,
    });

  }, []);




  const handleReaderChapterChange = async (chapterId: string) => {

    await loadReaderChapter({
      chapterId,
      setReadingChapter,
    });

  };

  const handleCloseReader = useCallback(() => {
    closeChapterReader({
      setReadingChapter,
      setReaderVisible,
    });
  }, []);


  // 关闭编辑器


  const handleCloseEditor = useCallback(() => {
    closeChapterEditor({
      setChapterQualityMetrics,
      setIsEditorOpen,
      setEditingId,
      setCurrentChapter,
    });
  }, [setCurrentChapter]);

  const editorAiSectionProps = useMemo(() => {
    if (!isEditorOpen) {
      return null;
    }

    return {
      currentEditingChapterNumber: currentEditingChapter?.chapter_number,
      applySingleCreationPreset,
      projectDefaultCreativeMode,
      setSelectedCreativeMode,
      projectDefaultStoryFocus,
      setSelectedStoryFocus,
      projectDefaultPlotStage,
      selectedPlotStage,
      setSelectedPlotStage,
      projectDefaultQualityPreset,
      projectDefaultQualityNotes,
      selectedQualityPreset,
      setSelectedQualityPreset,
      selectedQualityNotes,
      setSelectedQualityNotes,
      singleStoryCreationControlCard,
      isSingleStoryCreationControlCustomized,
      setSingleStoryCreationBriefDraft,
      singleSystemStoryCreationBrief,
      singleStoryCreationBriefDraft,
      isSingleStoryCreationBriefCustomized,
      singleStoryBeatPlannerDraft,
      setSingleStoryBeatPlannerDraft,
      singleSystemStoryBeatPlanner,
      isSingleStoryBeatPlannerCustomized,
      isSingleStorySceneOutlineCustomized,
      setSingleStorySceneOutlineDraft,
      singleSuggestedStorySceneOutline,
      singleStorySceneOutlineDraft,
      resolvedSingleStoryCreationBrief,
      singleStoryCreationPromptLayerLabels,
      singleStoryCreationPromptCharCount,
      isSingleStoryCreationPromptVerbose,
      copyStoryCreationPrompt,
      singleStoryCreationSnapshots,
      singleStoryCreationCurrentDraft,
      canSaveSingleStoryCreationSnapshot,
      saveSingleStoryCreationSnapshot,
      applySingleStoryCreationSnapshot,
      deleteSingleStoryCreationSnapshot,
      singleStoryAcceptanceCard,
      singleStoryCharacterArcCard,
      singleStoryExecutionChecklist,
      singleStoryObjectiveCard,
      singleStoryRepairTargetCard,
      singleStoryRepetitionRiskCard,
      singleStoryResultCard,
      isMobile,
      targetWordCount,
      CREATIVE_MODE_OPTIONS,
      selectedCreativeMode,
      STORY_FOCUS_OPTIONS,
      selectedStoryFocus,
      availableModels,
      selectedModel,
      setSelectedModel,
      setTargetWordCount,
      currentEditingChapterId: currentEditingChapter?.id,
      chapterQualityRefreshToken,
      onChapterQualityMetricsChange: setChapterQualityMetrics,
      knownStructureChapterCount,
    };
  }, [
    applySingleCreationPreset,
    applySingleStoryCreationSnapshot,
    availableModels,
    canSaveSingleStoryCreationSnapshot,
    chapterQualityRefreshToken,
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    deleteSingleStoryCreationSnapshot,
    isEditorOpen,
    isMobile,
    isSingleStoryBeatPlannerCustomized,
    isSingleStoryCreationBriefCustomized,
    isSingleStoryCreationControlCustomized,
    isSingleStoryCreationPromptVerbose,
    isSingleStorySceneOutlineCustomized,
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryFocus,
    resolvedSingleStoryCreationBrief,
    saveSingleStoryCreationSnapshot,
    selectedCreativeMode,
    selectedModel,
    selectedPlotStage,
    selectedQualityNotes,
    selectedQualityPreset,
    selectedStoryFocus,
    setSelectedCreativeMode,
    setSelectedModel,
    setSelectedPlotStage,
    setSelectedQualityNotes,
    setSelectedQualityPreset,
    setSelectedStoryFocus,
    setSingleStoryBeatPlannerDraft,
    setSingleStoryCreationBriefDraft,
    setSingleStorySceneOutlineDraft,
    setTargetWordCount,
    singleStoryAcceptanceCard,
    singleStoryBeatPlannerDraft,
    singleStoryCharacterArcCard,
    singleStoryCreationBriefDraft,
    singleStoryCreationControlCard,
    singleStoryCreationCurrentDraft,
    singleStoryCreationPromptCharCount,
    singleStoryCreationPromptLayerLabels,
    singleStoryCreationSnapshots,
    singleStoryExecutionChecklist,
    singleStoryObjectiveCard,
    singleStoryRepairTargetCard,
    singleStoryRepetitionRiskCard,
    singleStoryResultCard,
    singleStorySceneOutlineDraft,
    singleSuggestedStorySceneOutline,
    singleSystemStoryBeatPlanner,
    singleSystemStoryCreationBrief,
    targetWordCount,
  ]);

  const editorModalContentProps = useMemo(() => {
    if (!isEditorOpen || !editorAiSectionProps) {
      return null;
    }

    return {
      editorForm,
      handleEditorSubmit,
      isMobile,
      currentEditingChapter,
      currentEditingCanGenerate,
      currentEditingGenerateDisabledReason,
      showGenerateModal,
      isContinuing,
      canAnalyzeCurrentChapter,
      handleShowAnalysis,
      selectedStyleId,
      setSelectedStyleId,
      writingStyles,
      currentProjectNarrativePerspective: currentProjectNarrativePerspective || undefined,
      temporaryNarrativePerspective,
      setTemporaryNarrativePerspective,
      selectedPlotStage,
      setSelectedPlotStage,
      applyInferredSinglePlotStage,
      aiSectionProps: editorAiSectionProps,
      onCloseEditor: handleCloseEditor,
    };
  }, [
    applyInferredSinglePlotStage,
    canAnalyzeCurrentChapter,
    currentEditingCanGenerate,
    currentEditingChapter,
    currentEditingGenerateDisabledReason,
    currentProjectNarrativePerspective,
    editorAiSectionProps,
    editorForm,
    handleCloseEditor,
    handleEditorSubmit,
    handleShowAnalysis,
    isContinuing,
    isEditorOpen,
    isMobile,
    selectedPlotStage,
    selectedStyleId,
    showGenerateModal,
    temporaryNarrativePerspective,
    writingStyles,
  ]);

  const batchGenerateModalProps: ChapterBatchGenerateModalProps | null = useMemo(() => {
    if (!batchGenerateVisible && !batchGenerating) {
      return null;
    }

    return {
      applyBatchCreationPreset,
      applyBatchStoryCreationSnapshot,
      applyInferredBatchPlotStage,
      availableModels,
      batchEnableAnalysis,
      batchForm,
      batchGenerateVisible,
      batchGenerating,
      batchSelectedCreativeMode,
      batchSelectedModel,
      batchSelectedPlotStage,
      batchSelectedQualityNotes,
      batchSelectedQualityPreset,
      batchSelectedStoryFocus,
      batchStartChapterOptions,
      batchStoryBeatPlannerDraft,
      batchStoryCreationBriefDraft,
      batchStoryCreationCurrentDraft,
      batchStoryCreationSnapshots,
      batchStorySceneOutlineDraft,
      batchSuggestedStorySceneOutline,
      batchSystemStoryBeatPlanner,
      canSaveBatchStoryCreationSnapshot,
      copyStoryCreationPrompt,
      CREATIVE_MODE_OPTIONS,
      deleteBatchStoryCreationSnapshot,
      handleBatchGenerate,
      handleCancelBatchGenerate,
      isBatchStoryBeatPlannerCustomized,
      isBatchStoryCreationBriefCustomized,
      isBatchStoryCreationControlCustomized,
      isBatchStorySceneOutlineCustomized,
      isMobile,
      modal,
      knownStructureChapterCount,
      projectDefaultCreativeMode,
      projectDefaultPlotStage,
      projectDefaultQualityNotes,
      projectDefaultQualityPreset,
      projectDefaultStoryFocus,
      resolvedBatchStoryCreationBrief,
      batchStoryCreationPromptLayerLabels,
      batchStoryCreationPromptCharCount,
      isBatchStoryCreationPromptVerbose,
      STORY_CREATION_PROMPT_WARN_THRESHOLD,
      saveBatchStoryCreationSnapshot,
      selectedModel,
      selectedStyleId,
      setBatchGenerateVisible,
      setBatchSelectedCreativeMode,
      setBatchSelectedModel,
      setBatchSelectedPlotStage,
      setBatchSelectedQualityNotes,
      setBatchSelectedQualityPreset,
      setBatchSelectedStoryFocus,
      setBatchStoryBeatPlannerDraft,
      setBatchStoryCreationBriefDraft,
      setBatchStorySceneOutlineDraft,
      sortedChapters,
      STORY_FOCUS_OPTIONS,
      writingStyles,
    };
  }, [
    applyBatchCreationPreset,
    applyBatchStoryCreationSnapshot,
    applyInferredBatchPlotStage,
    availableModels,
    batchEnableAnalysis,
    batchForm,
    batchGenerateVisible,
    batchGenerating,
    batchSelectedCreativeMode,
    batchSelectedModel,
    batchSelectedPlotStage,
    batchSelectedQualityNotes,
    batchSelectedQualityPreset,
    batchSelectedStoryFocus,
    batchStartChapterOptions,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationCurrentDraft,
    batchStoryCreationPromptCharCount,
    batchStoryCreationPromptLayerLabels,
    batchStoryCreationSnapshots,
    batchStorySceneOutlineDraft,
    batchSuggestedStorySceneOutline,
    batchSystemStoryBeatPlanner,
    canSaveBatchStoryCreationSnapshot,
    copyStoryCreationPrompt,
    deleteBatchStoryCreationSnapshot,
    handleBatchGenerate,
    handleCancelBatchGenerate,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStoryCreationControlCustomized,
    isBatchStoryCreationPromptVerbose,
    isBatchStorySceneOutlineCustomized,
    isMobile,
    knownStructureChapterCount,
    modal,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryFocus,
    resolvedBatchStoryCreationBrief,
    saveBatchStoryCreationSnapshot,
    selectedModel,
    selectedStyleId,
    sortedChapters,
    writingStyles,
  ]);
  if (!currentProjectId || !currentProjectOutlineMode) return null;

  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 74%, #6f4638 26%) 0%,
    color-mix(in srgb, ${token.colorInfo} 26%, #18242d 74%) 100%)`;
  const editorialInk = '#fff9f0';
  const actionButtonStyle = {
    borderRadius: 999,
    height: 42,
    paddingInline: 16,
    borderColor: 'rgba(255,255,255,0.18)',
    background: 'rgba(255,255,255,0.08)',
    color: editorialInk,
    boxShadow: 'none',
  } as const;
  const panelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 95%, white 5%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 44%, ${token.colorBgContainer} 56%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
  const groupOrOutlineCount = currentProjectOutlineMode === 'one-to-many' ? groupedChapters.length : sortedChapters.length;
  const canGenerateCount = Object.values(chapterGenerationStateById).filter((item) => item.canGenerate).length;
  const chapterSummaryItems: Array<{ label: string; value: number | string; accent: string; compact?: boolean }> = [
    { label: '章节总数', value: sortedChapters.length, accent: editorialInk },
    { label: currentProjectOutlineMode === 'one-to-many' ? '大纲分组' : '章节序列', value: groupOrOutlineCount, accent: token.colorSuccess },
    { label: '可生成项', value: canGenerateCount, accent: token.colorInfo },
    { label: '当前模式', value: currentProjectOutlineMode === 'one-to-one' ? '一纲一章' : '一纲多章', accent: editorialInk, compact: true },
  ];

  const chapterGuideSteps = [
    '先看概览卡与当前模式，确认现在是在逐章推进还是按大纲分组管理。',
    '再从章节台账进入阅读、编辑、分析或计划入口，先判断当前章节链路卡在哪一步。',
    '最后再发起新建、批量生成或导出，把高影响操作放在看清上下文之后。',
  ];
  const activeSingleGenerationCount = Object.keys(runningSingleChapterTasks).length;
  const chapterWorkspaceFocus = batchGenerating || batchTaskId
    ? {
        title: '等待批量生成结果回流',
        note: '当前有一条批量生成任务在执行，适合先观察进度与失败提示，等新章节回流后再统一巡检内容。',
      }
    : batchGenerateVisible
      ? {
          title: '确认本轮批量生成范围',
          note: '批量生成面板已经打开，先核对起始章节、模型和创作设定，再决定是否正式启动整批任务。',
        }
      : isEditorOpen && currentEditingChapter
        ? {
            title: `正在编辑第 ${currentEditingChapter.chapter_number} 章`,
            note: '当前更适合先完成正文修订，再回到台账决定是否继续分析、规划或生成后续章节。',
          }
        : analysisVisible
          ? {
              title: '核对章节分析结果',
              note: '分析面板已打开，先看问题标签与建议，再回到章节台账决定下一步修订或生成动作。',
            }
          : readerVisible && readingChapter
            ? {
                title: `回看第 ${readingChapter.chapter_number} 章正文`,
                note: '阅读面板正在承接正文与标注复盘，适合先确认这一章是否稳定，再继续切换其他工作流。',
              }
            : planEditorVisible && editingPlanChapter
              ? {
                  title: `整理第 ${editingPlanChapter.chapter_number} 章计划`,
                  note: '当前正在补章节计划，先把结构与目标对齐，再返回列表推进生成或内容修订。',
                }
              : activeSingleGenerationCount > 0
                ? {
                    title: '等待单章生成完成',
                    note: `当前有 ${activeSingleGenerationCount} 条单章生成任务在运行，适合先保持章节顺序稳定，等待结果回流后再统一处理。`,
                  }
                : firstIncompleteChapter
                  ? {
                      title: `优先补齐第 ${firstIncompleteChapter.chapter_number} 章`,
                      note: '当前最顺手的路径是从第一条未完成章节继续推进，避免越过前序章节后再回头补内容。',
                    }
                  : sortedChapters.length > 0
                    ? {
                        title: '巡检当前章节台账',
                        note: '章节内容已经具备基础规模，适合结合列表状态、阅读入口和分析入口做一次整体排布检查。',
                      }
                    : {
                        title: '建立第一章骨架',
                        note: '当前还没有章节内容，建议先创建首章或从批量生成入口起步，再逐步补齐后续工作流。',
                      };

  return (
    <>
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: 16, overflow: 'hidden', paddingBottom: 24 }}>

      {contextHolder}

      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
        }}
        styles={{ body: { padding: isMobile ? 20 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -28, width: 170, height: 170, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: isMobile ? '56%' : '28%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={14}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Chapter Workspace
              </Text>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                章节管理
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                在这里统一管理章节列表、章节生成、导出与章节分析入口。它应该像创作中的章节台账，而不是单纯的列表页。
              </Paragraph>
              <Space wrap size={[10, 10]}>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {currentProjectTitle || '当前项目'}
                </Tag>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {currentProjectOutlineMode === 'one-to-one' ? 'One outline per chapter' : 'Grouped outline mode'}
                </Tag>
              </Space>
            </Space>
          </Col>
          <Col xs={24} lg={10}>
            <Row gutter={[12, 12]}>
              {chapterSummaryItems.map((item) => (
                <Col xs={12} key={item.label}>
                  <div
                    style={{
                      minHeight: 92,
                      borderRadius: 18,
                      padding: '12px 14px',
                      background: 'rgba(255,255,255,0.08)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      backdropFilter: 'blur(10px)',
                      display: 'flex',
                      flexDirection: 'column',
                      justifyContent: 'space-between',
                    }}
                  >
                    <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12, display: 'block' }}>{item.label}</Text>
                    <Text style={{ color: item.accent, fontWeight: 700, fontSize: item.compact ? 15 : 24, lineHeight: 1.2, wordBreak: 'break-word' }}>
                      {item.value}
                    </Text>
                  </div>
                </Col>
              ))}
            </Row>
          </Col>
        </Row>

        <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1, width: isMobile ? '100%' : 'auto' }}>

          {currentProjectOutlineMode === 'one-to-many' && (

            <Button
              icon={<PlusOutlined />}
              onClick={showManualCreateChapterModal}
              style={actionButtonStyle}
            >
              新建章节
            </Button>

          )}

          <Button

            type="primary"

            icon={<RocketOutlined />}

            onClick={handleOpenBatchGenerate}

            disabled={chapters.length === 0}

            style={{ background: '#722ed1', borderColor: '#722ed1' }}

          >

            批量生成

          </Button>

          <Button

            type="default"

            icon={<DownloadOutlined />}

            onClick={handleExport}

            disabled={chapters.length === 0}

            style={actionButtonStyle}
          >

            导出

          </Button>

        </Space>
      </Card>

      <Card
        variant="borderless"
        style={{
          borderRadius: 22,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 10%, white 90%) 0%, color-mix(in srgb, ${token.colorInfo} 10%, white 90%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 16%, white 84%)`,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { padding: isMobile ? 16 : 18 } }}
      >
        <Row gutter={[16, 16]}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                Chapter Guide
              </Text>
              <Paragraph style={{ margin: 0, color: token.colorText, lineHeight: 1.75 }}>
                这个页面更像章节调度与创作巡检的总控台。原有的新建、批量生成、阅读、分析、计划与导出流程都保持不变，这里只把先看什么、再做什么的顺序说明得更清楚。
              </Paragraph>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {chapterGuideSteps.map((item, index) => (
                  <span
                    key={item}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 8,
                      padding: '6px 12px',
                      borderRadius: 999,
                      background: token.colorBgContainer,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      color: token.colorTextBase,
                      fontSize: 12,
                    }}
                  >
                    <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                    {item}
                  </span>
                ))}
              </div>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <div
              style={{
                height: '100%',
                borderRadius: 18,
                padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Text style={{ display: 'block', color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                当前工作焦点
              </Text>
              <Title level={5} style={{ margin: '8px 0 6px', color: token.colorTextBase, fontFamily: designDisplayFont }}>
                {chapterWorkspaceFocus.title}
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                {chapterWorkspaceFocus.note}
              </Paragraph>
            </div>
          </Col>
        </Row>
      </Card>

      <Card
        variant="borderless"
        style={{
          flex: 1,
          overflow: 'hidden',
          background: panelBackground,
          borderRadius: 24,
          border: panelBorder,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { height: '100%', padding: isMobile ? 16 : 20 } }}
      >
        <Space direction="vertical" size={16} style={{ width: '100%', height: '100%' }}>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: isMobile ? 'flex-start' : 'center',
              gap: 12,
              flexDirection: isMobile ? 'column' : 'row',
            }}
          >
            <Space direction="vertical" size={4}>
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Chapter Ledger
              </Text>
              <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                章节列表工作区
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary }}>
                保留原有批量生成、分析、阅读与计划编辑流程，只升级外层布局和信息层级，让章节工作台更像可持续维护的编辑面板。
              </Paragraph>
            </Space>
            <Tag color="blue" style={{ borderRadius: 999, paddingInline: 12 }}>
              {sortedChapters.length > 0 ? `已载入 ${sortedChapters.length} 个章节` : '尚无章节'}
            </Tag>
          </div>

          <Divider style={{ margin: 0, borderColor: token.colorBorderSecondary }} />

      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0, paddingRight: isMobile ? 0 : 4 }}>
        <ChapterListSection
          sortedChapters={sortedChapters}
          outlineMode={currentProjectOutlineMode}
          groupedChapters={groupedChapters}
          expandedChapterGroupKeys={expandedChapterGroupKeys}
          isMobile={isMobile}
          chapterGenerationStateById={chapterGenerationStateById}
          onOpenReader={handleOpenReader}
          onOpenEditor={handleOpenEditor}
          onShowAnalysis={handleShowAnalysis}
          onOpenSettings={handleOpenModal}
          onDeleteChapter={handleDeleteChapter}
          onShowExpansionPlan={showExpansionPlanModal}
          onOpenPlanEditor={handleOpenPlanEditor}
        />
      </div>
        </Space>
      </Card>

      <ChapterBasicModalEntry
        open={isModalOpen}
        title={editingId ? "编辑章节" : "新建章节"}
        isMobile={isMobile}
        outlineMode={currentProjectOutlineMode}
        submitText={editingId ? "保存修改" : "创建章节"}
        form={form}
        onCancel={() => setIsModalOpen(false)}
        onFinish={handleSubmit}
      />

      {isEditorOpen ? (
        <Modal

        title={'编辑章节内容'}

        open={isEditorOpen}

        onCancel={handleCloseEditor}

        closable

        maskClosable={false}

        keyboard

        width={isMobile ? 'calc(100vw - 32px)' : '85%'}

        centered

        style={isMobile ? {

          maxWidth: 'calc(100vw - 32px)',

          margin: '0 auto',

          padding: '0 16px'

        } : undefined}

        styles={{

          body: {

            maxHeight: isMobile ? 'calc(100dvh - 200px)' : 'calc(100dvh - 110px)',

            overflowY: 'auto',

            padding: isMobile ? '16px 12px' : '8px'

          }

        }}

        footer={null}

      >

        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Editor Workspace"
              title="正在接管章节正文编辑工作区"
              message="系统正在恢复正文编辑、续写入口、分析链路和局部重写工具条，原有编辑状态与提交逻辑保持不变。"
              tags={[
                { label: '章节编辑', color: 'blue' },
                { label: '工作区恢复中', color: 'processing' },
                { label: '提交逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyChapterEditorModalContent
            contentProps={editorModalContentProps}
          />
        </Suspense>

      </Modal>
      ) : null}




      <ChapterAnalysisEntry
        chapterId={analysisChapterId}
        visible={analysisVisible}
        onClose={handleCloseAnalysis}
      />
      {batchGenerateModalProps ? (
        <ChapterBatchGenerateModalEntry
          visible={batchGenerateVisible || batchGenerating}
          modalProps={batchGenerateModalProps}
        />
      ) : null}
      <SingleChapterGenerationOverlayEntry />
      <ChapterBatchProgressEntry
        visible={batchGenerating}
        buildCheckpointHint={buildBatchGenerationCheckpointHint}
        onCancel={() => {
          modal.confirm({
            title: '取消批量生成',
            content: '确定要停止当前批量生成任务吗？',
            okText: '停止任务',
            cancelText: '继续运行',
            okButtonProps: { danger: true },
            centered: true,
            onOk: handleCancelBatchGenerate,
          });
        }}
      />

      <FloatingIndexPanelEntry
        floatingIndexPanelState={floatingIndexPanelState}
        floatingIndexPanelTriggerProps={floatingIndexPanelTriggerProps}
        onClose={handleCloseIndexPanel}
        onChapterSelect={handleChapterSelect}
      />




      {chapterReaderModalState ? (
        <ChapterReaderEntry
          chapterReaderModalState={chapterReaderModalState}
          onClose={handleCloseReader}
          onChapterChange={handleReaderChapterChange}
        />
      ) : null}




      {planEditorModalState ? (
        <ChapterPlanEditorEntry
          planEditorModalState={planEditorModalState}
          onSave={handleSavePlan}
          onCancel={handleClosePlanEditor}
        />
      ) : null}

    </div>
    </>
  );

}
