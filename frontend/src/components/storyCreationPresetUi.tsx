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
        padding: '10px 12px',
        border: '1px solid #b7eb8f',
        borderRadius: 8,
        background: '#f6ffed',
        ...options.style,
      }}
    >
      {renderCompactStoryControlHeader(
        '????',
        '??????????????????????',
        {
          tagText: `? ${items.length} ?`,
          tagColor: 'green',
          style: { marginBottom: 10 },
        },
      )}
      <Space size={[8, 8]} wrap>
        {items.map((item) => (
          <Button
            key={item.id}
            size="small"
            type={options.activePresetId === item.id ? 'primary' : 'default'}
            onClick={() => options.applyPreset(item.id)}
            title={item.reason || item.label}
          >
            {item.reason ? `${item.label} ? ${item.reason}` : item.label}
          </Button>
        ))}
      </Space>
    </div>
  );
}
