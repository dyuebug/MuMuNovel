import { Button, Space } from 'antd';
import type { CSSProperties } from 'react';

import { getCreationPresetById, type CreationPresetId } from '../utils/creationPresetsCore';
import { renderCompactStoryControlHeader } from './storyCreationCommonUi';

export function renderCompactPresetRecommendationBlock(
  recommendations: Array<{ id: CreationPresetId; reason?: string }>,
  options: {
    activePresetId?: CreationPresetId | null;
    applyPreset: (presetId: CreationPresetId) => void;
    style?: CSSProperties;
  },
) {
  const items = recommendations
    .map((item) => {
      const preset = getCreationPresetById(item.id);
      if (!preset) return null;
      return { ...item, label: preset.label };
    })
    .filter((item): item is { id: CreationPresetId; reason?: string; label: string } => Boolean(item));

  if (items.length === 0) return null;

  return (
    <div
      style={{
        marginTop: 12,
        padding: '14px 16px',
        border: '1px solid color-mix(in srgb, var(--ant-color-success) 20%, var(--ant-color-border-secondary) 80%)',
        borderRadius: 18,
        background: 'linear-gradient(135deg, color-mix(in srgb, var(--ant-color-success-bg) 74%, white 26%) 0%, color-mix(in srgb, var(--ant-color-bg-container) 92%, white 8%) 100%)',
        boxShadow: '0 16px 32px color-mix(in srgb, var(--ant-color-success) 9%, transparent)',
        ...options.style,
      }}
    >
      {renderCompactStoryControlHeader(
        '已应用预设',
        '当前已启用的创作预设会影响生成提示词和章节节奏。',
        {
          tagText: `共 ${items.length} 项`,
          tagColor: 'green',
          style: { marginBottom: 12 },
        },
      )}
      <div
        style={{
          color: 'var(--color-text-secondary)',
          fontSize: 12,
          lineHeight: 1.6,
          marginBottom: 12,
        }}
      >
        当前推荐会直接复用已有创作预设，只优化阅读顺序与按钮层级，不改变任何预设应用逻辑。
      </div>
      <Space size={[8, 8]} wrap>
        {items.map((item) => (
          <Button
            key={item.id}
            size="small"
            type={options.activePresetId === item.id ? 'primary' : 'default'}
            onClick={() => options.applyPreset(item.id)}
            title={item.reason || item.label}
            style={{
              borderRadius: 999,
              minHeight: 30,
              paddingInline: 12,
              fontWeight: options.activePresetId === item.id ? 700 : 500,
              boxShadow: options.activePresetId === item.id
                ? '0 10px 20px color-mix(in srgb, var(--ant-color-primary) 18%, transparent)'
                : 'none',
            }}
          >
            {item.reason ? `${item.label} - ${item.reason}` : item.label}
          </Button>
        ))}
      </Space>
    </div>
  );
}
