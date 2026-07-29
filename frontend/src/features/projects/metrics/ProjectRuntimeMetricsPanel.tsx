import { useEffect, useRef, useState } from 'react';
import { BarChartOutlined } from '@ant-design/icons';
import { Alert, Card, Space, Spin, Tag, Typography, theme } from 'antd';

import { projectApi } from '../../../services/modularApi';
import { isRequestCancelledError, silentRequestConfig } from '../../../services/core/httpClient';
import type { RuntimeMetricsDataState, RuntimeMetricsResponseV1 } from '../../../types';

const { Text } = Typography;

type RuntimeMetricsPanelProps = {
  projectId: string;
};

const DATA_STATE_PRESENTATION: Record<
  RuntimeMetricsDataState,
  { label: string; color: string }
> = {
  available: { label: '可用', color: 'success' },
  empty: { label: '暂无记录', color: 'default' },
  unavailable: { label: '暂不可用', color: 'warning' },
};

const QUALITY_TREND_LABELS = {
  rising: '上升',
  stable: '稳定',
  falling: '下降',
} as const;

const formatOptionalNumber = (value: number | null, suffix = '') => (
  value === null ? '—' : `${value}${suffix}`
);

const formatScore = (value: number | null) => (
  value === null ? '—' : value.toFixed(1)
);

const formatTimestamp = (value: string | null) => {
  if (!value) {
    return '—';
  }

  const timestamp = new Date(value);
  return Number.isNaN(timestamp.getTime())
    ? '—'
    : timestamp.toLocaleString('zh-CN', { hour12: false });
};

function MetricSection({
  title,
  state,
  children,
}: {
  title: string;
  state: RuntimeMetricsDataState;
  children: React.ReactNode;
}) {
  const presentation = DATA_STATE_PRESENTATION[state];
  const { token } = theme.useToken();

  return (
    <div
      style={{
        flex: '1 1 210px',
        minWidth: 0,
        borderRadius: 10,
        border: `1px solid ${token.colorBorderSecondary}`,
        padding: '10px 12px',
        background: token.colorFillQuaternary,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
        <Text strong style={{ fontSize: 13 }}>{title}</Text>
        <Tag color={presentation.color} style={{ marginInlineEnd: 0 }}>{presentation.label}</Tag>
      </div>
      <div style={{ marginTop: 8, color: token.colorTextSecondary, fontSize: 12, lineHeight: 1.65 }}>
        {children}
      </div>
    </div>
  );
}

export default function ProjectRuntimeMetricsPanel({ projectId }: RuntimeMetricsPanelProps) {
  const [metrics, setMetrics] = useState<RuntimeMetricsResponseV1 | null>(null);
  const [loading, setLoading] = useState(true);
  const [unavailable, setUnavailable] = useState(false);
  const requestSequenceRef = useRef(0);

  useEffect(() => {
    const controller = new AbortController();
    const requestSequence = ++requestSequenceRef.current;

    setMetrics(null);
    setLoading(true);
    setUnavailable(false);

    void projectApi.getRuntimeMetrics(
      projectId,
      silentRequestConfig({ signal: controller.signal }),
    ).then((response) => {
      if (controller.signal.aborted || requestSequence !== requestSequenceRef.current) {
        return;
      }

      setMetrics(response);
    }).catch((error: unknown) => {
      if (controller.signal.aborted || isRequestCancelledError(error)
        || requestSequence !== requestSequenceRef.current) {
        return;
      }

      setUnavailable(true);
    }).finally(() => {
      if (requestSequence === requestSequenceRef.current) {
        setLoading(false);
      }
    });

    return () => {
      controller.abort();
      requestSequenceRef.current += 1;
    };
  }, [projectId]);

  if (loading) {
    return (
      <Card size="small" bordered={false} style={{ borderRadius: 14 }}>
        <Space size={8}>
          <Spin size="small" />
          <Text type="secondary">正在加载派生运行指标…</Text>
        </Space>
      </Card>
    );
  }

  if (unavailable || !metrics) {
    return (
      <Alert
        type="warning"
        showIcon
        message="派生运行指标暂不可用"
        description="这不会改变项目、工作流、任务或审计记录；请在稍后重新进入项目页面查看。"
      />
    );
  }

  const { workflow, tasks, quality, autopilot_audits: autopilotAudits } = metrics;

  return (
    <Card
      size="small"
      title={(
        <Space size={8}>
          <BarChartOutlined />
          <span>运行指标</span>
          <Tag color="blue">派生只读</Tag>
        </Space>
      )}
      extra={<Text type="secondary" style={{ fontSize: 12 }}>不自动刷新</Text>}
      style={{ borderRadius: 14 }}
      styles={{ body: { paddingTop: 12 } }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
        <MetricSection title="工作流快照" state={workflow.state}>
          <div>阶段：{workflow.phase ?? '—'}</div>
          <div>更新时间：{formatTimestamp(workflow.updated_at)}</div>
        </MetricSection>
        <MetricSection title="任务观测" state={tasks.state}>
          <div>样本：{tasks.observed_count} / {tasks.observed_limit}</div>
          <div>待执行 {tasks.pending_count} · 运行 {tasks.running_count} · 完成 {tasks.completed_count}</div>
          <div>失败 {tasks.failed_count} · 已取消 {tasks.cancelled_count}</div>
        </MetricSection>
        <MetricSection title="质量趋势" state={quality.state}>
          <div>分析章节：{formatOptionalNumber(quality.analyzed_chapters)} / {formatOptionalNumber(quality.total_chapters)}</div>
          <div>评分：{formatScore(quality.latest_overall_score)} · 变化：{formatOptionalNumber(quality.overall_score_delta)}</div>
          <div>趋势：{quality.overall_score_trend ? QUALITY_TREND_LABELS[quality.overall_score_trend] : '—'}</div>
          <div>最近生成：{formatTimestamp(quality.last_generated_at)}</div>
        </MetricSection>
        <MetricSection title="受控调用审计" state={autopilotAudits.state}>
          <div>样本：{autopilotAudits.observed_count} / {autopilotAudits.observed_limit}</div>
          <div>排队 {autopilotAudits.queued_count} · 运行 {autopilotAudits.running_count} · 成功 {autopilotAudits.succeeded_count}</div>
          <div>失败 {autopilotAudits.failed_count} · 已取消 {autopilotAudits.cancelled_count}</div>
        </MetricSection>
      </div>
      <Text type="secondary" style={{ display: 'block', marginTop: 12, fontSize: 12, lineHeight: 1.6 }}>
        这是 owner-scoped 的派生只读摘要。任务与调用审计仅为固定上限的运行时观测样本，不是可恢复、可控制或规范化的历史；“暂不可用”仅表示当前读取失败。
      </Text>
    </Card>
  );
}
