import type {
  ChapterQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
  StoryRepairGuidance,
} from '../types';

export type QualityMetricItem = {
  key: string;
  label: string;
  value: number;
  tip?: string;
  displayValue?: string;
};

export type QualityProfileDisplayItem = {
  key: string;
  label: string;
  description: string;
};

export type QualityRepairGuidanceDisplay = {
  summary: string;
  repairTargets: string[];
  preserveStrengths: string[];
  focusAreas: string[];
  weakestMetricLabel?: string;
  weakestMetricValue?: number | null;
};

const QUALITY_METRIC_TIPS: Record<string, string> = {
  conflict: '衡量冲突升级和代价兑现。',
  rule: '衡量设定规则是否落到动作与结果。',
  opening: '衡量开头是否快速抛出目标、异常或受阻。',
  payoff: '衡量承诺、伏笔和阶段期待是否得到回收。',
  cliffhanger: '衡量章尾是否保留新的未决问题或追读牵引。',
  dialogue: '衡量对白是否自然、有立场差异并能推动剧情。',
  outline: '衡量正文推进是否命中本轮大纲任务、变化与收束。',
  pacing: '来自章节分析的节奏评分，满分 10 分。',
};


const QUALITY_FOCUS_AREA_LABELS: Record<string, string> = {
  conflict: '冲突链推进',
  rule_grounding: '规则落地',
  outline: '大纲贴合',
  dialogue: '对白自然度',
  opening: '开场钩子',
  payoff: '回报兑现',
  cliffhanger: '章尾牵引',
  pacing: '节奏稳定度',
};

const QUALITY_PROFILE_BLOCK_ORDER: Array<keyof Pick<ChapterQualityProfileSummary, 'generation' | 'checker' | 'reviser' | 'mcp_guard' | 'external_assets_block'>> = [
  'generation',
  'checker',
  'reviser',
  'mcp_guard',
  'external_assets_block',
];

const QUALITY_PROFILE_BLOCK_LABELS: Record<typeof QUALITY_PROFILE_BLOCK_ORDER[number], string> = {
  generation: '生成链路',
  checker: '质检链路',
  reviser: '修订链路',
  mcp_guard: 'MCP 守卫',
  external_assets_block: '外部素材限制',
};

export const getWeakestQualityMetric = (
  metrics: ChapterQualityMetrics,
): { label: string; value: number } => {
  const items = [
    { label: '冲突链推进', value: metrics.conflict_chain_hit_rate },
    { label: '规则落地', value: metrics.rule_grounding_hit_rate },
    { label: '开场钩子', value: metrics.opening_hook_rate },
    { label: '回报兑现', value: metrics.payoff_chain_rate },
    { label: '章尾牵引', value: metrics.cliffhanger_rate },
    { label: '对白自然度', value: metrics.dialogue_naturalness_rate },
    { label: '大纲贴合', value: metrics.outline_alignment_rate },
  ];

  return items.reduce((min, item) => (item.value < min.value ? item : min), items[0]);
};

export const getQualityMetricItems = (
  metrics: ChapterQualityMetrics,
): QualityMetricItem[] => [
  { key: 'conflict', label: '冲突链推进', value: metrics.conflict_chain_hit_rate, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: '规则落地', value: metrics.rule_grounding_hit_rate, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: '开场钩子', value: metrics.opening_hook_rate, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: '回报兑现', value: metrics.payoff_chain_rate, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: '章尾牵引', value: metrics.cliffhanger_rate, tip: QUALITY_METRIC_TIPS.cliffhanger },
  { key: 'dialogue', label: '对白自然度', value: metrics.dialogue_naturalness_rate, tip: QUALITY_METRIC_TIPS.dialogue },
  { key: 'outline', label: '大纲贴合', value: metrics.outline_alignment_rate, tip: QUALITY_METRIC_TIPS.outline },
];

export const getBatchSummaryMetricItems = (
  summary?: ChapterQualityMetricsSummary | null,
): QualityMetricItem[] => {
  const items: QualityMetricItem[] = [
    { key: 'conflict', label: '冲突链推进', value: summary?.avg_conflict_chain_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.conflict },
    { key: 'rule', label: '规则落地', value: summary?.avg_rule_grounding_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.rule },
    { key: 'outline', label: '大纲贴合', value: summary?.avg_outline_alignment_rate ?? 0, tip: QUALITY_METRIC_TIPS.outline },
    { key: 'dialogue', label: '对白自然度', value: summary?.avg_dialogue_naturalness_rate ?? 0, tip: QUALITY_METRIC_TIPS.dialogue },
    { key: 'opening', label: '开场钩子', value: summary?.avg_opening_hook_rate ?? 0, tip: QUALITY_METRIC_TIPS.opening },
    { key: 'payoff', label: '回报兑现', value: summary?.avg_payoff_chain_rate ?? 0, tip: QUALITY_METRIC_TIPS.payoff },
    { key: 'cliffhanger', label: '章尾牵引', value: summary?.avg_cliffhanger_rate ?? 0, tip: QUALITY_METRIC_TIPS.cliffhanger },
  ];

  if (typeof summary?.avg_pacing_score === 'number') {
    const pacingValue = Math.max(0, Math.min(100, summary.avg_pacing_score * 10));
    items.push({
      key: 'pacing',
      label: '节奏稳定度',
      value: pacingValue,
      displayValue: `${summary.avg_pacing_score.toFixed(1)}/10`,
      tip: QUALITY_METRIC_TIPS.pacing,
    });
  }

  return items;
};

export const getRepairGuidanceDisplay = (
  guidance?: StoryRepairGuidance | null,
): QualityRepairGuidanceDisplay | null => {
  if (!guidance) {
    return null;
  }

  const summary = guidance.summary?.trim() || '';
  const repairTargets = (guidance.repair_targets || []).map((item) => item.trim()).filter(Boolean);
  const preserveStrengths = (guidance.preserve_strengths || []).map((item) => item.trim()).filter(Boolean);
  const focusAreas = (guidance.focus_areas || []).map((item) => item.trim()).filter(Boolean).map((item) => QUALITY_FOCUS_AREA_LABELS[item] || item);

  if (
    !summary
    && repairTargets.length === 0
    && preserveStrengths.length === 0
    && focusAreas.length === 0
    && !guidance.weakest_metric_label
    && guidance.weakest_metric_value == null
  ) {
    return null;
  }

  return {
    summary,
    repairTargets,
    preserveStrengths,
    focusAreas,
    weakestMetricLabel: guidance.weakest_metric_label || undefined,
    weakestMetricValue: guidance.weakest_metric_value,
  };
};

export const getQualityProfileDisplayItems = (
  summary?: ChapterQualityProfileSummary | null,
): QualityProfileDisplayItem[] => {
  if (!summary) {
    return [];
  }

  const items: QualityProfileDisplayItem[] = [];

  if (summary.baseline_id) {
    items.push({ key: 'baseline', label: '基线', description: summary.baseline_id });
  }

  if (summary.version) {
    items.push({ key: 'version', label: '版本', description: summary.version });
  }

  if (summary.style_profile) {
    items.push({ key: 'style', label: '风格', description: summary.style_profile });
  }

  if (summary.genre_profiles?.length) {
    items.push({ key: 'genres', label: '题材', description: summary.genre_profiles.join(' / ') });
  }

  if (summary.quality_dimensions?.length) {
    items.push({ key: 'dimensions', label: '质量维度', description: summary.quality_dimensions.join(' / ') });
  }

  QUALITY_PROFILE_BLOCK_ORDER.forEach((blockKey) => {
    const block = summary[blockKey];
    const description = block?.summary || block?.title || block?.lines?.[0] || block?.prompt_blocks?.[0];
    if (description) {
      items.push({
        key: blockKey,
        label: QUALITY_PROFILE_BLOCK_LABELS[blockKey],
        description,
      });
    }
  });

  return items;
};
