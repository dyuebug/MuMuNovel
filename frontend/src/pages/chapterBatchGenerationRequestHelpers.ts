import type {
  ChapterQualityMetricsSummary,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../types';
import type { CreationPresetId } from '../utils/creationPresetsCore';
import type {
  StoryBeatPlannerDraft,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type {
  BatchProgressState,
  BatchTaskMeta,
} from './chapterBatchGenerationPollingHelpers';

export interface BatchGenerateFormValues {
  startChapterNumber: number;
  count: number;
  enableAnalysis: boolean;
  styleId?: number;
  targetWordCount?: number;
  model?: string;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  enableWebResearch?: boolean;
  webResearchQuery?: string;
}

export interface BatchGenerateRequestBody {
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
  quality_preset?: QualityPreset;
  quality_notes?: string;
  enable_web_research?: boolean;
  web_research_query?: string;
  story_repair_summary?: string;
  story_repair_targets?: string[];
  story_preserve_strengths?: string[];
}

interface StoryCreationPromptStateResult {
  prompt?: string | null;
}

interface ResolveStoryCreationPromptStateOptions {
  scope: 'batch';
  briefDraft?: string | null;
  defaultBrief?: string | null;
  beatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
  sceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
}

interface CreationPresetLike {
  id?: CreationPresetId | null;
}

export async function prepareBatchGenerationTaskRequest({
  values,
  projectId,
  styleId,
  targetWordCount,
  model,
  creativeMode,
  storyFocus,
  plotStage,
  qualityPreset,
  qualityNotes,
  batchQualityMetricsSummary,
  batchStoryCreationBriefDraft,
  batchDefaultStoryCreationBrief,
  batchStoryBeatPlannerDraft,
  batchStorySceneOutlineDraft,
  knownStructureChapterCount,
  resolveStoryCreationPromptState,
  resolveCreationPresetByModes,
}: {
  values: BatchGenerateFormValues;
  projectId: string;
  styleId: number;
  targetWordCount: number;
  model?: string;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  qualityPreset?: QualityPreset;
  qualityNotes?: string;
  batchQualityMetricsSummary?: ChapterQualityMetricsSummary | null;
  batchStoryCreationBriefDraft?: string | null;
  batchDefaultStoryCreationBrief?: string | null;
  batchStoryBeatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
  batchStorySceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
  knownStructureChapterCount?: number | null;
  resolveStoryCreationPromptState: (options: ResolveStoryCreationPromptStateOptions) => StoryCreationPromptStateResult;
  resolveCreationPresetByModes: (
    creativeMode?: CreativeMode,
    storyFocus?: StoryFocus,
  ) => Promise<CreationPresetLike | null | undefined>;
}): Promise<{ requestBody: BatchGenerateRequestBody; taskMeta: BatchTaskMeta }> {
  const requestBody: BatchGenerateRequestBody = {
    start_chapter_number: values.startChapterNumber,
    count: values.count,
    enable_analysis: false,
    style_id: styleId,
    target_word_count: targetWordCount,
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

  if (qualityPreset) {
    requestBody.quality_preset = qualityPreset;
  }

  if (qualityNotes?.trim()) {
    requestBody.quality_notes = qualityNotes.trim();
  }

  if (values.enableWebResearch) {
    requestBody.enable_web_research = true;
  }

  if (values.webResearchQuery?.trim()) {
    requestBody.web_research_query = values.webResearchQuery.trim();
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
    batchQualityMetricsSummary ?? null,
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

  return {
    requestBody,
    taskMeta: {
      startChapterNumber: values.startChapterNumber,
      count: values.count,
      autoAnalyze: values.enableAnalysis,
      projectId,
    },
  };
}

export function createInitialBatchProgressState({
  startChapterNumber,
  total,
  estimatedTimeMinutes,
}: {
  startChapterNumber: number;
  total: number;
  estimatedTimeMinutes?: number;
}): BatchProgressState {
  return {
    status: 'running',
    total,
    completed: 0,
    current_chapter_number: startChapterNumber,
    progress_percent: 0,
    checkpoint: {
      current_chapter_number: startChapterNumber,
      candidate_index: 1,
      candidate_count: 1,
      word_count: 0,
      generation_path: 'single_pass',
      attempt_kind: 'initial_candidate',
      rerank_used: false,
      word_budget_repair_used: false,
      winner_candidate_index: null,
    },
    estimated_time_minutes: estimatedTimeMinutes,
    latest_quality_metrics: undefined,
    quality_metrics_summary: undefined,
    quality_profile_summary: null,
    active_story_repair_payload: undefined,
  };
}
