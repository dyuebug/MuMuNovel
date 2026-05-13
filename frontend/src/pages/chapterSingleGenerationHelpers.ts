import { message, type FormInstance } from 'antd';
import type { Dispatch, MutableRefObject, SetStateAction } from 'react';
import { chapterApi } from '../services/modularApi';
import type {
  AnalysisTask,
  ApiError,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../types';
import type {
  StoryBeatPlannerDraft,
  StoryCreationSnapshotReason,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type {
  GenerateChapterContentStreamResult,
} from '../store/chapterGenerationWorkflow';

export type GenerateChapterContentStream = (
  chapterId: string,
  onProgress?: (content: string) => void,
  styleId?: number,
  targetWordCount?: number,
  onProgressUpdate?: (message: string, progress: number) => void,
  model?: string,
  narrativePerspective?: string,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  plotStage?: PlotStage,
  storyCreationBrief?: string,
  qualityPreset?: QualityPreset,
  qualityNotes?: string,
  storyRepairSummary?: string,
  storyRepairTargets?: string[],
  storyPreserveStrengths?: string[],
) => Promise<GenerateChapterContentStreamResult>;

export interface SingleStoryPresetStateLike {
  singleStoryCreationControlCard?: {
    promptBrief?: string | null;
  };
  singleStoryRepairPayload?: {
    storyRepairSummary?: string;
    storyRepairTargets?: string[];
    storyPreserveStrengths?: string[];
  };
}

export interface PreparedSingleChapterGenerationRequest {
  styleId?: number;
  targetWordCount?: number;
  model?: string;
  narrativePerspective?: string;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  storyCreationBrief?: string;
  qualityPreset?: QualityPreset;
  qualityNotes?: string;
  storyRepairSummary?: string;
  storyRepairTargets?: string[];
  storyPreserveStrengths?: string[];
}

export async function prepareSingleChapterGenerationRequest({
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
}: {
  loadSingleStoryPresetState: () => Promise<SingleStoryPresetStateLike>;
  resolveStoryCreationPromptState: (options: {
    scope: 'single';
    briefDraft?: string | null;
    defaultBrief?: string | null;
    beatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
    sceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  }) => { prompt?: string };
  singleStoryCreationBriefDraft: string;
  projectDefaultStoryCreationBrief: string;
  singleStoryBeatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
  singleStorySceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  selectedStyleId?: number;
  targetWordCount?: number;
  selectedModel?: string;
  temporaryNarrativePerspective?: string;
  selectedCreativeMode?: CreativeMode;
  selectedStoryFocus?: StoryFocus;
  selectedPlotStage?: PlotStage;
  selectedQualityPreset?: QualityPreset;
  selectedQualityNotes: string;
}): Promise<PreparedSingleChapterGenerationRequest> {
  const latestSingleStoryPresetState = await loadSingleStoryPresetState();
  const latestSingleSystemStoryCreationBrief = latestSingleStoryPresetState.singleStoryCreationControlCard?.promptBrief ?? '';
  const { prompt: latestResolvedSingleStoryCreationBrief } = resolveStoryCreationPromptState({
    scope: 'single',
    briefDraft: singleStoryCreationBriefDraft,
    defaultBrief: latestSingleSystemStoryCreationBrief || projectDefaultStoryCreationBrief,
    beatPlannerDraft: singleStoryBeatPlannerDraft,
    sceneOutlineDraft: singleStorySceneOutlineDraft,
  });

  return {
    styleId: selectedStyleId,
    targetWordCount,
    model: selectedModel,
    narrativePerspective: temporaryNarrativePerspective,
    creativeMode: selectedCreativeMode,
    storyFocus: selectedStoryFocus,
    plotStage: selectedPlotStage,
    storyCreationBrief: latestResolvedSingleStoryCreationBrief,
    qualityPreset: selectedQualityPreset,
    qualityNotes: selectedQualityNotes.trim() || undefined,
    storyRepairSummary: latestSingleStoryPresetState.singleStoryRepairPayload?.storyRepairSummary,
    storyRepairTargets: latestSingleStoryPresetState.singleStoryRepairPayload?.storyRepairTargets,
    storyPreserveStrengths: latestSingleStoryPresetState.singleStoryRepairPayload?.storyPreserveStrengths,
  };
}

export function trackSingleChapterGenerationResult({
  chapterId,
  progressMessageKey,
  result,
  currentProjectId,
  isPageActiveRef,
  editorForm,
  isEditorOpenRef,
  editingChapterIdRef,
  updateAnalysisTasksMap,
  startPollingTask,
  setRunningSingleChapterTasks,
  setChapterQualityRefreshToken,
}: {
  chapterId: string;
  progressMessageKey: string;
  result: GenerateChapterContentStreamResult;
  currentProjectId?: string;
  isPageActiveRef: MutableRefObject<boolean>;
  editorForm: FormInstance<{ content?: string }>;
  isEditorOpenRef: MutableRefObject<boolean>;
  editingChapterIdRef: MutableRefObject<string | null>;
  updateAnalysisTasksMap: (
    updater: Record<string, AnalysisTask> | ((prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)
  ) => void;
  startPollingTask: (chapterId: string) => void;
  setRunningSingleChapterTasks: Dispatch<SetStateAction<Record<string, string>>>;
  setChapterQualityRefreshToken: Dispatch<SetStateAction<number>>;
}) {
  if (result.generation_task_id) {
    setRunningSingleChapterTasks((prev) => ({
      ...prev,
      [chapterId]: result.generation_task_id,
    }));
  }

  message.open({
    key: progressMessageKey,
    type: 'loading',
    content: 'Generation task is still running in the background...',
    duration: 0,
  });

  result.completion
    .then(async (finalResult) => {
      if (!isPageActiveRef.current) {
        return;
      }

      if (isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {
        const hasContentTouched = editorForm.isFieldsTouched(['content']);

        if (!hasContentTouched && finalResult?.content) {
          editorForm.setFieldsValue({ content: finalResult.content });
        } else if (hasContentTouched) {
          message.info('Generated content is ready, but the editor content was changed locally.');
        }
      }

      message.open({
        key: progressMessageKey,
        type: finalResult?.content_source === 'candidate_draft' ? 'info' : 'success',
        content: finalResult?.content_source === 'candidate_draft'
          ? 'Generation completed. Candidate draft is ready for review.'
          : 'Chapter content updated.',
        duration: finalResult?.content_source === 'candidate_draft' ? 3 : 2,
      });

      if (finalResult?.analysis_task_id) {
        const taskId = finalResult.analysis_task_id;
        const pendingTask: AnalysisTask = {
          has_task: true,
          task_id: taskId,
          chapter_id: chapterId,
          status: 'pending',
          progress: 0,
        };

        updateAnalysisTasksMap((prev) => ({
          ...prev,
          [chapterId]: pendingTask,
        }));

        chapterApi.upsertChapterAnalysisTaskToStore(pendingTask, currentProjectId, 'chapter-analysis-task');
        if (isPageActiveRef.current) {
          startPollingTask(chapterId);
        }
      }

      if (isPageActiveRef.current && isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {
        setChapterQualityRefreshToken((prev) => prev + 1);
      }
    })
    .catch((error) => {
      if (!isPageActiveRef.current) {
        return;
      }

      const completionError = error as ApiError;
      message.open({
        key: progressMessageKey,
        type: 'error',
        content: 'Chapter generation failed: ' + (completionError.response?.data?.detail || completionError.message || 'Unknown error'),
        duration: 4,
      });
    })
    .finally(() => {
      if (!isPageActiveRef.current) {
        return;
      }

      setRunningSingleChapterTasks((prev) => {
        if (!(chapterId in prev)) return prev;
        const next = { ...prev };
        delete next[chapterId];
        return next;
      });
    });
}

export async function startSingleChapterGenerationWorkflow({
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
  currentProjectId,
  isPageActiveRef,
  editorForm,
  isEditorOpenRef,
  editingChapterIdRef,
  updateAnalysisTasksMap,
  startPollingTask,
  setRunningSingleChapterTasks,
  setChapterQualityRefreshToken,
}: {
  editingId: string | null;
  runningSingleChapterTasks: Record<string, string>;
  saveSingleStoryCreationSnapshot: (
    reason: StoryCreationSnapshotReason,
    options?: { silent?: boolean; label?: string },
  ) => Promise<unknown> | void;
  setIsContinuing: Dispatch<SetStateAction<boolean>>;
  setIsGenerating: Dispatch<SetStateAction<boolean>>;
  setSingleChapterProgress: Dispatch<SetStateAction<number>>;
  setSingleChapterProgressMessage: Dispatch<SetStateAction<string>>;
  loadSingleStoryPresetState: () => Promise<SingleStoryPresetStateLike>;
  resolveStoryCreationPromptState: (options: {
    scope: 'single';
    briefDraft?: string | null;
    defaultBrief?: string | null;
    beatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
    sceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  }) => { prompt?: string };
  singleStoryCreationBriefDraft: string;
  projectDefaultStoryCreationBrief: string;
  singleStoryBeatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
  singleStorySceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  selectedStyleId?: number;
  targetWordCount?: number;
  selectedModel?: string;
  temporaryNarrativePerspective?: string;
  selectedCreativeMode?: CreativeMode;
  selectedStoryFocus?: StoryFocus;
  selectedPlotStage?: PlotStage;
  selectedQualityPreset?: QualityPreset;
  selectedQualityNotes: string;
  generateChapterContentStream: GenerateChapterContentStream;
  currentProjectId?: string;
  isPageActiveRef: MutableRefObject<boolean>;
  editorForm: FormInstance<{ content?: string }>;
  isEditorOpenRef: MutableRefObject<boolean>;
  editingChapterIdRef: MutableRefObject<string | null>;
  updateAnalysisTasksMap: (
    updater: Record<string, AnalysisTask> | ((prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)
  ) => void;
  startPollingTask: (chapterId: string) => void;
  setRunningSingleChapterTasks: Dispatch<SetStateAction<Record<string, string>>>;
  setChapterQualityRefreshToken: Dispatch<SetStateAction<number>>;
}): Promise<void> {
  if (!editingId) {
    return;
  }

  const chapterId = editingId;

  if (runningSingleChapterTasks[chapterId]) {
    message.info('A generation task is already running for this chapter.');
    return;
  }

  const progressMessageKey = `chapter-generate-progress-${chapterId}`;

  try {
    void saveSingleStoryCreationSnapshot('generate', { silent: true });
    setIsContinuing(true);
    setIsGenerating(true);
    setSingleChapterProgress(0);
    setSingleChapterProgressMessage('Generating chapter...');

    const generationRequest = await prepareSingleChapterGenerationRequest({
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
    });

    const result = await generateChapterContentStream(
      chapterId,
      undefined,
      generationRequest.styleId,
      generationRequest.targetWordCount,
      (progressMsg, progressValue) => {
        setSingleChapterProgress(progressValue);
        setSingleChapterProgressMessage(progressMsg);
      },
      generationRequest.model,
      generationRequest.narrativePerspective,
      generationRequest.creativeMode,
      generationRequest.storyFocus,
      generationRequest.plotStage,
      generationRequest.storyCreationBrief,
      generationRequest.qualityPreset,
      generationRequest.qualityNotes,
      generationRequest.storyRepairSummary,
      generationRequest.storyRepairTargets,
      generationRequest.storyPreserveStrengths,
    );

    trackSingleChapterGenerationResult({
      chapterId,
      progressMessageKey,
      result,
      currentProjectId,
      isPageActiveRef,
      editorForm,
      isEditorOpenRef,
      editingChapterIdRef,
      updateAnalysisTasksMap,
      startPollingTask,
      setRunningSingleChapterTasks,
      setChapterQualityRefreshToken,
    });

    message.success('Chapter generation completed.');
  } catch (error) {
    const apiError = error as ApiError;
    message.error('Chapter generation failed: ' + (apiError.response?.data?.detail || apiError.message || 'Unknown error'));
  } finally {
    setIsContinuing(false);
    setIsGenerating(false);
  }
}
