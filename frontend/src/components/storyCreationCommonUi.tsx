import { InfoCircleOutlined } from '@ant-design/icons';
import { Space, Tag } from 'antd';
import type { CSSProperties, ReactNode } from 'react';

export type CompactSettingHintTone = 'info' | 'success' | 'warning';

export type CompactSelectionItem = {
  label: string;
  value: string;
  color?: string;
};

export type StoryControlHeaderOptions = {
  tagText?: string;
  tagColor?: string;
  action?: ReactNode;
  style?: CSSProperties;
};

const COMPACT_SETTING_HINT_STYLES: Record<CompactSettingHintTone, {
  background: string;
  border: string;
  icon: string;
}> = {
  info: {
    background: '#f7faff',
    border: '#d6e4ff',
    icon: '#1677ff',
  },
  success: {
    background: '#f6ffed',
    border: '#b7eb8f',
    icon: '#52c41a',
  },
  warning: {
    background: '#fffbe6',
    border: '#ffe58f',
    icon: '#faad14',
  },
};

export const renderCompactSettingHint = (
  title: string,
  detail?: string,
  options: {
    style?: CSSProperties;
    tone?: CompactSettingHintTone;
  } = {},
) => {
  const tone = options.tone ?? 'info';
  const palette = COMPACT_SETTING_HINT_STYLES[tone];

  return (
    <div
      style={{
        marginBottom: 12,
        padding: '8px 12px',
        border: `1px solid ${palette.border}`,
        borderRadius: 8,
        background: palette.background,
        ...options.style,
      }}
    >
      <Space size={8} align="start" style={{ width: '100%' }}>
        <InfoCircleOutlined style={{ color: palette.icon, marginTop: 2 }} />
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ fontWeight: 600, lineHeight: 1.5 }}>{title}</div>
          {detail && (
            <div
              style={{
                color: 'var(--color-text-secondary)',
                fontSize: 12,
                lineHeight: 1.5,
                marginTop: 2,
              }}
            >
              {detail}
            </div>
          )}
        </div>
      </Space>
    </div>
  );
};

export const renderCompactSettingFlow = (
  summary: string,
  detail: string,
  steps: string[],
  options: {
    style?: CSSProperties;
  } = {},
) => (
  <div
    style={{
      marginBottom: 12,
      padding: '10px 12px',
      border: '1px solid #d6e4ff',
      borderRadius: 8,
      background: '#fcfdff',
      ...options.style,
    }}
  >
    <div style={{ fontWeight: 600, lineHeight: 1.5 }}>{summary}</div>
    <div
      style={{
        color: 'var(--color-text-secondary)',
        fontSize: 12,
        lineHeight: 1.5,
        marginTop: 2,
      }}
    >
      {detail}
    </div>
    <Space size={[8, 8]} wrap style={{ marginTop: 8 }}>
      {steps.map((step, index) => (
        <Tag key={step} color="blue" style={{ marginInlineEnd: 0 }}>
          {index + 1}. {step}
        </Tag>
      ))}
    </Space>
  </div>
);

export const renderCompactStoryControlHeader = (
  title: string,
  detail: string,
  options: StoryControlHeaderOptions = {},
) => (
  <div
    style={{
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'flex-start',
      gap: 12,
      marginBottom: 8,
      ...options.style,
    }}
  >
    <div style={{ minWidth: 0, flex: 1 }}>
      <Space size={[8, 6]} wrap>
        <div style={{ fontWeight: 600 }}>{title}</div>
        {options.tagText && (
          <Tag color={options.tagColor ?? 'blue'} style={{ marginInlineEnd: 0 }}>
            {options.tagText}
          </Tag>
        )}
      </Space>
      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 4 }}>
        {detail}
      </div>
    </div>
    {options.action}
  </div>
);

export const renderCompactFactCard = (
  title: string,
  value: string,
  options: {
    style?: CSSProperties;
  } = {},
) => (
  <div
    style={{
      padding: '8px 10px',
      border: '1px solid #f0f0f0',
      borderRadius: 8,
      ...options.style,
    }}
  >
    <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 4 }}>{title}</div>
    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6 }}>{value}</div>
  </div>
);

export const renderCompactFactGrid = (
  items: Array<[string, string]>,
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
    {items.map(([title, value], index) => (
      <div key={`${title}-${index}`} style={{ minWidth: 0 }}>
        {renderCompactFactCard(title, value, { style: { height: '100%' } })}
      </div>
    ))}
  </div>
);

export const renderCompactSelectionSummary = (
  items: CompactSelectionItem[],
  options: {
    style?: CSSProperties;
  } = {},
) => (
  <Space size={[8, 8]} wrap style={{ marginBottom: 10, ...options.style }}>
    {items.map((item) => (
      <Tag key={`${item.label}-${item.value}`} color={item.color ?? 'default'} style={{ marginInlineEnd: 0 }}>
        {item.label}: {item.value}
      </Tag>
    ))}
  </Space>
);

export const renderCompactListCard = (
  title: string,
  items: string[],
  options: {
    numbered?: boolean;
    tagText?: string;
    tagColor?: string;
    style?: CSSProperties;
  } = {},
) => (
  <div
    style={{
      padding: '8px 10px',
      border: '1px solid #f0f0f0',
      borderRadius: 8,
      ...options.style,
    }}
  >
    <Space size={[8, 6]} wrap style={{ marginBottom: items.length > 0 ? 6 : 0 }}>
      <div style={{ fontWeight: 600, fontSize: 13 }}>{title}</div>
      <Tag color={options.tagColor ?? 'default'} style={{ marginInlineEnd: 0 }}>
        {options.tagText ?? `${items.length}?`}
      </Tag>
    </Space>
    <Space direction="vertical" size={3} style={{ display: 'flex' }}>
      {items.map((item, index) => (
        <div key={`${title}-${index}-${item}`} style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6 }}>
          {options.numbered ? `${index + 1}. ` : '? '}{item}
        </div>
      ))}
    </Space>
  </div>
);
