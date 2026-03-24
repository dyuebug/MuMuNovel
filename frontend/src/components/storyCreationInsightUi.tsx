import { Card } from 'antd';
import type { CSSProperties } from 'react';

import { renderCompactFactGrid } from './storyCreationCommonUi';

export type CompactInsightGridCard = {
  key: string;
  title: string;
  summary: string;
  items: Array<readonly [unknown, unknown] | unknown[]>;
};

export const renderCompactInsightCardGrid = (
  cards: Array<CompactInsightGridCard | null | undefined>,
  isMobile: boolean,
  options: {
    style?: CSSProperties;
  } = {},
) => {
  const normalizedCards = cards.filter((card): card is CompactInsightGridCard => Boolean(card));

  if (normalizedCards.length === 0) return null;

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: isMobile ? '1fr' : 'repeat(2, minmax(0, 1fr))',
        gap: 12,
        ...options.style,
      }}
    >
      {normalizedCards.map((card) => (
        <div key={card.key} style={{ minWidth: 0 }}>
          <Card size="small" title={card.title} style={{ height: '100%' }}>
            <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, lineHeight: 1.6, marginBottom: 8 }}>
              {card.summary}
            </div>
            {renderCompactFactGrid(
              card.items.map((item) => [String(item[0] ?? ''), String(item[1] ?? '')] as [string, string]),
              { minColumnWidth: isMobile ? 140 : 170 },
            )}
          </Card>
        </div>
      ))}
    </div>
  );
};
