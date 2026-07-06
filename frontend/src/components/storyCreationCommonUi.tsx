import { InfoCircleOutlined } from '@ant-design/icons';
import { Space, Tag } from 'antd';
import type { CSSProperties, ReactNode } from 'react';
import { designDisplayFont } from '../theme/themeConfig';

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

const alphaColor = (color: string, alpha: number) =>
  `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

const baseEditorialSurface = (
  toneColor: string,
  options: {
    borderAlpha?: number;
    surfaceAlpha?: number;
    shadowAlpha?: number;
  } = {},
): CSSProperties => ({
  border: `1px solid ${alphaColor(toneColor, options.borderAlpha ?? 0.14)}`,
  borderRadius: 18,
  background: `linear-gradient(180deg, ${alphaColor(toneColor, options.surfaceAlpha ?? 0.07)} 0%, color-mix(in srgb, var(--ant-color-fill-alter) 40%, var(--ant-color-bg-container) 60%) 100%)`,
  boxShadow: `0 14px 28px ${alphaColor(toneColor, options.shadowAlpha ?? 0.06)}`,
});

const COMPACT_SETTING_HINT_STYLES: Record<CompactSettingHintTone, {
  background: string;
  border: string;
  icon: string;
  shell: string;
}> = {
  info: {
    background: '#f7faff',
    border: '#d6e4ff',
    icon: '#1677ff',
    shell: 'linear-gradient(135deg, color-mix(in srgb, var(--ant-color-primary-bg) 78%, white 22%) 0%, color-mix(in srgb, var(--ant-color-bg-container) 92%, white 8%) 100%)',
  },
  success: {
    background: '#f6ffed',
    border: '#b7eb8f',
    icon: '#52c41a',
    shell: 'linear-gradient(135deg, color-mix(in srgb, var(--ant-color-success-bg) 78%, white 22%) 0%, color-mix(in srgb, var(--ant-color-bg-container) 92%, white 8%) 100%)',
  },
  warning: {
    background: '#fffbe6',
    border: '#ffe58f',
    icon: '#faad14',
    shell: 'linear-gradient(135deg, color-mix(in srgb, var(--ant-color-warning-bg) 82%, white 18%) 0%, color-mix(in srgb, var(--ant-color-bg-container) 92%, white 8%) 100%)',
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
        padding: '12px 14px',
        border: `1px solid ${palette.border}`,
        borderRadius: 18,
        background: palette.shell,
        boxShadow: `0 16px 30px ${alphaColor(palette.icon, 0.08)}`,
        ...options.style,
      }}
    >
      <Space size={8} align="start" style={{ width: '100%' }}>
        <InfoCircleOutlined style={{ color: palette.icon, marginTop: 2 }} />
        <div style={{ minWidth: 0, flex: 1 }}>
          <div
            style={{
              fontWeight: 700,
              lineHeight: 1.5,
              fontFamily: designDisplayFont,
              letterSpacing: '-0.02em',
            }}
          >
            {title}
          </div>
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
      padding: '16px 18px',
      ...baseEditorialSurface('var(--ant-color-primary)', {
        borderAlpha: 0.16,
        surfaceAlpha: 0.08,
        shadowAlpha: 0.07,
      }),
      ...options.style,
    }}
  >
    <div
      style={{
        fontWeight: 700,
        lineHeight: 1.5,
        fontFamily: designDisplayFont,
        letterSpacing: '-0.02em',
        fontSize: 15,
      }}
    >
      {summary}
    </div>
    <div
      style={{
        color: 'var(--color-text-secondary)',
        fontSize: 12,
        lineHeight: 1.65,
        marginTop: 4,
      }}
    >
      {detail}
    </div>
    <Space size={[8, 8]} wrap style={{ marginTop: 8 }}>
      {steps.map((step, index) => (
        <Tag
          key={step}
          color="blue"
          style={{
            marginInlineEnd: 0,
            borderRadius: 999,
            paddingInline: 12,
            paddingBlock: 3,
            fontWeight: 500,
          }}
        >
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
        <div
          style={{
            fontWeight: 700,
            fontFamily: designDisplayFont,
            letterSpacing: '-0.02em',
            fontSize: 15,
          }}
        >
          {title}
        </div>
        {options.tagText && (
          <Tag
            color={options.tagColor ?? 'blue'}
            style={{ marginInlineEnd: 0, borderRadius: 999, paddingInline: 12, paddingBlock: 2, fontWeight: 600 }}
          >
            {options.tagText}
          </Tag>
        )}
      </Space>
      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 5, lineHeight: 1.6 }}>
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
      padding: '12px 14px',
      ...baseEditorialSurface('var(--ant-color-primary)', {
        borderAlpha: 0.12,
        surfaceAlpha: 0.05,
        shadowAlpha: 0.04,
      }),
      ...options.style,
    }}
  >
    <div
      style={{
        fontWeight: 700,
        fontSize: 13,
        marginBottom: 6,
        fontFamily: designDisplayFont,
        letterSpacing: '-0.02em',
      }}
    >
      {title}
    </div>
    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.7 }}>{value}</div>
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
        {renderCompactFactCard(title, value, { style: { height: '100%', minHeight: 96 } })}
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
      <Tag
        key={`${item.label}-${item.value}`}
        color={item.color ?? 'default'}
        style={{ marginInlineEnd: 0, borderRadius: 999, paddingInline: 12, paddingBlock: 2, fontWeight: 500 }}
      >
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
      padding: '12px 14px',
      ...baseEditorialSurface('var(--ant-color-primary)', {
        borderAlpha: 0.12,
        surfaceAlpha: 0.05,
        shadowAlpha: 0.04,
      }),
      ...options.style,
    }}
  >
    <Space size={[8, 6]} wrap style={{ marginBottom: items.length > 0 ? 6 : 0 }}>
      <div
        style={{
        fontWeight: 700,
        fontSize: 13,
        fontFamily: designDisplayFont,
        letterSpacing: '-0.02em',
      }}
      >
        {title}
      </div>
      <Tag
        color={options.tagColor ?? 'default'}
        style={{ marginInlineEnd: 0, borderRadius: 999, paddingInline: 12, paddingBlock: 2, fontWeight: 500 }}
      >
        {options.tagText ?? `${items.length} 项`}
      </Tag>
    </Space>
    <Space direction="vertical" size={3} style={{ display: 'flex' }}>
      {items.map((item, index) => (
        <div key={`${title}-${index}-${item}`} style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.7 }}>
          {options.numbered ? `${index + 1}. ` : '• '}{item}
        </div>
      ))}
    </Space>
  </div>
);
