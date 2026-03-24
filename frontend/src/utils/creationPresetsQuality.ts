import type { ChapterQualityMetrics, ChapterQualityMetricsSummary, CreativeMode, StoryFocus } from '../types';
import {
  type CreationPlotStage,
  type CreationPresetId,
  type CreationPresetRecommendation,
  type ScoreDrivenRecommendationCard,
  type StoryAfterScorecard,
  type StoryCreationControlCard,
  type StoryRepairPromptPayload,
  type StoryRepairTargetCard,
  CREATIVE_MODE_LABELS,
  STORY_FOCUS_LABELS,
  PLOT_STAGE_LABELS,
  dedupeItems,
  getCreationPresetById,
  getCreationPresetByModes,
  resolveCreationPlotStageContext,
} from './creationPresetsCore';
import {
  buildStoryAcceptanceCard,
  buildStoryExecutionChecklist,
  buildStoryObjectiveCard,
  buildStoryRepetitionRiskCard,
  buildStoryResultCard,
} from './creationPresetsStory';

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
