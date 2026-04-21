import { Suspense, lazy, useState, useEffect, useRef, useMemo, useCallback } from 'react';

import { Button, Modal, Form, message, Space, Tag } from 'antd';

import { DownloadOutlined, RocketOutlined, BookOutlined, PlusOutlined } from '@ant-design/icons';

import { useStore } from '../store';
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
import { formatActiveStoryRepairLabel } from '../utils/activeStoryRepair';
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
      return 'Single pass';
    case 'rerank_retry':
      return 'Rerank retry';
    case 'word_budget_repair':
      return 'Word budget repair';
    default:
      return value ? value : '';
  }
};

const buildBatchGenerationCheckpointHint = (checkpoint?: BatchGenerationCheckpoint | null): string => {
  if (!checkpoint) return '';
  const parts: string[] = [];
  if (typeof checkpoint.candidate_index === 'number' && typeof checkpoint.candidate_count === 'number') {
    parts.push(`Candidate ${checkpoint.candidate_index}/${checkpoint.candidate_count}`);
  }
  if (typeof checkpoint.word_count === 'number' && checkpoint.word_count > 0) {
    parts.push(`${checkpoint.word_count} words`);
  }
  const generationPathLabel = getBatchGenerationPathLabel(checkpoint.generation_path);
  if (generationPathLabel) {
    parts.push(`Path: ${generationPathLabel}`);
  }
  if (typeof checkpoint.winner_candidate_index === 'number') {
    parts.push(`Winner: ${checkpoint.winner_candidate_index}`);
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

export default function Chapters() {

  const currentProject = useStore((state) => state.currentProject);
  const projectDefaultCreativeMode = currentProject?.default_creative_mode;
  const projectDefaultStoryFocus = currentProject?.default_story_focus;
  const projectDefaultPlotStage = currentProject?.default_plot_stage;
  const projectDefaultStoryCreationBrief = currentProject?.default_story_creation_brief?.trim() ?? '';
  const projectDefaultQualityPreset = currentProject?.default_quality_preset;
  const projectDefaultQualityNotes = currentProject?.default_quality_notes?.trim() ?? '';

  const chapters = useStore((state) => state.chapters);

  const outlines = useStore((state) => state.outlines);

  const setCurrentChapter = useStore((state) => state.setCurrentChapter);

  const setCurrentProject = useStore((state) => state.setCurrentProject);

  const [modal, contextHolder] = Modal.useModal();

  const [isModalOpen, setIsModalOpen] = useState(false);

  const [isEditorOpen, setIsEditorOpen] = useState(false);

  const [isContinuing, setIsContinuing] = useState(false);

  const [isGenerating, setIsGenerating] = useState(false);

  const [editingId, setEditingId] = useState<string | null>(null);

  const editingChapterIdRef = useRef<string | null>(null);

  const isEditorOpenRef = useRef(false);

  const [runningSingleChapterTasks, setRunningSingleChapterTasks] = useState<Record<string, string>>({});

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


  const [analysisTasksMap, setAnalysisTasksMap] = useState<Record<string, AnalysisTask>>({});
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
    setAnalysisTasksMap((prev) => {
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




  const [singleChapterProgress, setSingleChapterProgress] = useState(0);
  const [singleChapterProgressMessage, setSingleChapterProgressMessage] = useState('');
  const [chapterQualityMetrics, setChapterQualityMetrics] = useState<ChapterQualityMetrics | null>(null);
  const [chapterQualityRefreshToken, setChapterQualityRefreshToken] = useState(0);

  const [batchGenerateVisible, setBatchGenerateVisible] = useState(false);
  const [batchGenerating, setBatchGenerating] = useState(false);
  const [batchTaskId, setBatchTaskId] = useState<string | null>(null);
  const [batchForm] = Form.useForm();
  const [manualCreateForm] = Form.useForm();
  const batchStartChapterNumber = Form.useWatch('startChapterNumber', batchForm) as number | undefined;
  const batchEnableAnalysis = Form.useWatch('enableAnalysis', batchForm) as boolean | undefined;
  const [batchProgress, setBatchProgress] = useState<{
    status: string;

    total: number;

    completed: number;

    current_chapter_number: number | null;

    checkpoint?: BatchGenerationCheckpoint | null;

    estimated_time_minutes?: number;

    latest_quality_metrics?: ChapterLatestQualityMetrics | null;
    quality_metrics_summary?: ChapterQualityMetricsSummary | null;
    quality_profile_summary?: ChapterQualityProfileSummary | null;
    failed_chapters?: Array<Record<string, unknown>>;
    active_story_repair_payload?: ActiveStoryRepairPayload | null;
  } | null>(null);
  const batchProgressRepairLabel = useMemo(
    () => formatActiveStoryRepairLabel(batchProgress?.active_story_repair_payload),
    [batchProgress?.active_story_repair_payload],
  );
  const batchProgressCheckpointLabel = useMemo(
    () => buildBatchGenerationCheckpointHint(batchProgress?.checkpoint),
    [batchProgress?.checkpoint],
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
    () => (currentProject?.id && currentEditingChapter?.id
      ? buildSingleStoryCreationDraftStorageKey(currentProject.id, currentEditingChapter.id)
      : null),
    [currentProject?.id, currentEditingChapter?.id],
  );

  const batchStoryCreationDraftStorageKey = useMemo(
    () => (currentProject?.id ? buildBatchStoryCreationDraftStorageKey(currentProject.id) : null),
    [currentProject?.id],
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
  let cancelled = false;

  if (!isEditorOpen) {
    return () => {
      cancelled = true;
    };
  }

  void loadSingleStoryPresetState()
    .then((nextState) => {
      if (cancelled) {
        return;
      }

      setSingleStoryPresetState(nextState);
    })
    .catch((error) => {
      if (!cancelled) {
        console.error('Failed to load single-story preset state.', error);
      }
    });

  return () => {
    cancelled = true;
  };
}, [isEditorOpen, loadSingleStoryPresetState]);

useEffect(() => {
  let cancelled = false;

  void Promise.all([
    import('../utils/creationPresetsBatch'),
    resolveCreationPresetByModes(batchSelectedCreativeMode, batchSelectedStoryFocus),
  ]).then(([{
    buildBatchSuggestedStorySceneOutline,
    buildBatchSystemStoryBeatPlanner,
    buildBatchSystemStoryCreationBriefFromSummary,
  }, activeBatchCreationPreset]) => {
    if (cancelled) {
      return;
    }

    const nextBatchSystemStoryCreationBrief = buildBatchSystemStoryCreationBriefFromSummary(
      batchProgress?.quality_metrics_summary ?? null,
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
  });

  return () => {
    cancelled = true;
  };
}, [
  batchProgress?.quality_metrics_summary,
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
  () => buildStoryCreationDerivedState({
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
  }),
  [
    currentEditingChapter,
    projectDefaultStoryCreationBrief,
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
  () => buildStoryCreationDerivedState({
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
  }),
  [
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
    let cancelled = false;

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
      isCancelled: () => cancelled,
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
      cancelled = true;
    };
  }, [
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    inferPlotStage,
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
    let cancelled = false;

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
      isCancelled: () => cancelled,
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
      cancelled = true;
    };
  }, [
    batchDefaultStoryCreationBrief,
    batchStoryCreationDraftStorageKey,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultQualityNotes,
    projectDefaultQualityPreset,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetBatchStoryCreationCockpit,
  ]);

  useEffect(() => {
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

  const batchTaskMetaRef = useRef<Record<string, BatchTaskMeta>>({});



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
    initializeChapterProjectWorkflow({
      projectId: currentProject?.id ?? null,
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
  }, [currentProject?.id, setCurrentProject]);

  useEffect(() => {
    const currentBatchPollingIntervalId = batchPollingIntervalRef.current;

    return () => {
      stopAnalysisPolling();

      if (currentBatchPollingIntervalId) {
        clearInterval(currentBatchPollingIntervalId);
      }
    };
  }, [stopAnalysisPolling]);

  const loadAnalysisTasks = async (chaptersToLoad?: typeof chapters) => {
    await loadAnalysisTasksWorkflow({
      projectId: currentProject?.id,
      chapters,
      chaptersToLoad,
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
      currentProjectId: currentProject?.id,
      ensureAnalysisPolling,
    });
  }, [currentProject?.id, ensureAnalysisPolling]);

  const refreshChapterAnalysisTask = useCallback(async (chapterId: string) => {
    await refreshAnalysisTaskWorkflow({
      chapterId,
      currentProjectIdRef,
      currentProjectId: currentProject?.id,
      syncAnalysisTasksFromBatch,
      startPollingTask,
      pollingIntervalsRef,
      stopAnalysisPolling,
      isAnalysisTaskInProgress,
    });
  }, [currentProject?.id, startPollingTask, stopAnalysisPolling, syncAnalysisTasksFromBatch]);
  const reloadCurrentProject = useCallback(async () => {
    await reloadChapterProjectWorkflow({
      projectId: currentProject?.id,
      setCurrentProject,
    });
  }, [currentProject?.id, setCurrentProject]);
  const handleCloseAnalysis = useCallback(() => {
    closeAnalysisWorkflow({
      analysisChapterId,
      projectId: currentProject?.id,
      setAnalysisVisible,
      refreshChapters,
      reloadCurrentProject,
      refreshChapterAnalysisTask,
      setAnalysisChapterId,
    });
  }, [
    analysisChapterId,
    currentProject?.id,
    refreshChapterAnalysisTask,
    refreshChapters,
    reloadCurrentProject,
  ]);
  const triggerDeferredBatchAnalysis = async (

    startChapterNumber: number,

    count: number,

    latestChapters: Chapter[]

  ) => {

    if (!currentProject?.id || count <= 0) return;

    await queueDeferredBatchAnalysis({
      projectId: currentProject.id,
      startChapterNumber,
      count,
      latestChapters,
      analysisTasksMap,
      startPollingTask,
      loadAnalysisTasks,
    });

  };



  const loadWritingStyles = async () => {

    if (!currentProject?.id) return;

    await loadChapterWritingStyles({
      projectId: currentProject.id,
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
      projectId: currentProject?.id,
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

  const {

    sortedChapters,

    groupedChapters,

    chapterGenerationStateById,

    batchStartChapterOptions,

    firstIncompleteChapter,

    expandedChapterGroupKeys,

  } = useMemo(() => {

    const sorted = [...chapters].sort((a, b) => a.chapter_number - b.chapter_number);

    const groups: Record<string, GroupedChapterViewModel> = {};

    const generationStateById: Record<string, { canGenerate: boolean; disabledReason: string }> = {};

    const batchStartOptions: Chapter[] = [];

    let incompletePreviousChapterLabel = '';

    let currentChapterNumber: number | null = null;

    let currentChapterGroup: Array<{ chapter: Chapter; hasContent: boolean }> = [];

    let firstIncompleteChapter: Chapter | undefined;



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



    sorted.forEach(chapter => {

      if (currentChapterNumber !== null && chapter.chapter_number !== currentChapterNumber) {

        flushChapterGroup();

      }

      currentChapterNumber = chapter.chapter_number;

      const key = chapter.outline_id || 'uncategorized';

      const hasContent = Boolean(chapter.content?.trim());

      if (!groups[key]) {

        groups[key] = {

          key,

          outlineId: chapter.outline_id || null,

          outlineTitle: chapter.outline_title || "Untitled outline",

          outlineOrder: chapter.outline_order ?? 999,

          chapters: [],

          totalWordCount: 0,

        };

      }

      groups[key].chapters.push(chapter);

      groups[key].totalWordCount += chapter.word_count || 0;

      if (!firstIncompleteChapter && !hasContent) {

        firstIncompleteChapter = chapter;

      }

      const disabledReason = incompletePreviousChapterLabel

        ? `Complete previous chapters first: ${incompletePreviousChapterLabel}`

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



    const grouped = Object.values(groups).sort((a, b) => a.outlineOrder - b.outlineOrder);

    const expandedChapterGroupKeys = grouped.map((group) => group.key);

    return {

      sortedChapters: sorted,

      groupedChapters: grouped,

      expandedChapterGroupKeys,

      chapterGenerationStateById: generationStateById,

      batchStartChapterOptions: batchStartOptions,

      firstIncompleteChapter,

    };

  }, [chapters]);



  const sortedOutlines = useMemo(
    () => [...outlines].sort((a, b) => a.order_index - b.order_index),
    [outlines]
  );



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
      currentProjectId: currentProject?.id,
    }),
    [currentProject?.id, editingPlanChapter, editingPlanEditorData, planEditorVisible],
  );


  const handleOpenModal = useCallback((id: string) => {

    openChapterModalWorkflow({
      chapterId: id,
      chapters,
      form,
      setEditingId,
      setIsModalOpen,
    });

  }, [chapters, form]);



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

    openChapterEditorWorkflow({
      chapterId: id,
      chapters,
      editorForm,
      setCurrentChapter,
      resetSingleStoryCreationCockpit,
      setEditingId,
      setIsEditorOpen,
      setChapterQualityMetrics,
      loadAvailableModels,
    });

  }, [chapters, editorForm, loadAvailableModels, resetSingleStoryCreationCockpit, setCurrentChapter]);



  const handleEditorSubmit = async (values: ChapterUpdate) => {

    await submitChapterEditorWorkflow({
      editingId,
      currentProjectId: currentProject?.id,
      values,
      updateChapter,
      setCurrentProject,
      setChapterQualityMetrics,
      setIsEditorOpen,
    });

  };



  const handleGenerate = async () => {
    await startSingleChapterGenerationWorkflow({
      editingId,
      runningSingleChapterTasks,
      saveSingleStoryCreationSnapshot,
      setIsContinuing,
      setIsGenerating,
      setSingleChapterProgress,
      setSingleChapterProgressMessage,
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
      currentProjectId: currentProject?.id,
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
    if (!currentProject?.id) return;

    await startBatchGenerationWorkflow({
      values,
      projectId: currentProject.id,
      selectedStyleId,
      targetWordCount,
      model: batchSelectedModel,
      creativeMode: batchSelectedCreativeMode,
      storyFocus: batchSelectedStoryFocus,
      plotStage: batchSelectedPlotStage,
      qualityPreset: batchSelectedQualityPreset,
      qualityNotes: batchSelectedQualityNotes,
      qualityMetricsSummary: batchProgress?.quality_metrics_summary ?? null,
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
      projectId: currentProject?.id,
      projectTitle: currentProject?.title,
      batchPollingIntervalRef,
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
    });
  };
  const handleCancelBatchGenerate = async () => {
    await cancelBatchGenerationWorkflow({
      batchTaskId,
      projectId: currentProject?.id,
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
      currentProject,
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
      currentProject,
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



  const handleChapterSelect = (chapterId: string) => {

    selectChapterListItem({ chapterId });

  };



  // 濠电姷鏁告慨鐑藉极閸涘﹥鍙忛柣鎴ｆ閺嬩線鏌涘☉姗堟敾闁告瑥绻橀弻锝夊箣閿濆棭妫勯梺鍝勵儎缁舵岸寮诲☉妯锋婵鐗婇弫楣冩⒑閸涘﹦鎳冪紒缁橈耿瀵鏁愭径濠勵吅闂佹寧绻傚Λ顓炍涢崟顖涒拺闁告繂瀚烽崕搴ｇ磼閼搁潧鍝虹€殿喖顭烽幃銏ゅ礂鐏忔牗瀚介梺璇查叄濞佳勭珶婵犲伣锝夘敊閸撗咃紲闂佺粯鍔﹂崜娆撳礉閵堝棎浜滄い鎾跺Т閸樺鈧鍠栭…鐑藉极閹邦厼绶炲┑鐘插閺夊憡淇婇悙顏勨偓鏍暜婵犲洦鍊块柨鏇炲€哥壕鍧楁煙閹冾暢缁炬崘妫勯湁闁挎繂鎳忛幆鍫ュ冀閳ユ枼鏀芥い鏃囶潡濠婂牆绀夐柟杈剧畱閺勩儵鏌嶈閸撴岸濡甸崟顖氱闁糕剝銇炴竟鏇㈡⒒娴ｅ憡鍟為拑杈╃磼椤旇姤灏柣锝囧厴楠炲鏁冮埀顒傜不閼姐倗纾藉ù锝堫嚃閻掍粙鏌涘鈧禍璺侯潖濞差亜浼犻柛鏇ㄥ墮閸嬪秹姊洪崨濠冪叆闁哄牜鍓涚划瀣吋婢跺鈧兘鏌ｉ幋鐑嗙劷闁告妫勯埞鎴︽倷閺夋垹浠搁梺缁橆殕濞茬喎鐣峰┑瀣櫆闁兼亽鍎卞鎸庣節閻㈤潧孝闁哥噥鍨舵俊闈涒攽閸艾浜鹃悷娆忓婢跺嫰鏌涚€ｎ亷宸ラ柣锝囧厴閹垻鍠婃潏銊︽珝濠电姰鍨煎▔娑㈡嚐椤栫偛鐓濋柛顐犲劜閻撴盯鎮橀悙鎻掆挃婵炴彃顕埀顒侇問閸ｎ噣宕滃☉銏犵闁告洦鍨版儫闂佽婢橀懟顖炲箠婢舵劕纾婚柟鎹愬煐閸犲棝鏌涢弴銊ュ闁宠绋撶槐鎾诲磼濮樻瘷婊堟煕鐎ｎ偅灏电紒杈ㄦ尰閹峰懘宕崟鎴悼缁辨帞鈧綆鍋勯悘鎾煕閳瑰灝鍔滅€垫澘瀚换娑㈡倷椤掑倵鍋撴繝姘拺闁荤喐澹嗛幗鐘电磼鐠囨彃鈧崵鍒掗鐑嗘僵闁煎摜顣介幏濠氭⒑缁嬫寧婀伴柣鐔濆泚鍥晝閸屾稓鍘甸柣鐘叉厂閸涱垽绱甸梻浣烘嚀缁犲秹宕归挊澶屾殾闁圭儤鍨熼弨锕傛煙椤栧棗鍊搁ˉ姘節濞堝灝鏋熸い顓炴喘瀹曘垼顦叉い鏇秮椤㈡瑩鏌ㄩ姘闁荤喐鐟ョ€氼厾绮堥埀顒勬⒑閸濄儱鏋欐繛澶嬫礋瀹曪綁宕ㄩ褎瀵岄梺闈涚墕閹虫劗绮绘导瀛樼厵闁惧浚鍋勬慨宥団偓瑙勬磸閸ㄤ粙寮婚崱妤婂悑闁糕剝鐟ラ獮宥夋⒒娴ｇ鎮戠紒浣规尦瀵煡鎮欓懜纰夌磽闂傚倸鍊风欢姘跺焵椤掑倸浠滈柤娲诲灡閺呭爼顢涢悙瀵稿幈闂佸湱鍋撳娆撳传閾忓厜鍋撶憴鍕缂佽鍟銉╁礋椤栨氨鐤€濡炪倖鎸荤粙鎺斺偓姘偢濮婂宕掑顑藉亾閹间焦鍋嬪┑鐘插閻瑩鏌熼悜姗嗘當缂佺姴缍婇弻鐔煎箥椤旂⒈鏆梺绋匡工閻忔岸骞堥妸銉建闁糕剝顨呯粻铏圭磽娴ｈ姤纭剧€殿喖鐖兼俊鐢稿礋椤栨艾宓嗗銈呯箰濡稖鈪靛┑锛勫亼閸婃垿宕归崫鍕殕闁归棿绀侀弰銉╂煃瑜滈崜姘跺Φ閸曨垰绠抽柛鈩冦仦婢规洟姊绘担椋庝覆缂佹彃娼″畷妤€顫滈埀顒勬偘椤曗偓瀹曞崬鈽夊▎鎴濆Ш闂備焦瀵ч弻銊ㄣ亹閵娾晛惟闁挎柨澧介惁鍫濃攽閻愯尙澧曢柣蹇旂箞瀵悂鏁傛慨鎰盎濡炪倖鍔戦崺鍕熼埀顒勬⒑闂堟稒顥滈柛鐔告綑閻ｉ攱绺界粙娆炬綂闂佸疇妫勫Λ娆戠礊濡ゅ懏鈷掑ù锝囩摂濞兼劙鏌涙惔銏犫枙闁诡喗妞芥俊鎼佹晝閳ь剟宕归弮鍫熺厵缂備降鍨归弸鐔兼煟閹惧瓨绀嬮柡宀€鍠栭獮鍡氼槻闁哄棜浜槐鎺楁偑濞嗗繑鍣界紒鐘虫閺岀喓鈧數顭堟禒褏绱掗埦鈧崑鎾绘⒒娴ｅ湱婀介柛銊ㄦ椤洩顦崇紒鍌涘笒椤劑宕奸悢鍝勫箺闂備線娼ц噹闁告劑鍔岀粻锝夋⒒娴ｈ櫣銆婇柡鍌欑窔瀹曟垿骞橀幇浣瑰瘜闂侀潧鐗嗗Λ妤冪箔閸屾粎纾奸柍褜鍓氶幏鍛喆閸曨剛褰挎俊鐐€栭悧妤冪矙閹惧墎涓嶅Δ锝呭暞閻撴洟鐓崶銊ㄥ濞存粎鎳撻…鑳檨濞存粠浜璇差吋閸ャ劌鏋傞梺鍛婃处閸嬪棙瀵煎畝鍕拺閻犲洠鈧櫕鐏€闂佸搫鎳忕换鍫ュ春閳ь剚銇勯幒鍡椾壕濠电姭鍋撻弶鍫涘妽閸欏繘鏌熺紒銏犳殙濠㈣泛艌閺€浠嬫倵閿濆骸浜愰柟椋庣帛缁绘稒娼忛崜褎鍋у銈庡幖閻楁捇骞婂鍛枂闁告洦鍘鹃鏇㈡煟閻樺弶鍘傞柛鎰亾閸犳﹢姊绘担鐑樺殌缂佺姴绉瑰畷纭呫亹閹烘垹鍙€婵犮垼鍩栭崝鏇綖閸涘瓨鐓熸俊顖涙た閸熷繘鏌涘顒佸殗婵﹦绮幏鍛存惞閻熸壆顐奸梻浣规偠閸旀垵顭囪閻忓鈹戦悙鏉戠仧闁搞劌婀辩划鍫熷緞閹邦厸鎷洪梺鍓茬厛閸ｎ噣宕曢幋鐘电＜闁绘宕甸悾娲煙椤旂瓔娈滈柟顔挎閳绘挾鎹勯妸銉バ梻浣告惈缁嬪嫮鎹㈠┑鍡╂綎闁惧繐婀遍惌娆撴煕椤垵娅橀柛鏂款槹缁绘繈鎮介棃娑楃捕闂佹寧娲︽禍婊堟偩瀹勬噴娲敂閸涱厸鍋撻悜鑺ョ厱婵犻潧妫楅顐ｃ亜閹惧瓨銇濇慨濠冩そ楠炴劖鎯旈姀鈺傗挅婵犵妲呴崑鍕偓姘煎幘缁顓兼径瀣画闂備緡鍙忛梽鍕偓闈涚焸濮婃椽妫冨☉姘暫濠碘槅鍋呴悷鈺勬＂闂佺硶鍓濈粙鎺楁偂閺囥垺鍊甸柨婵嗛娴滄粌鈹戦鑲╁ⅱ缂佽鲸甯″畷鎺戔槈濡槒鐧佹俊銈囧Х閸嬫稑螞濠婂煪銊︽媴閸︻厾顔曢梺鍦亾閸撴岸鎮℃總鍛婄厸閻忕偟鍋撶粈瀣偓瑙勬礈閸樠囧煘閹寸姭鍋撻敐鍥舵毌闁稿鎸歌灃濞撴艾娲﹂鏃堟⒑缂佹ê濮囨い鏇ㄥ幗閺呭爼顢楅崒婊咃紲缂傚倷鐒﹂…鍥Υ閹烘嚚褰掓偑閸涱垳鏆ら悗瑙勬礃鐢帡锝炲┑瀣垫晢濠㈣泛澶囬崑鎾诲箮閼恒儮鎷洪梺鍛婄☉椤剙鈻撳鍏犵懓顭ㄩ崘顏勭厽閻庢鍠栭…鐑藉箖閵忋倕绀傞柣鎾崇凹缂冩洟姊绘担绋款棌闁稿妫濆畷浼村箻鐠囪尙顦╅梺鎸庣☉鐎氼喚澹曢挊澹濆綊鏁愰崨顔藉創闂傚顑呴埞鎴︽倷閸欏娅ゅ┑鐐插级椤洨鍒掑顓熺秶闁靛ě鍛闂備焦鎮堕崕娲倶濞戞粠妯勯梺鍝勬湰缁嬫挻绂掗敃鍌氱鐟滃繘宕ｅ┑鍥╃瘈闁靛骏缍嗗鎰箾閸欏鐭屾俊鍙夊姍楠炴鎷犻懠顒婄床婵犵數鍋涘Λ娆戞暜閹烘鍌ㄩ柍鈺佸暟缁♀偓濠电偛鐗嗛悘婵嬫倶閻樼偨浜滈柡鍥ュ妼楠炴牗銇勯弴妯哄姕缂佺粯绻堝畷鎯邦槾妞わ富鍙冮幃宄邦煥閸愵噮鈧鏌ｉ敐鍥у幋鐎规洩绲惧鍕暆閳ь剟鎯侀崼銉︹拻闁稿本姘ㄦ晶娑樸€掑顓ф疁鐎规洘娲熼獮鍥敇濠娾偓缁ㄥ姊洪棃娑辨Ф闁稿氦娅曢弲璺衡槈濮樿京锛滅紓鍌欑劍椤洤煤鐎涙﹩娈介柣鎰▕閸庢棃鏌熼鐣屾噮闁圭懓瀚粭鐔碱敍濞戣鲸锛堝┑鐘殿暜缁辨洟宕戦幋锕€纾归柟杈剧畱绾惧綊鏌熼悧鍫熺凡缁炬儳顭烽弻鐔兼倷椤掍胶浼囬梺琛″亾?
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



  // 闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳缍婇弻鐔兼⒒鐎靛壊妲紒鐐劤缂嶅﹪寮婚悢鍏尖拻閻庨潧澹婂Σ顔剧磼閻愵剙鍔ょ紓宥咃躬瀵鎮㈤崗灏栨嫽闁诲酣娼ф竟濠偽ｉ鍓х＜闁绘劦鍓欓崝銈囩磽瀹ュ拑韬€殿喖顭烽幃銏ゅ礂鐏忔牗瀚介梺璇查叄濞佳勭珶婵犲伣锝夘敊閸撗咃紲闂佺粯鍔﹂崜娆撳礉閵堝洨纾界€广儱鎷戦煬顒傗偓娈垮枛椤兘骞冮姀銈呯閻忓繑鐗楃€氫粙姊虹拠鏌ュ弰婵炰匠鍕彾濠电姴浼ｉ敐澶樻晪闁逞屽墮椤繘宕崟鎳峰洤鐐婄憸澶愬磻閹捐围濠㈣泛锕﹂悰銉モ攽鎺抽崐鏇㈠箠鎼淬埄鏀伴梻鍌欑閹测€趁洪敃鍌氬瀭闁规鍠氭稉宥嗙箾閹存瑥鐏柣鎾存礋閺岀喖骞戦幇顒冩暱闂佺绻愰惌鍌炲蓟閿熺姴骞㈡繛鍡楄閵壯呯＜閺夊牄鍔嶇亸浼存煙瀹勭増鍤囩€规洦鍋婂畷鐔煎箣濞嗗繐濮庨梺瀹狀潐閸ㄥ潡骞冨▎鎾崇厸濞达絽鍢查ˉ姘舵⒒閸屾艾鈧绮堟笟鈧獮鏍敃閿旇棄鍓舵繝闈涘€婚…鍫㈢玻濡ゅ懏鐓涚€广儱楠搁獮鏍磼閻樺磭澧紒缁樼洴瀹曞崬螣缂佹ê鍓梻浣芥〃缁€浣肝涘Δ鍛畳闂備焦瀵х换鍌炲箠鎼淬劌绠栧〒姘ｅ亾闁哄瞼鍠撻埀顒傛暩椤牓宕㈢€涙ɑ鍙忓┑鐘插鐢盯鏌熷畡鐗堝殗闁瑰磭鍋ゆ俊鐑芥晲閸屾矮澹曢梺鍛婂姦娴滅偟澹曟總鍛婄厓鐟滄粓宕滃杈╃當闁绘梻鍘ч悞鍨亜閹烘垵顏ラ柍褜鍏涚欢姘嚕娴犲鏁囬柣鎰皺閻涒晠姊虹拠鎻掝劉缂佸甯￠垾锕傚炊椤掍礁浠奸梺瑙勫劶婵倝鍩涢幒妤佺厱閻忕偛澧介幊鍡涙煕韫囨挻婀伴柕鍥у椤㈡﹢鎮欓棃娑掑彙闂備胶鎳撶粻宥夊垂娴犲宓侀柛銉墮缁狙囨煠閹颁礁鐎洪柡鍥ュ灪閻撶喖骞栭幖顓炵仯缂佸鏁婚弻娑㈠箻鐎垫悶鈧帞绱掗鑲╁缂佺粯绻堝畷鍫曞Ω瑜嶉獮宥夋⒑閸濆嫷妲搁柣妤佹尵缁寮借閻庤埖銇勯弴妤€浜鹃梺鍝勮嫰缁夊綊寮婚妸褉鍋撻敐搴濈凹閻犲洨鍋ゅ娲传閸曨剚鎷辩紓浣割儐閹歌崵绮嬮幒妤佹櫆闁绘劦鍓欓悵鏉库攽閻愬瓨缍戞い鎴濇噺缁傚秵銈ｉ崘鈺冨幐闂佹悶鍎崕閬嶆倶椤忓牊鐓曞┑鐘插€婚崺锝夋煛鐏炶濮傜€殿喗鎸抽幃娆徝圭€ｎ亙澹曢悷婊呭鐢宕戦崒娑氱闁瑰瓨鐟ラ悘顏堟煕婵犲倻鍩ｉ柡宀嬬秮楠炲洭鎮ч崼鐔兼暘濠电偛鐡ㄧ划灞炬櫠閻ｅ本顫曢柟鐑橆殔閻掑灚銇勯幒鎴濐仼缂佲偓閸愨斂浜滈煫鍥ㄦ尰椤ョ娀鏌ㄥ☉娆戠煂缂佽鲸鎹囧畷鎺戔枎閹存繂顬夋俊鐐€ら崢鐓幟洪妶鍥у疾闂佽娴烽弫鍝ユ兜閸洖纾婚柟鎹愬煐閸犲棝鏌涢弴銊ュ妞わ负鍎崇槐鎾诲磼濮樻瘷銏ゆ煥閺囥劋閭┑鈥崇摠閹峰懐鍖栭弴鐔衡偓濠氭⒑閸︻厼浜炬繛鍏肩懄缁傛帡顢橀悢鍓佺畾濡炪倖鍔х€靛矂寮抽幒鏃傜＜闁逞屽墯缁楃喖鍩€椤掑啯锛傛繝娈垮枟閿曗晠宕㈤崜褍濮柍褜鍓熼幃妤呭礂缁嬪灝绁梺琛″亾闁告鍎愰悢鍡樻叏濡炶浜鹃梺绯曟杹閸嬫挸顪冮妶鍡楃瑐闁绘帪濡囩划鍫⑩偓锝庡亽濞堜粙鏌ｉ幇顖ｅ殝濮掝偅姘ㄧ槐鎺楁偐瀹曞洠妲堥梺瀹犳椤︻垵鐏掔紒缁㈠弮椤ユ捇鐛姀銈嗏拻闁稿本鐟︾粊鐗堢箾婢跺绀嬬€规洑鍗抽獮妯尖偓娑櫭鍧楁⒑濮瑰洤鐏╅柟璇х節瀹曟垿宕掗悙闈涘絼闂佹悶鍎崝搴ㄥΥ閹烘鐓曟繛鍡樺姈瀹曞矂鏌＄仦鍓ф创濠碉紕鍏橀獮瀣攽婵犲偆浼冮梻浣告啞閹哥兘宕￠崘宸綎闁惧繗顫夌€氭岸鏌涘▎蹇ｆШ妞も晙鍗冲娲传閵夈儛锝団偓鍏夊亾闁归棿绀佺粻姘舵煃瑜滈崜姘跺Φ閸曨喚鐤€闁规崘娉涢。娲⒑濞茶骞楁い銊ワ躬瀵寮撮悢铏瑰骄濡炪倖鐗楃喊宥夊闯娴犲鐓曟慨妤€妫楅悘锔芥叏婵犲嫮甯涢柟宄版嚇瀹曘劍绻濋崘銊ュ闂傚倷绀侀幖顐︽偋濠婂牆绀堟繛鎴炶壘閸ㄦ繃绻涢崱妯诲碍缂佺姴顭烽幃妤呮濞戞﹩妫屽┑鐐存綑鐎氭澘顫忓ú顏勭閹兼番鍨婚ˇ銉╂⒑缁嬪尅宸ラ柟鑺ョ矌閸掓帡宕奸妷銉╁敹闂侀潧绻嗛埀顒冩珪閻庨箖姊虹拠鎻掑毐缂傚秴妫濆畷浼村冀椤撶喎鈧潡鏌涢…鎴濅簴濞存粍绮撻弻鐔煎传閸曨厜銈嗐亜閿旂厧顩紒杈ㄥ笒铻栧ù锝呮憸閻熸煡姊洪棃娑欐悙閻庢矮鍗抽妴浣割潨閳ь剟宕规ィ鍐ㄧ闁圭粯宕奸妷锔跨箚闁绘劦浜滈埀顑懐涓嶉柟鐑樻煣閻掑﹥绻涢崱妯诲鞍闁稿骸顦伴妵鍕疀閹炬惌妫ょ紒鐐劤閵堟悂寮婚弴锛勭杸闁哄洨鍊姀銏㈢＜闁绘ü璀﹂崵娆撴煃鐟欏嫬鐏撮柛銊╃畺瀹曟﹢鎳犻鍕礈闂傚倷绀侀幉鈥愁潖瑜版帗鍋￠柍鍝勬噺閸嬫ɑ銇勯弴妤€浜惧Δ鐘靛仦鐢帟鐏冮梺閫炲苯澧扮紒顔剧帛缁轰粙宕ㄦ繝鍕箞闂備礁鎼ú銏ゅ礉瀹€鍕祦婵せ鍋撻柡宀嬬秮楠炴﹢寮堕幋鐘辨缂傚倷鑳剁划顖滄崲閸儱绠栧ù鐘差儐椤ュ牊绻涢幋鐐茬瑲闁诲海澧楃换婵嬫偨闂堟稐绮ч梺鍛婄墱婵炩偓鐎规洘顨婇幃娆擃敆閸屾稑鍨遍柣搴㈩問閸ｎ噣宕抽敐鍛殾闁绘挸绨堕弨浠嬫煕椤愮姴鐏╅柣鎰攻缁绘繈鎮介棃娴躲儵鏌℃担鍛婂暈闁逛究鍔戝鍫曞箠閵婏附銇濋柡浣稿暣瀹曟帒顫濋幉瀣覆闂傚倷鐒﹂惇褰掑垂瑜版帒绠熼柨鐔哄Т绾惧鏌嶉埡浣告殶缂佺娀绠栭弻娑㈠焺閸愮偓鐣肩紓浣哄Х婢ф濡甸崟顔剧杸闁挎繂瀚弫鏍磽娴ｄ粙鍝洪悽顖ょ節瀹曟椽鍩€椤掍降浜滈柟鐑樺灥椤忊晝绱掗埀顒勫礃椤旂晫鍘繝鐢靛仜閻忔繈鎮橀鍫熺厸闁稿本顨呮禍楣冩⒒閸屾艾鈧兘鎳楅崜浣稿灊妞ゆ牜鍋涚粈澶嬫叏濡炶浜惧銈冨灪閼归箖鈥﹂妸鈺侀唶婵犻潧鐗嗛幗瀣⒑閼姐倕孝婵炲眰鍔岄…鍥箰鎼淬垹鍔呴梺闈涱焾閸庢娊宕愰悙鐑樺仭婵犲﹤瀚粻鐐烘煟閹垮啫澧存い銏☆殕缁楃喖宕堕…鎴濇櫖闂傚倷鑳剁划顖炲礉閺囥埄鏁嬫い鎾跺枑濞呯姵銇勯幒鎴濐仾闁绘挻鐟ч埀顒傛嚀鐎氼喗鏅跺Δ鍛棷濞寸姴顑嗛悡娆撴煕濞嗗浚妲归悘蹇ュ閳ь剝顫夊ú婊堝窗閺嶎厹鈧礁螖閸涱厾锛滃┑顔筋焾妞存悂宕戣濮婂宕掑顑藉亾閹间礁纾归柟闂寸绾剧懓顪冪€ｎ亜顒㈡い鎰Г閹便劌螣閹稿海銆愰梺缁樺笒閻忔岸濡甸崟顖氱闁瑰瓨绺鹃崑鎾诲川婵犲嫷娴勫┑鐘诧工閻楀﹪鎮￠悩宕囩闁煎ジ顤傞崵娆撴煟韫囥儳绡€闁哄矉绻濆畷銊╊敇閻樿尙鍘芥俊鐐€戦崹铏圭矙閹达腹鈧箓濡搁埡浣哥獩濡炪倖鐗撻崐妤佹償婵犲洦鈷掗柛灞剧懆閸忓本銇勯鐐靛ⅵ鐎殿喚鏁婚、妤呭磼濠婂懐鍘梻浣烘嚀椤曨厽鎱ㄦ搴ｄ笉闁哄啫鐗婇悡娆撴煟閹寸倖鎴犱焊椤忓娊鐟邦煥閸曨厾鐓侀梺闈涙搐鐎氫即鐛崶顒€绀堝ù锝囨嚀娴犲綊姊绘担瑙勩仧闁告鏅弫顕€鏁撻悩鑼舵憰濠电偞鍨崹褰掓偂濮椻偓閺岀喖顢涢崱妤€鈧悂藟濮橆厾绡€缁炬澘顦辩壕鍧楁煕鐎ｎ偄鐏寸€规洘鍔欏浠嬵敃閿濆棙顔囬梻浣告贡閸庛倝寮婚敓鐘茬；闁圭偓鍓氬鈺呮煟閹炬娊顎楅柍宄邦儔濮?


  const handleCloseEditor = useCallback(() => {
    closeChapterEditor({
      setChapterQualityMetrics,
      setIsEditorOpen,
    });
  }, []);

  const editorAiSectionProps = useMemo(() => ({
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
  }), [
    applySingleCreationPreset,
    applySingleStoryCreationSnapshot,
    availableModels,
    canSaveSingleStoryCreationSnapshot,
    chapterQualityRefreshToken,
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    deleteSingleStoryCreationSnapshot,
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

  const editorModalContentProps = {
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
    currentProjectNarrativePerspective: currentProject?.narrative_perspective,
    temporaryNarrativePerspective,
    setTemporaryNarrativePerspective,
    selectedPlotStage,
    setSelectedPlotStage,
    applyInferredSinglePlotStage,
    aiSectionProps: editorAiSectionProps,
    onCloseEditor: handleCloseEditor,
  };

  const batchGenerateModalProps: ChapterBatchGenerateModalProps = {
    applyBatchCreationPreset,
    applyBatchStoryCreationSnapshot,
    applyInferredBatchPlotStage,
    availableModels,
    batchEnableAnalysis,
    batchForm,
    batchGenerateVisible,
    batchGenerating,
    batchProgress,
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
  if (!currentProject) return null;



  return (
    <>
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>

      {contextHolder}

      <div style={{

        position: 'sticky',

        top: 0,

        zIndex: 10,

        backgroundColor: 'var(--color-bg-container)',

        padding: isMobile ? '12px 0' : '16px 0',

        marginBottom: isMobile ? 12 : 16,

        borderBottom: '1px solid #f0f0f0',

        display: 'flex',

        flexDirection: isMobile ? 'column' : 'row',

        gap: isMobile ? 12 : 0,

        justifyContent: 'space-between',

        alignItems: isMobile ? 'stretch' : 'center'

      }}>

        <h2 style={{ margin: 0, fontSize: isMobile ? 18 : 24 }}>

          <BookOutlined style={{ marginRight: 8 }} />

          缂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳缍婇弻鐔兼⒒鐎靛壊妲紒鐐劤缂嶅﹪寮婚悢鍏尖拻閻庨潧澹婂Σ顔剧磼閻愵剙鍔ょ紓宥咃躬瀵鎮㈤崗灏栨嫽闁诲酣娼ф竟濠偽ｉ鍓х＜闁绘劦鍓欓崝銈囩磽瀹ュ拑韬€殿喖顭烽弫鎰緞婵犲嫷鍚呴梻浣瑰缁诲倿骞夊☉銏犵缂備焦顭囬崢杈ㄧ節閻㈤潧孝闁稿﹤缍婂畷鎴﹀Ψ閳哄倻鍘搁柣蹇曞仩椤曆勬叏閸屾壕鍋撳▓鍨灍婵炲吋鐟ㄩ悘鍐⒑闁偛鑻晶顖滅磼閸屾氨效妤犵偛妫滈¨浣圭箾閹炬剚鐓奸柡灞炬礋瀹曠厧鈹戠€ｇ鍋撳Δ鈧湁婵犲﹤瀚惌宀€绱掓潏銊ョ缂佽鲸甯掕灒闁惧繗顕栭崕灞剧節绾版ǚ鍋撻崘鑼獓闂佸憡姊归悷鈺呮偘椤曗偓楠炴帒螖閳ь剛绮婚悩纰樺亾鐟欏嫭绀€婵炲眰鍊濆鎼佸籍閸喓鍘垫俊鐐差儏妤犳悂鍩㈤崼鐕佹闁绘劕鐡ㄥ畷灞绢殽閻愭潙绗掓い鎾炽偢瀹曞爼鍩℃繝鍐冄囨⒒閸屾艾鈧悂宕愰幖浣哥９闁归棿绀佺壕褰掓煟閹达絽袚闁搞倕瀚伴弻娑㈠箻閼碱剦妲梺鎼炲妽缁诲啴濡甸崟顖氬唨妞ゆ劦婢€缁墎绱撴担鎻掍壕婵犮垼鍩栭崝鏍偂閵夆晜鐓涢柛銉㈡櫅娴犳粓鏌嶈閸撴瑩骞楀鍛灊闁割偁鍎辩壕鍏肩箾閹寸偞鐨戞い鏃€娲熷娲偡闁箑娈堕梺绋款儑閸犳牠濡撮崒鐐村殤妞ゆ垼妫勬禍?

        </h2>

        <Space direction={isMobile ? 'vertical' : 'horizontal'} style={{ width: isMobile ? '100%' : 'auto' }}>

          {currentProject.outline_mode === 'one-to-many' && (

            <Button

              icon={<PlusOutlined />}

              onClick={showManualCreateChapterModal}

              block={isMobile}

              size={isMobile ? 'middle' : 'middle'}

            >

              闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳缍婇弻鐔兼⒒鐎靛壊妲紒鐐劤缂嶅﹪寮婚悢鍏尖拻閻庨潧澹婂Σ顔剧磼閹冣挃闁硅櫕鎹囬垾鏃堝礃椤忎礁浜鹃柨婵嗙凹缁ㄥジ鏌熼惂鍝ョМ闁哄矉缍侀、姗€鎮欓幖顓燁棧闂備線娼уΛ娆戞暜閹烘缍栨繝闈涱儐閺呮煡鏌涘☉鍗炲妞ゃ儲鑹鹃埞鎴炲箠闁稿﹥顨嗛幈銊╂倻閽樺锛涢梺缁樺姉閸庛倝宕戠€ｎ喗鐓熸俊顖濆吹濠€浠嬫煃瑜滈崗娑氭濮橆剦鍤曢柟缁㈠枛椤懘鏌嶉埡浣告殲闁绘繃鐗犲缁樼瑹閳ь剟鍩€椤掑倸浠滈柤娲诲灡閺呭爼寮跺▎鍓у數闁荤喐鐟ョ€氼厾绮堟径鎰厪闁搞儯鍔屾慨宥嗩殽閻愭潙娴鐐搭焽閹瑰嫰宕崟顓у晥闂傚倸鍊搁崐鐑芥嚄閸撲礁鍨濇い鏍ㄥ嚬濞兼牕鈹戦悩瀹犲闁稿被鍔庨幉鍛婃償閿濆洨鐒块悗骞垮劚濡顢氶柆宥嗗€垫繛鎴炵懐閻掍粙鏌涘Ο鍦煓婵﹥妞藉畷顐﹀礋椤愮喎浜鹃柛锔诲幐閸嬫挾绮☉妯荤〗濠㈣埖鍔栭崐鐑芥煟椤愵偄澧梺甯到閻ｇ兘骞嗛柇锔筋€夐梻鍌氼煬閸擄箓宕滃璺何﹂柛鏇ㄥ灠椤懘鏌ㄥ☉妯侯仱婵℃彃鐗撳铏光偓鍦濞兼劙鏌涢妸銉т虎闁伙絿鍏橀弻鍡楊吋閸涱厼绁梻浣虹帛濮婂宕曢妶鍥╃閹艰揪绲跨壕浠嬫煕鐏炴崘澹橀柍褜鍓涢崗姗€骞冮悙鐑樻櫇闁稿本姘ㄩ敍鐔兼煟鎼粹剝璐″┑顔惧缁傚秷銇愰幒鎾跺幗闂佽澹嗘晶妤€鈽夎閺屾盯濡搁敂鍓х暫缂備胶绮惄顖氱暦閸楃倣鐔烘嫚閼碱剦鏆￠梻鍌欐祰濡嫰宕导鏉戠獥闁哄稁鍘奸拑鐔兼煥濠靛棭妲哥紒顐㈢Ч閺屾稓浠﹂幑鎰棟婵炲濮甸…鍥╂閹惧瓨濯撮柛婵嗗珔閿濆棙鍙忔俊顖滎焾婵倹顨ラ悙宸Ш缂侇喗鐟ラ埢搴ㄥ箚瑜庨崐顖氣攽閻橆喖鐏辨繛澶嬬洴閹囧礃椤旇偐锛涢梺瑙勫礃椤曆呭婵傚憡鐓熼柟閭﹀灠閻ㄧ儤銇勯弬鎸庡殗婵﹥妞藉畷妤呮⒒绾惧鐒婚梻浣侯焾鐎涒晜绻涙繝鍌滄殾闁靛繈鍊曠涵鈧梺缁樺姀閺呮粓寮埀顒勬⒑閸︻厼鍔嬪┑鐐诧工閻ｇ兘骞囬弶璺啋闂佸憡鎸烽懗鍫曟倿閸忚偐绠鹃柟鐐綑閻掑綊鏌涚€ｎ偅灏甸柍褜鍓濋～澶娒洪敃鍌氱；闁告洦鍘煎鍙変繆閻愵亜鈧洜鎹㈤幇鏉跨疇闁圭増婢橀崒銊╂煙闂傚鍔嶉柛濠勬暬閺屾稖绠涢幙鍐┬︽繛瀛樼矒缁犳牠寮婚敐鍡樺劅妞ゆ牗绮庢牎闂備礁鎲￠幐濠氭偡閳轰胶鏆﹂柛婵嗗閺嗗棝鏌涢弴銊ュ闁告瑥瀚换娑㈠级閹存繃鍊梺鑽ゅ暀閸涱垳鐓嬪銈嗘磵閸嬫捇鏌＄仦绯曞亾瀹曞洦娈曢柣搴秵閸撴稖鈪靛┑掳鍊楁慨鐑藉磻濞戙埄鏁勫鑸靛姇缁犳牗绻涢崱妯诲碍妤犵偑鍨烘穱濠囶敍濠靛浂浠╂繛瀵稿帶閻倿骞冨Δ鍐╁枂闁告洦鍓涢ˇ銊モ攽閻愯泛鐨洪柛鐘崇墵閹即顢欓崲澶屽枛閹虫牠鍩￠崘璺ㄥ簥濠电姷顣藉Σ鍛村垂閹惰棄鍌ㄧ憸宥夘敋?

            </Button>

          )}

          <Button

            type="primary"

            icon={<RocketOutlined />}

            onClick={handleOpenBatchGenerate}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

            style={{ background: '#722ed1', borderColor: '#722ed1' }}

          >

            闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳缍婇弻鐔兼⒒鐎靛壊妲紒鐐劤缂嶅﹪寮婚悢鍏尖拻閻庨潧澹婂Σ顔剧磼閹冣挃闁硅櫕鎹囬垾鏃堝礃椤忎礁浜鹃柨婵嗙凹缁ㄥジ鏌熼惂鍝ョМ闁哄矉缍侀、姗€鎮欓幖顓燁棧闂傚倸娲らˇ鐢稿蓟閵娿儮鏀介柛鈩冪懃椤も偓婵＄偑鍊曠换鎺撴叏妞嬪孩顫曢柟鐑橆殔閻掑灚銇勯幒宥堝厡缂佲檧鍋撻梻浣侯焾閺堫剟銆冮崱娑樻闁逞屽墴濮婄粯鎷呴搹鐟扮闁藉啳浜幉鎼佸级閸喗娈茬紓浣稿€哥粔褰掑箖濞嗘挻鍊绘俊顖滃帶楠炴姊绘担鍛婅础闁稿簺鍊曢～蹇涙偡閹佃櫕鐎洪梺鍝勬储閸ㄦ椽鎮″▎鎾寸厱婵炲棗娴氬Σ娲煙閽樺鏆熺紒杈ㄥ浮閹晛鐣烽崶銊ュ灡闁诲孩顔栭崰妤呭箰閹惰棄绠栭柕鍫濇婵挳鏌涢敂璇插箻闁崇鍎靛濠氬磼濮橆兘鍋撻悜鑺ュ殑闁告挷绀侀崹婵囥亜閺嶎偄浠滅紒鈧径鎰厸闁搞儯鍎遍悞娲煛娴ｅ壊鍎愰柕鍥у楠炴鎹勯惄鎺嬪灩閳规垿顢氶崨顓炩拫闂佸搫鑻粔鐑铰ㄦ笟鈧弻娑㈠箻鐠虹儤鐎诲銈庡亜缁绘劗鍙呭銈呯箰鐎氼噣顢欓幇鐗堚拺缂備焦锚婵牏鎲搁弶鍨殲濞ｅ洤锕ㄩˇ瑙勬叏婵犲啯銇濇鐐寸墵閹瑩骞撻幒鎳躲倝姊绘担铏瑰笡闁瑰憡鎮傝棟闁告鍊ｉ敐澶婄疀闂侇叏闄勯弲銏ゆ⒑闁偛鑻晶鎾煕閳哄啫浠辨鐐差儔閺佸倿鎸婃径澶嬬潖闂傚倷绀佹竟濠囨偂閸儱纾婚柛娑卞帨閹烘绀嬫い鏍ㄧ▓閹疯櫣绱撴担鍓插剱妞ゆ垶鐟╁畷鏇㈠箛椤斿墽锛滈柣搴秵閸嬪嫰鎮橀柆宥嗙厽闊洦鏌ㄧ粭姘辩磼缂佹绠為柟顔荤矙濡啫霉闊彃鐏查柡灞剧洴閹垻鎹勯崫鍕偖闂備線娼уΛ妤呭箠濡櫣鏆﹂柨婵嗘缁剁偟鈧厜鍋撻柍褜鍓熼幆渚€宕奸妷锔规嫽闂佺鏈銊︽櫠濞戞ǜ鈧帒顫濋褎鐤侀悗瑙勬礃濞叉繄绮诲☉銏犲嵆闁绘顒茬槐锟犳⒒娴ｇ瓔鍤冮柛銊ラ閻ｆ繈鍩€椤掑嫬瑙﹂悗锝庡枟閳锋垿鏌涘┑鍡楊仾婵犫偓閻楀牏绠鹃柛娆忣樈閻掍粙鏌熼獮鍨仼闁宠棄顦垫慨鈧柍銉︽灱閸嬫捇鏌ㄧ€ｃ劋绨婚梺鐟版惈缁夌兘宕楀畝鈧幉鎼佸级閹稿寒妫﹀┑顔硷功缁垶骞忛崨瀛樻優闁荤喐澹嗛濂告⒒娴ｇ瓔娼愰柟顔煎€荤划濠氬冀瑜忛弳锕€霉閸忓吋缍戦柛鎰ㄥ亾婵＄偑鍊栭幐楣冨磻閻樿绠洪柡鍥ュ灪閳锋垿鎮归幁鎺戝闁哄鏌ㄩ埞鎴︻敊绾板崬娈剁紓渚囧枛閻楁挸鐣峰鈧、娆撴偩鐏炶棄濡囨繝鐢靛Х閺佹悂宕戝☉銏″€舵繝闈涱儏缁€澶嬬箾閸℃ê鐏╃痪鎯с偢閺屾洘绻涢崹顔瑰亾濡ゅ懏鍎楁繛鍡樺姈閸欏繐鈹戦悩鍙夊櫤妞ゅ繒濮风槐鎺撴綇閵娿儳鐟ㄩ柧浼欑秮閺岋綁骞嬮悜鍡欏姼闂佽皫鍌濆厡缂佽鲸鎹囧畷鎺戔枎閹烘垵甯┑鐘愁問閸犳岸寮繝姘畺鐟滄棃骞冮埡渚囧晠妞ゆ柨鍚嬮?

          </Button>

          <Button

            type="default"

            icon={<DownloadOutlined />}

            onClick={handleExport}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

          >

            闂傚倸鍊搁崐鎼佸磹閹间礁纾归柟闂寸绾惧綊鏌熼梻瀵割槮缁炬儳缍婇弻鐔兼⒒鐎靛壊妲紒鐐劤濠€閬嶅焵椤掑倹鍤€閻庢凹鍙冨畷宕囧鐎ｃ劋姹楅梺鍦劋閸ㄥ綊宕愰悙宸富闁靛牆妫楃粭鎺撱亜閿斿灝宓嗙€殿喗鐓￠、鏃堝醇閻旇渹鐢绘繝鐢靛Т閿曘倝宕悧鍫熸珡濠电姷鏁告慨顓㈠磻閹剧偨鈧帒顫濋敐鍛婵犳鍠栭敃銊モ枍閿濆洦顫曢柟鐑樺殾閻斿吋鎯為梺顐ｇ〒缁€鍐ㄢ攽閻樻鏆俊鎻掓嚇瀹曟垿宕熼娑樹壕婵﹩鍘界欢鍙夈亜閺囶亞绉€规洏鍔嶇换婵嬪礋椤撶喎娈為梻鍌欑窔閳ь剛鍋涢懟顖涙櫠鐎电硶鍋撳▓鍨灈妞ゎ厾鍏橀獮鍐閵堝懐顦ч柣蹇撶箲閻楁鈧矮绮欏铏规嫚閺屻儱寮板┑鐐板尃閸曨厾褰炬繝鐢靛Т娴硷綁鏁愭径妯绘櫔闂侀€炲苯澧い顐㈢箻閹煎綊宕烽鐘靛幆闂備礁鍚嬮崜姘跺垂閻撳寒鐒介柛顐ｆ礃閳锋垿鏌涘☉姗堟敾闁绘挶鍎靛铏规暜椤斿墽袦闂佺粯渚楅崰妤€顕ラ崟顖氱疀妞ゆ帒鍋嗛崯鍥р攽閻愯埖褰х紒韫矙楠炲鏁撻悩鎾愁槸閳规垹鈧綆鍋嗛崢浠嬫⒑缂佹◤顏堝储閺嶎厽鍤嬫い蹇撶墛閻撴洟鏌￠崒婵囩《鐎涙繈姊?

          </Button>

          {!isMobile && (

            <Tag color="blue">

              {currentProject.outline_mode === 'one-to-one'

                ? "One outline per chapter"

                : "Grouped outline mode"
              }
            </Tag>

          )}

        </Space>

      </div>
      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}>
        <ChapterListSection
          chapters={chapters}
          sortedChapters={sortedChapters}
          outlineMode={currentProject.outline_mode}
          groupedChapters={groupedChapters}
          expandedChapterGroupKeys={expandedChapterGroupKeys}
          isMobile={isMobile}
          analysisTasksMap={analysisTasksMap}
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

      <ChapterBasicModalEntry
        open={isModalOpen}
        title={editingId ? "Edit chapter" : "Create chapter"}
        isMobile={isMobile}
        outlineMode={currentProject.outline_mode}
        submitText={editingId ? "Save changes" : "Create chapter"}
        form={form}
        onCancel={() => setIsModalOpen(false)}
        onFinish={handleSubmit}
      />

      {isEditorOpen ? (
        <Modal

        title={'Edit chapter content'}

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

            maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(100vh - 110px)',

            overflowY: 'auto',

            padding: isMobile ? '16px 12px' : '8px'

          }

        }}

        footer={null}

      >

        <Suspense fallback={null}>
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
      <ChapterBatchGenerateModalEntry
        visible={batchGenerateVisible || batchGenerating}
        modalProps={batchGenerateModalProps}
      />
      <SingleChapterGenerationOverlayEntry
        loading={isGenerating}
        progress={singleChapterProgress}
        message={singleChapterProgressMessage}
      />
      <ChapterBatchProgressEntry
        visible={batchGenerating}
        progress={batchProgress ? Math.round((batchProgress.completed / batchProgress.total) * 100) : 0}
        message={
          batchProgress?.current_chapter_number
            ? [
                `Generating chapter ${batchProgress.current_chapter_number}/${batchProgress.total}`,
                batchProgress.latest_quality_metrics?.overall_score !== undefined
                  ? `Score ${batchProgress.latest_quality_metrics.overall_score}`
                  : null,
                batchProgressCheckpointLabel,
                batchProgressRepairLabel,
              ].filter(Boolean).join(' | ')
            : [
                'Preparing batch generation',
                batchProgress?.latest_quality_metrics?.overall_score !== undefined
                  ? `Score ${batchProgress.latest_quality_metrics.overall_score}`
                  : null,
                batchProgressCheckpointLabel,
                batchProgressRepairLabel,
              ].filter(Boolean).join(' | ')
        }
        onCancel={() => {
          modal.confirm({
            title: 'Cancel batch generation',
            content: 'Stop the current batch generation task?',
            okText: 'Stop',
            cancelText: 'Keep running',
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




      <ChapterReaderEntry
        chapterReaderModalState={chapterReaderModalState}
        onClose={handleCloseReader}
        onChapterChange={handleReaderChapterChange}
      />




      <ChapterPlanEditorEntry
        planEditorModalState={planEditorModalState}
        onSave={handleSavePlan}
        onCancel={handleClosePlanEditor}
      />

    </div>
    </>
  );

}
