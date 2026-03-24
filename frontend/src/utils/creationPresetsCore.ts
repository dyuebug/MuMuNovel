import type { ChapterQualityMetrics, CreativeMode, PlotStage, StoryFocus } from '../types';


export type CreationBlueprintScene = 'chapter' | 'outline';
export type CreationPlotStage = PlotStage;


export interface CreationBlueprint {
  title: string;
  summary: string;
  beats: string[];
  risks: string[];
}


export interface VolumePacingSegment {
  stage: CreationPlotStage;
  label: string;
  startChapter: number;
  endChapter: number;
  mission: string;
}


export interface VolumePacingPlan {
  title: string;
  summary: string;
  segments: VolumePacingSegment[];
  currentStage?: CreationPlotStage;
}


export interface StoryObjectiveCard {
  title: string;
  summary: string;
  objective: string;
  obstacle: string;
  turn: string;
  hook: string;
}


export interface StoryResultCard {
  title: string;
  summary: string;
  progress: string;
  reveal: string;
  relationship: string;
  fallout: string;
}


export interface StoryExecutionChecklist {
  title: string;
  summary: string;
  opening: string;
  pressure: string;
  pivot: string;
  closing: string;
}


export interface StoryRepetitionRiskCard {
  title: string;
  summary: string;
  openingRisk: string;
  pressureRisk: string;
  pivotRisk: string;
  closingRisk: string;
}


export interface StoryAcceptanceCard {
  title: string;
  summary: string;
  missionCheck: string;
  changeCheck: string;
  freshnessCheck: string;
  closingCheck: string;
}


export interface StoryCharacterArcCard {
  title: string;
  summary: string;
  externalLine: string;
  internalLine: string;
  relationshipLine: string;
  arcLanding: string;
}


export interface StoryAfterScorecard {
  title: string;
  summary: string;
  verdict: string;
  verdictColor: string;
  focusCheck: string;
  strengths: string[];
  gaps: string[];
  nextAction: string;
}


export interface ScoreDrivenRecommendationCard {
  title: string;
  summary: string;
  recommendedPresetId?: CreationPresetId;
  recommendedPresetLabel?: string;
  recommendedPresetReason?: string;
  recommendedStage?: CreationPlotStage;
  recommendedStageLabel?: string;
  stageReason: string;
  alternatives: Array<{ id: CreationPresetId; label: string; reason: string }>;
  applyHint: string;
}


export interface StoryRepairTargetCard {
  title: string;
  summary: string;
  repairSummary: string;
  priorityTarget: string;
  repairTargets: string[];
  preserveStrengths: string[];
  antiPattern: string;
  applyHint: string;
}

export interface StoryRepairPromptPayload {
  storyRepairSummary?: string;
  storyRepairTargets?: string[];
  storyPreserveStrengths?: string[];
}

export interface StoryCreationControlCard {
  title: string;
  summary: string;
  directive: string;
  executionPath: string[];
  expectedOutcomes: string[];
  guardrails: string[];
  promptBrief: string;
}



export type CreationPresetId =
  | 'steady_progress'
  | 'hook_drive'
  | 'conflict_pressure'
  | 'emotion_turn'
  | 'mystery_reveal'
  | 'relationship_shift'
  | 'payoff_harvest';


export interface CreationPreset {
  id: CreationPresetId;
  label: string;
  description: string;
  creativeMode: CreativeMode;
  storyFocus: StoryFocus;
}


export interface CreationPresetRecommendation {
  id: CreationPresetId;
  reason: string;
}


export const CREATIVE_MODE_LABELS: Record<CreativeMode, string> = {
  balanced: '均衡推进',
  hook: '钩子优先',
  emotion: '情绪沉浸',
  suspense: '悬念加压',
  relationship: '关系推进',
  payoff: '爽点回收',
};

export const STORY_FOCUS_LABELS: Record<StoryFocus, string> = {
  advance_plot: '主线推进',
  deepen_character: '人物塑形',
  escalate_conflict: '冲突升级',
  reveal_mystery: '谜团揭示',
  relationship_shift: '关系转折',
  foreshadow_payoff: '伏笔回收',
};

export const PLOT_STAGE_LABELS: Record<CreationPlotStage, string> = {
  development: '发展阶段',
  climax: '高潮阶段',
  ending: '结局阶段',
};

export const PLOT_STAGE_MISSIONS: Record<CreationPlotStage, string> = {
  development: '立局、铺变量、建立目标与第一轮压力。',
  climax: '持续抬压、逼近正面碰撞、推动关键反转。',
  ending: '回收承诺、兑现伏笔、收束关系并留下余味。',
};

export const CREATION_PLOT_STAGE_OPTIONS: Array<{
  value: CreationPlotStage;
  label: string;
  description: string;
}> = [
  { value: 'development', label: '发展阶段', description: '适合铺变量、推局势、持续抬高阻力。' },
  { value: 'climax', label: '高潮阶段', description: '适合正面碰撞、逼选、放大代价。' },
  { value: 'ending', label: '结局阶段', description: '适合回收主承诺、收束悬念、形成余味。' },
];

export function dedupeItems(items: string[]): string[] {
  return Array.from(new Set(items.map((item) => item.trim()).filter(Boolean)));
}


export function resolveCreationPlotStageContext(options: {
  chapterNumber?: number | null;
  totalChapters?: number | null;
  presetId?: CreationPresetId | null;
  storyFocus?: StoryFocus;
  metrics?: ChapterQualityMetrics | null;
}): { stage: CreationPlotStage; reason: string } {
  const total = Number(options.totalChapters ?? 0);
  const chapter = Number(options.chapterNumber ?? 0);
  const ratio = total > 0 && chapter > 0 ? chapter / total : 0;

  if (options.presetId === 'payoff_harvest' || options.storyFocus === 'foreshadow_payoff') {
    return {
      stage: 'ending',
      reason: '当前更适合进入结局阶段，优先回收伏笔、主承诺与情绪余波。',
    };
  }

  if (options.presetId === 'conflict_pressure' || options.storyFocus === 'escalate_conflict') {
    return {
      stage: 'climax',
      reason: '当前更适合进入高潮阶段，优先拉高冲突、代价与正面对撞。',
    };
  }

  if ((options.metrics?.payoff_chain_rate ?? 100) < 50) {
    return {
      stage: 'ending',
      reason: '最近爽点闭环偏弱，建议切向结局阶段，优先兑现伏笔与阶段回报。',
    };
  }

  if ((options.metrics?.conflict_chain_hit_rate ?? 100) < 55) {
    return {
      stage: 'climax',
      reason: '最近冲突链命中偏弱，建议切向高潮阶段，优先抬压并逼出选择。',
    };
  }

  if (ratio >= 0.8) {
    return {
      stage: 'ending',
      reason: total > 0 && chapter > 0
        ? `当前已接近尾段 ${chapter}/${total}，更适合进入结局阶段完成主承诺收束。`
        : '当前章节位置偏后，更适合进入结局阶段完成主线收束。',
    };
  }

  if (ratio >= 0.55) {
    return {
      stage: 'climax',
      reason: total > 0 && chapter > 0
        ? `当前已进入中后段 ${chapter}/${total}，更适合进入高潮阶段拉高正面对撞。`
        : '当前章节位置已进入中后段，更适合进入高潮阶段加压推进。',
    };
  }

  return {
    stage: 'development',
    reason: total > 0 && chapter > 0
      ? `当前仍处前中段 ${chapter}/${total}，更适合以发展阶段继续铺局、埋线和抬压。`
      : '当前更适合停留在发展阶段，继续铺局、埋线和抬压。',
  };
}

export function inferCreationPlotStage(options: {
  chapterNumber?: number | null;
  totalChapters?: number | null;
  presetId?: CreationPresetId | null;
  storyFocus?: StoryFocus;
  metrics?: ChapterQualityMetrics | null;
}): CreationPlotStage {
  return resolveCreationPlotStageContext(options).stage;
}

export const CREATION_PRESETS: CreationPreset[] = [
  {
    id: 'steady_progress',
    label: '稳步推进',
    description: '适合铺主线、补逻辑、稳节奏，让剧情持续往前走。',
    creativeMode: 'balanced',
    storyFocus: 'advance_plot',
  },
  {
    id: 'hook_drive',
    label: '开局立钩',
    description: '适合强化开场异常、章尾牵引和连续追读感。',
    creativeMode: 'hook',
    storyFocus: 'advance_plot',
  },
  {
    id: 'conflict_pressure',
    label: '冲突加压',
    description: '适合把阻力、代价和对立面持续抬高。',
    creativeMode: 'suspense',
    storyFocus: 'escalate_conflict',
  },
  {
    id: 'emotion_turn',
    label: '情绪转折',
    description: '适合写人物波动、反应余震和成长代价。',
    creativeMode: 'emotion',
    storyFocus: 'deepen_character',
  },
  {
    id: 'mystery_reveal',
    label: '线索揭晓',
    description: '适合推真相、给线索、修正认知与误导。',
    creativeMode: 'suspense',
    storyFocus: 'reveal_mystery',
  },
  {
    id: 'relationship_shift',
    label: '关系变局',
    description: '适合推动人物关系靠近、破裂、重排与站队变化。',
    creativeMode: 'relationship',
    storyFocus: 'relationship_shift',
  },
  {
    id: 'payoff_harvest',
    label: '回收爆点',
    description: '适合兑现伏笔、打出爽点和形成章内闭环。',
    creativeMode: 'payoff',
    storyFocus: 'foreshadow_payoff',
  },
];


export function getCreationPresetById(id?: CreationPresetId | null): CreationPreset | undefined {
  return CREATION_PRESETS.find((preset) => preset.id === id);
}


export function getCreationPresetByModes(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
): CreationPreset | undefined {
  if (!creativeMode || !storyFocus) return undefined;
  return CREATION_PRESETS.find(
    (preset) => preset.creativeMode === creativeMode && preset.storyFocus === storyFocus,
  );
}
