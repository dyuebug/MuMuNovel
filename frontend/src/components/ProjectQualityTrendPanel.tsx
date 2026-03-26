import React from 'react';
import { Card } from 'antd';

import type {
  ProjectChapterQualityTrendItem,
  ProjectChapterQualityTrendResponse,
  QualityRuntimeContextSummary,
  QualityRuntimeLedgerItem,
  QualityRuntimePlanItem,
  StoryPacingImbalanceSignal,
  StoryQualityRuntimePressure,
} from '../types';
import {
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingHint,
} from './storyCreationCommonUi';
import {
  getMetricRateColor,
  getOverallScoreColor,
  renderCompactMetricGrid,
} from './storyCreationQualityUi';
import {
  getBatchSummaryMetricItems,
  getQualityTrendLabel,
  getRepairGuidanceDisplay,
} from '../utils/storyCreationQualitySummary';

type HintTone = 'success' | 'info' | 'warning';
type TrendStage = 'opening' | 'development' | 'ending';

type TrendSeriesPoint = {
  chapterNumber: number;
  value: number | null;
};

interface ProjectQualityTrendPanelProps {
  trendData?: ProjectChapterQualityTrendResponse | null;
  loading?: boolean;
  compact?: boolean;
}

const INSIGHT_CARD_STYLE: React.CSSProperties = {
  padding: '8px 10px',
  border: '1px solid #f0f0f0',
  borderRadius: 8,
  height: '100%',
};

const TREND_STAGE_META: Record<TrendStage, { label: string; color: string }> = {
  opening: { label: '开篇段', color: '#1677ff' },
  development: { label: '发展段', color: '#722ed1' },
  ending: { label: '收束段', color: '#fa8c16' },
};

const QUALITY_STAGE_ALIASES: Record<string, TrendStage> = {
  opening: 'opening',
  setup: 'opening',
  beginning: 'opening',
  development: 'development',
  middle: 'development',
  escalation: 'development',
  climax: 'ending',
  ending: 'ending',
  finale: 'ending',
};

const getStatusTone = (status?: string | null): HintTone => {
  const normalized = status?.trim().toLowerCase();
  if (normalized === 'warning' || normalized === 'blocked') {
    return 'warning';
  }
  if (normalized === 'watch' || normalized === 'repairable') {
    return 'info';
  }
  return 'success';
};

const getStatusColor = (status?: string | null): string => {
  const normalized = status?.trim().toLowerCase();
  if (normalized === 'warning' || normalized === 'blocked') {
    return 'red';
  }
  if (normalized === 'watch' || normalized === 'repairable') {
    return 'gold';
  }
  return 'green';
};

const dedupeItems = (items: Array<string | null | undefined>, limit = 4): string[] => {
  const normalized: string[] = [];
  const seen = new Set<string>();
  items.forEach((item) => {
    const value = item?.trim();
    if (!value || seen.has(value) || normalized.length >= limit) {
      return;
    }
    seen.add(value);
    normalized.push(value);
  });
  return normalized;
};

const averageNumbers = (values: Array<number | null | undefined>, digits = 1): number | null => {
  const validValues = values.filter((value): value is number => typeof value === 'number');
  if (validValues.length === 0) {
    return null;
  }
  return Number((validValues.reduce((sum, value) => sum + value, 0) / validValues.length).toFixed(digits));
};

const formatNumber = (value?: number | null, digits = 1): string => {
  if (typeof value !== 'number') {
    return '--';
  }
  return value.toFixed(digits);
};

const resolveTrendStage = (
  item: ProjectChapterQualityTrendItem,
  totalCount: number,
): TrendStage => {
  const runtimeContext = item.latest_quality_metrics?.quality_runtime_context as QualityRuntimeContextSummary | null | undefined;
  const explicitStage = runtimeContext?.plot_stage?.trim().toLowerCase();
  if (explicitStage && QUALITY_STAGE_ALIASES[explicitStage]) {
    return QUALITY_STAGE_ALIASES[explicitStage];
  }

  const currentChapterNumber = runtimeContext?.current_chapter_number ?? item.chapter_number;
  const chapterCount = runtimeContext?.chapter_count ?? totalCount;
  if (!chapterCount || chapterCount <= 0) {
    return 'development';
  }
  const progress = currentChapterNumber / chapterCount;
  if (progress <= 0.22) {
    return 'opening';
  }
  if (progress >= 0.78) {
    return 'ending';
  }
  return 'development';
};

const formatRecentTrendItem = (item: ProjectChapterQualityTrendItem): string => {
  const latest = item.latest_quality_metrics;
  const segments: string[] = [];
  if (typeof latest?.overall_score === 'number') {
    segments.push(`总分 ${latest.overall_score.toFixed(1)}`);
  }
  if (typeof latest?.payoff_chain_rate === 'number') {
    segments.push(`回报 ${latest.payoff_chain_rate.toFixed(1)}%`);
  }
  if (typeof latest?.cliffhanger_rate === 'number') {
    segments.push(`章尾 ${latest.cliffhanger_rate.toFixed(1)}%`);
  }
  const suffix = segments.length > 0 ? ` · ${segments.join(' · ')}` : '';
  return `第${item.chapter_number}章 ${item.title}${suffix}`;
};

const formatPacingSignal = (signal: StoryPacingImbalanceSignal): string => {
  const label = signal?.label || signal?.key || '节奏异常';
  const severityLabel = signal?.severity === 'warning' ? '预警' : '关注';
  const metricText = typeof signal?.metric === 'number' ? `（指标 ${signal.metric.toFixed(1)}）` : '';
  const summary = signal?.summary?.trim();
  return summary ? `${label}${metricText} · ${severityLabel}：${summary}` : `${label}${metricText} · ${severityLabel}`;
};

const formatRuntimeLedgerItem = (item?: QualityRuntimeLedgerItem | null): string => {
  if (!item) {
    return "";
  }
  if (typeof item === "string") {
    return item.trim();
  }

  const primary = item.pair?.trim() || item.name?.trim() || item.label?.trim() || "";
  const secondary = item.state?.trim() || item.detail?.trim() || item.status?.trim() || "";
  if (primary && secondary) {
    return `${primary}：${secondary}`;
  }
  return primary || secondary;
};

const formatRuntimePlanItem = (item?: QualityRuntimePlanItem | null): string => {
  if (!item) {
    return "";
  }
  if (typeof item === "string") {
    return item.trim();
  }

  const head = item.name?.trim() || item.label?.trim() || item.summary?.trim() || "";
  const details = [
    item.summary?.trim() && item.summary?.trim() !== head ? item.summary.trim() : "",
    typeof item.target_chapter === "number" ? `目标第${item.target_chapter}章` : "",
    item.status?.trim() || "",
  ].filter((value): value is string => Boolean(value));

  return [head, ...details].filter(Boolean).join(" · ");
};

const buildRuntimeContextHighlights = (
  runtimeContext?: QualityRuntimeContextSummary | null,
  runtimePressure?: StoryQualityRuntimePressure | null,
): string[] => {
  if (!runtimeContext && !runtimePressure) {
    return [];
  }

  const highlights = dedupeItems([
    runtimeContext?.character_focus?.length
      ? `角色焦点：${runtimeContext.character_focus.slice(0, 3).join(" / ")}`
      : null,
    runtimeContext?.foreshadow_payoff_plan?.length
      ? `伏笔计划：${runtimeContext.foreshadow_payoff_plan.map((item) => formatRuntimePlanItem(item)).filter(Boolean).slice(0, 2).join("；")}`
      : null,
    runtimeContext?.foreshadow_state_ledger?.length
      ? `伏笔压力：${runtimeContext.foreshadow_state_ledger.map((item) => formatRuntimeLedgerItem(item)).filter(Boolean).slice(0, 2).join("；")}`
      : runtimePressure?.foreshadow_state_items?.length
        ? `伏笔压力：${runtimePressure.foreshadow_state_items.slice(0, 2).join("；")}`
        : null,
    runtimeContext?.character_state_ledger?.length
      ? `角色状态：${runtimeContext.character_state_ledger.map((item) => formatRuntimeLedgerItem(item)).filter(Boolean).slice(0, 2).join("；")}`
      : runtimePressure?.character_state_items?.length
        ? `角色状态：${runtimePressure.character_state_items.slice(0, 2).join("；")}`
        : null,
    runtimeContext?.relationship_state_ledger?.length
      ? `关系进展：${runtimeContext.relationship_state_ledger.map((item) => formatRuntimeLedgerItem(item)).filter(Boolean).slice(0, 2).join("；")}`
      : null,
    runtimeContext?.organization_state_ledger?.length
      ? `组织局势：${runtimeContext.organization_state_ledger.map((item) => formatRuntimeLedgerItem(item)).filter(Boolean).slice(0, 2).join("；")}`
      : null,
  ], 4);

  return highlights;
};

const buildTrendSeries = (
  items: ProjectChapterQualityTrendItem[],
  selector: (item: ProjectChapterQualityTrendItem) => number | null | undefined,
  maxPoints = 8,
): TrendSeriesPoint[] => items.slice(-maxPoints).map((item) => ({
  chapterNumber: item.chapter_number,
  value: typeof selector(item) === 'number' ? Number(selector(item)) : null,
}));

const renderTrendSparklineCard = (
  title: string,
  color: string,
  points: TrendSeriesPoint[],
  formatter: (value?: number | null) => string,
) => {
  const validPoints = points.filter((point): point is TrendSeriesPoint & { value: number } => typeof point.value === 'number');
  if (validPoints.length === 0) {
    return null;
  }

  const width = 240;
  const height = 72;
  const padding = 10;
  const values = validPoints.map((point) => point.value);
  const minValue = Math.min(...values);
  const maxValue = Math.max(...values);
  const span = Math.max(1, maxValue - minValue);
  const polylinePoints = validPoints.map((point, index) => {
    const x = padding + (index * (width - padding * 2)) / Math.max(validPoints.length - 1, 1);
    const y = height - padding - ((point.value - minValue) / span) * (height - padding * 2);
    return { x, y, value: point.value, chapterNumber: point.chapterNumber };
  });
  const polyline = polylinePoints.map((point) => `${point.x},${point.y}`).join(' ');
  const firstValue = validPoints[0].value;
  const lastValue = validPoints[validPoints.length - 1].value;
  const deltaValue = Number((lastValue - firstValue).toFixed(1));

  return (
    <div style={INSIGHT_CARD_STYLE}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8, marginBottom: 6 }}>
        <div style={{ fontWeight: 600, fontSize: 13 }}>{title}</div>
        <div style={{ color, fontSize: 12, fontWeight: 600 }}>{formatter(lastValue)}</div>
      </div>
      {renderCompactSelectionSummary([
        { label: '样本', value: `${validPoints.length}章`, color: 'blue' },
        { label: '起点', value: formatter(firstValue), color: 'default' },
        {
          label: '变化',
          value: `${deltaValue >= 0 ? '+' : ''}${formatter(deltaValue)}`,
          color: deltaValue >= 0 ? 'green' : 'red',
        },
      ], { style: { marginBottom: 8 } })}
      <svg width="100%" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={title}>
        <polyline
          fill="none"
          stroke={color}
          strokeWidth="3"
          strokeLinejoin="round"
          strokeLinecap="round"
          points={polyline}
        />
        {polylinePoints.map((point) => (
          <circle
            key={`${title}-${point.chapterNumber}`}
            cx={point.x}
            cy={point.y}
            r="3"
            fill={color}
            opacity={point.chapterNumber === validPoints[validPoints.length - 1].chapterNumber ? 1 : 0.75}
          />
        ))}
      </svg>
      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 4 }}>
        覆盖第{validPoints[0].chapterNumber}章 - 第{validPoints[validPoints.length - 1].chapterNumber}章
      </div>
    </div>
  );
};

const ProjectQualityTrendPanel: React.FC<ProjectQualityTrendPanelProps> = ({
  trendData,
  loading = false,
  compact = false,
}) => {
  const summary = trendData?.quality_metrics_summary ?? null;
  const guidance = getRepairGuidanceDisplay(summary?.repair_guidance ?? null);
  const metricItems = getBatchSummaryMetricItems(summary);
  const pacing = summary?.pacing_imbalance ?? null;
  const volumeGoal = summary?.volume_goal_completion ?? null;
  const foreshadowDelay = summary?.foreshadow_payoff_delay ?? null;
  const repairEffectiveness = summary?.repair_effectiveness ?? null;
  const runtimeContext = summary?.quality_runtime_context ?? null;
  const runtimePressure = summary?.quality_gate?.quality_runtime_pressure
    ?? summary?.repair_guidance?.quality_runtime_pressure
    ?? null;
  const runtimeContextHighlights = buildRuntimeContextHighlights(runtimeContext, runtimePressure);
  const trendLabel = getQualityTrendLabel(summary?.overall_score_trend ?? undefined);
  const analyzedCount = trendData?.analyzed_chapters ?? 0;
  const totalCount = trendData?.total_chapters ?? 0;
  const overallTone = summary?.quality_gate?.status
    ? getStatusTone(summary.quality_gate.status)
    : summary?.overall_score_trend === 'falling'
      ? 'warning'
      : summary?.overall_score_trend === 'rising'
        ? 'success'
        : 'info';

  const overviewItems = [
    ...(typeof summary?.avg_overall_score === 'number'
      ? [{ label: '均分', value: `${summary.avg_overall_score.toFixed(1)}`, color: getOverallScoreColor(summary.avg_overall_score) }]
      : []),
    { label: '章节', value: `${analyzedCount}/${totalCount}` },
    ...(trendLabel
      ? [{ label: '趋势', value: trendLabel, color: summary?.overall_score_trend === 'falling' ? 'red' : summary?.overall_score_trend === 'rising' ? 'green' : 'blue' }]
      : []),
    ...(typeof summary?.overall_score_delta === 'number'
      ? [{ label: '变化值', value: `${summary.overall_score_delta >= 0 ? '+' : ''}${summary.overall_score_delta.toFixed(1)}`, color: summary.overall_score_delta >= 0 ? 'green' : 'red' }]
      : []),
    ...(summary?.last_generated_at
      ? [{ label: '最近分析', value: new Date(summary.last_generated_at).toLocaleString() }]
      : []),
  ];

  const projectHealthItems = [
    ...(typeof volumeGoal?.completion_rate === 'number'
      ? [{ label: '卷级目标', value: `${volumeGoal.completion_rate.toFixed(1)}%`, color: getMetricRateColor(volumeGoal.completion_rate) }]
      : []),
    ...(volumeGoal?.current_stage_label
      ? [{ label: '当前阶段', value: volumeGoal.current_stage_label, color: getStatusColor(volumeGoal.status) }]
      : []),
    ...(typeof foreshadowDelay?.delay_index === 'number'
      ? [{ label: '伏笔压力', value: `${foreshadowDelay.delay_index.toFixed(1)}`, color: getStatusColor(foreshadowDelay.status) }]
      : []),
    ...(typeof foreshadowDelay?.backlog_count === 'number'
      ? [{ label: '积压数', value: `${foreshadowDelay.backlog_count}` }]
      : []),
    ...(typeof repairEffectiveness?.success_rate === 'number'
      ? [{ label: '修复成效率', value: `${repairEffectiveness.success_rate.toFixed(1)}%`, color: getStatusColor(repairEffectiveness.status) }]
      : []),
  ];

  const repairActions = dedupeItems([
    ...(guidance?.repairTargets ?? []),
    ...(volumeGoal?.repair_targets ?? []),
    ...(pacing?.repair_targets ?? []),
    ...(foreshadowDelay?.repair_targets ?? []),
  ], 4);
  const pacingSignals = (pacing?.signals ?? []).slice(0, 3).map(formatPacingSignal);
  const recentChapters = (trendData?.items ?? []).slice(-5).reverse().map(formatRecentTrendItem);

  const chartItems = trendData?.items ?? [];
  const overallSeries = buildTrendSeries(chartItems, (item) => item.latest_quality_metrics?.overall_score ?? null);
  const payoffSeries = buildTrendSeries(chartItems, (item) => item.latest_quality_metrics?.payoff_chain_rate ?? null);
  const cliffhangerSeries = buildTrendSeries(chartItems, (item) => item.latest_quality_metrics?.cliffhanger_rate ?? null);

  const stageBucketLines = (['opening', 'development', 'ending'] as TrendStage[])
    .map((stageKey) => {
      const stageItems = chartItems.filter((item) => resolveTrendStage(item, totalCount) === stageKey);
      if (stageItems.length === 0) {
        return null;
      }
      const avgOverall = averageNumbers(stageItems.map((item) => item.latest_quality_metrics?.overall_score));
      const avgPayoff = averageNumbers(stageItems.map((item) => item.latest_quality_metrics?.payoff_chain_rate));
      const avgCliffhanger = averageNumbers(stageItems.map((item) => item.latest_quality_metrics?.cliffhanger_rate));
      const detailParts = [`${stageItems.length}章`];
      if (typeof avgOverall === 'number') {
        detailParts.push(`均分 ${formatNumber(avgOverall)}`);
      }
      if (typeof avgPayoff === 'number') {
        detailParts.push(`回报 ${formatNumber(avgPayoff)}%`);
      }
      if (typeof avgCliffhanger === 'number') {
        detailParts.push(`章尾 ${formatNumber(avgCliffhanger)}%`);
      }
      return `${TREND_STAGE_META[stageKey].label} · ${detailParts.join(' · ')}`;
    })
    .filter((item): item is string => Boolean(item));

  const repairFocusLines = (repairEffectiveness?.focus_area_stats ?? [])
    .slice(0, 3)
    .map((item) => {
      const label = item.label || item.focus_area || '未命名焦点';
      const evaluatedPairs = item.evaluated_pairs ?? 0;
      const successfulPairs = item.successful_pairs ?? 0;
      const successRate = typeof item.success_rate === 'number' ? `${item.success_rate.toFixed(1)}%` : '--';
      const avgDelta = typeof item.avg_delta === 'number'
        ? ` · 均值 ${item.avg_delta >= 0 ? '+' : ''}${item.avg_delta.toFixed(1)}`
        : '';
      return `${label}：${successfulPairs}/${evaluatedPairs} 组成功，成功率 ${successRate}${avgDelta}`;
    });

  const repairRecovered = dedupeItems(repairEffectiveness?.recovered_focus_areas ?? [], 2);
  const repairUnresolved = dedupeItems(repairEffectiveness?.unresolved_focus_areas ?? [], 2);

  const sparklineCards = [
    renderTrendSparklineCard('总体分', '#1677ff', overallSeries, (value) => formatNumber(value)),
    renderTrendSparklineCard('回报兑现', '#52c41a', payoffSeries, (value) => `${formatNumber(value)}%`),
    renderTrendSparklineCard('章尾牵引', '#fa8c16', cliffhangerSeries, (value) => `${formatNumber(value)}%`),
  ].filter((item): item is JSX.Element => Boolean(item));

  return (
    <Card
      title="章节质量趋势"
      size={compact ? 'small' : 'default'}
      loading={loading}
      style={{ marginBottom: 16 }}
    >
      {!trendData?.has_metrics || !summary ? (
        renderCompactSettingHint(
          '暂无质量趋势',
          '完成章节分析后，这里会展示近期质量波动、卷级目标达成、伏笔压力与修复成效率。',
          { style: { marginBottom: 0 } },
        )
      ) : (
        <>
          {renderCompactSelectionSummary(overviewItems, { style: { marginBottom: 10 } })}
          {projectHealthItems.length > 0 && renderCompactSelectionSummary(projectHealthItems, { style: { marginBottom: 10 } })}
          {volumeGoal?.profile_summary && renderCompactSettingHint(
            '体裁 / 风格画像',
            volumeGoal.profile_summary,
            { tone: 'info', style: { marginBottom: 10 } },
          )}
          {guidance?.summary && renderCompactSettingHint(
            '当前修复建议',
            guidance.summary,
            { tone: overallTone, style: { marginBottom: 10 } },
          )}
          {runtimeContextHighlights.length > 0 && renderCompactListCard(
            '运行时账本焦点',
            runtimeContextHighlights,
            { tagText: `${runtimeContextHighlights.length}项`, tagColor: 'blue', style: { marginBottom: 10 } },
          )}
          {metricItems.length > 0 && renderCompactMetricGrid(metricItems, { style: { marginBottom: 10 } })}
          {volumeGoal?.summary && renderCompactSettingHint(
            '卷级目标达成',
            volumeGoal.summary,
            { tone: getStatusTone(volumeGoal.status), style: { marginBottom: 10 } },
          )}
          {pacing?.summary && renderCompactSettingHint(
            '长篇节奏信号',
            pacing.summary,
            { tone: getStatusTone(pacing.status), style: { marginBottom: 10 } },
          )}
          {foreshadowDelay?.summary && renderCompactSettingHint(
            '伏笔兑现压力',
            foreshadowDelay.summary,
            {
              tone: getStatusTone(foreshadowDelay.status),
              style: { marginBottom: 10 },
            },
          )}
          {(sparklineCards.length > 0 || stageBucketLines.length > 0 || repairEffectiveness?.summary) && (
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
                gap: 8,
                marginBottom: 10,
              }}
            >
              {sparklineCards.length > 0 && (
                <div style={{ minWidth: 0, display: 'grid', gap: 8 }}>
                  {sparklineCards}
                </div>
              )}
              {stageBucketLines.length > 0 && (
                <div style={{ minWidth: 0 }}>
                  {renderCompactListCard(
                    '阶段分层观察',
                    stageBucketLines,
                    { tagText: `${stageBucketLines.length}段`, tagColor: 'geekblue', style: { height: '100%' } },
                  )}
                </div>
              )}
              {repairEffectiveness?.summary && (
                <div style={INSIGHT_CARD_STYLE}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>修复成效率</div>
                    <div style={{ color: getStatusColor(repairEffectiveness.status), fontSize: 12, fontWeight: 600 }}>
                      {typeof repairEffectiveness.success_rate === 'number' ? `${repairEffectiveness.success_rate.toFixed(1)}%` : '--'}
                    </div>
                  </div>
                  <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6, marginBottom: 8 }}>
                    {repairEffectiveness.summary}
                  </div>
                  {renderCompactSelectionSummary([
                    ...(typeof repairEffectiveness.success_rate === 'number'
                      ? [{ label: '成功率', value: `${repairEffectiveness.success_rate.toFixed(1)}%`, color: getStatusColor(repairEffectiveness.status) }]
                      : []),
                    ...(typeof repairEffectiveness.evaluated_pairs === 'number'
                      ? [{ label: '样本', value: `${repairEffectiveness.evaluated_pairs}` }]
                      : []),
                    ...(typeof repairEffectiveness.successful_pairs === 'number'
                      ? [{ label: '成功组', value: `${repairEffectiveness.successful_pairs}`, color: 'green' }]
                      : []),
                  ], { style: { marginBottom: 8 } })}
                  {repairUnresolved.length > 0 && (
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6, marginBottom: repairRecovered.length > 0 || repairFocusLines.length > 0 ? 6 : 0 }}>
                      仍未稳定：{repairUnresolved.join(' / ')}
                    </div>
                  )}
                  {repairRecovered.length > 0 && (
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6, marginBottom: repairFocusLines.length > 0 ? 6 : 0 }}>
                      开始回收：{repairRecovered.join(' / ')}
                    </div>
                  )}
                  {repairFocusLines.length > 0 && (
                    <div style={{ display: 'grid', gap: 4 }}>
                      {repairFocusLines.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6 }}>
                          • {item}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
          {(repairActions.length > 0 || pacingSignals.length > 0 || recentChapters.length > 0) && (
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                gap: 8,
              }}
            >
              {repairActions.length > 0 && (
                <div style={{ minWidth: 0 }}>
                  {renderCompactListCard(
                    '下一阶段修复',
                    repairActions,
                    { tagText: `${repairActions.length}项`, tagColor: 'gold', style: { height: '100%' } },
                  )}
                </div>
              )}
              {pacingSignals.length > 0 && (
                <div style={{ minWidth: 0 }}>
                  {renderCompactListCard(
                    '节奏异常',
                    pacingSignals,
                    { tagText: `${pacingSignals.length}项`, tagColor: 'red', style: { height: '100%' } },
                  )}
                </div>
              )}
              {recentChapters.length > 0 && (
                <div style={{ minWidth: 0 }}>
                  {renderCompactListCard(
                    '最近分析章节',
                    recentChapters,
                    { tagText: `${recentChapters.length}章`, tagColor: 'blue', style: { height: '100%' } },
                  )}
                </div>
              )}
            </div>
          )}
        </>
      )}
    </Card>
  );
};

export default ProjectQualityTrendPanel;
