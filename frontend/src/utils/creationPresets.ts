import type { ChapterQualityMetrics, ChapterQualityMetricsSummary, CreativeMode, PlotStage, StoryFocus } from '../types';


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


type QualityScoreSnapshot = {
  overall_score: number;
  conflict_chain_hit_rate: number;
  rule_grounding_hit_rate: number;
  outline_alignment_rate: number;
  dialogue_naturalness_rate: number;
  opening_hook_rate: number;
  payoff_chain_rate: number;
  cliffhanger_rate: number;
};


type QualityMetricKey = Exclude<keyof QualityScoreSnapshot, 'overall_score'>;


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


const CREATIVE_MODE_LABELS: Record<CreativeMode, string> = {
  balanced: '均衡推进',
  hook: '钩子优先',
  emotion: '情绪沉浸',
  suspense: '悬念加压',
  relationship: '关系推进',
  payoff: '爽点回收',
};

const STORY_FOCUS_LABELS: Record<StoryFocus, string> = {
  advance_plot: '主线推进',
  deepen_character: '人物塑形',
  escalate_conflict: '冲突升级',
  reveal_mystery: '谜团揭示',
  relationship_shift: '关系转折',
  foreshadow_payoff: '伏笔回收',
};

const PLOT_STAGE_LABELS: Record<CreationPlotStage, string> = {
  development: '发展阶段',
  climax: '高潮阶段',
  ending: '结局阶段',
};

const PLOT_STAGE_MISSIONS: Record<CreationPlotStage, string> = {
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

function dedupeItems(items: string[]): string[] {
  return Array.from(new Set(items.map((item) => item.trim()).filter(Boolean)));
}


function resolveCreationPlotStageContext(options: {
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


export function buildVolumePacingPlan(
  chapterCount?: number | null,
  options?: {
    preferredStage?: CreationPlotStage | null;
    currentChapterNumber?: number | null;
  },
): VolumePacingPlan | undefined {
  const total = Math.max(0, Math.floor(Number(chapterCount ?? 0)));
  if (total <= 0) return undefined;

  let developmentCount = 1;
  let climaxCount = 0;
  let endingCount = 0;

  if (total === 1) {
    developmentCount = 1;
  } else if (total === 2) {
    developmentCount = 1;
    endingCount = 1;
  } else if (total === 3) {
    developmentCount = 1;
    climaxCount = 1;
    endingCount = 1;
  } else {
    developmentCount = Math.max(1, Math.round(total * 0.45));
    climaxCount = Math.max(1, Math.round(total * 0.35));
    endingCount = total - developmentCount - climaxCount;

    if (endingCount < 1) {
      endingCount = 1;
      if (developmentCount >= climaxCount && developmentCount > 1) {
        developmentCount -= 1;
      } else if (climaxCount > 1) {
        climaxCount -= 1;
      }
    }
  }

  const segments: VolumePacingSegment[] = [];
  let cursor = 1;
  const pushSegment = (stage: CreationPlotStage, count: number) => {
    if (count <= 0) return;
    const startChapter = cursor;
    const endChapter = cursor + count - 1;
    cursor = endChapter + 1;
    segments.push({
      stage,
      label: PLOT_STAGE_LABELS[stage],
      startChapter,
      endChapter,
      mission: PLOT_STAGE_MISSIONS[stage],
    });
  };

  pushSegment('development', developmentCount);
  pushSegment('climax', climaxCount);
  pushSegment('ending', endingCount);

  const currentChapter = Math.max(0, Math.floor(Number(options?.currentChapterNumber ?? 0)));
  const currentStage = currentChapter > 0
    ? segments.find((segment) => currentChapter >= segment.startChapter && currentChapter <= segment.endChapter)?.stage
    : undefined;

  const preferredStage = options?.preferredStage ?? undefined;
  const summaryParts = [`按 ${total} 章体量，建议整体分为 ${segments.length} 段节奏推进。`];
  if (preferredStage) {
    summaryParts.push(`当前重点阶段是「${PLOT_STAGE_LABELS[preferredStage]}」。`);
  }
  if (currentStage) {
    summaryParts.push(`当前章节位置落在「${PLOT_STAGE_LABELS[currentStage]}」。`);
  }

  return {
    title: '卷级节奏预览',
    summary: summaryParts.join(' '),
    segments,
    currentStage,
  };
}


export function buildStoryObjectiveCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryObjectiveCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let objective = scene === 'outline'
    ? '让本轮章节承担清晰主任务，不平均摊功能。'
    : '让本章推动一个看得见的目标，不写空转段落。';
  let obstacle = scene === 'outline'
    ? '让中段持续抬压，每一章都比上一章更难一点。'
    : '安排一次明确受阻、代价上升或信息错位。';
  let turn = scene === 'outline'
    ? '在后段安排一次会改写后续走向的结构转折。'
    : '在中后段安排一次认知或局面改写。';
  let hook = scene === 'outline'
    ? '尾段留下下一轮章节必须回应的问题或新任务。'
    : '章尾留下追读牵引，不平收。';

  switch (creativeMode) {
    case 'hook':
      hook = '把钩子放在异常、危险或未决选择上，尽量做到前段抓人、尾段牵引。';
      turn = '转折优先用信息缺口扩大、危险临门或局势突然偏转来触发。';
      break;
    case 'emotion':
      objective = scene === 'outline'
        ? '目标除了推进事件，还要逼出人物情绪波动和关系反馈。'
        : '让本章既推进事件，也逼出人物情绪与关系反应。';
      turn = '转折优先落在情绪反噬、误伤、和解受阻或认知偏移上。';
      hook = '钩子留在情绪未落地、关系未说破或选择仍有余震处。';
      break;
    case 'suspense':
      obstacle = '阻力优先来自信息差、误判、证据反噬或真相未全。';
      turn = '转折通过线索翻面、认知刷新、身份异动或危险升级完成。';
      hook = '钩子留在新疑点、半揭开的答案或更近一步的危险上。';
      break;
    case 'relationship':
      objective = scene === 'outline'
        ? '本轮重点推动人物关系位移，让站队和信任结构发生变化。'
        : '让本章推动一次明确的关系位移，而不只是情绪点缀。';
      obstacle = '阻力来自立场差、亏欠、信任裂缝或试探失手。';
      turn = '转折优先用关系破裂、突然靠近、站队变化或误会反转来完成。';
      hook = '钩子留在关系未定、话没说透、立场悬空的地方。';
      break;
    case 'payoff':
      objective = scene === 'outline'
        ? '本轮重点兑现前文铺垫、承诺或能力，并带出更大后果。'
        : '让本章承担一次明确兑现，让读者感到回报落地。';
      turn = '转折优先让兑现带出更大代价、更高目标或新的麻烦。';
      hook = '钩子放在回报之后的新失衡上，而不是只停在爽点本身。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      objective = '核心目标是把局势往前推一格，至少形成新的行动结果。';
      break;
    case 'deepen_character':
      objective = '核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。';
      break;
    case 'escalate_conflict':
      obstacle = '阻力必须逐层变强，让代价和对立面都更具体。';
      break;
    case 'reveal_mystery':
      turn = '转折优先通过线索出现、误导修正和认知刷新来完成。';
      break;
    case 'relationship_shift':
      turn = '转折必须带来关系位移、立场重排或信任结构变化。';
      break;
    case 'foreshadow_payoff':
      objective = '核心目标是兑现前文埋设，并顺手打开新的后续空间。';
      hook = '钩子留在兑现后的新承诺、新麻烦或更大代价上。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      objective = scene === 'outline'
        ? '当前阶段先立局、铺变量和主任务，把后续压力链搭起来。'
        : '当前阶段先把局势和眼前目标推到更难的位置。';
      break;
    case 'climax':
      obstacle = '阻力要逼近正面碰撞，选择代价必须明显抬高。';
      turn = '转折要接近核心碰撞点，不能只是小波动。';
      break;
    case 'ending':
      objective = scene === 'outline'
        ? '当前阶段优先回收主承诺、主悬念和关键关系线。'
        : '当前阶段让本章承担主承诺或关键关系线的回收职责。';
      hook = '钩子更适合留余味、次级悬念或收束后的新失衡，不能抢走主收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲目标卡' : '章节目标卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来组织本轮叙事任务。`
      : '当前会按默认叙事任务来组织本轮创作。',
    objective,
    obstacle,
    turn,
    hook,
  };
}


export function buildStoryResultCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryResultCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let progress = scene === 'outline'
    ? '这一轮结束后，主线应进入一个更具体、更难回头的新局面。'
    : '这一章结束后，局势应明确前移，人物不能还停在原地。';
  let reveal = scene === 'outline'
    ? '至少释放一轮信息、真相碎片或兑现回报，避免纯拖延。'
    : '至少交付一个新认知、新线索或一次有效兑现。';
  let relationship = scene === 'outline'
    ? '关键人物关系、站队或信任结构要出现可见位移。'
    : '至少有一条人物关系线出现可见变化，而不是只说情绪。';
  let fallout = scene === 'outline'
    ? '尾段要把下一轮章节必须回应的压力、问题或任务钉住。'
    : '章尾要留下一个会逼出下章动作的余波，而不是平稳收住。';

  switch (creativeMode) {
    case 'hook':
      progress = scene === 'outline'
        ? '本轮结束后，读者要感到故事被明显拽进下一段更危险的局面。'
        : '本章结束后，局势必须被推到一个不继续看就会难受的节点。';
      fallout = '余波优先落在未决选择、临门危险或刚被挑开的异常上。';
      break;
    case 'emotion':
      reveal = '结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。';
      relationship = '关系结果要落到互动后果上，让人物之后的做法因此改变。';
      break;
    case 'suspense':
      reveal = '至少留下一个更接近真相的新证据，同时制造新的误判空间。';
      fallout = '余波留在新疑点、身份异动或危险升级上，不能只剩空白遮掩。';
      break;
    case 'relationship':
      relationship = '结果里必须出现一次明确的关系位移、立场变化或信任重排。';
      fallout = '余波最好落在关系未定、话未说透或站队未稳上。';
      break;
    case 'payoff':
      reveal = '结果要让读者看到铺垫兑现、回报落地，并感到不是白等。';
      progress = scene === 'outline'
        ? '兑现之后，主线要进入一个新的阶段，而不是只做结算。'
        : '兑现之后，局势要被顺势推向更高目标或更大麻烦。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      progress = '推进结果必须清晰可见：行动产生了后果，局势换了位置。';
      break;
    case 'deepen_character':
      reveal = '结果要让人物的弱点、执念或价值判断真正显形，而非停在说明。';
      relationship = '人物变化要影响他与他人的互动方式或后续选择。';
      break;
    case 'escalate_conflict':
      progress = '推进结果不是前进一步，而是把人推入更高代价的冲突区。';
      fallout = '余波要把冲突继续抬高，让下一轮没有轻松退路。';
      break;
    case 'reveal_mystery':
      reveal = '揭示结果必须真实推进谜团，不只是制造更多模糊表述。';
      break;
    case 'relationship_shift':
      relationship = '关系结果必须足够明确，能改变两人之后的说话方式、站位或合作条件。';
      break;
    case 'foreshadow_payoff':
      reveal = '结果要让前文埋设获得兑现，同时打开新的后续空间。';
      fallout = '余波放在兑现后的新承诺、新代价或更大失衡上。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      progress = scene === 'outline'
        ? '这一轮结束后，故事应完成立局并把压力链真正搭起来。'
        : '这一章结束后，故事要进入一个更难但更清晰的推进区。';
      fallout = '余波要把后续任务钉住，让读者知道下一章不是重复上一章。';
      break;
    case 'climax':
      progress = '推进结果要逼近或触发正面碰撞，不能只是外围晃动。';
      reveal = '揭示结果要掀开关键底牌、核心真相或决定性误判。';
      break;
    case 'ending':
      reveal = '揭示结果优先服务主承诺、主悬念与关键伏笔的回收。';
      relationship = '关系结果要体现收束、定局或带余温的最终位移。';
      fallout = '余波更适合留余味、后效和新失衡，不能抢走主收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲结果卡' : '章节结果卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先要求本轮写完后留下「${comboLabels.join(' / ')}」对应的结果落点。`
      : '当前会按默认结果落点来组织本轮创作产出。',
    progress,
    reveal,
    relationship,
    fallout,
  };
}


export function buildStoryExecutionChecklist(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryExecutionChecklist | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let opening = scene === 'outline'
    ? '前段先用 1-2 章立主任务、人物站位和局势缺口，尽快进入事件。'
    : '开场 30% 内抛出目标、异常或受阻点，不平铺背景。';
  let pressure = scene === 'outline'
    ? '中段持续加压，每一章追加一个新阻力、代价或变量。'
    : '中段用动作、对话和反馈连续加压，避免解释停顿。';
  let pivot = scene === 'outline'
    ? '后段安排一次会改写路线的关键转折、揭示或站队变化。'
    : '中后段安排一次改写认知或局面的关键动作。';
  let closing = scene === 'outline'
    ? '尾段先给阶段性结果，再把下一轮问题抛实。'
    : '收尾先落结果，再留下逼出下章的余波。';

  switch (creativeMode) {
    case 'hook':
      opening = scene === 'outline'
        ? '前段优先让异常、危险或未决任务尽快冒头，不慢热铺垫。'
        : '开场尽快抛出异常、险情或未决选择，让读者立刻进入状态。';
      closing = '收尾把悬而未决的危险、选择或信息缺口钉牢，形成追读牵引。';
      break;
    case 'emotion':
      pressure = '中段用互动、误伤、退让受阻或情绪回弹来持续加压。';
      pivot = '关键转折优先落在情绪爆裂、和解失败或认知刺痛上。';
      closing = '收尾保留情绪余震，让人物无法当场彻底消化。';
      break;
    case 'suspense':
      opening = '开场先扔出异常线索、误判苗头或危险信号，再补背景。';
      pressure = '中段不断扩大信息差、证据变化和错误判断的代价。';
      pivot = '转折优先让线索翻面、身份异动或危险升级来改写局面。';
      closing = '收尾留下更尖锐的新疑点，而不是只把答案藏起来。';
      break;
    case 'relationship':
      opening = '开场先把关系张力、站位差或试探动作摆上台面。';
      pressure = '中段持续通过对话、行动和站队测试来挤压关系。';
      pivot = '转折优先用关系破裂、突然靠近或立场变化来触发。';
      closing = '收尾把关系悬在未定状态，逼出下一轮互动。';
      break;
    case 'payoff':
      opening = '开场尽快回扣前文埋设，提醒读者这轮会有兑现。';
      pressure = '中段不断把兑现条件推近，同时抬高兑现所需代价。';
      pivot = '转折优先让铺垫兑现落地，但必须伴随新后果。';
      closing = '收尾不要停在爽点，要顺手抛出兑现后的新失衡。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      opening = '开场先亮明本轮要推进的事，别让读者等太久才知道这章要干嘛。';
      pressure = '中段每次推进都要带来新结果，避免原地解释和空转。';
      break;
    case 'deepen_character':
      pressure = '中段把压力尽量变成选择题，让人物性格在决策里显形。';
      pivot = '关键转折最好来自人物自己的选择、软肋或价值判断。';
      closing = '收尾保留人物做完选择后的余震，而不是只交代事件结束。';
      break;
    case 'escalate_conflict':
      pressure = '中段每一轮加压都要比上一轮更狠，别重复同级冲突。';
      pivot = '转折要把冲突推向正面碰撞，而不是继续绕圈。';
      closing = '收尾把人物钉在更高代价区，确保下一轮没法轻退。';
      break;
    case 'reveal_mystery':
      opening = '开场尽快抛出线索、异常或疑点，别先讲设定。';
      pressure = '中段通过调查、误导修正和证据变化推进认知。';
      pivot = '转折要真正修正一次认知，而不是只多说一点背景。';
      break;
    case 'relationship_shift':
      pressure = '中段每次互动都要推动信任、亏欠或站队发生偏移。';
      pivot = '转折必须带来明确关系位移，而不是轻微情绪波动。';
      closing = '收尾把关系停在新的不稳定位置，逼出后续回应。';
      break;
    case 'foreshadow_payoff':
      opening = '开场尽快回扣旧埋设，提示这轮不是凭空发生。';
      pivot = '转折优先让伏笔兑现落地，同时带出新的空间。';
      closing = '收尾把兑现后的新承诺、新代价或新任务明确抛出。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      opening = scene === 'outline'
        ? '前段优先立局、铺变量和主任务，让整轮章节知道往哪推。'
        : '开场先把局势、变量和眼前目标摆上桌，不要空转导入。';
      pivot = '转折可以不必最大，但必须让方向感发生变化。';
      closing = '收尾把下一轮任务钉住，让压力链接得上。';
      break;
    case 'climax':
      opening = '开场就贴近核心矛盾，不要再绕外围做长引子。';
      pressure = '中段压缩时间、代价和回旋空间，让碰撞不可拖延。';
      pivot = '转折必须接近决定性碰撞、底牌掀开或局势断裂。';
      closing = '收尾把后果锁死，让人物进入无法回避的下一步。';
      break;
    case 'ending':
      opening = '开场快速回扣未收束的主承诺、主悬念或关键关系线。';
      pressure = '中段优先处理最重要的收束任务，不分散到支线琐事。';
      pivot = '转折优先落在主承诺兑现、主真相揭开或关键关系定局上。';
      closing = '收尾保留余味和后效，但不要再制造喧宾夺主的新主线。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲执行清单' : '章节执行清单',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来拆分本轮执行步骤。`
      : '当前会按默认执行步骤来组织本轮创作。',
    opening,
    pressure,
    pivot,
    closing,
  };
}


export function buildStoryRepetitionRiskCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryRepetitionRiskCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let openingRisk = scene === 'outline'
    ? '不要每轮前段都只做设定铺陈，读者会感觉整轮大纲在原地起步。'
    : '不要反复用回忆、说明或同一种异常开场，容易让章节起手发闷。';
  let pressureRisk = scene === 'outline'
    ? '不要每章都用同一级别阻力灌水，中段会失去递进感。'
    : '不要把受阻写成同一种争吵、误会或嘴上发狠，压力会显得空。';
  let pivotRisk = scene === 'outline'
    ? '不要把每次转折都写成临时加设定或生硬插入新人物。'
    : '不要把转折写成假反转、硬转念或只靠旁白解释。';
  let closingRisk = scene === 'outline'
    ? '不要每轮都只用“下回更精彩”式尾章，下一轮任务必须具体。'
    : '不要每章都用同一种问句、敲门声或电话铃收尾，钩子会疲劳。';

  switch (creativeMode) {
    case 'hook':
      openingRisk = '钩子模式下不要每次都靠突发危险硬拽开场，异常类型需要变化。';
      closingRisk = '不要连续多章都用悬空危险硬切章尾，读者会识别套路。';
      break;
    case 'emotion':
      pressureRisk = '不要反复靠争吵、沉默或内心独白制造情绪，否则张力会钝化。';
      pivotRisk = '不要把情绪转折写成突然想通，缺少事件触发会显得虚。';
      break;
    case 'suspense':
      openingRisk = '悬念模式下不要只会丢疑点不交代有效信息，否则会像故意遮掩。';
      pivotRisk = '不要连续用“其实另有隐情”做反转，真相推进需要层次。';
      closingRisk = '不要只留空白疑问而不给新证据，悬念会变成拖延。';
      break;
    case 'relationship':
      pressureRisk = '不要把关系推进写成重复拉扯却没有立场后果，读者会觉得没变化。';
      pivotRisk = '不要每次都靠误会触发关系变化，站队和选择也要轮换。';
      break;
    case 'payoff':
      openingRisk = '回收模式下不要一上来就罗列旧伏笔目录，读者需要事件化兑现。';
      closingRisk = '不要每次回收完都再塞一个更大的谜团，容易冲淡回报感。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      pressureRisk = '主线推进不要只做位移和赶路，缺少阻力变化会像流水账。';
      break;
    case 'deepen_character':
      openingRisk = '人物塑形不要总从心理描写起手，最好让性格先在动作里显形。';
      pressureRisk = '不要把成长写成同一种自责或回忆，人物弧线会发虚。';
      break;
    case 'escalate_conflict':
      pressureRisk = '冲突升级不要一直放大音量不抬高代价，否则只是吵得更大声。';
      pivotRisk = '不要把冲突转折只写成新敌人登场，最好让旧矛盾也发生质变。';
      break;
    case 'reveal_mystery':
      pivotRisk = '谜团揭示不要总靠旁人解释，证据和事件本身也要承担揭示功能。';
      closingRisk = '不要连续多次只留下谜面不回收谜底，读者会怀疑作者在拖。';
      break;
    case 'relationship_shift':
      pressureRisk = '关系转折不要只换台词腔调，最好同步改变合作方式和站位。';
      break;
    case 'foreshadow_payoff':
      closingRisk = '伏笔回收不要每次都变成新伏笔发射器，需保留真正落地的满足。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      openingRisk = '发展阶段不要长时间停在铺垫准备态，必须尽快把变量推上桌。';
      closingRisk = '发展阶段不要每章都只留一个模糊目标，任务应逐步具体化。';
      break;
    case 'climax':
      pressureRisk = '高潮阶段不要反复假装要碰撞却不断拖开，读者会明显感到泄劲。';
      pivotRisk = '高潮阶段不要只有大声量和快节奏，没有决定性变化就不算高潮。';
      break;
    case 'ending':
      openingRisk = '结局阶段不要又重新搭新盘子，优先收最重要的旧承诺。';
      closingRisk = '结局阶段不要为了续作感强行再开主线，否则会稀释收束力度。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲重复风险卡' : '章节重复风险卡',
    summary: comboLabels.length > 0
      ? `当前组合容易在「${comboLabels.join(' / ')}」上形成固定套路，建议提前避重。`
      : '当前会按默认重复风险维度提醒本轮创作避重。',
    openingRisk,
    pressureRisk,
    pivotRisk,
    closingRisk,
  };
}


export function buildStoryAcceptanceCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryAcceptanceCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let missionCheck = scene === 'outline'
    ? '验收时先看这轮章节是否承担了明确主任务，而不是平均摊功能。'
    : '验收时先看本章是否完成了一个清晰主任务，而不是热闹但空转。';
  let changeCheck = scene === 'outline'
    ? '至少要看到局势、关系或认知层面的阶段性变化，不能只搭台。'
    : '至少要看到局势、关系或认知有一项明确变化，不能原地踏步。';
  let freshnessCheck = scene === 'outline'
    ? '检查本轮关键章法是否和上一轮过度同构，避免整卷节拍重复。'
    : '检查开场、加压、转折、收尾是否又落回同一种旧套路。';
  let closingCheck = scene === 'outline'
    ? '尾段既要交代阶段结果，也要给下一轮留下具体任务。'
    : '章尾既要完成本章收束，也要留下合适的追读牵引或余味。';

  switch (creativeMode) {
    case 'hook':
      missionCheck = '验收时重点看开场和章尾是否真正形成牵引，而不只是制造噪音。';
      closingCheck = '结尾要让读者有继续读的冲动，但不能只有硬切和悬空。';
      break;
    case 'emotion':
      changeCheck = '验收时要看到情绪余震和关系后果，而不是只有一段抒情。';
      freshnessCheck = '检查情绪推进是否又只是争吵、沉默或内心独白轮换。';
      break;
    case 'suspense':
      changeCheck = '验收时至少要有一个有效线索、认知刷新或危险升级真正落地。';
      closingCheck = '结尾要留下更尖锐的问题，但不能完全不给有效信息。';
      break;
    case 'relationship':
      missionCheck = '验收时看人物关系是否真的发生位移，而不是只多说了几句狠话。';
      changeCheck = '关系变化最好能改动人物之后的站位、合作或信任条件。';
      break;
    case 'payoff':
      missionCheck = '验收时要确认前文铺垫是否真正兑现，而不是只口头提到。';
      closingCheck = '兑现之后要有后效和新失衡，不能只停在一次性爽点。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      missionCheck = '验收时先看主线是否实打实前进，而不是忙了很多事却没推局势。';
      break;
    case 'deepen_character':
      changeCheck = '验收时看人物是否在选择里显形，而不是只补充背景说明。';
      freshnessCheck = '检查人物塑形是否又回到同一种回忆、自责或旁白总结。';
      break;
    case 'escalate_conflict':
      changeCheck = '验收时要能看见代价升级、对立加深或冲突进入新层级。';
      closingCheck = '本轮结束后人物应被留在更难的位置，而不是轻松退回安全区。';
      break;
    case 'reveal_mystery':
      missionCheck = '验收时必须确认谜团有真实推进，而不是只多堆了一层雾。';
      break;
    case 'relationship_shift':
      changeCheck = '验收时看关系是否足以改变说话方式、行动选择或站队逻辑。';
      break;
    case 'foreshadow_payoff':
      missionCheck = '验收时确认伏笔是否兑现落地，同时打开了新的后续空间。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      missionCheck = '发展阶段验收重点是：有没有把局势、变量和主任务真正搭起来。';
      closingCheck = '收尾应让下一轮任务更具体，而不是继续停留在准备态。';
      break;
    case 'climax':
      changeCheck = '高潮阶段验收重点是：有没有形成决定性碰撞、底牌掀开或局势断裂。';
      freshnessCheck = '检查高潮是否只是声量更大，还是确实发生了不可逆变化。';
      break;
    case 'ending':
      missionCheck = '结局阶段验收重点是：主承诺、主悬念和关键关系线是否得到有效回收。';
      closingCheck = '收尾应保留余味，但不能为了留白再次打散已经完成的收束。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲验收卡' : '章节验收卡',
    summary: comboLabels.length > 0
      ? `当前组合建议按「${comboLabels.join(' / ')}」来验收本轮成稿质量。`
      : '当前会按默认验收标准来检查本轮创作是否达标。',
    missionCheck,
    changeCheck,
    freshnessCheck,
    closingCheck,
  };
}


export function buildStoryCharacterArcCard(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): StoryCharacterArcCard | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  let externalLine = scene === 'outline'
    ? '这一轮至少要让核心人物的外在线任务更明确，不只推动事件壳子。'
    : '本章要让人物在外在线上做出能看见后果的动作，而不是被剧情拖着走。';
  let internalLine = scene === 'outline'
    ? '安排一次会暴露人物执念、伤口或价值判断的压力测试。'
    : '本章要逼出一次能暴露人物软肋、执念或底线的反应。';
  let relationshipLine = scene === 'outline'
    ? '让关键关系在信任、站队或依赖上出现可见变化。'
    : '至少让一条关系线发生可见位移，而不只是多说几句情绪台词。';
  let arcLanding = scene === 'outline'
    ? '尾段给出人物阶段性变化，让下一轮成长方向更清晰。'
    : '章尾要留下人物状态的新落点，让后续成长有承接。';

  switch (creativeMode) {
    case 'hook':
      externalLine = '人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。';
      arcLanding = '弧光落点要落在人物被推入新处境上，而不只是事件悬空。';
      break;
    case 'emotion':
      internalLine = '内在线重点看人物如何被情绪反噬、误伤他人或压抑失败。';
      relationshipLine = '关系线最好呈现安慰失败、靠近受阻或误伤后的余震。';
      break;
    case 'suspense':
      externalLine = '人物外在线尽量和追查、判断、求生或拆解异常绑定。';
      internalLine = '通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。';
      break;
    case 'relationship':
      relationshipLine = '关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。';
      arcLanding = '落点应让人物在关系位置上进入一个再也回不到原点的新阶段。';
      break;
    case 'payoff':
      externalLine = '人物外在线要和旧承诺兑现、旧目标回收或能力回报直接挂钩。';
      arcLanding = '落点要让人物因为兑现获得成长回报，或承担兑现带来的新责任。';
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      externalLine = '人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。';
      break;
    case 'deepen_character':
      internalLine = '内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。';
      arcLanding = '落点最好形成一次人物自我认知偏移，而不只是事件结束。';
      break;
    case 'escalate_conflict':
      internalLine = '冲突升级时要逼出人物底线，看看他在更高代价下会怎么变。';
      relationshipLine = '更强冲突最好同步改写人物之间的站位与依赖结构。';
      break;
    case 'reveal_mystery':
      externalLine = '人物外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。';
      internalLine = '认知刷新应反照人物偏见、恐惧或执念，而不是只补世界观信息。';
      break;
    case 'relationship_shift':
      relationshipLine = '关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。';
      break;
    case 'foreshadow_payoff':
      arcLanding = '人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。';
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      externalLine = scene === 'outline'
        ? '发展阶段先让人物想要什么、怕什么、要付什么代价变得清楚。'
        : '发展阶段先把人物眼前要争什么、躲什么、赌什么摆清楚。';
      arcLanding = '落点应把人物推入更难但更清晰的成长压力链。';
      break;
    case 'climax':
      internalLine = '高潮阶段要逼出人物真正底线、真实选择或最不愿面对的自我。';
      relationshipLine = '高潮中的关系变化最好是定向性变化，而不是小幅试探。';
      break;
    case 'ending':
      relationshipLine = '结局阶段要让关键关系线出现收束、定局或带余温的最终位移。';
      arcLanding = '落点要给人物阶段性定局、余味或代价后的新平衡。';
      break;
    default:
      break;
  }

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲角色弧光卡' : '章节角色弧光卡',
    summary: comboLabels.length > 0
      ? `当前组合会优先按「${comboLabels.join(' / ')}」来推动人物弧光。`
      : '当前会按默认人物弧光标准来组织本轮创作。',
    externalLine,
    internalLine,
    relationshipLine,
    arcLanding,
  };
}


const QUALITY_SCORECARD_LABELS: Record<QualityMetricKey, string> = {
  conflict_chain_hit_rate: '冲突链',
  rule_grounding_hit_rate: '规则落地',
  outline_alignment_rate: '大纲贴合',
  dialogue_naturalness_rate: '对白自然度',
  opening_hook_rate: '开场钩子',
  payoff_chain_rate: '爽点链',
  cliffhanger_rate: '章尾钩子',
};


const QUALITY_SCORECARD_ACTIONS: Record<QualityMetricKey, string> = {
  conflict_chain_hit_rate: '补一轮真正升级的阻力链，让人物在更高代价下被迫选择。',
  rule_grounding_hit_rate: '把设定和规则落到动作代价、限制条件和事件结果上，不只停在说明。',
  outline_alignment_rate: '回对大纲、目标卡和结果卡，确保任务、变化与收束至少命中本轮关键项。',
  dialogue_naturalness_rate: '删掉解释型对白，改成带潜台词、立场碰撞和即时反馈的说话方式。',
  opening_hook_rate: '把前 300 字改成“目标 / 异常 / 受阻”三选一的强起手，不要慢热导入。',
  payoff_chain_rate: '回收一个前文承诺、伏笔或阶段性期待，让读者感到回报真的落地。',
  cliffhanger_rate: '章尾补一个未决选择、新失衡或更尖锐的问题，让下一章有明确牵引。',
};


const QUALITY_PRIORITY_BY_MODE: Partial<Record<CreativeMode, QualityMetricKey[]>> = {
  hook: ['opening_hook_rate', 'cliffhanger_rate'],
  emotion: ['dialogue_naturalness_rate', 'outline_alignment_rate'],
  suspense: ['opening_hook_rate', 'cliffhanger_rate', 'conflict_chain_hit_rate'],
  relationship: ['dialogue_naturalness_rate', 'conflict_chain_hit_rate'],
  payoff: ['payoff_chain_rate', 'outline_alignment_rate'],
  balanced: ['outline_alignment_rate', 'conflict_chain_hit_rate'],
};


const QUALITY_PRIORITY_BY_FOCUS: Partial<Record<StoryFocus, QualityMetricKey[]>> = {
  advance_plot: ['conflict_chain_hit_rate', 'outline_alignment_rate'],
  deepen_character: ['dialogue_naturalness_rate', 'outline_alignment_rate'],
  escalate_conflict: ['conflict_chain_hit_rate', 'cliffhanger_rate'],
  reveal_mystery: ['opening_hook_rate', 'outline_alignment_rate'],
  relationship_shift: ['dialogue_naturalness_rate', 'conflict_chain_hit_rate'],
  foreshadow_payoff: ['payoff_chain_rate', 'outline_alignment_rate'],
};


const QUALITY_PRIORITY_BY_STAGE: Partial<Record<CreationPlotStage, QualityMetricKey[]>> = {
  development: ['opening_hook_rate', 'outline_alignment_rate'],
  climax: ['conflict_chain_hit_rate', 'cliffhanger_rate'],
  ending: ['payoff_chain_rate', 'outline_alignment_rate'],
};


function normalizeChapterQualitySnapshot(metrics?: ChapterQualityMetrics | null): QualityScoreSnapshot | undefined {
  if (!metrics) return undefined;

  return {
    overall_score: metrics.overall_score ?? 0,
    conflict_chain_hit_rate: metrics.conflict_chain_hit_rate ?? 0,
    rule_grounding_hit_rate: metrics.rule_grounding_hit_rate ?? 0,
    outline_alignment_rate: metrics.outline_alignment_rate ?? 0,
    dialogue_naturalness_rate: metrics.dialogue_naturalness_rate ?? 0,
    opening_hook_rate: metrics.opening_hook_rate ?? 0,
    payoff_chain_rate: metrics.payoff_chain_rate ?? 0,
    cliffhanger_rate: metrics.cliffhanger_rate ?? 0,
  };
}


function normalizeBatchQualitySnapshot(summary?: ChapterQualityMetricsSummary | null): QualityScoreSnapshot | undefined {
  if (!summary || summary.avg_overall_score === undefined) return undefined;

  return {
    overall_score: summary.avg_overall_score ?? 0,
    conflict_chain_hit_rate: summary.avg_conflict_chain_hit_rate ?? 0,
    rule_grounding_hit_rate: summary.avg_rule_grounding_hit_rate ?? 0,
    outline_alignment_rate: summary.avg_outline_alignment_rate ?? 0,
    dialogue_naturalness_rate: summary.avg_dialogue_naturalness_rate ?? 0,
    opening_hook_rate: summary.avg_opening_hook_rate ?? 0,
    payoff_chain_rate: summary.avg_payoff_chain_rate ?? 0,
    cliffhanger_rate: summary.avg_cliffhanger_rate ?? 0,
  };
}


function buildAfterScorecardFromSnapshot(
  snapshot: QualityScoreSnapshot | undefined,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    scope?: 'chapter' | 'batch';
  },
): StoryAfterScorecard | undefined {
  if (!snapshot) return undefined;

  const scope = options?.scope ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;
  const metricKeys = Object.keys(QUALITY_SCORECARD_LABELS) as QualityMetricKey[];
  const metricItems = metricKeys.map((key) => ({
    key,
    label: QUALITY_SCORECARD_LABELS[key],
    value: snapshot[key],
  }));

  const strongestItems = [...metricItems].sort((a, b) => b.value - a.value).slice(0, 2);
  const weakestItem = [...metricItems].sort((a, b) => a.value - b.value)[0];

  const priorityKeys = dedupeItems([
    ...(creativeMode ? (QUALITY_PRIORITY_BY_MODE[creativeMode] ?? []) : []),
    ...(storyFocus ? (QUALITY_PRIORITY_BY_FOCUS[storyFocus] ?? []) : []),
    ...(plotStage ? (QUALITY_PRIORITY_BY_STAGE[plotStage] ?? []) : []),
  ]) as QualityMetricKey[];

  const focusItem = priorityKeys.length > 0
    ? metricItems.filter((item) => priorityKeys.includes(item.key)).sort((a, b) => a.value - b.value)[0] ?? weakestItem
    : weakestItem;

  let verdict = '结构稳定';
  let verdictColor = 'success';
  if (snapshot.overall_score < 55) {
    verdict = '建议重做一轮';
    verdictColor = 'error';
  } else if (snapshot.overall_score < 70) {
    verdict = '建议重点修一轮';
    verdictColor = 'warning';
  } else if (snapshot.overall_score < 85) {
    verdict = '可优化后使用';
    verdictColor = 'processing';
  }

  const focusCheck = priorityKeys.length > 0
    ? `${focusItem.label} 是当前组合的关键项，当前 ${focusItem.value}%。`
    : `当前最短板是 ${focusItem.label}，命中率 ${focusItem.value}%。`;

  const strengths = strongestItems.map((item) => `${item.label} ${item.value}%`);
  const gapKeys = Array.from(new Set([focusItem.key, weakestItem.key])).slice(0, 2) as QualityMetricKey[];
  const gaps = gapKeys.map((key) => `${QUALITY_SCORECARD_LABELS[key]}偏弱：${snapshot[key]}%`);
  const nextAction = QUALITY_SCORECARD_ACTIONS[gapKeys[0] ?? weakestItem.key];

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];
  const scopeLabel = scope === 'batch' ? '批量成稿' : '当前章节';

  return {
    title: scope === 'batch' ? '批量后验评分卡' : '章节后验评分卡',
    summary: `${scopeLabel}综合 ${snapshot.overall_score} 分${comboLabels.length > 0 ? `，当前按「${comboLabels.join(' / ')}」验收` : ''}，最需要优先修的是 ${focusItem.label}。`,
    verdict,
    verdictColor,
    focusCheck,
    strengths,
    gaps,
    nextAction,
  };
}


export function buildStoryAfterScorecard(
  metrics?: ChapterQualityMetrics | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
  },
): StoryAfterScorecard | undefined {
  return buildAfterScorecardFromSnapshot(
    normalizeChapterQualitySnapshot(metrics),
    creativeMode,
    storyFocus,
    { plotStage: options?.plotStage, scope: 'chapter' },
  );
}


export function buildBatchStoryAfterScorecard(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
  },
): StoryAfterScorecard | undefined {
  return buildAfterScorecardFromSnapshot(
    normalizeBatchQualitySnapshot(summary),
    creativeMode,
    storyFocus,
    { plotStage: options?.plotStage, scope: 'batch' },
  );
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


export function buildCreationPresetRecommendation(
  metrics?: ChapterQualityMetrics | null,
): CreationPresetRecommendation[] {
  if (!metrics) return [];

  const recommendations: CreationPresetRecommendation[] = [];
  const seen = new Set<CreationPresetId>();

  const push = (id: CreationPresetId, reason: string) => {
    if (seen.has(id)) return;
    seen.add(id);
    recommendations.push({ id, reason });
  };

  if ((metrics.opening_hook_rate ?? 100) < 60 || (metrics.cliffhanger_rate ?? 100) < 60) {
    push('hook_drive', '最近章节的开场钩子或章尾追读牵引偏弱。');
  }

  if ((metrics.conflict_chain_hit_rate ?? 100) < 60) {
    push('conflict_pressure', '最近章节的冲突链偏弱，适合继续抬压。');
  }

  if ((metrics.payoff_chain_rate ?? 100) < 60) {
    push('payoff_harvest', '最近章节的爽点闭环或伏笔兑现偏弱。');
  }

  if ((metrics.outline_alignment_rate ?? 100) < 60 || (metrics.rule_grounding_hit_rate ?? 100) < 60) {
    push('steady_progress', '最近章节的主线落地或规则作用感偏弱。');
  }

  if ((metrics.dialogue_naturalness_rate ?? 100) < 60) {
    push('relationship_shift', '最近章节的对白与互动张力偏弱。');
  }

  if ((metrics.overall_score ?? 100) < 55 && recommendations.length === 0) {
    push('steady_progress', '综合评分偏低，建议先回到更稳的主线推进。');
  }

  return recommendations.slice(0, 3);
}


function toChapterQualityMetrics(snapshot?: QualityScoreSnapshot): ChapterQualityMetrics | undefined {
  if (!snapshot) return undefined;

  return {
    overall_score: snapshot.overall_score,
    conflict_chain_hit_rate: snapshot.conflict_chain_hit_rate,
    rule_grounding_hit_rate: snapshot.rule_grounding_hit_rate,
    outline_alignment_rate: snapshot.outline_alignment_rate,
    dialogue_naturalness_rate: snapshot.dialogue_naturalness_rate,
    opening_hook_rate: snapshot.opening_hook_rate,
    payoff_chain_rate: snapshot.payoff_chain_rate,
    cliffhanger_rate: snapshot.cliffhanger_rate,
  };
}


function buildScoreDrivenRecommendationFromSnapshot(
  snapshot: QualityScoreSnapshot | undefined,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
    scope?: 'chapter' | 'batch';
  },
): ScoreDrivenRecommendationCard | undefined {
  const metrics = toChapterQualityMetrics(snapshot);
  if (!metrics) return undefined;

  const recommendations = buildCreationPresetRecommendation(metrics);
  const currentPreset = getCreationPresetById(options?.activePresetId ?? null)
    ?? getCreationPresetByModes(creativeMode, storyFocus);
  const primaryRecommendation = recommendations.find((item) => item.id !== currentPreset?.id)
    ?? recommendations[0];
  const recommendedPreset = getCreationPresetById(primaryRecommendation?.id ?? null);
  const stageContextPreset = recommendedPreset ?? currentPreset;
  const stageContext = resolveCreationPlotStageContext({
    chapterNumber: options?.chapterNumber,
    totalChapters: options?.totalChapters,
    presetId: stageContextPreset?.id ?? options?.activePresetId ?? null,
    storyFocus: stageContextPreset?.storyFocus ?? storyFocus,
    metrics,
  });

  const recommendedStageLabel = PLOT_STAGE_LABELS[stageContext.stage];
  const alternatives = recommendations
    .filter((item) => item.id !== primaryRecommendation?.id)
    .map((item) => {
      const preset = getCreationPresetById(item.id);
      if (!preset) return undefined;
      return {
        id: item.id,
        label: preset.label,
        reason: item.reason,
      };
    })
    .filter((item): item is NonNullable<typeof item> => Boolean(item));

  const scopeLabel = options?.scope === 'batch' ? '批量成稿' : '当前章节';
  const recommendedPresetReason = primaryRecommendation?.reason
    ?? (currentPreset ? '当前预设还能用，但需要搭配更合适的推进节拍。' : undefined);

  let summary = `${scopeLabel}更适合切到${recommendedStageLabel}推进。`;
  if (recommendedPreset) {
    summary = `${scopeLabel}建议改用「${recommendedPreset.label}」，并切到${recommendedStageLabel}推进。`;
  } else if (currentPreset) {
    summary = `${scopeLabel}建议保留「${currentPreset.label}」，但切到${recommendedStageLabel}推进。`;
  }
  if (recommendedPresetReason) {
    summary += ` 原因：${recommendedPresetReason}`;
  }

  const presetChanged = Boolean(recommendedPreset && recommendedPreset.id !== currentPreset?.id);
  const stageChanged = options?.plotStage ? options.plotStage !== stageContext.stage : true;

  let applyHint = `先把${scopeLabel}切到${recommendedStageLabel}，再按对应节拍重写本轮内容。`;
  if (presetChanged && stageChanged && recommendedPreset) {
    applyHint = `先切到「${recommendedPreset.label}」 + ${recommendedStageLabel}，再按这组节拍重写本轮内容。`;
  } else if (presetChanged && recommendedPreset) {
    applyHint = `先切到「${recommendedPreset.label}」，再按对应节拍重写本轮内容。`;
  } else if (!presetChanged && stageChanged) {
    applyHint = currentPreset
      ? `保留当前预设，但把推进节拍切到${recommendedStageLabel}再重写。`
      : `先把推进节拍切到${recommendedStageLabel}，再重写本轮内容。`;
  } else if (!presetChanged && !stageChanged) {
    applyHint = currentPreset
      ? '当前预设和阶段都可沿用，重点把薄弱项写实、写满、写出后果。'
      : '当前阶段判断可沿用，重点补齐薄弱项，不要只换表面说法。';
  }

  return {
    title: options?.scope === 'batch' ? '批量推荐动作' : '章节推荐动作',
    summary,
    recommendedPresetId: recommendedPreset?.id,
    recommendedPresetLabel: recommendedPreset?.label,
    recommendedPresetReason,
    recommendedStage: stageContext.stage,
    recommendedStageLabel,
    stageReason: stageContext.reason,
    alternatives,
    applyHint,
  };
}

export function buildScoreDrivenRecommendationCard(
  metrics?: ChapterQualityMetrics | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): ScoreDrivenRecommendationCard | undefined {
  return buildScoreDrivenRecommendationFromSnapshot(
    normalizeChapterQualitySnapshot(metrics),
    creativeMode,
    storyFocus,
    {
      plotStage: options?.plotStage,
      chapterNumber: options?.chapterNumber,
      totalChapters: options?.totalChapters,
      activePresetId: options?.activePresetId,
      scope: 'chapter',
    },
  );
}


export function buildBatchScoreDrivenRecommendationCard(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): ScoreDrivenRecommendationCard | undefined {
  return buildScoreDrivenRecommendationFromSnapshot(
    normalizeBatchQualitySnapshot(summary),
    creativeMode,
    storyFocus,
    {
      plotStage: options?.plotStage,
      chapterNumber: options?.chapterNumber,
      totalChapters: options?.totalChapters,
      activePresetId: options?.activePresetId,
      scope: 'batch',
    },
  );
}


function buildRepairAntiPattern(presetId?: CreationPresetId): string {
  switch (presetId) {
    case 'hook_drive':
      return '不要只在开头堆异常或悬问，却没有后续动作承接。';
    case 'conflict_pressure':
      return '不要把冲突写成反复吵架，没有代价升级和局势变化。';
    case 'payoff_harvest':
      return '不要只口头回收伏笔，却没有兑现结果和情绪回响。';
    case 'relationship_shift':
      return '不要只让人物多说几句情绪台词，关系位置却毫无变化。';
    case 'mystery_reveal':
      return '不要只抛设定答案，缺少线索翻面、误判修正和后续影响。';
    case 'emotion_turn':
      return '不要把情绪转折全写成内心独白，缺少动作和互动反应。';
    case 'steady_progress':
      return '不要只做流水式推进，没有新的阻力、信息和局面变化。';
    default:
      return '不要只换措辞不换事件，真正的问题要落实到动作、选择和后果上。';
  }
}

function buildStoryRepairTargetCardFromSnapshot(
  snapshot: QualityScoreSnapshot | undefined,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
    scope?: 'chapter' | 'batch';
  },
): StoryRepairTargetCard | undefined {
  if (!snapshot) return undefined;

  const afterScorecard = buildAfterScorecardFromSnapshot(snapshot, creativeMode, storyFocus, {
    plotStage: options?.plotStage,
    scope: options?.scope ?? 'chapter',
  });
  const recommendation = buildScoreDrivenRecommendationFromSnapshot(snapshot, creativeMode, storyFocus, options);
  if (!afterScorecard || !recommendation) return undefined;

  const scopeLabel = options?.scope === 'batch' ? '批量成稿' : '当前章节';
  const recommendedPresetId = recommendation.recommendedPresetId;
  const recommendedStageLabel = recommendation.recommendedStageLabel || recommendation.recommendedStage || '当前阶段';
  const repairSummary = `${scopeLabel}下一轮要优先修复「${afterScorecard.nextAction}」，不要只做表面润色。`;
  const repairTargets = dedupeItems([
    `先补强：${afterScorecard.nextAction}`,
    recommendation.recommendedPresetLabel
      ? `改用「${recommendation.recommendedPresetLabel}」节拍重写，重点解决${recommendation.recommendedPresetReason || '当前薄弱项'}`
      : '',
    recommendation.recommendedStageLabel
      ? `按${recommendedStageLabel}推进，重点落实：${recommendation.stageReason}`
      : '',
  ]).slice(0, 3);
  const preserveStrengths = afterScorecard.strengths.length > 0
    ? afterScorecard.strengths.slice(0, 2)
    : ['保留当前已有效的推进节奏、人物语气和已有记忆点。'];

  return {
    title: options?.scope === 'batch' ? '批量修复目标卡' : '章节修复目标卡',
    summary: `${afterScorecard.summary} ${repairSummary}`,
    repairSummary,
    priorityTarget: afterScorecard.nextAction,
    repairTargets,
    preserveStrengths,
    antiPattern: buildRepairAntiPattern(recommendedPresetId),
    applyHint: recommendation.applyHint,
  };
}

export function buildStoryRepairTargetCard(
  metrics?: ChapterQualityMetrics | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): StoryRepairTargetCard | undefined {
  return buildStoryRepairTargetCardFromSnapshot(
    normalizeChapterQualitySnapshot(metrics),
    creativeMode,
    storyFocus,
    {
      plotStage: options?.plotStage,
      chapterNumber: options?.chapterNumber,
      totalChapters: options?.totalChapters,
      activePresetId: options?.activePresetId,
      scope: 'chapter',
    },
  );
}


export function buildBatchStoryRepairTargetCard(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): StoryRepairTargetCard | undefined {
  return buildStoryRepairTargetCardFromSnapshot(
    normalizeBatchQualitySnapshot(summary),
    creativeMode,
    storyFocus,
    {
      plotStage: options?.plotStage,
      chapterNumber: options?.chapterNumber,
      totalChapters: options?.totalChapters,
      activePresetId: options?.activePresetId,
      scope: 'batch',
    },
  );
}

export function buildStoryRepairPromptPayload(
  card?: StoryRepairTargetCard | null,
): StoryRepairPromptPayload | undefined {
  if (!card) return undefined;

  const storyRepairSummary = card.repairSummary.trim();
  const storyRepairTargets = dedupeItems(card.repairTargets).slice(0, 3);
  const storyPreserveStrengths = dedupeItems(card.preserveStrengths).slice(0, 2);

  if (!storyRepairSummary && storyRepairTargets.length === 0 && storyPreserveStrengths.length === 0) {
    return undefined;
  }

  return {
    storyRepairSummary: storyRepairSummary || undefined,
    storyRepairTargets: storyRepairTargets.length > 0 ? storyRepairTargets : undefined,
    storyPreserveStrengths: storyPreserveStrengths.length > 0 ? storyPreserveStrengths : undefined,
  };
}

function buildStoryCreationControlCardBase(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scope?: 'chapter' | 'batch';
    plotStage?: CreationPlotStage | null;
    activePresetId?: CreationPresetId | null;
    repairCard?: StoryRepairTargetCard | null;
  },
): StoryCreationControlCard | undefined {
  const objectiveCard = buildStoryObjectiveCard(creativeMode, storyFocus, {
    scene: 'chapter',
    plotStage: options?.plotStage,
  });
  const resultCard = buildStoryResultCard(creativeMode, storyFocus, {
    scene: 'chapter',
    plotStage: options?.plotStage,
  });
  const executionChecklist = buildStoryExecutionChecklist(creativeMode, storyFocus, {
    scene: 'chapter',
    plotStage: options?.plotStage,
  });
  const repetitionRiskCard = buildStoryRepetitionRiskCard(creativeMode, storyFocus, {
    scene: 'chapter',
    plotStage: options?.plotStage,
  });
  const acceptanceCard = buildStoryAcceptanceCard(creativeMode, storyFocus, {
    scene: 'chapter',
    plotStage: options?.plotStage,
  });
  const repairCard = options?.repairCard ?? undefined;

  if (!objectiveCard && !resultCard && !executionChecklist && !repetitionRiskCard && !acceptanceCard && !repairCard) {
    return undefined;
  }

  const activePreset = getCreationPresetById(options?.activePresetId ?? null)
    ?? getCreationPresetByModes(creativeMode, storyFocus);
  const scopeLabel = options?.scope === 'batch' ? '批量成稿' : '当前章节';
  const strategyLabels = activePreset
    ? [activePreset.label, options?.plotStage ? PLOT_STAGE_LABELS[options.plotStage] : undefined].filter(Boolean) as string[]
    : [
        creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
        storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
        options?.plotStage ? PLOT_STAGE_LABELS[options.plotStage] : undefined,
      ].filter(Boolean) as string[];

  const summary = [
    `${scopeLabel}建议先统一目标、执行路径和验收标准，再开始成稿。`,
    strategyLabels.length > 0 ? `当前主策略：${strategyLabels.join(' / ')}。` : '',
  ].filter(Boolean).join(' ');

  const directive = dedupeItems([
    objectiveCard?.objective ?? '',
    objectiveCard?.turn ? `中后段重点落实：${objectiveCard.turn}` : '',
    repairCard?.priorityTarget ? `优先修复：${repairCard.priorityTarget}` : '',
  ]).join('；') || '先让本轮内容真正推进，再做细节润色。';

  const executionPath = dedupeItems([
    executionChecklist?.opening ?? '',
    executionChecklist?.pressure ?? '',
    executionChecklist?.pivot ?? '',
    executionChecklist?.closing ?? '',
  ]).slice(0, 4);

  const expectedOutcomes = dedupeItems([
    resultCard?.progress ?? '',
    resultCard?.reveal ?? '',
    resultCard?.relationship ?? '',
    resultCard?.fallout ?? '',
    acceptanceCard?.missionCheck ?? '',
  ]).slice(0, 4);

  const guardrails = dedupeItems([
    repairCard?.antiPattern ?? '',
    repetitionRiskCard?.openingRisk ?? '',
    repetitionRiskCard?.pivotRisk ?? '',
    acceptanceCard?.freshnessCheck ?? '',
  ]).slice(0, 4);

  const promptBrief = [
    strategyLabels.length > 0 ? `本轮按「${strategyLabels.join(' / ')}」创作。` : '',
    objectiveCard?.objective ? `目标：${objectiveCard.objective}` : '',
    objectiveCard?.obstacle ? `阻力：${objectiveCard.obstacle}` : '',
    objectiveCard?.turn ? `转折：${objectiveCard.turn}` : '',
    resultCard?.progress ? `结果：${resultCard.progress}` : '',
    repairCard?.priorityTarget ? `修复：${repairCard.priorityTarget}` : '',
    repairCard?.preserveStrengths?.[0] ? `保留：${repairCard.preserveStrengths[0]}` : '',
    guardrails[0] ? `避免：${guardrails[0]}` : '',
    acceptanceCard?.missionCheck ? `验收：${acceptanceCard.missionCheck}` : '',
  ].filter(Boolean).join(' ');

  return {
    title: options?.scope === 'batch' ? '批量创作总控卡' : '章节创作总控卡',
    summary,
    directive,
    executionPath,
    expectedOutcomes,
    guardrails,
    promptBrief,
  };
}

export function buildStoryCreationControlCard(
  metrics?: ChapterQualityMetrics | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): StoryCreationControlCard | undefined {
  return buildStoryCreationControlCardBase(creativeMode, storyFocus, {
    scope: 'chapter',
    plotStage: options?.plotStage,
    activePresetId: options?.activePresetId,
    repairCard: buildStoryRepairTargetCard(metrics, creativeMode, storyFocus, options),
  });
}

export function buildBatchStoryCreationControlCard(
  summary?: ChapterQualityMetricsSummary | null,
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    plotStage?: CreationPlotStage | null;
    chapterNumber?: number | null;
    totalChapters?: number | null;
    activePresetId?: CreationPresetId | null;
  },
): StoryCreationControlCard | undefined {
  return buildStoryCreationControlCardBase(creativeMode, storyFocus, {
    scope: 'batch',
    plotStage: options?.plotStage,
    activePresetId: options?.activePresetId,
    repairCard: buildBatchStoryRepairTargetCard(summary, creativeMode, storyFocus, options),
  });
}


export function buildCreationBlueprint(
  creativeMode?: CreativeMode,
  storyFocus?: StoryFocus,
  options?: {
    scene?: CreationBlueprintScene;
    plotStage?: CreationPlotStage | null;
  },
): CreationBlueprint | undefined {
  const scene = options?.scene ?? 'chapter';
  const plotStage = options?.plotStage ?? undefined;

  if (!creativeMode && !storyFocus && !plotStage) {
    return undefined;
  }

  const priorityBeats: string[] = [];
  const priorityRisks: string[] = [];

  switch (creativeMode) {
    case 'hook':
      priorityBeats.push('开场更早抛出异常、危险或未完成目标，先抓住读者注意力。');
      priorityBeats.push('尾段优先保留信息缺口、危险临门或选择未决，不要平收。');
      priorityRisks.push('不要只堆钩子和异常，却缺少实质推进。');
      break;
    case 'emotion':
      priorityBeats.push('关键转折后要写出人物情绪余震和关系反应，不只交代结果。');
      priorityBeats.push('让动作、停顿和对白共同承载情绪，而不是全靠抒情说明。');
      priorityRisks.push('不要让情绪独自悬空，必须落回选择与后果。');
      break;
    case 'suspense':
      priorityBeats.push('中前段持续制造信息差、误判或证据变化，让压力逐步抬升。');
      priorityBeats.push('每个阶段都给出一点新认知，但不要一次讲透底牌。');
      priorityRisks.push('避免把悬念写成纯遮掩，读者需要看到有效推进。');
      break;
    case 'relationship':
      priorityBeats.push('把关键冲突尽量落在人与人之间的立场差、亏欠感或试探上。');
      priorityBeats.push('安排一次关系位移，让后续行动因为关系变化而改道。');
      priorityRisks.push('不要只有关系情绪，没有行动层面的后续影响。');
      break;
    case 'payoff':
      priorityBeats.push('优先安排前文铺垫兑现、收获反馈或阶段性反转，给读者明确回报。');
      priorityBeats.push('兑现后顺手打开下一轮更大的目标或麻烦，不把气口写死。');
      priorityRisks.push('不要只顾爽点回收，忽略代价与后续空间。');
      break;
    case 'balanced':
      priorityBeats.push('推进、情绪、信息释放和回报要彼此穿插，不让单一节拍统治全文。');
      break;
    default:
      break;
  }

  switch (storyFocus) {
    case 'advance_plot':
      priorityBeats.push('每个关键段都要写出行动结果和局势变化，避免原地解释。');
      priorityRisks.push('避免设定说明和情绪回旋挤压主线推进。');
      break;
    case 'deepen_character':
      priorityBeats.push('至少安排一次能暴露人物弱点、执念或价值判断的选择。');
      priorityRisks.push('不要把人物塑形写成静态介绍，必须落到行为上。');
      break;
    case 'escalate_conflict':
      priorityBeats.push('让阻力、代价和对立面逐段变强，形成持续抬压链条。');
      priorityRisks.push('避免重复同级冲突，读者会觉得原地踏步。');
      break;
    case 'reveal_mystery':
      priorityBeats.push('优先安排线索出现、误导修正和认知刷新，至少推进一点真相。');
      priorityRisks.push('不要把揭示写成解释堆叠，尽量通过事件和证据推进。');
      break;
    case 'relationship_shift':
      priorityBeats.push('对话、动作和站队变化都要服务关系转折，而不只是口头表态。');
      priorityRisks.push('不要让关系变化只停留在情绪层，没有后续选择代价。');
      break;
    case 'foreshadow_payoff':
      priorityBeats.push('回收时既要兑现前文承诺，也要带出新的悬念或任务。');
      priorityRisks.push('避免只用说明句回收伏笔，最好落在事件结果上。');
      break;
    default:
      break;
  }

  switch (plotStage) {
    case 'development':
      priorityBeats.push('当前阶段优先扩张局势、铺开变量，并把选择成本逐章抬高。');
      priorityRisks.push('避免太早交底或提前透支高潮。');
      break;
    case 'climax':
      priorityBeats.push('当前阶段要让核心矛盾正面碰撞，把选择逼到无法拖延的节点。');
      priorityRisks.push('避免高潮只有声量，没有清晰结果与代价。');
      break;
    case 'ending':
      priorityBeats.push('当前阶段要优先收束主承诺、主悬念和关键关系线，再留余味。');
      priorityRisks.push('避免只顾收尾，忘了兑现前文最重要的铺垫。');
      break;
    default:
      break;
  }

  const baseBeats = scene === 'outline'
    ? [
      '前段先放出主目标、局势缺口或新任务，不要直接堆设定。',
      '中段持续抬高阻力、代价或信息差，让章节彼此形成递进关系。',
      '后段安排一次明显转折、揭示或关系位移，改变后续走向。',
      '收尾既给阶段性结果，也留下下一轮想追下去的问题。',
    ]
    : [
      '开场尽快抛出异常、目标或受阻点，不做平铺导入。',
      '中段用连续动作推进局势，并让阻力或代价升级。',
      '后段安排一次局势改写、信息刷新或关系位移。',
      '结尾保留明确追读牵引，不要平收。',
    ];

  const baseRisks = scene === 'outline'
    ? ['不要把整轮大纲写成同一种功能，节拍必须有起伏。']
    : ['不要把节拍写成说明书，关键节点都要有动作和即时结果。'];

  const comboLabels = [
    creativeMode ? CREATIVE_MODE_LABELS[creativeMode] : undefined,
    storyFocus ? STORY_FOCUS_LABELS[storyFocus] : undefined,
    plotStage ? PLOT_STAGE_LABELS[plotStage] : undefined,
  ].filter(Boolean) as string[];

  return {
    title: scene === 'outline' ? '大纲结构蓝图' : '章节结构蓝图',
    summary: comboLabels.length > 0
      ? `当前组合会按「${comboLabels.join(' / ')}」组织节拍。`
      : '当前会按默认节拍组织结构。',
    beats: dedupeItems([...priorityBeats, ...baseBeats]).slice(0, 4),
    risks: dedupeItems([...priorityRisks, ...baseRisks]).slice(0, 2),
  };
}
