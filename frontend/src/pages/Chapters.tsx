import { Suspense, lazy, useState, useEffect, useRef, useMemo, useCallback } from 'react';

import { List, Button, Modal, Form, message, Empty, Space, Badge, Tag, Collapse, FloatButton } from 'antd';

import { DownloadOutlined, RocketOutlined, CaretRightOutlined, BookOutlined, PlusOutlined } from '@ant-design/icons';

import { useStore } from '../store';
import { useChapterSync } from '../store/hooks';
import { projectApi, writingStyleApi, chapterApi, chapterBatchTaskApi } from '../services/api';
import type { Chapter, ChapterUpdate, ApiError, WritingStyle, AnalysisTask, ExpansionPlanData, ChapterLatestQualityMetrics, ChapterQualityMetrics, ChapterQualityMetricsSummary, ChapterQualityProfileSummary, CreativeMode, PlotStage, StoryFocus } from '../types';
import { hasUsableApiCredentials } from '../utils/apiKey';
import ChapterListItem from '../components/ChapterListItem';

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
  buildStoryBeatPlannerPrompt,
  buildStoryCreationPromptLayerLabels,
  buildStorySceneOutlinePrompt,
  buildStorySceneOutlineSuggestion,
  mergeStoryCreationInstructions,
  STORY_CREATION_PROMPT_WARN_THRESHOLD,
} from '../utils/storyCreationPrompt';
import {
  EMPTY_STORY_BEAT_PLANNER_DRAFT,
  EMPTY_STORY_SCENE_OUTLINE_DRAFT,
  areStoryBeatPlannerDraftsEqual,
  areStoryCreationDraftContentsEqual,
  areStorySceneOutlineDraftsEqual,
  hasMeaningfulStoryCreationDraft,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  normalizeOptionalText,
  normalizeStoryBeatPlannerDraft,
  normalizeStorySceneOutlineDraft,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshot,
  type StoryCreationSnapshotReason,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';


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

const LazyChapterBasicModal = lazy(() => import('../components/ChapterBasicModal'));
const LazyChapterAnalysis = lazy(() => import('../components/ChapterAnalysis'));
const LazyChapterEditorModalContent = lazy(() => import('../components/ChapterEditorModalContent'));
const LazyChapterBatchGenerateModal = lazy(() => import('../components/ChapterBatchGenerateModal'));
const LazyChapterReader = lazy(() => import('../components/ChapterReader'));

const LazyExpansionPlanEditor = lazy(() => import('../components/ExpansionPlanEditor'));
const LazyFloatingIndexPanel = lazy(() => import('../components/FloatingIndexPanel'));
const LazySSELoadingOverlay = lazy(async () => {
  const module = await import('../components/SSELoadingOverlay');
  return { default: module.SSELoadingOverlay };
});

const LazySSEProgressModal = lazy(async () => {
  const module = await import('../components/SSEProgressModal');
  return { default: module.SSEProgressModal };
});

const loadStoryCreationPersistence = () => import('../utils/storyCreationPersistence');
const isAnalysisTaskInProgress = (task?: AnalysisTask | null): boolean => (
  task?.status === 'pending' || task?.status === 'running'
);

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

const BATCH_TASK_META_STORAGE_KEY = 'chapter_batch_task_meta_map_v1';

const writingStylesLoadPromises = new Map<string, Promise<void>>();
const batchTaskRestorePromises = new Map<string, Promise<void>>();
const writingStylesCache = new Map<string, { styles: WritingStyle[]; defaultStyleId?: number }>();
const chapterAnalysisTasksCache = new Map<string, Record<string, AnalysisTask>>();

type ModelOption = {
  value: string;
  label: string;
};

const normalizeOptionalSelectValue = (value: unknown): string | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }

  const normalizedValue = value.trim();
  return normalizedValue ? normalizedValue : undefined;
};

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

const normalizeModelOptions = (rawModels: unknown): ModelOption[] => {
  if (!Array.isArray(rawModels)) {
    return [];
  }

  const seenModelValues = new Set<string>();
  const normalizedModels: ModelOption[] = [];

  rawModels.forEach((rawModel) => {
    let nextValue: string | undefined;
    let nextLabel: string | undefined;

    if (typeof rawModel === 'string') {
      nextValue = normalizeOptionalSelectValue(rawModel);
      nextLabel = nextValue;
    } else if (rawModel && typeof rawModel === 'object') {
      const modelRecord = rawModel as Record<string, unknown>;
      nextValue = normalizeOptionalSelectValue(
        modelRecord.value ?? modelRecord.id ?? modelRecord.name ?? modelRecord.label,
      );
      nextLabel = normalizeOptionalSelectValue(
        modelRecord.label ?? modelRecord.name ?? modelRecord.value ?? modelRecord.id,
      );
    }

    if (!nextValue || seenModelValues.has(nextValue)) {
      return;
    }

    seenModelValues.add(nextValue);
    normalizedModels.push({
      value: nextValue,
      label: nextLabel ?? nextValue,
    });
  });

  return normalizedModels;
};

const areModelOptionsEqual = (leftOptions: ModelOption[], rightOptions: ModelOption[]): boolean => (
  leftOptions.length === rightOptions.length
  && leftOptions.every((option, index) => {
    const rightOption = rightOptions[index];
    return Boolean(rightOption)
      && option.value === rightOption.value
      && option.label === rightOption.label;
  })
);

const CREATIVE_MODE_OPTIONS: Array<{ value: CreativeMode; label: string; description: string }> = [
  { value: 'balanced', label: '????', description: '??????????????' },
  { value: 'hook', label: '????', description: '???????????' },
  { value: 'emotion', label: '????', description: '??????????' },
  { value: 'suspense', label: '????', description: '??????????' },
  { value: 'relationship', label: '????', description: '?????????' },
  { value: 'payoff', label: '????', description: '????????????' },
];

const STORY_FOCUS_OPTIONS: Array<{ value: StoryFocus; label: string; description: string }> = [
  { value: 'advance_plot', label: '????', description: '???????????' },
  { value: 'deepen_character', label: '????', description: '??????????' },
  { value: 'escalate_conflict', label: '????', description: '??????????' },
  { value: 'reveal_mystery', label: '????', description: '??????????' },
  { value: 'relationship_shift', label: '????', description: '????????????' },
  { value: 'foreshadow_payoff', label: '????', description: '??????????' },
];





const MANUAL_STORY_CREATION_BRIEF_SENTINEL = '__manual_story_creation_brief__';




const buildSingleStoryCreationDraftStorageKey = (projectId: string, chapterId: string): string => (
  `${projectId}::single::${chapterId}`
);

const buildBatchStoryCreationDraftStorageKey = (projectId: string): string => (
  `${projectId}::batch`
);


type BatchTaskMeta = {

  startChapterNumber: number;

  count: number;

  autoAnalyze: boolean;

  projectId?: string;

};



const isValidBatchTaskMeta = (value: unknown): value is BatchTaskMeta => {

  if (!value || typeof value !== 'object') {

    return false;

  }



  const meta = value as Record<string, unknown>;

  return (

    typeof meta.startChapterNumber === 'number' &&

    typeof meta.count === 'number' &&

    typeof meta.autoAnalyze === 'boolean'

  );

};



const readPersistedBatchTaskMetaMap = (): Record<string, BatchTaskMeta> => {

  try {

    const raw = localStorage.getItem(BATCH_TASK_META_STORAGE_KEY);

    if (!raw) {

      return {};

    }



    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {

      return {};

    }



    const normalized: Record<string, BatchTaskMeta> = {};

    Object.entries(parsed).forEach(([taskId, value]) => {

      if (isValidBatchTaskMeta(value)) {

        normalized[taskId] = value;

      }

    });

    return normalized;

  } catch (error) {

    console.warn('Failed to read persisted batch task metadata.', error);

    return {};

  }

};



const writePersistedBatchTaskMetaMap = (map: Record<string, BatchTaskMeta>): void => {

  try {

    localStorage.setItem(BATCH_TASK_META_STORAGE_KEY, JSON.stringify(map));

  } catch (error) {

    console.warn('Failed to persist batch task metadata.', error);

  }

};



const persistBatchTaskMeta = (taskId: string, meta: BatchTaskMeta): void => {

  const map = readPersistedBatchTaskMetaMap();

  map[taskId] = meta;

  writePersistedBatchTaskMetaMap(map);

};



const getPersistedBatchTaskMeta = (taskId: string, projectId?: string): BatchTaskMeta | undefined => {

  const map = readPersistedBatchTaskMetaMap();

  const meta = map[taskId];

  if (!meta) {

    return undefined;

  }



  if (projectId && meta.projectId && meta.projectId !== projectId) {

    return undefined;

  }



  return meta;

};



const removePersistedBatchTaskMeta = (taskId: string): void => {

  const map = readPersistedBatchTaskMetaMap();

  if (!(taskId in map)) {

    return;

  }



  delete map[taskId];

  writePersistedBatchTaskMetaMap(map);

};



export default function Chapters() {

  const currentProject = useStore((state) => state.currentProject);
  const projectDefaultCreativeMode = currentProject?.default_creative_mode;
  const projectDefaultStoryFocus = currentProject?.default_story_focus;
  const projectDefaultPlotStage = currentProject?.default_plot_stage;
  const projectDefaultStoryCreationBrief = currentProject?.default_story_creation_brief?.trim() ?? '';

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
  const [batchSelectedModel, setBatchSelectedModel] = useState<string | undefined>(); // 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭鏌ｉ妸銉ヮ仾閼垛晠鏌涢妸銉剶闁逞屽墮椤︽壆鈧?
  const [temporaryNarrativePerspective, setTemporaryNarrativePerspective] = useState<string | undefined>(); // 婵炴垶鎸搁悺銊ヮ渻閸屾稓顩查柧蹇撳ⅲ閻愮儤鐒诲璺侯儏椤?
  const [selectedCreativeMode, setSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [batchSelectedCreativeMode, setBatchSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [selectedStoryFocus, setSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [batchSelectedStoryFocus, setBatchSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [selectedPlotStage, setSelectedPlotStage] = useState<PlotStage | undefined>();
  const [batchSelectedPlotStage, setBatchSelectedPlotStage] = useState<PlotStage | undefined>();
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

  const [isIndexPanelVisible, setIsIndexPanelVisible] = useState(false);




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

    estimated_time_minutes?: number;

    latest_quality_metrics?: ChapterLatestQualityMetrics | null;
    quality_metrics_summary?: ChapterQualityMetricsSummary | null;
    quality_profile_summary?: ChapterQualityProfileSummary | null;
  } | null>(null);

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
    setBatchStoryCreationBriefDraft(projectDefaultStoryCreationBrief);
    setBatchStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setBatchStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
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

  const singleDefaultStoryCreationBrief = singleSystemStoryCreationBrief || projectDefaultStoryCreationBrief || '';

  const batchDefaultStoryCreationBrief = batchSystemStoryCreationBrief || projectDefaultStoryCreationBrief || '';

  const normalizedSingleStoryCreationBriefDraft = singleStoryCreationBriefDraft.trim();

  const normalizedBatchStoryCreationBriefDraft = batchStoryCreationBriefDraft.trim();

  const singleStoryBeatPlannerBrief = useMemo(
    () => buildStoryBeatPlannerPrompt(singleStoryBeatPlannerDraft, 'single'),
    [singleStoryBeatPlannerDraft],
  );

  const singleStorySceneOutlineBrief = useMemo(
    () => buildStorySceneOutlinePrompt(singleStorySceneOutlineDraft, 'single'),
    [singleStorySceneOutlineDraft],
  );

  const resolvedSingleStoryCreationBrief = useMemo(
    () => mergeStoryCreationInstructions(
      normalizedSingleStoryCreationBriefDraft || singleDefaultStoryCreationBrief || undefined,
      singleStoryBeatPlannerBrief,
      singleStorySceneOutlineBrief,
    ),
    [
      normalizedSingleStoryCreationBriefDraft,
      singleDefaultStoryCreationBrief,
      singleStoryBeatPlannerBrief,
      singleStorySceneOutlineBrief,
    ],
  );

  const singleStoryCreationPromptLayerLabels = useMemo(
    () => buildStoryCreationPromptLayerLabels({
      summary: normalizedSingleStoryCreationBriefDraft || singleDefaultStoryCreationBrief || undefined,
      beat: singleStoryBeatPlannerBrief,
      scene: singleStorySceneOutlineBrief,
    }),
    [
      normalizedSingleStoryCreationBriefDraft,
      singleDefaultStoryCreationBrief,
      singleStoryBeatPlannerBrief,
      singleStorySceneOutlineBrief,
    ],
  );

  const singleStoryCreationPromptCharCount = resolvedSingleStoryCreationBrief?.length ?? 0;

  const isSingleStoryCreationPromptVerbose = singleStoryCreationPromptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD;

  const batchStoryBeatPlannerBrief = useMemo(
    () => buildStoryBeatPlannerPrompt(batchStoryBeatPlannerDraft, 'batch'),
    [batchStoryBeatPlannerDraft],
  );

  const batchStorySceneOutlineBrief = useMemo(
    () => buildStorySceneOutlinePrompt(batchStorySceneOutlineDraft, 'batch'),
    [batchStorySceneOutlineDraft],
  );

  const resolvedBatchStoryCreationBrief = useMemo(
    () => mergeStoryCreationInstructions(
      normalizedBatchStoryCreationBriefDraft || batchDefaultStoryCreationBrief || undefined,
      batchStoryBeatPlannerBrief,
      batchStorySceneOutlineBrief,
    ),
    [
      normalizedBatchStoryCreationBriefDraft,
      batchDefaultStoryCreationBrief,
      batchStoryBeatPlannerBrief,
      batchStorySceneOutlineBrief,
    ],
  );

  const batchStoryCreationPromptLayerLabels = useMemo(
    () => buildStoryCreationPromptLayerLabels({
      summary: normalizedBatchStoryCreationBriefDraft || batchDefaultStoryCreationBrief || undefined,
      beat: batchStoryBeatPlannerBrief,
      scene: batchStorySceneOutlineBrief,
    }),
    [
      normalizedBatchStoryCreationBriefDraft,
      batchDefaultStoryCreationBrief,
      batchStoryBeatPlannerBrief,
      batchStorySceneOutlineBrief,
    ],
  );

  const batchStoryCreationPromptCharCount = resolvedBatchStoryCreationBrief?.length ?? 0;

  const isBatchStoryCreationPromptVerbose = batchStoryCreationPromptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD;

  const resolveStoryCreationPromptState = useCallback((options: {
    scope: 'single' | 'batch';
    briefDraft?: string | null;
    defaultBrief?: string | null;
    beatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
    sceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  }) => {
    const baseBrief = options.briefDraft?.trim() || options.defaultBrief?.trim() || undefined;
    const beatBrief = buildStoryBeatPlannerPrompt(options.beatPlannerDraft, options.scope);
    const sceneBrief = buildStorySceneOutlinePrompt(options.sceneOutlineDraft, options.scope);
    const prompt = mergeStoryCreationInstructions(baseBrief, beatBrief, sceneBrief);
    const promptLayerLabels = buildStoryCreationPromptLayerLabels({
      summary: baseBrief,
      beat: beatBrief,
      scene: sceneBrief,
    });
    const promptCharCount = prompt?.length ?? 0;

    return {
      prompt,
      promptLayerLabels,
      promptCharCount,
      isVerbose: promptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD,
    };
  }, []);

  const isSingleStoryCreationBriefCustomized = Boolean(
    normalizedSingleStoryCreationBriefDraft
    && normalizedSingleStoryCreationBriefDraft !== singleDefaultStoryCreationBrief.trim(),
  );

  const isBatchStoryCreationBriefCustomized = Boolean(
    normalizedBatchStoryCreationBriefDraft
    && normalizedBatchStoryCreationBriefDraft !== batchDefaultStoryCreationBrief.trim(),
  );

  const isSingleStoryBeatPlannerCustomized = Boolean(
    !isStoryBeatPlannerDraftEmpty(singleStoryBeatPlannerDraft)
    && !areStoryBeatPlannerDraftsEqual(singleStoryBeatPlannerDraft, singleSystemStoryBeatPlanner),
  );

  const isBatchStoryBeatPlannerCustomized = Boolean(
    !isStoryBeatPlannerDraftEmpty(batchStoryBeatPlannerDraft)
    && !areStoryBeatPlannerDraftsEqual(batchStoryBeatPlannerDraft, batchSystemStoryBeatPlanner),
  );

  const isSingleStorySceneOutlineCustomized = Boolean(
    !isStorySceneOutlineDraftEmpty(singleStorySceneOutlineDraft)
    && !areStorySceneOutlineDraftsEqual(singleStorySceneOutlineDraft, singleSuggestedStorySceneOutline),
  );

  const isBatchStorySceneOutlineCustomized = Boolean(
    !isStorySceneOutlineDraftEmpty(batchStorySceneOutlineDraft)
    && !areStorySceneOutlineDraftsEqual(batchStorySceneOutlineDraft, batchSuggestedStorySceneOutline),
  );

  const isSingleStoryCreationControlCustomized = isSingleStoryCreationBriefCustomized
    || isSingleStoryBeatPlannerCustomized
    || isSingleStorySceneOutlineCustomized;

  const isBatchStoryCreationControlCustomized = isBatchStoryCreationBriefCustomized
    || isBatchStoryBeatPlannerCustomized
    || isBatchStorySceneOutlineCustomized;


  const singleStoryCreationCurrentDraft = useMemo<PersistedStoryCreationDraft>(() => ({
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
  }), [
    isSingleStoryBeatPlannerCustomized,
    isSingleStoryCreationBriefCustomized,
    isSingleStorySceneOutlineCustomized,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
    singleStoryBeatPlannerDraft,
    singleStoryCreationBriefDraft,
    singleStorySceneOutlineDraft,
    temporaryNarrativePerspective,
  ]);

  const batchStoryCreationCurrentDraft = useMemo<PersistedStoryCreationDraft>(() => ({
    creativeMode: batchSelectedCreativeMode,
    storyFocus: batchSelectedStoryFocus,
    plotStage: batchSelectedPlotStage,
    storyCreationBriefDraft: batchStoryCreationBriefDraft,
    beatPlannerDraft: batchStoryBeatPlannerDraft,
    sceneOutlineDraft: batchStorySceneOutlineDraft,
    isBriefCustomized: isBatchStoryCreationBriefCustomized,
    isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
    isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
  }), [
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStorySceneOutlineDraft,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStorySceneOutlineCustomized,
  ]);

  const canSaveSingleStoryCreationSnapshot = Boolean(
    singleStoryCreationDraftStorageKey
    && currentEditingChapter
    && hasMeaningfulStoryCreationDraft(singleStoryCreationCurrentDraft)
  );

  const canSaveBatchStoryCreationSnapshot = Boolean(
    batchStoryCreationDraftStorageKey
    && hasMeaningfulStoryCreationDraft(batchStoryCreationCurrentDraft)
  );


  useEffect(() => {
    const currentChapterId = currentEditingChapter?.id;
    const currentChapterNumber = currentEditingChapter?.chapter_number;

    if (!currentChapterId || currentChapterNumber == null) {
      return;
    }

    if (!singleStoryCreationDraftStorageKey) {
      resetSingleStoryCreationCockpit(currentChapterNumber);
      return;
    }

    let cancelled = false;

    void loadStoryCreationPersistence().then(({ getPersistedStoryCreationDraft }) => {
      const persistedDraft = getPersistedStoryCreationDraft(singleStoryCreationDraftStorageKey);

      if (cancelled) {
        return;
      }

      if (!persistedDraft) {
        resetSingleStoryCreationCockpit(currentChapterNumber);
        return;
      }

      singleStoryCreationAutoBriefRef.current = persistedDraft.isBriefCustomized
        ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
        : persistedDraft.storyCreationBriefDraft ?? singleDefaultStoryCreationBrief;
      singleStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
        ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
        : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
      singleStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
        ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
        : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

      setTemporaryNarrativePerspective(persistedDraft.narrativePerspective);
      setSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
      setSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
      setSelectedPlotStage(persistedDraft.plotStage ?? projectDefaultPlotStage);

      if (!persistedDraft.plotStage && !projectDefaultPlotStage) {
        void inferPlotStage({
          chapterNumber: currentChapterNumber,
          totalChapters: knownStructureChapterCount,
        }).then((stage) => {
          if (!cancelled) {
            setSelectedPlotStage(stage);
          }
        });
      }

      setSingleStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultStoryCreationBrief);
      setSingleStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
      setSingleStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
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
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetSingleStoryCreationCockpit,
    singleDefaultStoryCreationBrief,
    singleStoryCreationDraftStorageKey,
  ]);

  useEffect(() => {
    if (!batchStoryCreationDraftStorageKey) {
      resetBatchStoryCreationCockpit();
      return;
    }

    let cancelled = false;

    void loadStoryCreationPersistence().then(({ getPersistedStoryCreationDraft }) => {
      const persistedDraft = getPersistedStoryCreationDraft(batchStoryCreationDraftStorageKey);

      if (cancelled) {
        return;
      }

      if (!persistedDraft) {
        resetBatchStoryCreationCockpit();
        return;
      }

      batchStoryCreationAutoBriefRef.current = persistedDraft.isBriefCustomized
        ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
        : persistedDraft.storyCreationBriefDraft ?? batchDefaultStoryCreationBrief;
      batchStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
        ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
        : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
      batchStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
        ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
        : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

      setBatchSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
      setBatchSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
      setBatchSelectedPlotStage(persistedDraft.plotStage ?? projectDefaultPlotStage);
      setBatchStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultStoryCreationBrief);
      setBatchStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
      setBatchStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
    });

    return () => {
      cancelled = true;
    };
  }, [
    batchDefaultStoryCreationBrief,
    batchStoryCreationDraftStorageKey,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetBatchStoryCreationCockpit,
  ]);

  useEffect(() => {
    if (!singleStoryCreationDraftStorageKey) {
      setSingleStoryCreationSnapshots([]);
      return;
    }

    let cancelled = false;

    void loadStoryCreationPersistence().then(({ getPersistedStoryCreationSnapshots }) => {
      const snapshots = getPersistedStoryCreationSnapshots(singleStoryCreationDraftStorageKey);

      if (!cancelled) {
        setSingleStoryCreationSnapshots(snapshots);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [singleStoryCreationDraftStorageKey]);

  useEffect(() => {
    if (!batchStoryCreationDraftStorageKey) {
      setBatchStoryCreationSnapshots([]);
      return;
    }

    let cancelled = false;

    void loadStoryCreationPersistence().then(({ getPersistedStoryCreationSnapshots }) => {
      const snapshots = getPersistedStoryCreationSnapshots(batchStoryCreationDraftStorageKey);

      if (!cancelled) {
        setBatchStoryCreationSnapshots(snapshots);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [batchStoryCreationDraftStorageKey]);

  useEffect(() => {
    const currentChapterId = currentEditingChapter?.id;

    if (!singleStoryCreationDraftStorageKey || !currentChapterId) {
      return;
    }

    void loadStoryCreationPersistence().then(({ persistStoryCreationDraft }) => {
      persistStoryCreationDraft(singleStoryCreationDraftStorageKey, {
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
        updatedAt: new Date().toISOString(),
      });
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
    if (!batchStoryCreationDraftStorageKey) {
      return;
    }

    void loadStoryCreationPersistence().then(({ persistStoryCreationDraft }) => {
      persistStoryCreationDraft(batchStoryCreationDraftStorageKey, {
        creativeMode: batchSelectedCreativeMode,
        storyFocus: batchSelectedStoryFocus,
        plotStage: batchSelectedPlotStage,
        storyCreationBriefDraft: batchStoryCreationBriefDraft,
        beatPlannerDraft: batchStoryBeatPlannerDraft,
        sceneOutlineDraft: batchStorySceneOutlineDraft,
        isBriefCustomized: isBatchStoryCreationBriefCustomized,
        isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
        isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
        updatedAt: new Date().toISOString(),
      });
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
  ): Promise<StoryCreationSnapshot | null> => {
    if (!singleStoryCreationDraftStorageKey || !currentEditingChapter) {
      return null;
    }

    if (!hasMeaningfulStoryCreationDraft(singleStoryCreationCurrentDraft)) {
      if (!options?.silent) {
        message.warning('??????????????');
      }
      return null;
    }

    const { prompt, promptLayerLabels } = resolveStoryCreationPromptState({
      scope: 'single',
      briefDraft: singleStoryCreationBriefDraft,
      defaultBrief: singleDefaultStoryCreationBrief,
      beatPlannerDraft: singleStoryBeatPlannerDraft,
      sceneOutlineDraft: singleStorySceneOutlineDraft,
    });
    const normalizedPrompt = prompt?.trim();
    const latestSnapshot = singleStoryCreationSnapshots[0];

    if (
      latestSnapshot
      && latestSnapshot.reason === reason
      && areStoryCreationDraftContentsEqual(latestSnapshot, singleStoryCreationCurrentDraft, { includeNarrativePerspective: true })
      && normalizeOptionalText(latestSnapshot.prompt) === normalizeOptionalText(normalizedPrompt)
    ) {
      if (!options?.silent && reason === 'manual') {
        message.info('?????????????????');
      }
      return latestSnapshot;
    }

    const createdAt = new Date().toISOString();
    const chapterLabel = currentEditingChapter.chapter_number ? `?${currentEditingChapter.chapter_number}?` : '???';
    const { buildStoryCreationSnapshotId, persistStoryCreationSnapshot } = await loadStoryCreationPersistence();
    const snapshot: StoryCreationSnapshot = {
      ...singleStoryCreationCurrentDraft,
      id: buildStoryCreationSnapshotId(),
      scope: 'single',
      createdAt,
      updatedAt: createdAt,
      reason,
      label: options?.label?.trim() || `${chapterLabel} / ${reason === 'generate' ? '????' : '????'}`,
      prompt: normalizedPrompt || undefined,
      promptLayerLabels: [...promptLayerLabels],
      promptCharCount: normalizedPrompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshot);
    setSingleStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '????????' : '????????');
    }

    return nextSnapshots[0] ?? snapshot;
  }, [
    currentEditingChapter,
    resolveStoryCreationPromptState,
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
  ): Promise<StoryCreationSnapshot | null> => {
    if (!batchStoryCreationDraftStorageKey) {
      return null;
    }

    if (!hasMeaningfulStoryCreationDraft(batchStoryCreationCurrentDraft)) {
      if (!options?.silent) {
        message.warning('??????????????');
      }
      return null;
    }

    const { prompt, promptLayerLabels } = resolveStoryCreationPromptState({
      scope: 'batch',
      briefDraft: batchStoryCreationBriefDraft,
      defaultBrief: batchDefaultStoryCreationBrief,
      beatPlannerDraft: batchStoryBeatPlannerDraft,
      sceneOutlineDraft: batchStorySceneOutlineDraft,
    });
    const normalizedPrompt = prompt?.trim();
    const latestSnapshot = batchStoryCreationSnapshots[0];

    if (
      latestSnapshot
      && latestSnapshot.reason === reason
      && areStoryCreationDraftContentsEqual(latestSnapshot, batchStoryCreationCurrentDraft)
      && normalizeOptionalText(latestSnapshot.prompt) === normalizeOptionalText(normalizedPrompt)
    ) {
      if (!options?.silent && reason === 'manual') {
        message.info('?????????????????');
      }
      return latestSnapshot;
    }

    const createdAt = new Date().toISOString();
    const { buildStoryCreationSnapshotId, persistStoryCreationSnapshot } = await loadStoryCreationPersistence();
    const snapshot: StoryCreationSnapshot = {
      ...batchStoryCreationCurrentDraft,
      id: buildStoryCreationSnapshotId(),
      scope: 'batch',
      createdAt,
      updatedAt: createdAt,
      reason,
      label: options?.label?.trim() || `?? / ${reason === 'generate' ? '????' : '????'}`,
      prompt: normalizedPrompt || undefined,
      promptLayerLabels: [...promptLayerLabels],
      promptCharCount: normalizedPrompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshot);
    setBatchStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '????????' : '????????');
    }

    return nextSnapshots[0] ?? snapshot;
  }, [
    batchDefaultStoryCreationBrief,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationCurrentDraft,
    batchStoryCreationDraftStorageKey,
    batchStoryCreationSnapshots,
    batchStorySceneOutlineDraft,
    resolveStoryCreationPromptState,
  ]);

  const applySingleStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
    singleStoryCreationAutoBriefRef.current = snapshot.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : snapshot.storyCreationBriefDraft ?? '';
    singleStoryBeatPlannerAutoRef.current = snapshot.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft);
    singleStorySceneOutlineAutoRef.current = snapshot.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft);

    setTemporaryNarrativePerspective(snapshot.narrativePerspective);
    setSelectedCreativeMode(snapshot.creativeMode);
    setSelectedStoryFocus(snapshot.storyFocus);
    setSelectedPlotStage(snapshot.plotStage);

    if (!snapshot.plotStage) {
      void inferPlotStage({
        chapterNumber: currentEditingChapter?.chapter_number,
        totalChapters: knownStructureChapterCount,
      }).then((stage) => {
        setSelectedPlotStage(stage);
      });
    }
    setSingleStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
    setSingleStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
    setSingleStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
    message.success(`已应用快照：${snapshot.label}`);
  }, [currentEditingChapter?.chapter_number, inferPlotStage, knownStructureChapterCount]);

  const applyBatchStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
    batchStoryCreationAutoBriefRef.current = snapshot.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : snapshot.storyCreationBriefDraft ?? '';
    batchStoryBeatPlannerAutoRef.current = snapshot.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft);
    batchStorySceneOutlineAutoRef.current = snapshot.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft);

    setBatchSelectedCreativeMode(snapshot.creativeMode);
    setBatchSelectedStoryFocus(snapshot.storyFocus);
    setBatchSelectedPlotStage(snapshot.plotStage);
    setBatchStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
    setBatchStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
    setBatchStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
    message.success(`已加载快照：${snapshot.label}`);
  }, []);

  const deleteSingleStoryCreationSnapshot = useCallback(async (snapshotId: string) => {
    if (!singleStoryCreationDraftStorageKey) {
      return;
    }

    const { removePersistedStoryCreationSnapshot } = await loadStoryCreationPersistence();
    const nextSnapshots = removePersistedStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshotId);
    setSingleStoryCreationSnapshots(nextSnapshots);
    message.success('??????');
  }, [singleStoryCreationDraftStorageKey]);

  const deleteBatchStoryCreationSnapshot = useCallback(async (snapshotId: string) => {
    if (!batchStoryCreationDraftStorageKey) {
      return;
    }

    const { removePersistedStoryCreationSnapshot } = await loadStoryCreationPersistence();
    const nextSnapshots = removePersistedStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshotId);
    setBatchStoryCreationSnapshots(nextSnapshots);
    message.success('??????');
  }, [batchStoryCreationDraftStorageKey]);

  const copyStoryCreationPrompt = useCallback(async (
    content: string | undefined,
    scopeLabel: 'single' | 'batch',
  ) => {
    const normalizedContent = content?.trim();
    if (!normalizedContent) {
      message.warning(`${scopeLabel === 'single' ? '单章' : '批量'}提示词暂无可复制内容。`);

      return;
    }

    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(normalizedContent);
      } else {
        const tempTextArea = document.createElement('textarea');
        tempTextArea.value = normalizedContent;
        tempTextArea.setAttribute('readonly', 'true');
        tempTextArea.style.position = 'fixed';
        tempTextArea.style.opacity = '0';
        document.body.appendChild(tempTextArea);
        tempTextArea.select();
        document.execCommand('copy');
        document.body.removeChild(tempTextArea);
      }

    message.success(`已将${scopeLabel === 'single' ? '单章' : '批量'}提示词复制到剪贴板。`);
    } catch (error) {
      console.error('Failed to copy prompt.', error);
      message.error('复制失败，请重试。');
    }
  }, []);
  useEffect(() => {
    const previousAutoBrief = singleStoryCreationAutoBriefRef.current;

    if (!singleDefaultStoryCreationBrief) {
      singleStoryCreationAutoBriefRef.current = '';
      setSingleStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setSingleStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return singleDefaultStoryCreationBrief;
      }

      return prev;
    });

    singleStoryCreationAutoBriefRef.current = singleDefaultStoryCreationBrief;
  }, [singleDefaultStoryCreationBrief]);

  useEffect(() => {
    const previousAutoBrief = batchStoryCreationAutoBriefRef.current;

    if (!batchDefaultStoryCreationBrief) {
      batchStoryCreationAutoBriefRef.current = '';
      setBatchStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setBatchStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return batchDefaultStoryCreationBrief;
      }

      return prev;
    });

    batchStoryCreationAutoBriefRef.current = batchDefaultStoryCreationBrief;
  }, [batchDefaultStoryCreationBrief]);

  useEffect(() => {
    const previousAutoPlanner = singleStoryBeatPlannerAutoRef.current;

    if (isStoryBeatPlannerDraftEmpty(singleSystemStoryBeatPlanner)) {
      singleStoryBeatPlannerAutoRef.current = EMPTY_STORY_BEAT_PLANNER_DRAFT;
      setSingleStoryBeatPlannerDraft((prev) => (isStoryBeatPlannerDraftEmpty(prev) ? prev : { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }));
      return;
    }

    setSingleStoryBeatPlannerDraft((prev) => {
      if (isStoryBeatPlannerDraftEmpty(prev) || areStoryBeatPlannerDraftsEqual(prev, previousAutoPlanner)) {
        return singleSystemStoryBeatPlanner;
      }

      return prev;
    });

    singleStoryBeatPlannerAutoRef.current = singleSystemStoryBeatPlanner;
  }, [singleSystemStoryBeatPlanner]);

  useEffect(() => {
    const previousAutoPlanner = batchStoryBeatPlannerAutoRef.current;

    if (isStoryBeatPlannerDraftEmpty(batchSystemStoryBeatPlanner)) {
      batchStoryBeatPlannerAutoRef.current = EMPTY_STORY_BEAT_PLANNER_DRAFT;
      setBatchStoryBeatPlannerDraft((prev) => (isStoryBeatPlannerDraftEmpty(prev) ? prev : { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }));
      return;
    }

    setBatchStoryBeatPlannerDraft((prev) => {
      if (isStoryBeatPlannerDraftEmpty(prev) || areStoryBeatPlannerDraftsEqual(prev, previousAutoPlanner)) {
        return batchSystemStoryBeatPlanner;
      }

      return prev;
    });

    batchStoryBeatPlannerAutoRef.current = batchSystemStoryBeatPlanner;
  }, [batchSystemStoryBeatPlanner]);

  useEffect(() => {
    const previousSuggestedOutline = singleStorySceneOutlineAutoRef.current;

    if (isStorySceneOutlineDraftEmpty(singleSuggestedStorySceneOutline)) {
      singleStorySceneOutlineAutoRef.current = EMPTY_STORY_SCENE_OUTLINE_DRAFT;
      setSingleStorySceneOutlineDraft((prev) => (isStorySceneOutlineDraftEmpty(prev) ? prev : { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }));
      return;
    }

    setSingleStorySceneOutlineDraft((prev) => {
      if (isStorySceneOutlineDraftEmpty(prev) || areStorySceneOutlineDraftsEqual(prev, previousSuggestedOutline)) {
        return singleSuggestedStorySceneOutline;
      }

      return prev;
    });

    singleStorySceneOutlineAutoRef.current = singleSuggestedStorySceneOutline;
  }, [singleSuggestedStorySceneOutline]);

  useEffect(() => {
    const previousSuggestedOutline = batchStorySceneOutlineAutoRef.current;

    if (isStorySceneOutlineDraftEmpty(batchSuggestedStorySceneOutline)) {
      batchStorySceneOutlineAutoRef.current = EMPTY_STORY_SCENE_OUTLINE_DRAFT;
      setBatchStorySceneOutlineDraft((prev) => (isStorySceneOutlineDraftEmpty(prev) ? prev : { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }));
      return;
    }

    setBatchStorySceneOutlineDraft((prev) => {
      if (isStorySceneOutlineDraftEmpty(prev) || areStorySceneOutlineDraftsEqual(prev, previousSuggestedOutline)) {
        return batchSuggestedStorySceneOutline;
      }

      return prev;
    });

    batchStorySceneOutlineAutoRef.current = batchSuggestedStorySceneOutline;
  }, [batchSuggestedStorySceneOutline]);

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
    if (analysisPollingIntervalRef.current) {
      clearInterval(analysisPollingIntervalRef.current);
      analysisPollingIntervalRef.current = null;
    }

    if (clearTrackedChapterIds) {
      pollingIntervalsRef.current.clear();
    }
  }, []);

  const syncAnalysisTasksFromBatch = useCallback((
    items: Record<string, AnalysisTask>,
    options?: {
      reset?: boolean;
      notifyOnTerminalTransitions?: boolean;
    }
  ) => {
    const previousTasks = analysisTasksMapRef.current;
    const nextTasks = options?.reset ? {} : { ...previousTasks };

    Object.entries(items).forEach(([chapterId, task]) => {
      nextTasks[chapterId] = task;

      if (options?.notifyOnTerminalTransitions && previousTasks[chapterId]?.status !== task.status) {
        if (task.status === 'completed') {
          message.success('????????');
        } else if (task.status === 'failed') {
          message.error(`???????${task.error_message || '????'}`);
        }
      }
    });

    updateAnalysisTasksMap(nextTasks);
    return nextTasks;
  }, [updateAnalysisTasksMap]);

  const pollAnalysisTasksBatch = useCallback(async (projectId: string) => {
    const chapterIds = Array.from(pollingIntervalsRef.current);
    if (!projectId || chapterIds.length === 0) {
      stopAnalysisPolling(false);
      return;
    }

    try {
      const response = await chapterApi.getBatchChapterAnalysisStatus(chapterIds, projectId);
      if (currentProjectIdRef.current !== projectId) {
        return;
      }

      syncAnalysisTasksFromBatch(response.items, { notifyOnTerminalTransitions: true });

      pollingIntervalsRef.current = new Set(
        chapterIds.filter((chapterId) => isAnalysisTaskInProgress(response.items[chapterId]))
      );

      if (pollingIntervalsRef.current.size === 0) {
        stopAnalysisPolling(false);
      }
    } catch (error) {
      console.error('Failed to poll analysis tasks.', error);
    }
  }, [stopAnalysisPolling, syncAnalysisTasksFromBatch]);

  const ensureAnalysisPolling = useCallback((projectId: string) => {
    if (!projectId || pollingIntervalsRef.current.size === 0) {
      stopAnalysisPolling(false);
      return;
    }

    if (analysisPollingIntervalRef.current) {
      return;
    }

    const poll = () => {
      void pollAnalysisTasksBatch(projectId);
    };

    poll();
    analysisPollingIntervalRef.current = window.setInterval(poll, 2000);
  }, [pollAnalysisTasksBatch, stopAnalysisPolling]);

  const applyAnalysisPollingState = useCallback((projectId: string, tasksMap: Record<string, AnalysisTask>) => {
    pollingIntervalsRef.current = new Set(collectActiveAnalysisChapterIds(tasksMap));

    if (pollingIntervalsRef.current.size > 0) {
      ensureAnalysisPolling(projectId);
    } else {
      stopAnalysisPolling(false);
    }
  }, [ensureAnalysisPolling, stopAnalysisPolling]);

  useEffect(() => {
    const projectId = currentProject?.id ?? null;
    currentProjectIdRef.current = projectId;
    stopAnalysisPolling();
    updateAnalysisTasksMap(projectId ? (chapterAnalysisTasksCache.get(projectId) ?? {}) : {});

    if (projectId) {
      if (chapters.length === 0) {
        refreshChapters();
      }

      loadWritingStyles();
      loadAnalysisTasks();
      checkAndRestoreBatchTask();
    }

    return () => {
      stopAnalysisPolling();
    };

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProject?.id]);

  useEffect(() => {
    return () => {
      stopAnalysisPolling();

      if (batchPollingIntervalRef.current) {
        clearInterval(batchPollingIntervalRef.current);
      }
    };
  }, [stopAnalysisPolling]);

  const loadAnalysisTasks = async (chaptersToLoad?: typeof chapters) => {
    const projectId = currentProject?.id;
    const targetChapters = chaptersToLoad || chapters;

    if (!projectId) {
      stopAnalysisPolling();
      return;
    }

    currentProjectIdRef.current = projectId;

    if (!targetChapters || targetChapters.length === 0) {
      if (!chaptersToLoad) {
        updateAnalysisTasksMap({});
      }
      stopAnalysisPolling();
      return;
    }

    if (!chaptersToLoad) {
      const cachedTasks = chapterAnalysisTasksCache.get(projectId);
      if (cachedTasks) {
        updateAnalysisTasksMap(cachedTasks);
        applyAnalysisPollingState(projectId, cachedTasks);
        return;
      }
    }

    const targetChapterIds = targetChapters
      .filter((chapter) => chapter.content && chapter.content.trim() !== '')
      .map((chapter) => chapter.id);

    if (targetChapterIds.length === 0) {
      updateAnalysisTasksMap(chaptersToLoad ? { ...analysisTasksMapRef.current } : {});
      stopAnalysisPolling();
      return;
    }

    try {
      const response = await chapterApi.getBatchChapterAnalysisStatus(targetChapterIds, projectId);
      if (currentProjectIdRef.current !== projectId) {
        return;
      }

      const tasksMap = chaptersToLoad ? { ...analysisTasksMapRef.current } : {};
      targetChapterIds.forEach((chapterId) => {
        const task = response.items[chapterId];
        if (task) {
          const previousTask = analysisTasksMapRef.current[chapterId];
          tasksMap[chapterId] = areAnalysisTaskSnapshotsEqual(previousTask, task) ? previousTask : task;
        }
      });

      applyAnalysisPollingState(projectId, tasksMap);

      syncAnalysisTasksFromBatch(tasksMap, { reset: !chaptersToLoad });
    } catch (error) {
      console.error('Failed to load chapter analysis tasks.', error);
    }
  };

  const startPollingTask = (chapterId: string) => {
    pollingIntervalsRef.current.add(chapterId);

    const projectId = currentProjectIdRef.current ?? currentProject?.id;
    if (!projectId) {
      return;
    }

    ensureAnalysisPolling(projectId);
  };
  const refreshChapterAnalysisTask = async (chapterId: string) => {
    const projectId = currentProjectIdRef.current ?? currentProject?.id;
    if (!projectId) {
      return;
    }

    const task = await chapterApi.getChapterAnalysisStatus(chapterId, projectId);
    if (currentProjectIdRef.current !== projectId) {
      return;
    }

    syncAnalysisTasksFromBatch({ [chapterId]: task }, { notifyOnTerminalTransitions: true });

    if (isAnalysisTaskInProgress(task)) {
      startPollingTask(chapterId);
      return;
    }

    pollingIntervalsRef.current.delete(chapterId);
    if (pollingIntervalsRef.current.size === 0) {
      stopAnalysisPolling(false);
    }
  };

  const triggerDeferredBatchAnalysis = async (

    startChapterNumber: number,

    count: number,

    latestChapters: Chapter[]

  ) => {

    if (!currentProject?.id || count <= 0) return;



    const targetChapterNumbers = new Set(

      Array.from({ length: count }, (_, index) => startChapterNumber + index)

    );



    const candidateChapters = latestChapters.filter(ch =>

      targetChapterNumbers.has(ch.chapter_number) &&

      Boolean(ch.content && ch.content.trim() !== '')

    );



    if (candidateChapters.length === 0) return;



    let queuedCount = 0;

    let skippedCount = 0;

    let failedCount = 0;



    const ensureAnalysisTask = async (chapter: Chapter) => {

      const localTask = analysisTasksMap[chapter.id];

      if (localTask?.has_task && ['pending', 'running', 'completed'].includes(localTask.status)) {

        skippedCount += 1;

        if (localTask.status === 'pending' || localTask.status === 'running') {

          startPollingTask(chapter.id);

        }

        return;

      }



      try {

        const remoteTask = await chapterApi.getChapterAnalysisStatus(chapter.id, currentProject.id);

        if (remoteTask.has_task && ['pending', 'running', 'completed'].includes(remoteTask.status)) {

          skippedCount += 1;

          if (remoteTask.status === 'pending' || remoteTask.status === 'running') {

            startPollingTask(chapter.id);

          }

          return;

        }

      } catch (error) {

        console.error('Failed to query existing analysis task.', error);

      }



      try {

        await chapterApi.triggerChapterAnalysis(chapter.id, currentProject.id);

        queuedCount += 1;

        startPollingTask(chapter.id);

      } catch (error) {

        failedCount += 1;

        console.error(`Failed to queue analysis for chapter ${chapter.chapter_number}.`, error);

      }

    };



    const chunkSize = 3;

    for (let index = 0; index < candidateChapters.length; index += chunkSize) {

      const chunk = candidateChapters.slice(index, index + chunkSize);

      await Promise.all(chunk.map(ensureAnalysisTask));

    }



    if (queuedCount > 0) {

      message.info(`已将 ${queuedCount} 章加入分析队列。`);

    } else if (skippedCount > 0 && failedCount === 0) {

      message.info('已跳过完成分析的章节。');

    }



    if (failedCount > 0) {

      message.warning(`有 ${failedCount} 个章节分析失败。`);

    }



    await loadAnalysisTasks(latestChapters);

  };





  const loadWritingStyles = async () => {

    if (!currentProject?.id) return;



    const projectId = currentProject.id;

    const cachedStyles = writingStylesCache.get(projectId);

    if (cachedStyles) {

      setWritingStyles(cachedStyles.styles);

      setSelectedStyleId(cachedStyles.defaultStyleId);

      return;

    }



    const existingPromise = writingStylesLoadPromises.get(projectId);

    if (existingPromise) {

      await existingPromise;

      return;

    }



    const loadPromise = (async () => {

      try {

        const response = await writingStyleApi.getProjectStyles(projectId);
        const normalizedStyles = normalizeWritingStyleOptions(response.styles);

        setWritingStyles((previousStyles) => (
          areWritingStylesEqual(previousStyles, normalizedStyles) ? previousStyles : normalizedStyles
        ));

        const defaultStyle = normalizedStyles.find((style) => style.is_default);

        setSelectedStyleId((previousStyleId) => (
          previousStyleId === defaultStyle?.id ? previousStyleId : defaultStyle?.id
        ));

        writingStylesCache.set(projectId, {

          styles: normalizedStyles,

          defaultStyleId: defaultStyle?.id,

        });

      } catch (error) {

        console.error('Failed to load writing styles.', error);

        message.error('加载写作风格失败。');

      }

    })();



    writingStylesLoadPromises.set(projectId, loadPromise);

    try {

      await loadPromise;

    } finally {

      writingStylesLoadPromises.delete(projectId);

    }

  };



  const loadAvailableModels = useCallback(async () => {

    try {


      const settingsResponse = await fetch('/api/settings');

      if (settingsResponse.ok) {

        const settings = await settingsResponse.json();

        const { api_key, api_base_url, api_provider } = settings;
        const preferredModel = normalizeOptionalSelectValue(settings.llm_model);



        if (hasUsableApiCredentials(api_key, api_base_url)) {

          try {

            const modelsResponse = await fetch(

              `/api/settings/models?api_key=${encodeURIComponent(api_key)}&api_base_url=${encodeURIComponent(api_base_url)}&provider=${api_provider}`

            );

            if (modelsResponse.ok) {

              const data = await modelsResponse.json();
              const normalizedModels = normalizeModelOptions(data.models);

              setAvailableModels((previousModels) => (
                areModelOptionsEqual(previousModels, normalizedModels) ? previousModels : normalizedModels
              ));

              setSelectedModel((previousModel) => (
                previousModel === preferredModel ? previousModel : preferredModel
              ));

              return preferredModel ?? null;

            }

          } catch (error) {

            console.error('Failed to load models list.', error);

          }

        }

      }

    } catch (error) {

      console.error('Failed to load model settings.', error);

    }

    return null;

  }, []);



  // Check and restore batch task state.



  const checkAndRestoreBatchTask = async () => {

    if (!currentProject?.id) return;



    const projectId = currentProject.id;

    const existingPromise = batchTaskRestorePromises.get(projectId);

    if (existingPromise) {

      await existingPromise;

      return;

    }



    const restorePromise = (async () => {

      try {

        const data = await chapterBatchTaskApi.getActiveBatchGenerateTask(projectId);



        if (data.has_active_task && data.task) {

          const task = data.task;

          const persistedTaskMeta = getPersistedBatchTaskMeta(task.batch_id, projectId);

          if (persistedTaskMeta) {

            batchTaskMetaRef.current[task.batch_id] = persistedTaskMeta;

          }



          setBatchTaskId(task.batch_id);

          setBatchProgress({

            status: task.status,

            total: task.total,

            completed: task.completed,

            current_chapter_number: task.current_chapter_number ?? null,

            latest_quality_metrics: (task.latest_quality_metrics as {

              overall_score?: number;

              conflict_chain_hit_rate?: number;

              rule_grounding_hit_rate?: number;

              opening_hook_rate?: number;

              payoff_chain_rate?: number;

              cliffhanger_rate?: number;

            } | null | undefined) ?? undefined,

            quality_metrics_summary: (task.quality_metrics_summary as {

              avg_overall_score?: number;

              avg_conflict_chain_hit_rate?: number;

              avg_rule_grounding_hit_rate?: number;

              avg_opening_hook_rate?: number;

              avg_payoff_chain_rate?: number;

              avg_cliffhanger_rate?: number;

              chapter_count?: number;

            } | null | undefined) ?? undefined,

            quality_profile_summary: task.quality_profile_summary ?? null,

          });

          setBatchGenerating(true);

          setBatchGenerateVisible(false);

          startBatchPolling(task.batch_id);

          message.info('已恢复批量生成任务并继续运行。');

        }

      } catch (error) {

        console.error('Failed to restore batch task.', error);

      }

    })();



    batchTaskRestorePromises.set(projectId, restorePromise);

    try {

      await restorePromise;

    } finally {

      batchTaskRestorePromises.delete(projectId);

    }

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

        silent: false, // 闂佸湱铏庨崢浠嬪棘娓氣偓楠炴捇骞囬杞扮驳闂?

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

          outlineTitle: chapter.outline_title || '???',

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

  const parsedEditingPlanData = useMemo(() => {
    if (!editingPlanChapter?.expansion_plan) {
      return null;
    }

    try {
      return JSON.parse(editingPlanChapter.expansion_plan);
    } catch (error) {
      console.error('Failed to parse expansion plan JSON.', error);
      return null;
    }
  }, [editingPlanChapter?.expansion_plan]);

  const handleOpenModal = useCallback((id: string) => {

    const chapter = chapters.find(c => c.id === id);

    if (chapter) {

      form.setFieldsValue(chapter);

      setEditingId(id);

      setIsModalOpen(true);

    }

  }, [chapters, form]);



  const handleSubmit = async (values: ChapterUpdate) => {

    if (!editingId) return;



    try {

      await updateChapter(editingId, values);




      await refreshChapters();



      message.success('章节已更新。');

      setIsModalOpen(false);

      form.resetFields();

    } catch {

      message.error('更新章节失败。');

    }

  };



  const handleOpenEditor = useCallback((id: string) => {

    const chapter = chapters.find(c => c.id === id);

    if (chapter) {

      setCurrentChapter(chapter);

      editorForm.setFieldsValue({

        title: chapter.title,

        content: chapter.content,

      });
      resetSingleStoryCreationCockpit(chapter.chapter_number);
      setEditingId(id);
      setIsEditorOpen(true);
      setChapterQualityMetrics(null);



      // 闂佺懓鐏氶幐鍝ユ閹寸姷纾介柡宥庡墰鐢棛绱掗幇顓ф當鐟滅増鐩顔炬崉閸濆嫷娼遍柡澶屽仩婵倛鍟梺鎼炲妼椤戝懘宕归鍡樺仒?

      loadAvailableModels();



    }

  }, [chapters, editorForm, loadAvailableModels, resetSingleStoryCreationCockpit, setCurrentChapter]);



  const handleEditorSubmit = async (values: ChapterUpdate) => {

    if (!editingId || !currentProject) return;



    try {

      await updateChapter(editingId, values);




      const updatedProject = await projectApi.getProject(currentProject.id);

      setCurrentProject(updatedProject);



      message.success('??????');

      setIsEditorOpen(false);

    } catch {

      message.error('???????');

    }

  };



  const handleGenerate = async () => {

    if (!editingId) return;

    const chapterId = editingId;

    if (runningSingleChapterTasks[chapterId]) {

      message.info('该章节正在生成中。');

      return;

    }

    const progressMessageKey = `chapter-generate-progress-${chapterId}`;



    try {

      void saveSingleStoryCreationSnapshot('generate', { silent: true });

      setIsContinuing(true);

      setIsGenerating(true);

      setSingleChapterProgress(0);

      setSingleChapterProgressMessage('Generating chapter...');



      const latestSingleStoryPresetState = await loadSingleStoryPresetState();
      const latestSingleSystemStoryCreationBrief = latestSingleStoryPresetState.singleStoryCreationControlCard?.promptBrief ?? '';
      const { prompt: latestResolvedSingleStoryCreationBrief } = resolveStoryCreationPromptState({
        scope: 'single',
        briefDraft: singleStoryCreationBriefDraft,
        defaultBrief: latestSingleSystemStoryCreationBrief || projectDefaultStoryCreationBrief,
        beatPlannerDraft: singleStoryBeatPlannerDraft,
        sceneOutlineDraft: singleStorySceneOutlineDraft,
      });

      const result = await generateChapterContentStream(
        chapterId,
        undefined,
        selectedStyleId,
        targetWordCount,

        (progressMsg, progressValue) => {


          setSingleChapterProgress(progressValue);

          setSingleChapterProgressMessage(progressMsg);

        },
        selectedModel,
        temporaryNarrativePerspective,
        selectedCreativeMode,
        selectedStoryFocus,
        selectedPlotStage,
        latestResolvedSingleStoryCreationBrief,
        latestSingleStoryPresetState.singleStoryRepairPayload?.storyRepairSummary,
        latestSingleStoryPresetState.singleStoryRepairPayload?.storyRepairTargets,
        latestSingleStoryPresetState.singleStoryRepairPayload?.storyPreserveStrengths,
      );


      if (result.generation_task_id) {

        setRunningSingleChapterTasks(prev => ({

          ...prev,

          [chapterId]: result.generation_task_id

        }));

      }



      message.open({

        key: progressMessageKey,

        type: 'loading',

        content: '正在继续生成章节内容，请稍候...',

        duration: 0,

      });




      result.completion

        .then(async (finalResult) => {

          if (isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {

            const hasContentTouched = editorForm.isFieldsTouched(['content']);

            if (!hasContentTouched && finalResult?.content) {

              editorForm.setFieldsValue({ content: finalResult.content });

            } else if (hasContentTouched) {

              message.info('内容已被编辑，已保留你的修改。');

            }

          }



          message.open({

            key: progressMessageKey,

            type: 'success',

            content: '生成完成。',

            duration: 2,

          });



          if (finalResult?.analysis_task_id) {

            const taskId = finalResult.analysis_task_id;

            const pendingTask: AnalysisTask = {

              has_task: true,

              task_id: taskId,

              chapter_id: chapterId,

              status: 'pending',

              progress: 0

            };

            updateAnalysisTasksMap(prev => ({

              ...prev,

              [chapterId]: pendingTask

            }));

            chapterApi.upsertChapterAnalysisTaskToStore(pendingTask, currentProject?.id, 'chapter-analysis-task');

            startPollingTask(chapterId);

          }

          if (isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {
            setChapterQualityRefreshToken((prev) => prev + 1);
          }

        })

        .catch((error) => {

          const completionError = error as ApiError;

          message.open({

            key: progressMessageKey,

            type: 'error',

            content: '章节分析失败：' + (completionError.response?.data?.detail || completionError.message || '未知错误'),

            duration: 4,

          });

        })

        .finally(() => {

          setRunningSingleChapterTasks(prev => {

            if (!(chapterId in prev)) return prev;

            const next = { ...prev };

            delete next[chapterId];

            return next;

          });

        });



      message.success('已创建章节分析任务。');

    } catch (error) {

      const apiError = error as ApiError;

      message.error('续写失败：' + (apiError.response?.data?.detail || apiError.message || '未知错误'));

    } finally {

      setIsContinuing(false);

      setIsGenerating(false);

    }

  };

  const showGenerateModal = async (chapter: Chapter) => {
    const { openContinueGenerateDialog } = await import('../utils/chapterActionDialogs');

    openContinueGenerateDialog({
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
  const handleBatchGenerate = async (values: {
    startChapterNumber: number;
    count: number;
    enableAnalysis: boolean;
    styleId?: number;
    targetWordCount?: number;
    model?: string;
    creativeMode?: CreativeMode;
    storyFocus?: StoryFocus;
    plotStage?: PlotStage;
  }) => {
    if (!currentProject?.id) return;









    const styleId = values.styleId || selectedStyleId;

    const wordCount = values.targetWordCount || targetWordCount;



    const model = batchSelectedModel;
    const creativeMode = batchSelectedCreativeMode;
    const storyFocus = batchSelectedStoryFocus;
    const plotStage = batchSelectedPlotStage;





    if (!styleId) {

      message.error('请先选择写作风格。');

      return;

    }



    try {

      void saveBatchStoryCreationSnapshot('generate', { silent: true });

      setBatchGenerating(true);

      setBatchGenerateVisible(false); // 闂佺绻戞繛濠偽涢幘顔界厐鐎广儱娲ㄩ弸鍌炴倵閻㈡鏀伴柣锕佹椤╁ジ宕遍鐘殿槷闂備緡鍓欓悘婵嬪储閵堝鐒兼い鏃囨閻繝寮堕埡鍌溾槈閻庣懓鍟块锝夊捶椤撶姴鐐?



      const requestBody: {
        start_chapter_number: number;
        count: number;
        enable_analysis: boolean;
        style_id: number;
        target_word_count: number;
        model?: string;
        creative_mode?: CreativeMode;
        story_focus?: StoryFocus;
        plot_stage?: PlotStage;
        story_creation_brief?: string;
        story_repair_summary?: string;
        story_repair_targets?: string[];
        story_preserve_strengths?: string[];
      } = {
        start_chapter_number: values.startChapterNumber,
        count: values.count,
        enable_analysis: false,
        style_id: styleId,
        target_word_count: wordCount,
      };



      if (model) {

        requestBody.model = model;

      }

      if (creativeMode) {
        requestBody.creative_mode = creativeMode;
      }

      if (storyFocus) {
        requestBody.story_focus = storyFocus;
      }

      if (plotStage) {
        requestBody.plot_stage = plotStage;
      }

      const { prompt: resolvedBatchStoryCreationBrief } = resolveStoryCreationPromptState({
        scope: 'batch',
        briefDraft: batchStoryCreationBriefDraft,
        defaultBrief: batchDefaultStoryCreationBrief,
        beatPlannerDraft: batchStoryBeatPlannerDraft,
        sceneOutlineDraft: batchStorySceneOutlineDraft,
      });

      if (resolvedBatchStoryCreationBrief) {
        requestBody.story_creation_brief = resolvedBatchStoryCreationBrief;
      }

      const [{ buildBatchStoryRepairPromptPayloadFromSummary }, activeBatchCreationPreset] = await Promise.all([
        import('../utils/creationPresetsBatch'),
        resolveCreationPresetByModes(creativeMode, storyFocus),
      ]);
      const batchStoryRepairPayload = buildBatchStoryRepairPromptPayloadFromSummary(
        batchProgress?.quality_metrics_summary ?? null,
        creativeMode,
        storyFocus,
        {
          plotStage,
          chapterNumber: values.startChapterNumber,
          totalChapters: knownStructureChapterCount,
          activePresetId: activeBatchCreationPreset?.id,
        },
      );

      if (batchStoryRepairPayload?.storyRepairSummary) {
        requestBody.story_repair_summary = batchStoryRepairPayload.storyRepairSummary;
      }

      if (batchStoryRepairPayload?.storyRepairTargets?.length) {
        requestBody.story_repair_targets = batchStoryRepairPayload.storyRepairTargets;
      }

      if (batchStoryRepairPayload?.storyPreserveStrengths?.length) {
        requestBody.story_preserve_strengths = batchStoryRepairPayload.storyPreserveStrengths;
      }





      const result = await chapterBatchTaskApi.createBatchGenerateTask(currentProject.id, requestBody);

      setBatchTaskId(result.batch_id);

      batchTaskMetaRef.current[result.batch_id] = {

        startChapterNumber: values.startChapterNumber,

        count: values.count,

        autoAnalyze: values.enableAnalysis,

        projectId: currentProject.id,

      };

      persistBatchTaskMeta(result.batch_id, batchTaskMetaRef.current[result.batch_id]);

      setBatchProgress({

        status: 'running',

        total: result.chapters_to_generate.length,

        completed: 0,

        current_chapter_number: values.startChapterNumber,

        estimated_time_minutes: result.estimated_time_minutes,

        latest_quality_metrics: undefined,

        quality_metrics_summary: undefined,

        quality_profile_summary: null,

      });



      message.success(`已开始批量生成，预计耗时 ${result.estimated_time_minutes} 分钟。`);




      showBrowserNotification(

        '批量生成已开始',

        `计划生成 ${result.chapters_to_generate.length} 章，预计耗时 ${result.estimated_time_minutes} 分钟。`,

        'info'

      );



      // 閻庢鍠掗崑鎾斥攽椤旂⒈鍎撻柣銈呮閹风娀锝為鐔峰簥闂佸憡妫戠槐鏇熸叏閹间礁绠?

      startBatchPolling(result.batch_id);



    } catch (error: unknown) {

      const err = error as Error;

      message.error('批量生成失败：' + (err.message || '未知错误'));

      setBatchGenerating(false);

      setBatchGenerateVisible(false);

    }

  };




  const startBatchPolling = (taskId: string) => {

    if (batchPollingIntervalRef.current) {

      clearInterval(batchPollingIntervalRef.current);

    }



    const poll = async () => {

      try {

        const status = await chapterBatchTaskApi.getBatchGenerateStatus(taskId, currentProject?.id);

        setBatchProgress({

          status: status.status,

          total: status.total,

          completed: status.completed,

          current_chapter_number: status.current_chapter_number ?? null,

          latest_quality_metrics: (status.latest_quality_metrics as {

            overall_score?: number;

            conflict_chain_hit_rate?: number;

            rule_grounding_hit_rate?: number;

            opening_hook_rate?: number;

            payoff_chain_rate?: number;

            cliffhanger_rate?: number;

          } | null | undefined) ?? undefined,

          quality_metrics_summary: (status.quality_metrics_summary as {

            avg_overall_score?: number;

            avg_conflict_chain_hit_rate?: number;

            avg_rule_grounding_hit_rate?: number;

            avg_opening_hook_rate?: number;

            avg_payoff_chain_rate?: number;

            avg_cliffhanger_rate?: number;

            chapter_count?: number;

          } | null | undefined) ?? undefined,

          quality_profile_summary: status.quality_profile_summary ?? null,

        });





        if (status.completed > 0) {

          const latestChapters = await refreshChapters();

          await loadAnalysisTasks(latestChapters);




          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }

        }




        if (status.status === 'completed' || status.status === 'failed' || status.status === 'cancelled') {

          if (batchPollingIntervalRef.current) {

            clearInterval(batchPollingIntervalRef.current);

            batchPollingIntervalRef.current = null;

          }



          setBatchGenerating(false);

          const taskMeta = batchTaskMetaRef.current[taskId] ?? getPersistedBatchTaskMeta(taskId, currentProject?.id);





          const finalChapters = await refreshChapters();

          await loadAnalysisTasks(finalChapters);




          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }



          if (status.status === 'completed') {

            message.success(`批量生成完成，共生成 ${status.completed} 章。`);


            showBrowserNotification(

              '批量生成已完成',

              `项目“${currentProject?.title || '未命名项目'}”已完成 ${status.completed} 章生成。`,

              'success'

            );



            if (taskMeta?.autoAnalyze) {

              void triggerDeferredBatchAnalysis(taskMeta.startChapterNumber, taskMeta.count, finalChapters);

            }

          } else if (status.status === 'failed') {

            message.error(`批量生成失败：${status.error_message || '未知错误'}`);


            showBrowserNotification(

              '批量生成失败',

              status.error_message || '未知错误',

              'error'

            );

          } else if (status.status === 'cancelled') {

            message.warning('批量生成已取消。');

          }



          delete batchTaskMetaRef.current[taskId];

          removePersistedBatchTaskMeta(taskId);




          setTimeout(() => {

            setBatchGenerateVisible(false);

            setBatchTaskId(null);

            setBatchProgress(null);

          }, 2000);

        }

      } catch (error) {

        console.error('Failed to cancel batch generate task.', error);

      }

    };




    poll();




    batchPollingIntervalRef.current = window.setInterval(poll, 2000);

  };




  const handleCancelBatchGenerate = async () => {

    if (!batchTaskId) return;



    try {

      await chapterBatchTaskApi.cancelBatchGenerateTask(batchTaskId, currentProject?.id);

      delete batchTaskMetaRef.current[batchTaskId];

      removePersistedBatchTaskMeta(batchTaskId);



      message.success('批量生成已取消。');




      await refreshChapters();

      await loadAnalysisTasks();




      if (currentProject?.id) {

        const updatedProject = await projectApi.getProject(currentProject.id);

        setCurrentProject(updatedProject);

      }

    } catch (error: unknown) {

      const err = error as Error;

      message.error('取消批量生成失败：' + (err.message || '未知错误'));

    }

  };




  const handleOpenBatchGenerate = async () => {

    if (batchGenerating) {

      message.info('批量生成进行中，请等待当前任务完成。');

      return;

    }





    if (!firstIncompleteChapter) {

      message.info('没有可生成的未完成章节。');

      return;

    }




    if (!canGenerateChapter(firstIncompleteChapter)) {

      const reason = getGenerateDisabledReason(firstIncompleteChapter);

      message.warning(reason);

      return;

    }




    const defaultModel = await loadAvailableModels();







    resetBatchStoryCreationCockpit();
    setBatchSelectedModel(defaultModel || undefined);
    setBatchSelectedPlotStage(projectDefaultPlotStage);

    if (!projectDefaultPlotStage) {
      const inferredStage = await inferPlotStage({
        chapterNumber: firstIncompleteChapter.chapter_number,
        totalChapters: knownStructureChapterCount,
      });
      setBatchSelectedPlotStage(inferredStage);
    }



    batchForm.setFieldsValue({

      startChapterNumber: firstIncompleteChapter.chapter_number,

      count: 5,

      enableAnalysis: true,

      styleId: selectedStyleId,

      targetWordCount: getCachedWordCount(),

    });



    setBatchGenerateVisible(true);

  };




  const getStatusText = (status: string) => {
    const texts: Record<string, string> = {
      draft: '??',
      writing: '???',
      completed: '???',
    };

    return texts[status] || status;
  };

  const handleExport = () => {
    if (!currentProject) {
      return;
    }

    if (chapters.length === 0) {
      message.warning('???????');
      return;
    }

    modal.confirm({
      title: '????',
      content: `???????${currentProject.title}??`,
      centered: true,
      okText: '??',
      cancelText: '??',
      onOk: () => {
        try {
          projectApi.exportProject(currentProject.id);
          message.success('??????');
        } catch {
          message.error('??????');
        }
      },
    });
  };

  const handleShowAnalysis = useCallback((chapterId: string) => {
    setAnalysisChapterId(chapterId);
    setAnalysisVisible(true);
  }, []);

  const showManualCreateChapterModal = async () => {
    const { openManualCreateChapterDialog } = await import('../utils/chapterActionDialogs');

    openManualCreateChapterDialog({
      modal,
      chapters,
      manualCreateForm,
      sortedOutlines,
      currentProject,
      chapterApi,
      projectApi,
      refreshChapters,
      setCurrentProject,
      message,
      handleDeleteChapter,
      getStatusText,
    });
  };

  const handleDeleteChapter = useCallback(async (chapterId: string) => {
    try {
      await deleteChapter(chapterId);
      await refreshChapters();

      if (currentProject) {
        const updatedProject = await projectApi.getProject(currentProject.id);
        setCurrentProject(updatedProject);
      }

      message.success('??????');
    } catch (error: unknown) {
      const err = error as Error;
      message.error('???????' + (err.message || '????'));
    }
  }, [currentProject, deleteChapter, refreshChapters, setCurrentProject]);

  const showExpansionPlanModal = useCallback(async (chapter: Chapter) => {
    const { openExpansionPlanPreviewDialog } = await import('../utils/chapterActionDialogs');

    openExpansionPlanPreviewDialog({
      modal,
      chapter,
      isMobile,
      message,
    });
  }, [isMobile, modal]);

  const handleOpenPlanEditor = useCallback((chapter: Chapter) => {


    setEditingPlanChapter(chapter);

    setPlanEditorVisible(true);

  }, []);




  const handleSavePlan = async (planData: ExpansionPlanData) => {

    if (!editingPlanChapter) return;



    try {

      const response = await fetch(`/api/chapters/${editingPlanChapter.id}/expansion-plan`, {

        method: 'PUT',

        headers: {

          'Content-Type': 'application/json',

        },

        body: JSON.stringify(planData),

      });



      if (!response.ok) {

        const error = await response.json();

        throw new Error(error.detail || 'Save plan failed.');

      }




      await refreshChapters();



      message.success('章节规划已保存。');




      setPlanEditorVisible(false);

      setEditingPlanChapter(null);

    } catch (error: unknown) {

      const err = error as Error;

      message.error('保存章节规划失败：' + (err.message || '未知错误'));

      throw error;

    }

  };



  const handleChapterSelect = (chapterId: string) => {

    const element = document.getElementById(`chapter-item-${chapterId}`);

    if (element) {

      element.scrollIntoView({ behavior: 'smooth', block: 'center' });

      // Optional: add a visual highlight effect

      element.style.transition = 'background-color 0.5s ease';

      element.style.backgroundColor = '#e6f7ff';

      setTimeout(() => {

        element.style.backgroundColor = '';

      }, 1500);

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹达附鈷撻柛娑㈠亰閸ゃ垽鏌?

  const handleOpenReader = useCallback((chapter: Chapter) => {

    setReadingChapter(chapter);

    setReaderVisible(true);

  }, []);




  const handleReaderChapterChange = async (chapterId: string) => {

    try {

      const response = await fetch(`/api/chapters/${chapterId}`);

      if (!response.ok) throw new Error('Failed to load chapter.');

      const newChapter = await response.json();

      setReadingChapter(newChapter);

    } catch {

      message.error('加载章节失败。');

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹寸姳娌柍褜鍓熼弻鍫ュΩ閳轰焦顏熼梺鍛婂姈閻熴儵鎳樻繝鍕幓?



  const handleCloseEditor = useCallback(() => {
    setChapterQualityMetrics(null);
    setIsEditorOpen(false);
  }, []);

  const editorAiSectionProps = useMemo(() => ({
    currentEditingChapterNumber: currentEditingChapter?.chapter_number,
    applySingleCreationPreset,
    projectDefaultCreativeMode,
    setSelectedCreativeMode,
    projectDefaultStoryFocus,
    setSelectedStoryFocus,
    selectedPlotStage,
    setSelectedPlotStage,
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
    copyStoryCreationPrompt,
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
    projectDefaultStoryFocus,
    resolvedSingleStoryCreationBrief,
    saveSingleStoryCreationSnapshot,
    selectedCreativeMode,
    selectedModel,
    selectedPlotStage,
    selectedStoryFocus,
    setSelectedCreativeMode,
    setSelectedModel,
    setSelectedPlotStage,
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

          章节

        </h2>

        <Space direction={isMobile ? 'vertical' : 'horizontal'} style={{ width: isMobile ? '100%' : 'auto' }}>

          {currentProject.outline_mode === 'one-to-many' && (

            <Button

              icon={<PlusOutlined />}

              onClick={showManualCreateChapterModal}

              block={isMobile}

              size={isMobile ? 'middle' : 'middle'}

            >

              ????

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

            ????

          </Button>

          <Button

            type="default"

            icon={<DownloadOutlined />}

            onClick={handleExport}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

          >

            ??

          </Button>

          {!isMobile && (

            <Tag color="blue">

              {currentProject.outline_mode === 'one-to-one'

                ? '??????'

                : '??????'}

            </Tag>

          )}

        </Space>

      </div>



      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}>

        {chapters.length === 0 ? (

          <Empty description="?????" />

        ) : currentProject.outline_mode === 'one-to-one' ? (

          <List

            rowKey="id"

            dataSource={sortedChapters}

            renderItem={(item) => (

              <ChapterListItem

                chapter={item}

                variant="flat"

                isMobile={isMobile}

                showOutlineActions={false}

                analysisTask={analysisTasksMap[item.id]}

                canGenerate={chapterGenerationStateById[item.id]?.canGenerate ?? false}

                generateDisabledReason={chapterGenerationStateById[item.id]?.disabledReason ?? ''}

                onOpenReader={handleOpenReader}

                onOpenEditor={handleOpenEditor}

                onShowAnalysis={handleShowAnalysis}

                onOpenSettings={handleOpenModal}

                onDeleteChapter={handleDeleteChapter}

                onShowExpansionPlan={showExpansionPlanModal}

                onOpenPlanEditor={handleOpenPlanEditor}

              />

            )}

          />

        ) : (


          <Collapse

            bordered={false}

            defaultActiveKey={expandedChapterGroupKeys}

            expandIcon={({ isActive }) => <CaretRightOutlined rotate={isActive ? 90 : 0} />}

            style={{ background: 'transparent' }}

          >

            {groupedChapters.map((group) => (

              <Collapse.Panel

                key={group.key}

                header={

                  <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>

                    <Tag color={group.outlineId ? 'blue' : 'default'} style={{ margin: 0 }}>

                      {group.outlineId ? `?? ${group.outlineOrder}` : '?????'}

                    </Tag>

                    <span style={{ fontWeight: 600, fontSize: 16 }}>

                      {group.outlineTitle}

                    </span>

                    <Badge

                      count={`${group.chapters.length}?`}

                      style={{ backgroundColor: 'var(--color-success)' }}

                    />

                    <Badge

                      count={`${group.totalWordCount}?`}

                      style={{ backgroundColor: 'var(--color-primary)' }}

                    />

                  </div>

                }

                style={{

                  marginBottom: 16,

                  background: '#fff',

                  borderRadius: 8,

                  border: '1px solid #f0f0f0',

                }}

              >

                <List

                  rowKey="id"

                  dataSource={group.chapters}

                  renderItem={(item) => (

                    <ChapterListItem

                      chapter={item}

                      variant="grouped"

                      isMobile={isMobile}

                      showOutlineActions={currentProject.outline_mode === 'one-to-many'}

                      analysisTask={analysisTasksMap[item.id]}

                      canGenerate={chapterGenerationStateById[item.id]?.canGenerate ?? false}

                      generateDisabledReason={chapterGenerationStateById[item.id]?.disabledReason ?? ''}

                      onOpenReader={handleOpenReader}

                      onOpenEditor={handleOpenEditor}

                      onShowAnalysis={handleShowAnalysis}

                      onOpenSettings={handleOpenModal}

                      onDeleteChapter={handleDeleteChapter}

                      onShowExpansionPlan={showExpansionPlanModal}

                      onOpenPlanEditor={handleOpenPlanEditor}

                    />

                  )}

                />

              </Collapse.Panel>

            ))}

          </Collapse>

        )}

      </div>



      {isModalOpen ? (
        <Suspense fallback={null}>
          <LazyChapterBasicModal
            open={isModalOpen}
            title={editingId ? '????' : '????'}
            isMobile={isMobile}
            outlineMode={currentProject.outline_mode}
            submitText={editingId ? '????' : '????'}
            form={form}
            onCancel={() => setIsModalOpen(false)}
            onFinish={handleSubmit}
          />
        </Suspense>
      ) : null}

      {isEditorOpen ? (
        <Modal

        title="编辑章节内容"

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




      {analysisChapterId ? (

        <Suspense fallback={null}>

          <LazyChapterAnalysis

            chapterId={analysisChapterId}

            visible={analysisVisible}

            onClose={() => {

              setAnalysisVisible(false);




            refreshChapters();




            if (currentProject) {

              projectApi.getProject(currentProject.id)

                .then(updatedProject => {

                  setCurrentProject(updatedProject);

                })

                .catch(error => {

                  console.error('Failed to refresh chapter analysis after closing modal.', error);

                });

            }




            if (analysisChapterId) {

              const chapterIdToRefresh = analysisChapterId;



              setTimeout(() => {

                refreshChapterAnalysisTask(chapterIdToRefresh)

                  .catch(error => {

                    console.error('Failed to refresh chapter analysis after delayed retry.', error);


                    setTimeout(() => {

                      refreshChapterAnalysisTask(chapterIdToRefresh)

                        .catch(err => console.error('Failed to refresh chapter analysis after second retry.', err));

                    }, 1000);

                  });

              }, 500);

            }



            setAnalysisChapterId(null);

          }}

          />

        </Suspense>

      ) : null}



      {batchGenerateVisible || batchGenerating ? (
        <Suspense fallback={null}>
          <LazyChapterBatchGenerateModal
            applyBatchCreationPreset={applyBatchCreationPreset}
            applyBatchStoryCreationSnapshot={applyBatchStoryCreationSnapshot}
            applyInferredBatchPlotStage={applyInferredBatchPlotStage}
            availableModels={availableModels}
            batchEnableAnalysis={batchEnableAnalysis}
            batchForm={batchForm}
            batchGenerateVisible={batchGenerateVisible}
            batchGenerating={batchGenerating}
            batchProgress={batchProgress}
            batchSelectedCreativeMode={batchSelectedCreativeMode}
            batchSelectedModel={batchSelectedModel}
            batchSelectedPlotStage={batchSelectedPlotStage}
            batchSelectedStoryFocus={batchSelectedStoryFocus}
            batchStartChapterOptions={batchStartChapterOptions}
            batchStoryBeatPlannerDraft={batchStoryBeatPlannerDraft}
            batchStoryCreationBriefDraft={batchStoryCreationBriefDraft}
            batchStoryCreationCurrentDraft={batchStoryCreationCurrentDraft}
            batchStoryCreationSnapshots={batchStoryCreationSnapshots}
            batchStorySceneOutlineDraft={batchStorySceneOutlineDraft}
            batchSuggestedStorySceneOutline={batchSuggestedStorySceneOutline}
            batchSystemStoryBeatPlanner={batchSystemStoryBeatPlanner}
            canSaveBatchStoryCreationSnapshot={canSaveBatchStoryCreationSnapshot}
            copyStoryCreationPrompt={copyStoryCreationPrompt}
            CREATIVE_MODE_OPTIONS={CREATIVE_MODE_OPTIONS}
            deleteBatchStoryCreationSnapshot={deleteBatchStoryCreationSnapshot}
            handleBatchGenerate={handleBatchGenerate}
            handleCancelBatchGenerate={handleCancelBatchGenerate}
            isBatchStoryBeatPlannerCustomized={isBatchStoryBeatPlannerCustomized}
            isBatchStoryCreationBriefCustomized={isBatchStoryCreationBriefCustomized}
            isBatchStoryCreationControlCustomized={isBatchStoryCreationControlCustomized}
            isBatchStorySceneOutlineCustomized={isBatchStorySceneOutlineCustomized}
            isMobile={isMobile}
            modal={modal}
            knownStructureChapterCount={knownStructureChapterCount}
            projectDefaultCreativeMode={projectDefaultCreativeMode}
            projectDefaultStoryFocus={projectDefaultStoryFocus}
            resolvedBatchStoryCreationBrief={resolvedBatchStoryCreationBrief}
            batchStoryCreationPromptLayerLabels={batchStoryCreationPromptLayerLabels}
            batchStoryCreationPromptCharCount={batchStoryCreationPromptCharCount}
            isBatchStoryCreationPromptVerbose={isBatchStoryCreationPromptVerbose}
            STORY_CREATION_PROMPT_WARN_THRESHOLD={STORY_CREATION_PROMPT_WARN_THRESHOLD}
            saveBatchStoryCreationSnapshot={saveBatchStoryCreationSnapshot}
            selectedModel={selectedModel}
            selectedStyleId={selectedStyleId}
            setBatchGenerateVisible={setBatchGenerateVisible}
            setBatchSelectedCreativeMode={setBatchSelectedCreativeMode}
            setBatchSelectedModel={setBatchSelectedModel}
            setBatchSelectedPlotStage={setBatchSelectedPlotStage}
            setBatchSelectedStoryFocus={setBatchSelectedStoryFocus}
            setBatchStoryBeatPlannerDraft={setBatchStoryBeatPlannerDraft}
            setBatchStoryCreationBriefDraft={setBatchStoryCreationBriefDraft}
            setBatchStorySceneOutlineDraft={setBatchStorySceneOutlineDraft}
            sortedChapters={sortedChapters}
            STORY_FOCUS_OPTIONS={STORY_FOCUS_OPTIONS}
            writingStyles={writingStyles}
          />
        </Suspense>
      ) : null}


      {isGenerating ? (

        <Suspense fallback={null}>

          <LazySSELoadingOverlay

            loading={isGenerating}

            progress={singleChapterProgress}

            message={singleChapterProgressMessage}

            blocking={false}

          />

        </Suspense>

      ) : null}




      {batchGenerating ? (

        <Suspense fallback={null}>

          <LazySSEProgressModal

            visible={batchGenerating}

            progress={batchProgress ? Math.round((batchProgress.completed / batchProgress.total) * 100) : 0}

            message={

              batchProgress?.current_chapter_number

                ? `正在生成第 ${batchProgress.current_chapter_number} 章... (${batchProgress.completed}/${batchProgress.total})${

                    batchProgress.latest_quality_metrics?.overall_score !== undefined

                      ? ` 质量分：${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

                : `批量生成进行中... (${batchProgress?.completed || 0}/${batchProgress?.total || 0})${

                    batchProgress?.latest_quality_metrics?.overall_score !== undefined

                      ? ` 质量分：${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

            }

            title="批量生成进度"

            onCancel={() => {

              modal.confirm({

                title: '确认取消批量生成？',

                content: '取消后当前批量生成任务将停止，已生成的章节会保留。确定要取消吗？',

                okText: '确认取消',

                cancelText: '继续生成',

                okButtonProps: { danger: true },

                centered: true,

                onOk: handleCancelBatchGenerate,

              });

            }}

            cancelButtonText="取消生成"

            blocking={false}

          />

        </Suspense>

      ) : null}



      <FloatButton

        icon={<BookOutlined />}

        type="primary"

        tooltip="打开章节目录"

        onClick={() => setIsIndexPanelVisible(true)}

        style={{ right: isMobile ? 24 : 48, bottom: isMobile ? 80 : 48 }}

      />



      {isIndexPanelVisible ? (

        <Suspense fallback={null}>

          <LazyFloatingIndexPanel

            visible={isIndexPanelVisible}

            onClose={() => setIsIndexPanelVisible(false)}

            groupedChapters={groupedChapters}

            onChapterSelect={handleChapterSelect}

          />

        </Suspense>

      ) : null}




      {readerVisible && readingChapter ? (

        <Suspense fallback={null}>

          <LazyChapterReader

            visible={readerVisible}

            chapter={readingChapter}

            onClose={() => {

              setReaderVisible(false);

              setReadingChapter(null);

            }}

            onChapterChange={handleReaderChapterChange}

          />

        </Suspense>

      ) : null}




      {planEditorVisible && editingPlanChapter && currentProject ? (
        <Suspense fallback={null}>
          <LazyExpansionPlanEditor
            visible={planEditorVisible}
            planData={parsedEditingPlanData}
            chapterSummary={editingPlanChapter.summary || null}
            projectId={currentProject.id}
            onSave={handleSavePlan}
            onCancel={() => {
              setPlanEditorVisible(false);
              setEditingPlanChapter(null);
            }}
          />
        </Suspense>
      ) : null}

    </div>
    </>
  );

}


