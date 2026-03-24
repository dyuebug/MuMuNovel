import type { ChapterQualityMetrics, ChapterQualityMetricsSummary, CreativeMode, PlotStage, StoryFocus } from '../types';
import { type CreationPresetId } from './creationPresetsCore';
import {
  buildStoryAcceptanceCard,
  buildStoryCharacterArcCard,
  buildStoryExecutionChecklist,
  buildStoryObjectiveCard,
  buildStoryRepetitionRiskCard,
  buildStoryResultCard,
} from './creationPresetsStory';
import {
  buildBatchStoryCreationControlCard,
  buildBatchStoryRepairTargetCard,
  buildCreationPresetRecommendation,
  buildScoreDrivenRecommendationCard,
  buildStoryAfterScorecard,
  buildStoryRepairPromptPayload,
} from './creationPresetsQuality';

interface StoryBeatPlannerDraft {
  openingHook: string;
  chapterGoal: string;
  conflictPressure: string;
  turningPoint: string;
  endingHook: string;
}

interface StorySceneOutlineDraft {
  setupScene: string;
  confrontationScene: string;
  reversalScene: string;
  payoffScene: string;
}

function toBatchQualityMetrics(summary?: ChapterQualityMetricsSummary | null): ChapterQualityMetrics | null {
  if (!summary) return null;

  return {
    overall_score: summary.avg_overall_score ?? 0,
    conflict_chain_hit_rate: summary.avg_conflict_chain_hit_rate ?? 0,
    rule_grounding_hit_rate: summary.avg_rule_grounding_hit_rate ?? 0,
    outline_alignment_rate: summary.avg_outline_alignment_rate ?? 0,
    dialogue_naturalness_rate: summary.avg_dialogue_naturalness_rate ?? 0,
    opening_hook_rate: summary.avg_opening_hook_rate ?? 0,
    payoff_chain_rate: summary.avg_payoff_chain_rate ?? 0,
    cliffhanger_rate: summary.avg_cliffhanger_rate ?? 0,
  } as ChapterQualityMetrics;
}

export function buildBatchCreationPresetRecommendation(summary?: ChapterQualityMetricsSummary | null) {
  return buildCreationPresetRecommendation(toBatchQualityMetrics(summary));
}

export function buildBatchScoreDrivenRecommendationCardFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
) {
  return buildScoreDrivenRecommendationCard(toBatchQualityMetrics(summary), creativeMode, storyFocus, options);
}

export function buildBatchStoryAfterScorecardFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
  },
) {
  return buildStoryAfterScorecard(toBatchQualityMetrics(summary), creativeMode, storyFocus, options);
}

export function buildBatchStoryRepairTargetCardFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
) {
  return buildBatchStoryRepairTargetCard(summary, creativeMode, storyFocus, options);
}

export function buildBatchStoryCreationControlCardFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
) {
  return buildBatchStoryCreationControlCard(summary, creativeMode, storyFocus, options);
}

export function buildBatchSystemStoryCreationBriefFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
) {
  return buildBatchStoryCreationControlCardFromSummary(summary, creativeMode, storyFocus, options)?.promptBrief ?? '';
}

export function buildBatchStoryRepairPromptPayloadFromSummary(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: PlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
) {
  return buildStoryRepairPromptPayload(
    buildBatchStoryRepairTargetCardFromSummary(summary, creativeMode, storyFocus, options),
  );
}


type BatchStorySupportOptions = {
  plotStage?: PlotStage | null;
};

type BatchInsightCard = {
  key: string;
  title: string;
  summary: string;
  items: Array<[string, string]>;
};

function normalizeStoryBeatPlannerDraft(
  draft?: Partial<StoryBeatPlannerDraft> | null,
): StoryBeatPlannerDraft {
  return {
    openingHook: draft?.openingHook?.trim() ?? '',
    chapterGoal: draft?.chapterGoal?.trim() ?? '',
    conflictPressure: draft?.conflictPressure?.trim() ?? '',
    turningPoint: draft?.turningPoint?.trim() ?? '',
    endingHook: draft?.endingHook?.trim() ?? '',
  };
}

function buildJoinedInstruction(...parts: Array<string | undefined>): string {
  return parts
    .map((item) => item?.trim())
    .filter((item): item is string => Boolean(item))
    .join('; ');
}

function buildBatchStorySupportCards(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: BatchStorySupportOptions,
) {
  const sharedOptions = {
    scene: 'chapter' as const,
    plotStage: options?.plotStage,
  };

  return {
    objective: buildStoryObjectiveCard(creativeMode, storyFocus, sharedOptions),
    result: buildStoryResultCard(creativeMode, storyFocus, sharedOptions),
    executionChecklist: buildStoryExecutionChecklist(creativeMode, storyFocus, sharedOptions),
    repetitionRisk: buildStoryRepetitionRiskCard(creativeMode, storyFocus, sharedOptions),
    acceptance: buildStoryAcceptanceCard(creativeMode, storyFocus, sharedOptions),
    characterArc: buildStoryCharacterArcCard(creativeMode, storyFocus, sharedOptions),
  };
}

export function buildBatchSystemStoryBeatPlanner(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: BatchStorySupportOptions,
): StoryBeatPlannerDraft {
  const { objective, result, executionChecklist } = buildBatchStorySupportCards(creativeMode, storyFocus, options);

  return {
    openingHook: objective?.hook || executionChecklist?.opening || '',
    chapterGoal: objective?.objective || result?.progress || '',
    conflictPressure: objective?.obstacle || executionChecklist?.pressure || '',
    turningPoint: objective?.turn || executionChecklist?.pivot || '',
    endingHook: executionChecklist?.closing || result?.fallout || '',
  };
}

export function buildBatchSuggestedStorySceneOutline(
  beatPlanner?: Partial<StoryBeatPlannerDraft> | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: BatchStorySupportOptions,
): StorySceneOutlineDraft {
  const normalizedBeatPlanner = normalizeStoryBeatPlannerDraft(beatPlanner);
  const { objective, result, acceptance } = buildBatchStorySupportCards(creativeMode, storyFocus, options);

  return {
    setupScene: buildJoinedInstruction(normalizedBeatPlanner.openingHook, normalizedBeatPlanner.chapterGoal),
    confrontationScene: buildJoinedInstruction(normalizedBeatPlanner.conflictPressure, objective?.obstacle),
    reversalScene: buildJoinedInstruction(normalizedBeatPlanner.turningPoint, result?.reveal, result?.relationship),
    payoffScene: buildJoinedInstruction(normalizedBeatPlanner.endingHook, result?.fallout, acceptance?.missionCheck),
  };
}

export function buildBatchStoryInsightCards(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: BatchStorySupportOptions,
): BatchInsightCard[] {
  const { objective, result, executionChecklist, repetitionRisk, acceptance, characterArc } = buildBatchStorySupportCards(
    creativeMode,
    storyFocus,
    options,
  );

  return [
    objective
      ? {
          key: 'batch-objective',
          title: '故事目标',
          summary: objective.summary,
          items: [
            ['目标', objective.objective],
            ['阻碍', objective.obstacle],
            ['转折', objective.turn],
            ['钩子', objective.hook],
          ],
        }
      : null,
    result
      ? {
          key: 'batch-result',
          title: '故事结果',
          summary: result.summary,
          items: [
            ['推进结果', result.progress],
            ['揭示信息', result.reveal],
            ['关系变化', result.relationship],
            ['后续影响', result.fallout],
          ],
        }
      : null,
    executionChecklist
      ? {
          key: 'batch-execution',
          title: '执行清单',
          summary: executionChecklist.summary,
          items: [
            ['开篇', executionChecklist.opening],
            ['压力', executionChecklist.pressure],
            ['转折', executionChecklist.pivot],
            ['收束', executionChecklist.closing],
          ],
        }
      : null,
    repetitionRisk
      ? {
          key: 'batch-repetition',
          title: '重复风险',
          summary: repetitionRisk.summary,
          items: [
            ['开篇风险', repetitionRisk.openingRisk],
            ['压力风险', repetitionRisk.pressureRisk],
            ['转折风险', repetitionRisk.pivotRisk],
            ['收束风险', repetitionRisk.closingRisk],
          ],
        }
      : null,
    acceptance
      ? {
          key: 'batch-acceptance',
          title: '验收清单',
          summary: acceptance.summary,
          items: [
            ['目标达成检查', acceptance.missionCheck],
            ['变化检查', acceptance.changeCheck],
            ['新鲜度检查', acceptance.freshnessCheck],
            ['收束检查', acceptance.closingCheck],
          ],
        }
      : null,
    characterArc
      ? {
          key: 'batch-character-arc',
          title: '人物弧光',
          summary: characterArc.summary,
          items: [
            ['外在线', characterArc.externalLine],
            ['内在线', characterArc.internalLine],
            ['关系线', characterArc.relationshipLine],
            ['弧光落点', characterArc.arcLanding],
          ],
        }
      : null,
  ].filter((item): item is BatchInsightCard => Boolean(item));
}
