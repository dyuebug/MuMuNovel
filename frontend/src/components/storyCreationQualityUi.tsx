import { InfoCircleOutlined } from '@ant-design/icons';
import { Progress, Space, Tag, Tooltip } from 'antd';
import type { CSSProperties } from 'react';

type CompactSettingHintTone = 'info' | 'success' | 'warning';

type CompactMetricItem = {
  key: string;
  label: string;
  value: number;
  tip?: string;
  displayValue?: string;
};

export const getCompactHintToneByAlertType = (
  tone?: 'success' | 'info' | 'warning' | 'error',
): CompactSettingHintTone => {
  if (tone === 'success') return 'success';
  if (tone === 'warning' || tone === 'error') return 'warning';
  return 'info';
};

export const getOverallScoreColor = (score?: number): string => {
  if ((score ?? 0) >= 75) return 'green';
  if ((score ?? 0) >= 60) return 'gold';
  return 'red';
};

export const getMetricRateColor = (rate?: number): string => {
  if ((rate ?? 0) >= 70) return 'green';
  if ((rate ?? 0) >= 45) return 'gold';
  return 'red';
};

const getMetricStrokeColor = (rate?: number): string => {
  if ((rate ?? 0) >= 70) return '#52c41a';
  if ((rate ?? 0) >= 45) return '#faad14';
  return '#ff4d4f';
};

const getMetricShellStyle = (rate?: number): CSSProperties => {
  const tone = getMetricStrokeColor(rate);
  return {
    padding: '12px 14px',
    border: `1px solid color-mix(in srgb, ${tone} 18%, var(--ant-color-border-secondary) 82%)`,
    borderRadius: 18,
    background: `linear-gradient(180deg, color-mix(in srgb, ${tone} 8%, var(--ant-color-bg-container) 92%) 0%, color-mix(in srgb, var(--ant-color-fill-alter) 44%, var(--ant-color-bg-container) 56%) 100%)`,
    boxShadow: `0 14px 28px color-mix(in srgb, ${tone} 10%, transparent)`,
  };
};

export const renderCompactMetricGrid = (
  items: CompactMetricItem[],
  options: {
    minColumnWidth?: number;
    style?: CSSProperties;
  } = {},
) => (
  <div
    style={{
      display: 'grid',
      gridTemplateColumns: `repeat(auto-fit, minmax(${options.minColumnWidth ?? 220}px, 1fr))`,
      gap: 8,
      ...options.style,
    }}
  >
    {items.map((item) => (
      <div
        key={item.key}
        style={getMetricShellStyle(item.value)}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 8, marginBottom: 8 }}>
          <Space size={4} wrap>
            <span
              style={{
                fontWeight: 700,
                fontSize: 13,
                lineHeight: 1.5,
                letterSpacing: '-0.01em',
              }}
            >
              {item.label}
            </span>
            {item.tip && (
              <Tooltip title={item.tip}>
                <InfoCircleOutlined style={{ color: 'var(--ant-color-text-tertiary)', marginTop: 2 }} />
              </Tooltip>
            )}
          </Space>
          <Tag
            color={getMetricRateColor(item.value)}
            style={{
              marginInlineEnd: 0,
              borderRadius: 999,
              paddingInline: 10,
              fontWeight: 600,
            }}
          >
            {item.displayValue ?? `${item.value}%`}
          </Tag>
        </div>
        <Progress
          percent={item.value}
          showInfo={false}
          size="small"
          strokeColor={getMetricStrokeColor(item.value)}
          strokeLinecap="round"
          trailColor="color-mix(in srgb, var(--ant-color-border-secondary) 56%, transparent)"
        />
      </div>
    ))}
  </div>
);
