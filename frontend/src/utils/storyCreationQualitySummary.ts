import type {
  ChapterQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
} from '../types';

export type QualityMetricItem = {
  key: string;
  label: string;
  value: number;
  tip?: string;
};

export type QualityProfileDisplayItem = {
  key: string;
  label: string;
  description: string;
};

const QUALITY_METRIC_TIPS: Record<string, string> = {
  conflict: '????????',
  rule: '???????',
  opening: '???????',
  payoff: '??????',
  cliffhanger: '???????',
  dialogue: '??????',
  outline: '??????',
};

const QUALITY_PROFILE_BLOCK_ORDER: Array<keyof Pick<ChapterQualityProfileSummary, 'generation' | 'checker' | 'reviser' | 'mcp_guard' | 'external_assets_block'>> = [
  'generation',
  'checker',
  'reviser',
  'mcp_guard',
  'external_assets_block',
];

const QUALITY_PROFILE_BLOCK_LABELS: Record<typeof QUALITY_PROFILE_BLOCK_ORDER[number], string> = {
  generation: '??',
  checker: '??',
  reviser: '??',
  mcp_guard: 'MCP ??',
  external_assets_block: '????',
};

export const getWeakestQualityMetric = (
  metrics: ChapterQualityMetrics,
): { label: string; value: number } => {
  const items = [
    { label: '???', value: metrics.conflict_chain_hit_rate },
    { label: '????', value: metrics.rule_grounding_hit_rate },
    { label: '????', value: metrics.opening_hook_rate },
    { label: '???', value: metrics.payoff_chain_rate },
    { label: '????', value: metrics.cliffhanger_rate },
    { label: '?????', value: metrics.dialogue_naturalness_rate },
    { label: '?????', value: metrics.outline_alignment_rate },
  ];

  return items.reduce((min, item) => (item.value < min.value ? item : min), items[0]);
};

export const getQualityMetricItems = (
  metrics: ChapterQualityMetrics,
): QualityMetricItem[] => [
  { key: 'conflict', label: '???', value: metrics.conflict_chain_hit_rate, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: '????', value: metrics.rule_grounding_hit_rate, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: '????', value: metrics.opening_hook_rate, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: '???', value: metrics.payoff_chain_rate, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: '????', value: metrics.cliffhanger_rate, tip: QUALITY_METRIC_TIPS.cliffhanger },
  { key: 'dialogue', label: '?????', value: metrics.dialogue_naturalness_rate, tip: QUALITY_METRIC_TIPS.dialogue },
  { key: 'outline', label: '?????', value: metrics.outline_alignment_rate, tip: QUALITY_METRIC_TIPS.outline },
];

export const getBatchSummaryMetricItems = (
  summary?: ChapterQualityMetricsSummary | null,
): QualityMetricItem[] => [
  { key: 'conflict', label: '???', value: summary?.avg_conflict_chain_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: '????', value: summary?.avg_rule_grounding_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: '????', value: summary?.avg_opening_hook_rate ?? 0, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: '???', value: summary?.avg_payoff_chain_rate ?? 0, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: '????', value: summary?.avg_cliffhanger_rate ?? 0, tip: QUALITY_METRIC_TIPS.cliffhanger },
];

export const getQualityProfileDisplayItems = (
  summary?: ChapterQualityProfileSummary | null,
): QualityProfileDisplayItem[] => {
  if (!summary) {
    return [];
  }

  const items: QualityProfileDisplayItem[] = [];

  if (summary.baseline_id) {
    items.push({ key: 'baseline', label: '??', description: summary.baseline_id });
  }

  if (summary.version) {
    items.push({ key: 'version', label: '??', description: summary.version });
  }

  if (summary.style_profile) {
    items.push({ key: 'style', label: '??', description: summary.style_profile });
  }

  if (summary.genre_profiles?.length) {
    items.push({ key: 'genres', label: '??', description: summary.genre_profiles.join(' / ') });
  }

  if (summary.quality_dimensions?.length) {
    items.push({ key: 'dimensions', label: '??', description: summary.quality_dimensions.join(' / ') });
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
