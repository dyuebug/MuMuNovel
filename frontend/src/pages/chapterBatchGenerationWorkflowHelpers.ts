import { message, type FormInstance } from 'antd';
import { chapterBatchTaskApi } from '../services/modularApi';
import type {
  Chapter,
  ChapterQualityMetrics,
  ChapterQualityMetricsSummary,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../types';
import type { CreationPresetId } from '../utils/creationPresetsCore';
import type {
  StoryBeatPlannerDraft,
  StoryCreationSnapshotReason,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import {
  createInitialBatchProgressState,
  prepareBatchGenerationTaskRequest,
  type BatchGenerateFormValues,
} from './chapterBatchGenerationRequestHelpers';
import type {
  BatchProgressState,
  BatchTaskMeta,
} from './chapterBatchGenerationPollingHelpers';
import type { ResolveStoryCreationPromptState } from './chapterStoryCreationPromptHelpers';

type ShowBrowserNotification = (
  title: string,
  body: string,
  type?: 'success' | 'error' | 'info',
) => void;

export async function startBatchGenerationWorkflow({
  values,
  projectId,
  selectedStyleId,
  targetWordCount,
  model,
  creativeMode,
  storyFocus,
  plotStage,
  qualityPreset,
  qualityNotes,
  qualityMetricsSummary,
  batchStoryCreationBriefDraft,
  batchDefaultStoryCreationBrief,
  batchStoryBeatPlannerDraft,
  batchStorySceneOutlineDraft,
  knownStructureChapterCount,
  resolveStoryCreationPromptState,
  resolveCreationPresetByModes,
  saveStoryCreationSnapshot,
  setBatchGenerating,
  setBatchGenerateVisible,
  setBatchTaskId,
  rememberTaskMeta,
  persistTaskMeta,
  setBatchProgress,
  startBatchPolling,
  showBrowserNotification,
}: {
  values: BatchGenerateFormValues;
  projectId: string;
  selectedStyleId?: number;
  targetWordCount: number;
  model?: string;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  qualityPreset?: QualityPreset;
  qualityNotes?: string;
  qualityMetricsSummary?: ChapterQualityMetricsSummary | null;
  batchStoryCreationBriefDraft: string;
  batchDefaultStoryCreationBrief: string;
  batchStoryBeatPlannerDraft: StoryBeatPlannerDraft;
  batchStorySceneOutlineDraft: StorySceneOutlineDraft;
  knownStructureChapterCount?: number | null;
  resolveStoryCreationPromptState: ResolveStoryCreationPromptState;
  resolveCreationPresetByModes: (
    creativeMode?: CreativeMode,
    storyFocus?: StoryFocus,
  ) => Promise<{ id?: CreationPresetId | null } | null | undefined>;
  saveStoryCreationSnapshot: (
    reason: StoryCreationSnapshotReason,
    options?: { silent?: boolean; label?: string },
  ) => Promise<unknown> | void;
  setBatchGenerating: (value: boolean) => void;
  setBatchGenerateVisible: (value: boolean) => void;
  setBatchTaskId: (taskId: string | null) => void;
  rememberTaskMeta: (taskId: string, taskMeta: BatchTaskMeta) => void;
  persistTaskMeta: (taskId: string, taskMeta: BatchTaskMeta) => void;
  setBatchProgress: (progress: BatchProgressState | null) => void;
  startBatchPolling: (taskId: string) => void;
  showBrowserNotification: ShowBrowserNotification;
}): Promise<void> {
  const styleId = values.styleId || selectedStyleId;
  const resolvedTargetWordCount = values.targetWordCount || targetWordCount;

  if (!styleId) {
    message.error('Select a writing style first.');
    return;
  }

  try {
    void saveStoryCreationSnapshot('generate', { silent: true });

    setBatchGenerating(true);
    setBatchGenerateVisible(false);

    const { requestBody, taskMeta } = await prepareBatchGenerationTaskRequest({
      values,
      projectId,
      styleId,
      targetWordCount: resolvedTargetWordCount,
      model,
      creativeMode,
      storyFocus,
      plotStage,
      qualityPreset,
      qualityNotes,
      batchQualityMetricsSummary: qualityMetricsSummary ?? null,
      batchStoryCreationBriefDraft,
      batchDefaultStoryCreationBrief,
      batchStoryBeatPlannerDraft,
      batchStorySceneOutlineDraft,
      knownStructureChapterCount,
      resolveStoryCreationPromptState,
      resolveCreationPresetByModes,
    });

    const result = await chapterBatchTaskApi.createBatchGenerateTask(projectId, requestBody);

    setBatchTaskId(result.batch_id);
    rememberTaskMeta(result.batch_id, taskMeta);
    persistTaskMeta(result.batch_id, taskMeta);
    setBatchProgress(createInitialBatchProgressState({
      startChapterNumber: values.startChapterNumber,
      total: result.chapters_to_generate.length,
      estimatedTimeMinutes: result.estimated_time_minutes,
    }));

    message.success(`Batch generation started. ETA ${result.estimated_time_minutes} min.`);
    showBrowserNotification(
      'Batch generation started',
      `Started ${result.chapters_to_generate.length} chapters. ETA ${result.estimated_time_minutes} min.`,
      'info',
    );

    startBatchPolling(result.batch_id);
  } catch (error: unknown) {
    const err = error as Error;
    message.error('Batch generation failed: ' + (err.message || 'Unknown error'));
    setBatchGenerating(false);
    setBatchGenerateVisible(false);
  }
}


export async function openBatchGenerationWorkflow({
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
  cachedWordCount,
  setBatchGenerateVisible,
}: {
  batchGenerating: boolean;
  firstIncompleteChapter?: Chapter;
  canGenerateChapter: (chapter: Chapter) => boolean;
  getGenerateDisabledReason: (chapter: Chapter) => string;
  loadAvailableModels: () => Promise<string | null | undefined>;
  resetBatchStoryCreationCockpit: () => void;
  setBatchSelectedModel: (value: string | undefined) => void;
  setBatchSelectedPlotStage: (value: PlotStage | undefined) => void;
  projectDefaultPlotStage?: PlotStage;
  inferPlotStage: (options: {
    chapterNumber?: number | null;
    totalChapters?: number | null;
    presetId?: CreationPresetId | null;
    storyFocus?: StoryFocus;
    metrics?: ChapterQualityMetrics | null;
  }) => Promise<PlotStage | undefined>;
  knownStructureChapterCount?: number | null;
  batchForm: FormInstance;
  selectedStyleId?: number;
  cachedWordCount: number;
  setBatchGenerateVisible: (value: boolean) => void;
}): Promise<void> {
  if (batchGenerating) {
    message.info('Batch generation is already running.');
    return;
  }

  if (!firstIncompleteChapter) {
    message.info('No remaining chapters to generate.');
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
    targetWordCount: cachedWordCount,
  });

  setBatchGenerateVisible(true);
}
export async function cancelBatchGenerationWorkflow({
  batchTaskId,
  projectId,
  isPageActiveRef,
  currentProjectIdRef,
  removeTaskMeta,
  refreshChapters,
  loadAnalysisTasks,
  reloadCurrentProject,
}: {
  batchTaskId: string | null;
  projectId?: string;
  isPageActiveRef?: { current: boolean };
  currentProjectIdRef?: { current: string | null };
  removeTaskMeta: (taskId: string) => void;
  refreshChapters: () => Promise<Chapter[]>;
  loadAnalysisTasks: (chaptersToLoad?: Chapter[]) => Promise<void>;
  reloadCurrentProject: () => Promise<void>;
}): Promise<void> {
  if (!batchTaskId) {
    return;
  }

  try {
    await chapterBatchTaskApi.cancelBatchGenerateTask(batchTaskId, projectId);
    if (
      (isPageActiveRef && !isPageActiveRef.current)
      || (projectId && currentProjectIdRef && currentProjectIdRef.current !== projectId)
    ) {
      return;
    }

    removeTaskMeta(batchTaskId);
    message.success('Batch generation cancelled.');

    const latestChapters = await refreshChapters();
    if (
      (isPageActiveRef && !isPageActiveRef.current)
      || (projectId && currentProjectIdRef && currentProjectIdRef.current !== projectId)
    ) {
      return;
    }
    await loadAnalysisTasks(latestChapters);
    if (
      (isPageActiveRef && !isPageActiveRef.current)
      || (projectId && currentProjectIdRef && currentProjectIdRef.current !== projectId)
    ) {
      return;
    }
    await reloadCurrentProject();
  } catch (error: unknown) {
    if (
      (isPageActiveRef && !isPageActiveRef.current)
      || (projectId && currentProjectIdRef && currentProjectIdRef.current !== projectId)
    ) {
      return;
    }
    const err = error as Error;
    message.error('Cancel batch generation failed: ' + (err.message || 'Unknown error'));
  }
}
