import { InfoCircleOutlined } from '@ant-design/icons';
import { Progress, Space, Tag, Tooltip } from 'antd';
import type { CSSProperties } from 'react';

type CompactSettingHintTone = 'info' | 'success' | 'warning';

type CompactMetricItem = {
  key: string;
  label: string;
  value: number;
  tip?: string;
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
        style={{
          padding: '8px 10px',
          border: '1px solid #f0f0f0',
          borderRadius: 8,
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <Space size={4} wrap>
            <span style={{ fontWeight: 600, fontSize: 13 }}>{item.label}</span>
            {item.tip && (
              <Tooltip title={item.tip}>
                <InfoCircleOutlined style={{ color: '#8c8c8c' }} />
              </Tooltip>
            )}
          </Space>
          <Tag color={getMetricRateColor(item.value)} style={{ marginInlineEnd: 0 }}>
            {item.value}%
          </Tag>
        </div>
        <Progress
          percent={item.value}
          showInfo={false}
          size="small"
          strokeColor={getMetricStrokeColor(item.value)}
        />
      </div>
    ))}
  </div>
);
