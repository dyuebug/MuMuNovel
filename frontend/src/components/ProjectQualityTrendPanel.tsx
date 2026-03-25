import React from 'react';
import { Card } from 'antd';

import type {
  ProjectChapterQualityTrendItem,
  ProjectChapterQualityTrendResponse,
  StoryPacingImbalanceSignal,
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

interface ProjectQualityTrendPanelProps {
  trendData?: ProjectChapterQualityTrendResponse | null;
  loading?: boolean;
  compact?: boolean;
}

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

const formatRecentTrendItem = (item: ProjectChapterQualityTrendItem): string => {
  const latest = item.latest_quality_metrics;
  const segments: string[] = [];
  if (typeof latest?.overall_score === 'number') {
    segments.push(`综合 ${latest.overall_score.toFixed(1)}`);
  }
  if (typeof latest?.payoff_chain_rate === 'number') {
    segments.push(`兑现 ${latest.payoff_chain_rate.toFixed(1)}%`);
  }
  if (typeof latest?.cliffhanger_rate === 'number') {
    segments.push(`牵引 ${latest.cliffhanger_rate.toFixed(1)}%`);
  }
  const suffix = segments.length > 0 ? ` · ${segments.join(' · ')}` : '';
  return `第${item.chapter_number}章 ${item.title}${suffix}`;
};

const formatPacingSignal = (signal: StoryPacingImbalanceSignal): string => {
  const label = signal?.label || signal?.key || '节奏异常';
  const severityLabel = signal?.severity === 'warning' ? '预警' : '关注';
  const metricText = typeof signal?.metric === 'number' ? `（${signal.metric.toFixed(1)}）` : '';
  const summary = signal?.summary?.trim();
  return summary ? `${label}${metricText} · ${severityLabel}：${summary}` : `${label}${metricText} · ${severityLabel}`;
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
    { label: '已分析', value: `${analyzedCount}/${totalCount}` },
    ...(trendLabel
      ? [{ label: '趋势', value: trendLabel, color: summary?.overall_score_trend === 'falling' ? 'red' : summary?.overall_score_trend === 'rising' ? 'green' : 'blue' }]
      : []),
    ...(typeof summary?.overall_score_delta === 'number'
      ? [{ label: '综合分变化', value: `${summary.overall_score_delta >= 0 ? '+' : ''}${summary.overall_score_delta.toFixed(1)}`, color: summary.overall_score_delta >= 0 ? 'green' : 'red' }]
      : []),
    ...(summary?.last_generated_at
      ? [{ label: '最近更新时间', value: new Date(summary.last_generated_at).toLocaleString() }]
      : []),
  ];

  const projectHealthItems = [
    ...(typeof volumeGoal?.completion_rate === 'number'
      ? [{ label: '卷级达成', value: `${volumeGoal.completion_rate.toFixed(1)}%`, color: getMetricRateColor(volumeGoal.completion_rate) }]
      : []),
    ...(volumeGoal?.current_stage_label
      ? [{ label: '当前阶段', value: volumeGoal.current_stage_label, color: getStatusColor(volumeGoal.status) }]
      : []),
    ...(typeof foreshadowDelay?.delay_index === 'number'
      ? [{ label: '兑现延迟', value: `${foreshadowDelay.delay_index.toFixed(1)}`, color: getStatusColor(foreshadowDelay.status) }]
      : []),
    ...(typeof foreshadowDelay?.backlog_count === 'number'
      ? [{ label: '待兑伏笔', value: `${foreshadowDelay.backlog_count}` }]
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

  return (
    <Card
      title="项目质量趋势"
      size={compact ? 'small' : 'default'}
      loading={loading}
      style={{ marginBottom: 16 }}
    >
      {!trendData?.has_metrics || !summary ? (
        renderCompactSettingHint(
          '暂无项目级质量趋势',
          '当项目内累计出现章节质量记录后，这里会汇总最近章节的整体趋势、节奏预警与长篇修复方向。',
          { style: { marginBottom: 0 } },
        )
      ) : (
        <>
          {renderCompactSelectionSummary(overviewItems, { style: { marginBottom: 10 } })}
          {projectHealthItems.length > 0 && renderCompactSelectionSummary(projectHealthItems, { style: { marginBottom: 10 } })}
          {guidance?.summary && renderCompactSettingHint(
            '整体修复判断',
            guidance.summary,
            { tone: overallTone, style: { marginBottom: 10 } },
          )}
          {metricItems.length > 0 && renderCompactMetricGrid(metricItems, { style: { marginBottom: 10 } })}
          {volumeGoal?.summary && renderCompactSettingHint(
            '卷级目标达成',
            volumeGoal.summary,
            { tone: getStatusTone(volumeGoal.status), style: { marginBottom: 10 } },
          )}
          {pacing?.summary && renderCompactSettingHint(
            '长篇节奏预警',
            pacing.summary,
            { tone: getStatusTone(pacing.status), style: { marginBottom: 10 } },
          )}
          {foreshadowDelay?.summary && renderCompactSettingHint(
            '伏笔兑现延迟',
            foreshadowDelay.summary,
            {
              tone: getStatusTone(foreshadowDelay.status),
              style: { marginBottom: repairActions.length > 0 || pacingSignals.length > 0 || recentChapters.length > 0 ? 10 : 0 },
            },
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
